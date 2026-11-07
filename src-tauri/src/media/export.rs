//! Exporting edited video/audio. An edit is a list of kept segments in source time.
//!   lossless:  stream copy (same codec, bitrate, container, every track, chapters, metadata);
//!              cuts land on the keyframe at or before each start.
//!   accurate:  frame-exact cuts, re-encoded with settings read from the source (same codec,
//!              bitrate, pixel format, color tags, audio codec/rate/channels).
//!   smaller:   H.264/AAC MP4 aimed at a target size (for chat apps with upload limits).
//! Progress arrives as "export" events; `media_cancel` stops it.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};
use std::process::{Child, Stdio};
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use serde_json::Value;
use tauri::{AppHandle, Emitter, Manager, Runtime};

use crate::files::unique_path;

#[derive(Deserialize, Clone, Copy, Debug)]
pub struct Segment {
    pub start: f64,
    pub end: f64,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ExportRequest {
    source: String,
    segments: Vec<Segment>,
    /// "lossless" | "accurate" | "smaller"
    mode: String,
    #[serde(default)]
    mute: bool,
    /// Volume change in dB (0 = unchanged). Video is still copied when it's the only change.
    #[serde(default)]
    gain_db: f64,
    /// "copy" | "replace" | "path"
    target: String,
    dest: Option<String>,
    suffix: String,
    /// Target size for "smaller", in megabytes.
    size_mb: Option<f64>,
    /// The source's ffprobe JSON (the UI already has it).
    probe: Value,
}

#[derive(Serialize, Clone)]
struct Progress {
    progress: f64,
    /// Seconds remaining, when known.
    eta: Option<f64>,
}

static CHILD: Mutex<Option<Child>> = Mutex::new(None);
static CANCELLED: std::sync::atomic::AtomicBool = std::sync::atomic::AtomicBool::new(false);

#[tauri::command]
pub fn media_cancel() {
    CANCELLED.store(true, std::sync::atomic::Ordering::SeqCst);
    if let Ok(mut c) = CHILD.lock() {
        if let Some(child) = c.as_mut() {
            let _ = child.kill();
        }
    }
}

fn streams<'a>(probe: &'a Value, kind: &str) -> Vec<&'a Value> {
    probe["streams"].as_array().map(|s| s.iter().filter(|x| x["codec_type"] == kind).collect()).unwrap_or_default()
}

fn num(v: &Value) -> Option<f64> {
    v.as_f64().or_else(|| v.as_str()?.parse().ok())
}

fn ext_of(p: &Path) -> String {
    p.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

/// Runs ffmpeg, reporting progress against `total` seconds of output.
fn run_with_progress<R: Runtime>(app: &AppHandle<R>, ffmpeg: &Path, args: &[String], total: f64, base: f64, share: f64) -> Result<(), String> {
    if CANCELLED.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("cancelled".into());
    }
    run_ffmpeg(app, ffmpeg, args, total, base, share, "export", true)
}

