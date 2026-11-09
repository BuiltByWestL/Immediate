//! Video and audio: reading files with ffprobe, and the pictures the timeline needs (keyframes,
//! waveform, frame thumbnails). Editing and export live in export.rs; installing ffmpeg in
//! install.rs.

mod convert;
pub mod hw;
pub mod export;
pub mod install;
pub mod record;

use std::path::{Path, PathBuf};
use std::process::Command;
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Runtime};

#[derive(Serialize, Clone, Debug)]
#[serde(rename_all = "camelCase")]
pub struct Tools {
    pub ffmpeg: PathBuf,
    pub ffprobe: PathBuf,
    pub version: String,
    /// Installed by Immediate (vs. found on this computer).
    pub managed: bool,
}

static TOOLS: Mutex<Option<Tools>> = Mutex::new(None);

pub fn forget_tools() {
    if let Ok(mut t) = TOOLS.lock() {
        *t = None;
    }
}

const NEEDS_TOOLS: &str = "needs-tools";
const MIN_MAJOR: u32 = 6;

fn exe(name: &str) -> String {
    if cfg!(windows) { format!("{name}.exe") } else { name.to_owned() }
}

fn version_of(ffmpeg: &Path) -> Option<String> {
    let out = crate::tools::quiet_command(ffmpeg).args(["-hide_banner", "-version"]).output().ok()?;
    let first = String::from_utf8_lossy(&out.stdout).lines().next()?.to_owned();
    first.split_whitespace().nth(2).map(str::to_owned)
}

/// Major version from strings like "8.1.2-full_build", "n9.0.2-48-g…", "N-128560-g…" (git master).
fn major(v: &str) -> Option<u32> {
    let v = v.trim_start_matches('n');
    if v.starts_with("N-") {
        return Some(99);
    }
    v.split(['.', '-']).next()?.parse().ok()
}

/// Immediate's own copy first, else a recent enough one already on this computer.
pub fn locate<R: Runtime>(app: &AppHandle<R>) -> Option<Tools> {
    if let Some(t) = TOOLS.lock().ok().and_then(|t| t.clone()) {
        return Some(t);
    }
    let managed = install::tools_dir(app).map(|d| d.join("bin")).and_then(|bin| {
        let (f, p) = (bin.join(exe("ffmpeg")), bin.join(exe("ffprobe")));
        (f.is_file() && p.is_file()).then(|| (f, p, true))
    });
    let system = || {
        let (f, p) = (crate::tools::which("ffmpeg")?, crate::tools::which("ffprobe")?);
        Some((f, p, false))
    };
    let (ffmpeg, ffprobe, managed) = managed.or_else(system)?;
    let version = version_of(&ffmpeg)?;
    if major(&version).is_none_or(|m| m < MIN_MAJOR) {
        return None;
    }
    let t = Tools { ffmpeg, ffprobe, version, managed };
    if let Ok(mut c) = TOOLS.lock() {
        *c = Some(t.clone());
    }
    Some(t)
}

fn tools<R: Runtime>(app: &AppHandle<R>) -> Result<Tools, String> {
    locate(app).ok_or_else(|| NEEDS_TOOLS.to_owned())
}

pub fn run(mut cmd: Command) -> Result<Vec<u8>, String> {
    let out = cmd.output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        let err = String::from_utf8_lossy(&out.stderr);
        return Err(err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("ffmpeg failed").trim().to_owned());
    }
    Ok(out.stdout)
}

async fn blocking<T: Send + 'static>(f: impl FnOnce() -> Result<T, String> + Send + 'static) -> Result<T, String> {
    tauri::async_runtime::spawn_blocking(f).await.map_err(|e| e.to_string())?
}

/// Which tools are in use (Settings → Media tools); None when there are none yet.
#[tauri::command]
pub async fn media_tools<R: Runtime>(app: AppHandle<R>) -> Option<Tools> {
    blocking(move || Ok(locate(&app))).await.ok().flatten()
}

/// Everything ffprobe knows about the file: format, streams, chapters (ffprobe's JSON).
#[tauri::command]
pub async fn media_probe<R: Runtime>(app: AppHandle<R>, path: String) -> Result<serde_json::Value, String> {
    let t = tools(&app)?;
    blocking(move || {
        let mut cmd = crate::tools::quiet_command(&t.ffprobe);
        cmd.args(["-v", "error", "-print_format", "json", "-show_format", "-show_streams", "-show_chapters"]).arg(&path);
        serde_json::from_slice(&run(cmd)?).map_err(|e| e.to_string())
    })
    .await
}