/// Runs ffmpeg with `-progress`, emitting `event` with {progress, eta}. Only a `cancellable` run
/// can be stopped with media_cancel (a background preview never is).
#[allow(clippy::too_many_arguments)]
pub(crate) fn run_ffmpeg<R: Runtime>(app: &AppHandle<R>, ffmpeg: &Path, args: &[String], total: f64, base: f64, share: f64, event: &str, cancellable: bool) -> Result<(), String> {
    let mut cmd = crate::tools::quiet_command(ffmpeg);
    cmd.args(["-hide_banner", "-nostats", "-progress", "pipe:1", "-y"]).args(args);
    cmd.stdout(Stdio::piped()).stderr(Stdio::piped());
    let mut child = cmd.spawn().map_err(|e| e.to_string())?;
    let stdout = child.stdout.take().ok_or("no output")?;
    let stderr = child.stderr.take().ok_or("no output")?;
    let mut own: Option<Child> = None;
    if cancellable {
        if let Ok(mut c) = CHILD.lock() {
            *c = Some(child);
        }
    } else {
        own = Some(child);
    }
    // Keep the last lines of ffmpeg's messages for a useful error.
    let err_thread = std::thread::spawn(move || {
        let mut tail: Vec<String> = Vec::new();
        for line in BufReader::new(stderr).lines().map_while(Result::ok) {
            tail.push(line);
            if tail.len() > 20 {
                tail.remove(0);
            }
        }
        tail
    });
    let started = std::time::Instant::now();
    for line in BufReader::new(stdout).lines().map_while(Result::ok) {
        if let Some(us) = line.strip_prefix("out_time_us=").and_then(|v| v.parse::<f64>().ok()) {
            let done = (us / 1e6 / total.max(0.001)).clamp(0.0, 1.0);
            let p = base + done * share;
            let elapsed = started.elapsed().as_secs_f64();
            let eta = (done > 0.02).then(|| elapsed / done * (1.0 - done) + (1.0 - base - share) * elapsed / done.max(0.02));
            let _ = app.emit(event, Progress { progress: p, eta });
        }
    }
    let status = match own.as_mut() {
        Some(c) => Some(c.wait()),
        None => CHILD.lock().ok().and_then(|mut c| c.take()).map(|mut c| c.wait()),
    };
    let tail = err_thread.join().unwrap_or_default();
    if cancellable && CANCELLED.load(std::sync::atomic::Ordering::SeqCst) {
        return Err("cancelled".into());
    }
    match status {
        Some(Ok(s)) if s.success() => Ok(()),
        _ => {
            let msg = tail.iter().rev().find(|l| !l.trim().is_empty()).cloned().unwrap_or_else(|| "ffmpeg stopped".into());
            Err(msg)
        }
    }
}

fn fmt_t(t: f64) -> String {
    format!("{:.6}", t.max(0.0))
}

/// Encoder that writes the same codec as `codec_name`, if there is one.
fn video_encoder(codec: &str) -> &'static str {
    match codec {
        "h264" => "libx264",
        "hevc" => "libx265",
        "vp9" => "libvpx-vp9",
        "vp8" => "libvpx",
        "av1" => "libsvtav1",
        "mpeg4" => "mpeg4",
        "mpeg2video" => "mpeg2video",
        "prores" => "prores_ks",
        "mjpeg" => "mjpeg",
        "theora" => "libtheora",
        "wmv2" => "wmv2",
        _ => "libx264",
    }
}

fn audio_encoder(codec: &str) -> &'static str {
    match codec {
        "aac" => "aac",
        "mp3" => "libmp3lame",
        "opus" => "libopus",
        "vorbis" => "libvorbis",
        "flac" => "flac",
        "ac3" => "ac3",
        "eac3" => "eac3",
        "alac" => "alac",
        "mp2" => "mp2",
        "wmav2" => "wmav2",
        "pcm_s16le" | "pcm_s24le" | "pcm_s32le" | "pcm_f32le" | "pcm_s16be" | "pcm_s24be" => {
            match codec {
                "pcm_s16le" => "pcm_s16le",
                "pcm_s24le" => "pcm_s24le",
                "pcm_s32le" => "pcm_s32le",
                "pcm_f32le" => "pcm_f32le",
                "pcm_s16be" => "pcm_s16be",
                _ => "pcm_s24be",
            }
        }
        _ => "aac",
    }
}

fn is_pcm_or_lossless(codec: &str) -> bool {
    codec.starts_with("pcm_") || matches!(codec, "flac" | "alac")
}

/// Encoder settings that reproduce the source stream as closely as possible.
fn video_settings(v: &Value, probe: &Value, out_index: usize) -> Vec<String> {
    let codec = v["codec_name"].as_str().unwrap_or("h264");
    let enc = video_encoder(codec);
    let mut a = vec![format!("-c:v:{out_index}"), enc.into()];
    let br = num(&v["bit_rate"]).or_else(|| {
        // Container bitrate minus the audio, when the stream doesn't say.
        let total = num(&probe["format"]["bit_rate"])?;
        let audio: f64 = streams(probe, "audio").iter().filter_map(|s| num(&s["bit_rate"])).sum();
        Some((total - audio).max(total * 0.5))
    });
    if let Some(br) = br {
        let br = br.round() as u64;
        a.extend(["-b:v".into(), br.to_string(), "-maxrate".into(), (br * 3 / 2).to_string(), "-bufsize".into(), (br * 2).to_string()]);
    }
    if let Some(pf) = v["pix_fmt"].as_str() {
        a.extend(["-pix_fmt".into(), pf.into()]);
    }
    if enc == "libx264" || enc == "libx265" {
        if let Some(profile) = v["profile"].as_str() {
            let p = profile.to_lowercase().replace(' ', "");
            let ok = ["baseline", "main", "high", "high10", "high422", "high444", "main10", "main422-10", "main444-8"];
            let p = p.trim_start_matches("constrained").to_owned();
            if ok.contains(&p.as_str()) {
                a.extend(["-profile:v".into(), p]);
            }
        }
        a.extend(["-preset".into(), "medium".into()]);
    }
    if enc == "libsvtav1" {
        a.extend(["-preset".into(), "8".into()]);
    }
    for (key, flag) in [("color_primaries", "-color_primaries"), ("color_transfer", "-color_trc"), ("color_space", "-colorspace"), ("color_range", "-color_range")] {
        if let Some(val) = v[key].as_str().filter(|s| *s != "unknown") {
            a.extend([flag.into(), val.into()]);
        }
    }
    // Keyframe spacing like a typical camera/phone: about 2 seconds.
    if let Some(fps) = v["avg_frame_rate"].as_str().and_then(|r| {
        let (n, d) = r.split_once('/')?;
        let (n, d): (f64, f64) = (n.parse().ok()?, d.parse().ok()?);
        (d > 0.0).then(|| n / d)
    }) {
        a.extend(["-g".into(), ((fps * 2.0).round().max(1.0) as u32).to_string()]);
    }
    a
}

fn audio_settings(s: &Value, out_index: usize) -> Vec<String> {
    let codec = s["codec_name"].as_str().unwrap_or("aac");
    let mut a = vec![format!("-c:a:{out_index}"), audio_encoder(codec).into()];
    if !is_pcm_or_lossless(codec) {
        if let Some(br) = num(&s["bit_rate"]) {
            a.extend([format!("-b:a:{out_index}"), (br.round() as u64).to_string()]);
        }
    }
    if let Some(sr) = s["sample_rate"].as_str() {
        a.extend([format!("-ar:a:{out_index}"), sr.into()]);
    }
    if let Some(ch) = s["channels"].as_u64() {
        a.extend([format!("-ac:a:{out_index}"), ch.to_string()]);
    }
    a
}

fn destination(req: &ExportRequest, ext: &str) -> Result<PathBuf, String> {
    let source = PathBuf::from(&req.source);
    match req.target.as_str() {
        "copy" => {
            let dir = source.parent().ok_or("no folder")?;
            let stem = source.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            Ok(unique_path(dir, &format!("{stem}{}", req.suffix), ext))
        }
        "replace" => Ok(source),
        _ => Ok(PathBuf::from(req.dest.as_deref().ok_or("no destination")?)),
    }
}

/// Exports the edit. Returns the saved file's path.
#[tauri::command]
pub async fn media_export<R: Runtime>(app: AppHandle<R>, req: ExportRequest) -> Result<String, String> {
    let tools = super::locate(&app).ok_or("needs-tools")?;
    CANCELLED.store(false, std::sync::atomic::Ordering::SeqCst);
    let handle = app.clone();
    let saved = tauri::async_runtime::spawn_blocking(move || export(&handle, &tools.ffmpeg, &req))
        .await
        .map_err(|e| e.to_string())??;
    let _ = app.asset_protocol_scope().allow_file(&saved);
    Ok(saved.to_string_lossy().into_owned())
}