/// Times (seconds) of the first video stream's keyframes: where lossless cuts can land.
/// Reads packet headers only, so it is quick even for long videos.
#[tauri::command]
pub async fn media_keyframes<R: Runtime>(app: AppHandle<R>, path: String) -> Result<Vec<f64>, String> {
    let t = tools(&app)?;
    blocking(move || {
        let mut cmd = crate::tools::quiet_command(&t.ffprobe);
        cmd.args(["-v", "error", "-select_streams", "v:0", "-show_entries", "packet=pts_time,flags", "-of", "csv=p=0"]).arg(&path);
        let out = run(cmd)?;
        let mut times: Vec<f64> = String::from_utf8_lossy(&out)
            .lines()
            .filter_map(|l| {
                let (pts, flags) = l.split_once(',')?;
                flags.contains('K').then(|| pts.parse().ok()).flatten()
            })
            .collect();
        times.sort_by(f64::total_cmp);
        times.dedup();
        Ok(times)
    })
    .await
}

/// Audio overview for the timeline: `buckets` (min, max) pairs in -1..1, of the first audio
/// stream mixed to mono.
#[tauri::command]
pub async fn media_waveform<R: Runtime>(app: AppHandle<R>, path: String, buckets: usize) -> Result<Vec<f32>, String> {
    let t = tools(&app)?;
    let buckets = buckets.clamp(16, 20_000);
    blocking(move || {
        let mut cmd = crate::tools::quiet_command(&t.ffmpeg);
        cmd.args(["-v", "error", "-i"]).arg(&path).args(["-map", "0:a:0", "-ac", "1", "-ar", "4000", "-f", "s16le", "-"]);
        let pcm = run(cmd)?;
        let samples: Vec<i16> = pcm.chunks_exact(2).map(|c| i16::from_le_bytes([c[0], c[1]])).collect();
        Ok(peaks(&samples, buckets))
    })
    .await
}

/// The gain (dB) "Even out loudness" will apply when saving, and whether it needs the peak
/// limiter, for hearing it live: the same
/// measurement the save makes, of the first audio track's kept parts after volume, noise
/// reduction, tone and leveling.
#[tauri::command]
#[allow(clippy::too_many_arguments)]
pub async fn media_loudness<R: Runtime>(app: AppHandle<R>, path: String, segments: Vec<export::Segment>, sound: export::Sound, peak: bool) -> Result<(f64, bool), String> {
    let t = tools(&app)?;
    blocking(move || {
        let segs: Vec<export::Segment> = segments.into_iter().filter(|s| s.end - s.start > 0.01).collect();
        if segs.is_empty() {
            return Ok((0.0, false));
        }
        let mut cmd = crate::tools::quiet_command(&t.ffmpeg);
        cmd.args(["-hide_banner", "-nostats"]).args(export::measure_args(&path, &segs, 0, &sound, peak));
        let out = cmd.output().map_err(|e| e.to_string())?;
        let messages = String::from_utf8_lossy(&out.stderr);
        if peak {
            return Ok((export::peak_gain(&messages)?, false));
        }
        let (i, tp) = export::parse_loudness(&messages)?;
        Ok(export::loudness_gain(i, tp))
    })
    .await
}

fn peaks(samples: &[i16], buckets: usize) -> Vec<f32> {
    if samples.is_empty() {
        return vec![0.0; buckets * 2];
    }
    let per = samples.len() as f64 / buckets as f64;
    (0..buckets)
        .flat_map(|b| {
            let a = (b as f64 * per) as usize;
            let z = (((b + 1) as f64 * per) as usize).clamp(a + 1, samples.len());
            let slice = &samples[a.min(samples.len() - 1)..z];
            let (mn, mx) = slice.iter().fold((i16::MAX, i16::MIN), |(lo, hi), &s| (lo.min(s), hi.max(s)));
            [mn as f32 / 32768.0, mx as f32 / 32768.0]
        })
        .collect()
}