fn export<R: Runtime>(app: &AppHandle<R>, ffmpeg: &Path, req: &ExportRequest) -> Result<PathBuf, String> {
    let source = PathBuf::from(&req.source);
    let src_ext = ext_of(&source);
    let segs: Vec<Segment> = req.segments.iter().copied().filter(|s| s.end - s.start > 0.01).collect();
    if segs.is_empty() {
        return Err("Nothing is left to save: every part was cut out.".into());
    }
    let total: f64 = segs.iter().map(|s| s.end - s.start).sum();
    let has_video = !streams(&req.probe, "video").iter().all(|v| v["disposition"]["attached_pic"] == 1);
    let audio = streams(&req.probe, "audio");

    let (ext, mode) = match req.mode.as_str() {
        "smaller" => (if has_video { "mp4".to_owned() } else { "m4a".to_owned() }, "smaller"),
        // Changing the volume needs the audio re-encoded; the video can still be copied.
        "lossless" if req.gain_db.abs() > 0.01 => (src_ext.clone(), "lossless"),
        m => (src_ext.clone(), if m == "accurate" { "accurate" } else { "lossless" }),
    };
    let dest = destination(req, &ext)?;
    let dir = dest.parent().ok_or("no folder")?.to_path_buf();
    let tmp_out = dir.join(format!(".{}.immediate-tmp.{ext}", dest.file_stem().map(|s| s.to_string_lossy()).unwrap_or_default()));
    let work = std::env::temp_dir().join(format!("immediate-export-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&work);
    let cleanup = || {
        let _ = std::fs::remove_dir_all(&work);
        let _ = std::fs::remove_file(&tmp_out);
    };

    let movflags: Vec<String> = if matches!(ext.as_str(), "mp4" | "m4v" | "mov" | "m4a") { vec!["-movflags".into(), "+faststart".into()] } else { vec![] };
    let gain: Vec<String> = if req.gain_db.abs() > 0.01 { vec!["-af".into(), format!("volume={:.2}dB", req.gain_db)] } else { vec![] };

    let result = (|| -> Result<(), String> {
        match mode {
            "lossless" => {
                let audio_args: Vec<String> = if req.mute {
                    vec!["-an".into()]
                } else if !gain.is_empty() {
                    let mut a = gain.clone();
                    for (i, s) in audio.iter().enumerate() {
                        a.extend(audio_settings(s, i));
                    }
                    a
                } else {
                    vec![]
                };
                // Each part starts on a keyframe (the UI sends keyframe-aligned starts). Seeking
                // before the input backs up to an earlier keyframe, so: a coarse seek there for
                // speed, then an exact one after the input that lands on the keyframe itself.
                let one = |seg: &Segment, out: &Path, base: f64, share: f64| -> Result<(), String> {
                    let coarse = (seg.start - 10.0).max(0.0);
                    let mut a: Vec<String> = Vec::new();
                    if coarse > 0.0 {
                        a.extend(["-ss".into(), fmt_t(coarse)]);
                    }
                    a.extend(["-i".into(), req.source.clone()]);
                    if seg.start - coarse > 0.0005 {
                        a.extend(["-ss".into(), fmt_t(seg.start - coarse - 0.0005)]);
                    }
                    a.extend(["-to".into(), fmt_t(seg.end - coarse)]);
                    a.extend(["-map".into(), "0".into(), "-c".into(), "copy".into(), "-ignore_unknown".into()]);
                    a.extend(audio_args.iter().cloned());
                    a.extend(["-avoid_negative_ts".into(), "make_zero".into()]);
                    a.extend(movflags.iter().cloned());
                    a.push(out.to_string_lossy().into_owned());
                    run_with_progress(app, ffmpeg, &a, seg.end - seg.start, base, share)
                };
                if segs.len() == 1 {
                    one(&segs[0], &tmp_out, 0.0, 1.0)
                } else {
                    let mut list = String::new();
                    let mut done = 0.0;
                    for (i, seg) in segs.iter().enumerate() {
                        let part = work.join(format!("part{i}.{ext}"));
                        let share = (seg.end - seg.start) / total * 0.9;
                        one(seg, &part, done, share)?;
                        done += share;
                        list.push_str(&format!("file '{}'\n", part.to_string_lossy().replace('\\', "/").replace('\'', "'\\''")));
                    }
                    let list_path = work.join("list.txt");
                    std::fs::write(&list_path, list).map_err(|e| e.to_string())?;
                    let mut a: Vec<String> = vec!["-f".into(), "concat".into(), "-safe".into(), "0".into(), "-i".into(), list_path.to_string_lossy().into_owned()];
                    a.extend(["-i".into(), req.source.clone(), "-map".into(), "0".into(), "-map_metadata".into(), "1".into(), "-map_chapters".into(), "-1".into()]);
                    a.extend(["-c".into(), "copy".into(), "-ignore_unknown".into()]);
                    a.extend(movflags.iter().cloned());
                    a.push(tmp_out.to_string_lossy().into_owned());
                    run_with_progress(app, ffmpeg, &a, total, 0.9, 0.1)
                }
            }
            _ => {
                // Frame-exact: trim + concat filters, then encode.
                let video = if has_video { streams(&req.probe, "video").first().copied() } else { None };
                let keep_audio: Vec<&Value> = if req.mute { vec![] } else { audio.clone() };
                let mut fc = String::new();
                let mut concat_in = String::new();
                for (i, s) in segs.iter().enumerate() {
                    if video.is_some() {
                        fc.push_str(&format!("[0:v:0]trim=start={}:end={},setpts=PTS-STARTPTS[v{i}];", fmt_t(s.start), fmt_t(s.end)));
                        concat_in.push_str(&format!("[v{i}]"));
                    }
                    for k in 0..keep_audio.len() {
                        fc.push_str(&format!("[0:a:{k}]atrim=start={}:end={},asetpts=PTS-STARTPTS[a{k}_{i}];", fmt_t(s.start), fmt_t(s.end)));
                        concat_in.push_str(&format!("[a{k}_{i}]"));
                    }
                }
                let nv = usize::from(video.is_some());
                let na = keep_audio.len();
                let mut outs = String::new();
                if nv == 1 {
                    outs.push_str("[vout]");
                }
                for k in 0..na {
                    outs.push_str(&format!("[aout{k}]"));
                }
                fc.push_str(&format!("{concat_in}concat=n={}:v={nv}:a={na}{outs}", segs.len()));
                if req.gain_db.abs() > 0.01 {
                    for k in 0..na {
                        fc = fc.replacen(&format!("[aout{k}]"), &format!("[aout{k}pre]"), 1);
                        fc.push_str(&format!(";[aout{k}pre]volume={:.2}dB[aout{k}]", req.gain_db));
                    }
                }
                let mut a: Vec<String> = vec!["-i".into(), req.source.clone(), "-filter_complex".into(), fc];
                if nv == 1 {
                    a.extend(["-map".into(), "[vout]".into()]);
                }
                for k in 0..na {
                    a.extend(["-map".into(), format!("[aout{k}]")]);
                }
                a.extend(["-map_metadata".into(), "0".into()]);
                if mode == "smaller" {
                    let target_bits = req.size_mb.unwrap_or(10.0) * 8.0 * 1_000_000.0 * 0.94;
                    let audio_bps = if na > 0 { 96_000.0 * na as f64 } else { 0.0 };
                    let mut vbps = (target_bits / total - audio_bps).max(80_000.0);
                    // Never bigger than it was: a short clip that already fits keeps its bitrate.
                    if let Some(src) = video.and_then(|v| num(&v["bit_rate"])).or_else(|| num(&req.probe["format"]["bit_rate"])) {
                        vbps = vbps.min(src);
                    }
                    if nv == 1 {
                        // Fewer pixels when the bitrate is too low for the full size.
                        let (w, h) = (video.and_then(|v| v["width"].as_f64()).unwrap_or(1920.0), video.and_then(|v| v["height"].as_f64()).unwrap_or(1080.0));
                        let long = w.max(h);
                        let target_long: f64 = if vbps < 400_000.0 { 640.0 } else if vbps < 1_200_000.0 { 960.0 } else if vbps < 2_500_000.0 { 1280.0 } else { 1920.0 };
                        if long > target_long {
                            let idx = a.iter().position(|x| x == "-filter_complex").unwrap() + 1;
                            let scale = if w >= h { format!("scale={}:-2", target_long as u32) } else { format!("scale=-2:{}", target_long as u32) };
                            a[idx] = a[idx].replacen("[vout]", "[vpre]", 1) + &format!(";[vpre]{scale}[vout]");
                        }
                        vbps = vbps.round();
                        a.extend(["-c:v".into(), "libx264".into(), "-preset".into(), "medium".into(), "-b:v".into(), (vbps as u64).to_string()]);
                        a.extend(["-maxrate".into(), ((vbps * 1.2) as u64).to_string(), "-bufsize".into(), ((vbps * 2.0) as u64).to_string(), "-pix_fmt".into(), "yuv420p".into()]);
                    }
                    if na > 0 {
                        a.extend(["-c:a".into(), "aac".into(), "-b:a".into(), "96k".into()]);
                    }
                } else {
                    if let Some(v) = video {
                        a.extend(video_settings(v, &req.probe, 0));
                    }
                    for (k, s) in keep_audio.iter().enumerate() {
                        a.extend(audio_settings(s, k));
                    }
                }
                a.extend(movflags.iter().cloned());
                a.push(tmp_out.to_string_lossy().into_owned());
                run_with_progress(app, ffmpeg, &a, total, 0.0, 1.0)
            }
        }
    })();

    if let Err(e) = result {
        cleanup();
        return Err(if e == "cancelled" { e } else { format!("Couldn’t save the video: {e}") });
    }
    if req.target == "replace" {
        if let Err(e) = trash::delete(&source) {
            cleanup();
            return Err(format!("Couldn’t move the original to the trash: {e}"));
        }
    }
    std::fs::rename(&tmp_out, &dest).map_err(|e| {
        cleanup();
        e.to_string()
    })?;
    let _ = std::fs::remove_dir_all(&work);
    let _ = app.emit("export", Progress { progress: 1.0, eta: Some(0.0) });
    Ok(dest)
}

/// Saves the frame at `time` as a PNG next to the video ("clip frame 0m12s.png").
#[tauri::command]
pub async fn media_grab_frame<R: Runtime>(app: AppHandle<R>, path: String, time: f64) -> Result<String, String> {
    let tools = super::locate(&app).ok_or("needs-tools")?;
    let saved = tauri::async_runtime::spawn_blocking(move || {
        let src = PathBuf::from(&path);
        let stem = src.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
        let t = time.max(0.0);
        let label = format!("{}m{:02}s", (t / 60.0).floor() as u64, (t % 60.0).floor() as u64);
        let dest = unique_path(src.parent().ok_or("no folder")?, &format!("{stem} frame {label}"), "png");
        let mut cmd = crate::tools::quiet_command(&tools.ffmpeg);
        cmd.args(["-v", "error", "-ss", &fmt_t(t), "-i"]).arg(&src).args(["-frames:v", "1", "-y"]).arg(&dest);
        super::run(cmd)?;
        Ok::<_, String>(dest)
    })
    .await
    .map_err(|e| e.to_string())??;
    let _ = app.asset_protocol_scope().allow_file(&saved);
    Ok(saved.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn matched_video_settings() {
        let probe = serde_json::json!({ "format": { "bit_rate": "8500000" }, "streams": [
            { "codec_type": "video", "codec_name": "h264", "profile": "High", "pix_fmt": "yuv420p", "bit_rate": "8200000",
              "avg_frame_rate": "30000/1001", "color_primaries": "bt709", "color_transfer": "bt709", "color_space": "bt709" },
            { "codec_type": "audio", "codec_name": "aac", "bit_rate": "256000", "sample_rate": "48000", "channels": 2 } ] });
        let v = &probe["streams"][0];
        let a = video_settings(v, &probe, 0);
        let s = a.join(" ");
        assert!(s.contains("-c:v:0 libx264"), "{s}");
        assert!(s.contains("-b:v 8200000"), "{s}");
        assert!(s.contains("-profile:v high"), "{s}");
        assert!(s.contains("-pix_fmt yuv420p"), "{s}");
        assert!(s.contains("-color_primaries bt709"), "{s}");
        assert!(s.contains("-g 60"), "{s}");
        let au = audio_settings(&probe["streams"][1], 0).join(" ");
        assert_eq!(au, "-c:a:0 aac -b:a:0 256000 -ar:a:0 48000 -ac:a:0 2");
    }

    #[test]
    fn hevc_and_pcm_map_to_matching_encoders() {
        assert_eq!(video_encoder("hevc"), "libx265");
        assert_eq!(audio_encoder("pcm_s24le"), "pcm_s24le");
        let s = audio_settings(&serde_json::json!({ "codec_name": "flac", "bit_rate": "900000", "sample_rate": "44100", "channels": 2 }), 1).join(" ");
        assert!(!s.contains("-b:a"), "lossless audio has no bitrate target: {s}");
    }
}