/// A playable stand-in for files the built-in player can't decode (ProRes, MPEG-2, AVI, AC-3…):
/// H.264/AAC MP4 at up to 720p, made once and kept in the temp folder. Edits still apply to the
/// original. Progress arrives as "proxy" events.
#[tauri::command]
pub async fn media_proxy<R: Runtime>(app: AppHandle<R>, path: String, duration: f64, video: bool) -> Result<String, String> {
    use std::hash::{Hash, Hasher};
    use tauri::Manager;
    let t = tools(&app)?;
    let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
    let mut h = std::collections::hash_map::DefaultHasher::new();
    (&path, meta.len(), meta.modified().ok()).hash(&mut h);
    let dir = crate::files::temp_dir().join("previews");
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let out = dir.join(format!("{:016x}.{}", h.finish(), if video { "mp4" } else { "m4a" }));
    if !out.is_file() {
        let tmp = out.with_extension("part.mp4");
        let mut a: Vec<String> = vec!["-i".into(), path.clone()];
        if video {
            a.extend(["-map", "0:v:0", "-map", "0:a:0?", "-c:v", "libx264", "-preset", "veryfast", "-crf", "24", "-pix_fmt", "yuv420p"].map(String::from));
            a.extend(["-vf".into(), "scale=-2:'min(720,ih)'".into()]);
        } else {
            a.extend(["-map", "0:a:0", "-vn"].map(String::from));
        }
        a.extend(["-c:a", "aac", "-b:a", "160k", "-movflags", "+faststart", "-f", "mp4"].map(String::from));
        a.push(tmp.to_string_lossy().into_owned());
        let handle = app.clone();
        let ffmpeg = t.ffmpeg.clone();
        blocking(move || export::run_ffmpeg(&handle, &ffmpeg, &a, duration.max(0.1), 0.0, 1.0, "proxy", false)).await?;
        std::fs::rename(&tmp, &out).map_err(|e| e.to_string())?;
    }
    let _ = app.asset_protocol_scope().allow_file(&out);
    Ok(out.to_string_lossy().into_owned())
}

/// One frame as a small JPEG (timeline thumbnails, frame grab previews).
#[tauri::command]
pub async fn media_frame<R: Runtime>(app: AppHandle<R>, path: String, time: f64, height: u32) -> Result<tauri::ipc::Response, String> {
    let t = tools(&app)?;
    let jpg = blocking(move || {
        let mut cmd = crate::tools::quiet_command(&t.ffmpeg);
        cmd.args(["-v", "error", "-ss", &format!("{:.3}", time.max(0.0)), "-i"])
            .arg(&path)
            .args(["-frames:v", "1", "-vf", &format!("scale=-2:{}", height.clamp(16, 2160)), "-q:v", "4", "-f", "image2pipe", "-c:v", "mjpeg", "-"]);
        run(cmd)
    })
    .await?;
    if jpg.is_empty() {
        return Err("no frame".into());
    }
    Ok(tauri::ipc::Response::new(jpg))
}

/// The cover art inside an audio file, as a JPEG up to 600 px (empty when it has none).
#[tauri::command]
pub async fn media_cover<R: Runtime>(app: AppHandle<R>, path: String) -> Result<tauri::ipc::Response, String> {
    let t = tools(&app)?;
    let jpg = blocking(move || {
        let mut cmd = crate::tools::quiet_command(&t.ffmpeg);
        cmd.args(["-v", "error", "-i"]).arg(&path).args([
            "-map", "0:v:0?", "-frames:v", "1", "-vf", "scale='min(600,iw)':-2", "-q:v", "3", "-f", "image2pipe", "-c:v", "mjpeg", "-",
        ]);
        run(cmd)
    })
    .await
    .unwrap_or_default();
    Ok(tauri::ipc::Response::new(jpg))
}

/// A spectrogram of the sound as a PNG: time across, pitch up (log scale), loudness as color.
#[tauri::command]
pub async fn media_spectrogram<R: Runtime>(app: AppHandle<R>, path: String, width: u32, height: u32) -> Result<tauri::ipc::Response, String> {
    let t = tools(&app)?;
    let png = blocking(move || {
        let (w, h) = (width.clamp(64, 4096), height.clamp(64, 2048));
        let mut cmd = crate::tools::quiet_command(&t.ffmpeg);
        cmd.args(["-v", "error", "-i"]).arg(&path).args([
            "-filter_complex",
            &format!("[0:a:0]showspectrumpic=s={w}x{h}:legend=0:scale=log:fscale=log:color=intensity[s]"),
            "-map",
            "[s]",
            "-frames:v",
            "1",
            "-f",
            "image2pipe",
            "-c:v",
            "png",
            "-",
        ]);
        run(cmd)
    })
    .await?;
    Ok(tauri::ipc::Response::new(png))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn version_parsing() {
        assert_eq!(major("8.1.2-full_build-www.gyan.dev"), Some(8));
        assert_eq!(major("n9.0.2-48-gd4070a14ce-20261107"), Some(9));
        assert_eq!(major("N-128560-g967f0ffb10"), Some(99));
        assert_eq!(major("4.4.2-0ubuntu0.22.04.1"), Some(4));
    }

    #[test]
    fn waveform_peaks() {
        let s = [0i16, 16384, -16384, 0, 32767, -32768];
        let p = peaks(&s, 3);
        assert_eq!(p.len(), 6);
        assert!((p[1] - 0.5).abs() < 1e-3);
        assert!((p[4] + 1.0).abs() < 1e-3 || (p[5] - 1.0).abs() < 1e-3);
    }
}
