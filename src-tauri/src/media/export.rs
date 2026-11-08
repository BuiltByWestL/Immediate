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

use super::convert::{self, Crop};
use crate::files::unique_path;

fn one() -> f64 {
    1.0
}

fn yes() -> bool {
    true
}

// Tone bands, where laptop and phone speakers can play them (MediaEditor.svelte uses the same).
const BASS_HZ: u32 = 200;
const MID_HZ: u32 = 1000;
const MID_Q: f64 = 0.7;
const TREBLE_HZ: u32 = 4000;

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
    /// Fade in / out lengths in seconds (0 = none), on the saved result.
    #[serde(default)]
    fade_in: f64,
    #[serde(default)]
    fade_out: f64,
    /// Even out loudness (EBU R128, -16 LUFS: a common level for speech and music online).
    #[serde(default)]
    normalize: bool,
    /// Tone in dB: bass (shelf below BASS_HZ), mid (peak at MID_HZ), treble (shelf above TREBLE_HZ).
    #[serde(default)]
    eq: [f64; 3],
    #[serde(default)]
    denoise: bool,
    /// Gentle compressor that evens out loud and quiet parts.
    #[serde(default)]
    compress: bool,
    /// Picture: clockwise turn in degrees (0/90/180/270), mirror, crop of the turned picture.
    #[serde(default)]
    rotate: u32,
    #[serde(default)]
    flip: bool,
    #[serde(default)]
    crop: Option<Crop>,
    /// Playback speed of the result (1 = unchanged; 0.25–4).
    #[serde(default = "one")]
    speed: f64,
    /// With a speed change: keep the pitch (true), or let it follow like a tape (false).
    #[serde(default = "yes")]
    keep_pitch: bool,
    /// Which audio and subtitle tracks to keep, by their order in the file (missing = keep).
    #[serde(default)]
    keep_audio: Vec<bool>,
    #[serde(default)]
    keep_subs: Vec<bool>,
    /// A subtitle file (.srt, .vtt, .ass) to add as a track, retimed to the kept parts.
    #[serde(default)]
    subtitle_file: Option<String>,
    /// "" (the edited file), "audio" (the sound only) or "gif" (an animated GIF, no sound).
    #[serde(default)]
    output: String,
    /// Audio files: changed tags (title, artist, album, album_artist, date, track, genre);
    /// an empty value removes the tag.
    #[serde(default)]
    tags: std::collections::BTreeMap<String, String>,
    /// Audio files: the cover art. None keeps it, "" removes it, a path uses that picture.
    #[serde(default)]
    cover: Option<String>,
    /// Audio files: a new sample rate (Hz) and bit depth (16/24, for WAV, FLAC and AIFF).
    #[serde(default)]
    sample_rate: Option<u32>,
    #[serde(default)]
    bit_depth: Option<u32>,
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
    run_ffmpeg_tail(app, ffmpeg, args, total, base, share, event, cancellable).map(|_| ())
}

/// `run_ffmpeg`, returning the last lines ffmpeg printed (filters like loudnorm report there).
#[allow(clippy::too_many_arguments)]
fn run_ffmpeg_tail<R: Runtime>(app: &AppHandle<R>, ffmpeg: &Path, args: &[String], total: f64, base: f64, share: f64, event: &str, cancellable: bool) -> Result<Vec<String>, String> {
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
            if tail.len() > 60 {
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
        Some(Ok(s)) if s.success() => Ok(tail),
        _ => {
            let msg = tail.iter().rev().find(|l| !l.trim().is_empty()).cloned().unwrap_or_else(|| "ffmpeg stopped".into());
            Err(msg)
        }
    }
}

fn fmt_t(t: f64) -> String {
    format!("{:.6}", t.max(0.0))
}

/// The sound settings that come before loudness: volume, noise reduction, tone, leveling. The
/// live preview plays the same steps in the same order (MediaEditor.svelte).
pub(crate) fn shaping_chain(gain_db: f64, eq: [f64; 3], denoise: bool, compress: bool) -> Vec<String> {
    let mut chain: Vec<String> = Vec::new();
    if gain_db.abs() > 0.01 {
        chain.push(format!("volume={gain_db:.2}dB"));
    }
    if denoise {
        chain.push("afftdn=nf=-25".into());
    }
    let [bass, mid, treble] = eq;
    if bass.abs() > 0.05 {
        // Q 0.707 matches the Web Audio shelves heard while playing (slope 1).
        chain.push(format!("bass=g={bass:.1}:f={BASS_HZ}:t=q:w=0.707"));
    }
    if mid.abs() > 0.05 {
        chain.push(format!("equalizer=f={MID_HZ}:t=q:w={MID_Q}:g={mid:.1}"));
    }
    if treble.abs() > 0.05 {
        chain.push(format!("treble=g={treble:.1}:f={TREBLE_HZ}:t=q:w=0.707"));
    }
    if compress {
        chain.push("acompressor=threshold=-20dB:ratio=3:attack=15:release=250:makeup=2".into());
    }
    chain
}

/// "Even out loudness" as one fixed volume change: the gain (dB) that brings the track to -16
/// LUFS. One gain for the whole file keeps the loud and quiet parts as they were (ffmpeg's
/// one-pass loudnorm rides the level as it goes). When that gain would push peaks past -1.5 dB,
/// `true` asks for a peak limiter after it, which touches only those peaks.
pub(crate) fn loudness_gain(integrated: f64, true_peak: f64) -> (f64, bool) {
    let gain = (-16.0 - integrated).clamp(-30.0, 30.0);
    (gain, true_peak + gain > -1.5)
}

/// The filters for "Even out loudness" (see `loudness_gain`). The live preview mirrors the
/// limiter with a fast, hard DynamicsCompressor at the same level.
pub(crate) fn loudness_filters(integrated: f64, true_peak: f64) -> Vec<String> {
    let (gain, limit) = loudness_gain(integrated, true_peak);
    let mut f = vec![format!("volume={gain:.2}dB")];
    if limit {
        // -1.5 dB is 0.841 linear; level=0 stops alimiter from raising the result again.
        f.push("alimiter=limit=0.841:attack=1:release=50:level=0".into());
    }
    f
}

/// The ffmpeg arguments that measure audio track `track` over the kept `segs`, after `chain`.
pub(crate) fn measure_args(source: &str, segs: &[Segment], track: usize, chain: &[String]) -> Vec<String> {
    let mut fc = String::new();
    let mut ins = String::new();
    for (i, s) in segs.iter().enumerate() {
        fc.push_str(&format!("[0:a:{track}]atrim=start={}:end={},asetpts=PTS-STARTPTS[s{i}];", fmt_t(s.start), fmt_t(s.end)));
        ins.push_str(&format!("[s{i}]"));
    }
    fc.push_str(&format!("{ins}concat=n={}:v=0:a=1", segs.len()));
    for c in chain {
        fc.push(',');
        fc.push_str(c);
    }
    fc.push_str(",loudnorm=I=-16:TP=-1.5:LRA=11:print_format=json[m]");
    ["-i", source, "-filter_complex", &fc, "-map", "[m]", "-f", "null", "-"].iter().map(|s| s.to_string()).collect()
}

/// Reads (integrated LUFS, true peak dBTP) from loudnorm's report in ffmpeg's messages.
pub(crate) fn parse_loudness(messages: &str) -> Result<(f64, f64), String> {
    let json = messages.rfind('{').and_then(|a| messages[a..].find('}').map(|b| &messages[a..=a + b])).ok_or("Couldn’t measure the loudness.")?;
    let v: Value = serde_json::from_str(json).map_err(|e| e.to_string())?;
    let num = |k: &str| v[k].as_str().and_then(|s| s.parse::<f64>().ok()).filter(|x| x.is_finite());
    Ok((num("input_i").ok_or("Silent track.")?, num("input_tp").unwrap_or(0.0)))
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

/// Encoder settings that reproduce the source stream as closely as possible (with `enc`, in
/// another codec the new container can hold).
fn video_settings(v: &Value, probe: &Value, out_index: usize, enc: Option<&'static str>) -> Vec<String> {
    let codec = v["codec_name"].as_str().unwrap_or("h264");
    let enc = enc.unwrap_or_else(|| video_encoder(codec));
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
    if enc == "libvpx-vp9" {
        // Good quality at a usable speed (the default "good" deadline at cpu-used 0 is very slow).
        a.extend(["-row-mt".into(), "1".into(), "-deadline".into(), "good".into(), "-cpu-used".into(), "4".into()]);
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
    // FLAC and ALAC keep the source's bit depth (filters hand them float samples otherwise).
    if matches!(codec, "flac" | "alac") {
        let fmt = s["sample_fmt"].as_str().unwrap_or_default();
        if fmt.starts_with("s16") {
            a.extend([format!("-sample_fmt:a:{out_index}"), (if codec == "alac" { "s16p" } else { "s16" }).into()]);
        } else if fmt.starts_with("s32") {
            a.extend([format!("-sample_fmt:a:{out_index}"), (if codec == "alac" { "s32p" } else { "s32" }).into()]);
            if s["bits_per_raw_sample"].as_str() == Some("24") {
                a.extend([format!("-bits_per_raw_sample:a:{out_index}"), "24".into()]);
            }
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

/// Encoder settings for saving audio as another format, chosen by file extension.
fn audio_for_ext(ext: &str, s: &Value, i: usize) -> Vec<String> {
    let codec_name = s["codec_name"].as_str().unwrap_or_default();
    let deep = codec_name.contains("24") || codec_name.contains("32") || s["bits_per_raw_sample"].as_str() == Some("24");
    let (codec, rate): (&str, Option<&str>) = match ext {
        "mp3" => ("libmp3lame", Some("192k")),
        "m4a" | "aac" | "m4b" => ("aac", Some("192k")),
        "opus" => ("libopus", Some("128k")),
        "ogg" | "oga" => ("libvorbis", Some("192k")),
        "flac" => ("flac", None),
        "wav" | "bwf" => (if deep { "pcm_s24le" } else { "pcm_s16le" }, None),
        "aif" | "aiff" => (if deep { "pcm_s24be" } else { "pcm_s16be" }, None),
        _ => ("aac", Some("192k")),
    };
    let mut a = vec![format!("-c:a:{i}"), codec.into()];
    if let Some(r) = rate {
        a.extend([format!("-b:a:{i}"), r.into()]);
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

/// One sound track's encoder settings with a chosen sample rate and bit depth (bit depth applies
/// to WAV, AIFF and FLAC).
fn with_format(args: Vec<String>, k: usize, ext: &str, rate: Option<u32>, depth: Option<u32>) -> Vec<String> {
    let ar = format!("-ar:a:{k}");
    let depth_keys = [format!("-sample_fmt:a:{k}"), format!("-bits_per_raw_sample:a:{k}")];
    let mut out: Vec<String> = Vec::new();
    let mut it = args.into_iter();
    while let Some(x) = it.next() {
        // Drop the settings being replaced (each is a flag and its value).
        if (rate.is_some() && x == ar) || (depth.is_some() && ext == "flac" && depth_keys.contains(&x)) {
            it.next();
            continue;
        }
        out.push(x);
    }
    if let Some(r) = rate {
        out.extend([ar, r.to_string()]);
    }
    if let Some(d) = depth.filter(|d| matches!(d, 16 | 24)) {
        let codec = format!("-c:a:{k}");
        let pcm = match ext {
            "wav" | "bwf" => Some(format!("pcm_s{d}le")),
            "aif" | "aiff" => Some(format!("pcm_s{d}be")),
            _ => None,
        };
        if let (Some(pcm), Some(i)) = (pcm, out.iter().position(|x| *x == codec)) {
            out[i + 1] = pcm;
        } else if ext == "flac" {
            out.extend([format!("-sample_fmt:a:{k}"), (if d == 16 { "s16" } else { "s32" }).into()]);
            if d == 24 {
                out.extend([format!("-bits_per_raw_sample:a:{k}"), "24".into()]);
            }
        }
    }
    out
}

fn keep(flags: &[bool], i: usize) -> bool {
    flags.get(i).copied().unwrap_or(true)
}

/// Runs ffmpeg for a quick side job (subtitle conversion, the final subtitle mux).
fn run_quiet(ffmpeg: &Path, args: &[String]) -> Result<(), String> {
    let out = crate::tools::quiet_command(ffmpeg).args(["-hide_banner", "-nostats", "-y"]).args(args).output().map_err(|e| e.to_string())?;
    if out.status.success() {
        Ok(())
    } else {
        let err = String::from_utf8_lossy(&out.stderr);
        Err(err.lines().rev().find(|l| !l.trim().is_empty()).unwrap_or("ffmpeg stopped").to_owned())
    }
}

/// A subtitle track to add to the saved file: an SRT file and its language and title.
struct ExtraSubs {
    file: PathBuf,
    language: Option<String>,
    title: Option<String>,
}

fn export<R: Runtime>(app: &AppHandle<R>, ffmpeg: &Path, req: &ExportRequest) -> Result<PathBuf, String> {
    let source = PathBuf::from(&req.source);
    let src_ext = ext_of(&source);
    let segs: Vec<Segment> = req.segments.iter().copied().filter(|s| s.end - s.start > 0.01).collect();
    if segs.is_empty() {
        return Err("Nothing is left to save: every part was cut out.".into());
    }
    let total: f64 = segs.iter().map(|s| s.end - s.start).sum();
    let speed = if req.speed.is_finite() && req.speed > 0.0 { req.speed.clamp(0.25, 4.0) } else { 1.0 };
    let sped = (speed - 1.0).abs() > 1e-6;
    // Length of the result: progress and fades are measured on it.
    let out_total = total / speed;
    let gif = req.output == "gif";
    let audio_only = req.output == "audio";
    let video = streams(&req.probe, "video").into_iter().find(|v| v["disposition"]["attached_pic"] != 1);
    let has_video = video.is_some();
    let video_out = has_video && !audio_only;
    if gif && !has_video {
        return Err("Only a video can be saved as a GIF.".into());
    }
    // Tracks to write, each with its position among the file's tracks of that kind.
    let audio: Vec<(usize, &Value)> = if req.mute || gif {
        vec![]
    } else {
        streams(&req.probe, "audio").into_iter().enumerate().filter(|(i, _)| keep(&req.keep_audio, *i)).take(if audio_only { 1 } else { usize::MAX }).collect()
    };
    if audio_only && audio.is_empty() {
        return Err("There’s no sound to save: the sound is removed or every track is turned off.".into());
    }
    let subs: Vec<(usize, &Value)> = if video_out && !gif { streams(&req.probe, "subtitle").into_iter().enumerate().filter(|(i, _)| keep(&req.keep_subs, *i)).collect() } else { vec![] };

    // Fades, loudness, tone and speed change the sound over the whole result, so they need the
    // frame-exact path (for audio files that costs nothing extra: audio is re-encoded either way).
    let tone = req.eq.iter().any(|g| g.abs() > 0.05) || req.denoise || req.compress;
    let shaping = !audio.is_empty() && (req.fade_in > 0.0 || req.fade_out > 0.0 || req.normalize || tone || sped);
    let turned = video_out && (!req.rotate.is_multiple_of(360) || req.flip);
    let crop = if video_out { req.crop.and_then(Crop::effective) } else { None };
    let dest_ext = (req.target == "path").then(|| req.dest.as_deref().map(|d| ext_of(Path::new(d)))).flatten();

    // The sound saved as another audio format is converted: Save As on an audio file, or a
    // video's sound in a type that can't hold its codec as it is.
    let (ext, convert_to): (String, Option<String>) = if gif {
        ("gif".into(), None)
    } else if audio_only {
        let codec = audio[0].1["codec_name"].as_str().unwrap_or_default();
        let natural = convert::natural_audio_ext(codec);
        let e = dest_ext.clone().unwrap_or_else(|| natural.to_owned());
        let copyable = e == natural || e == "mka";
        (e.clone(), (!copyable).then_some(e))
    } else if req.mode == "smaller" {
        ((if has_video { "mp4" } else { "m4a" }).into(), None)
    } else {
        match dest_ext.clone().filter(|e| *e != src_ext) {
            Some(e) if !has_video => (e.clone(), Some(e)),
            Some(e) => (e, None),
            None => (src_ext.clone(), None),
        }
    };
    let same_container = ext == src_ext;
    // Turning the picture is lossless where the container carries a display matrix; cropping
    // and speed always redraw it.
    let redraw = video_out && (crop.is_some() || sped || gif || (turned && !convert::rotates_losslessly(&ext)));
    // A new sample rate or bit depth (audio files) means re-encoding the sound.
    let src_rate = audio.first().and_then(|(_, s)| s["sample_rate"].as_str()?.parse::<u32>().ok());
    let reformat = !has_video && (req.sample_rate.is_some_and(|r| Some(r) != src_rate) || req.bit_depth.is_some());
    // Lossless sound (WAV, FLAC, ALAC) loses nothing when re-encoded, so it is always cut that
    // way: to the exact sample, with a correct length in the header (copying FLAC keeps the old one).
    let lossless_sound = !has_video && audio.first().is_some_and(|(_, s)| is_pcm_or_lossless(s["codec_name"].as_str().unwrap_or_default()));
    let mode = match req.mode.as_str() {
        _ if gif => "accurate",
        "smaller" if !audio_only => "smaller",
        _ if convert_to.is_some() || shaping || redraw || reformat || lossless_sound => "accurate",
        "accurate" => "accurate",
        // Changing the volume needs the audio re-encoded; the video can still be copied.
        _ => "lossless",
    };
    if mode == "lossless" && video_out && !same_container {
        let codec = video.map(|v| v["codec_name"].as_str().unwrap_or_default()).unwrap_or_default();
        if !convert::video_fits(codec, &ext) {
            return Err(format!("{} can’t hold this video’s picture as it is. Choose Frame-exact to convert it, or save as MKV.", ext.to_uppercase()));
        }
    }

    let dest = destination(req, &ext)?;
    let dir = dest.parent().ok_or("no folder")?.to_path_buf();
    let stem = dest.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
    let tmp_out = dir.join(format!(".{stem}.immediate-tmp.{ext}"));
    let tmp_subs = dir.join(format!(".{stem}.immediate-subs.{ext}"));
    let work = std::env::temp_dir().join(format!("immediate-export-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&work);
    let cleanup = || {
        let _ = std::fs::remove_dir_all(&work);
        let _ = std::fs::remove_file(&tmp_out);
        let _ = std::fs::remove_file(&tmp_subs);
    };

    let movflags: Vec<String> = if matches!(ext.as_str(), "mp4" | "m4v" | "mov" | "m4a") { vec!["-movflags".into(), "+faststart".into()] } else { vec![] };
    let gain: Vec<String> = if req.gain_db.abs() > 0.01 && !audio.is_empty() { vec!["-af".into(), format!("volume={:.2}dB", req.gain_db)] } else { vec![] };
    let source_cw = video.map(convert::source_rotation).unwrap_or(0);
    let display = if mode == "lossless" && turned { convert::display_args(source_cw, req.rotate, req.flip) } else { vec![] };
    // How many subtitle tracks the main pass writes (added ones come after them).
    let mut subs_written = 0usize;

    let result = (|| -> Result<(), String> {
        match mode {
            "lossless" => {
                // Streams to keep and how to write each: copied, unless the new container can't
                // hold it as it is.
                let mut maps: Vec<String> = Vec::new();
                let mut codecs: Vec<String> = vec!["-c".into(), "copy".into()];
                if video_out {
                    maps.extend(["-map".into(), "0:v?".into()]);
                }
                for (j, (i, s)) in audio.iter().enumerate() {
                    maps.extend(["-map".into(), format!("0:a:{i}")]);
                    let codec = s["codec_name"].as_str().unwrap_or_default();
                    if !gain.is_empty() {
                        codecs.extend(audio_settings(s, j));
                    } else if !same_container && !convert::audio_fits(codec, &ext) {
                        let (enc, br) = convert::audio_fallback(&ext);
                        codecs.extend([format!("-c:a:{j}"), enc.into(), format!("-b:a:{j}"), br.into()]);
                    }
                }
                for (i, s) in &subs {
                    let codec = s["codec_name"].as_str().unwrap_or_default();
                    let how = if same_container { Some("copy") } else { convert::subtitle_codec(codec, &ext) };
                    if let Some(how) = how {
                        maps.extend(["-map".into(), format!("0:s:{i}")]);
                        if how != "copy" {
                            codecs.extend([format!("-c:s:{subs_written}"), how.into()]);
                        }
                        subs_written += 1;
                    }
                }
                if same_container {
                    // Fonts attached to MKV subtitles, timecode and other data tracks.
                    maps.extend(["-map".into(), "0:t?".into(), "-map".into(), "0:d?".into()]);
                }
                // Each part starts on a keyframe (the UI sends keyframe-aligned starts). Seeking
                // before the input backs up to an earlier keyframe, so: a coarse seek there for
                // speed, then an exact one after the input that lands on the keyframe itself.
                let one = |seg: &Segment, out: &Path, base: f64, share: f64, turn: &[String]| -> Result<(), String> {
                    let coarse = (seg.start - 10.0).max(0.0);
                    let mut a: Vec<String> = Vec::new();
                    if coarse > 0.0 {
                        a.extend(["-ss".into(), fmt_t(coarse)]);
                    }
                    a.extend(turn.iter().cloned());
                    a.extend(["-i".into(), req.source.clone()]);
                    if seg.start - coarse > 0.0005 {
                        a.extend(["-ss".into(), fmt_t(seg.start - coarse - 0.0005)]);
                    }
                    a.extend(["-to".into(), fmt_t(seg.end - coarse)]);
                    a.extend(maps.iter().cloned());
                    a.extend(codecs.iter().cloned());
                    a.push("-ignore_unknown".into());
                    a.extend(gain.iter().cloned());
                    a.extend(["-avoid_negative_ts".into(), "make_zero".into()]);
                    a.extend(movflags.iter().cloned());
                    a.push(out.to_string_lossy().into_owned());
                    run_with_progress(app, ffmpeg, &a, seg.end - seg.start, base, share)
                };
                if segs.len() == 1 {
                    one(&segs[0], &tmp_out, 0.0, 1.0, &display)
                } else {
                    let mut list = String::new();
                    let mut done = 0.0;
                    for (i, seg) in segs.iter().enumerate() {
                        let part = work.join(format!("part{i}.{ext}"));
                        let share = (seg.end - seg.start) / total * 0.9;
                        one(seg, &part, done, share, &[])?;
                        done += share;
                        list.push_str(&format!("file '{}'\n", part.to_string_lossy().replace('\\', "/").replace('\'', "'\\''")));
                    }
                    let list_path = work.join("list.txt");
                    std::fs::write(&list_path, list).map_err(|e| e.to_string())?;
                    let mut a: Vec<String> = display.clone();
                    a.extend(["-f".into(), "concat".into(), "-safe".into(), "0".into(), "-i".into(), list_path.to_string_lossy().into_owned()]);
                    a.extend(["-i".into(), req.source.clone(), "-map".into(), "0".into(), "-map_metadata".into(), "1".into(), "-map_chapters".into(), "-1".into()]);
                    a.extend(["-c".into(), "copy".into(), "-ignore_unknown".into()]);
                    a.extend(movflags.iter().cloned());
                    a.push(tmp_out.to_string_lossy().into_owned());
                    run_with_progress(app, ffmpeg, &a, total, 0.9, 0.1)
                }
            }
            _ => {
                // Frame-exact: trim + concat filters, then encode.
                let v = if video_out { video } else { None };
                let mut fc = String::new();
                let mut concat_in = String::new();
                for (i, s) in segs.iter().enumerate() {
                    if v.is_some() {
                        fc.push_str(&format!("[0:v:0]trim=start={}:end={},setpts=PTS-STARTPTS[v{i}];", fmt_t(s.start), fmt_t(s.end)));
                        concat_in.push_str(&format!("[v{i}]"));
                    }
                    for (k, (orig, _)) in audio.iter().enumerate() {
                        fc.push_str(&format!("[0:a:{orig}]atrim=start={}:end={},asetpts=PTS-STARTPTS[a{k}_{i}];", fmt_t(s.start), fmt_t(s.end)));
                        concat_in.push_str(&format!("[a{k}_{i}]"));
                    }
                }
                let nv = usize::from(v.is_some());
                let na = audio.len();
                let mut outs = String::new();
                if nv == 1 {
                    outs.push_str("[vout]");
                }
                for k in 0..na {
                    outs.push_str(&format!("[aout{k}]"));
                }
                fc.push_str(&format!("{concat_in}concat=n={}:v={nv}:a={na}{outs}", segs.len()));
                // The picture: turn, mirror, crop, speed, then what the output needs.
                if nv == 1 {
                    let mut chain = convert::picture_filters(req.rotate, req.flip, crop);
                    if sped {
                        chain.push(format!("setpts=PTS/{speed:.4}"));
                    }
                    if gif {
                        // 15 frames a second at up to 480 px wide, one palette for the clip.
                        chain.push("fps=15,scale='min(480,iw)':-2:flags=lanczos,split[g0][g1];[g0]palettegen=stats_mode=diff[gp];[g1][gp]paletteuse=dither=bayer:bayer_scale=4:diff_mode=rectangle".into());
                    }
                    if !chain.is_empty() {
                        fc = fc.replacen("[vout]", "[vraw]", 1);
                        fc.push_str(&format!(";[vraw]{}[vout]", chain.join(",")));
                    }
                }
                // The sound: volume, tone… then speed, loudness, fades.
                let pre = shaping_chain(req.gain_db, req.eq, req.denoise, req.compress);
                let mut fades: Vec<String> = Vec::new();
                if req.fade_in > 0.0 {
                    fades.push(format!("afade=t=in:st=0:d={:.3}", req.fade_in.min(out_total)));
                }
                if req.fade_out > 0.0 {
                    let d = req.fade_out.min(out_total);
                    fades.push(format!("afade=t=out:st={:.3}:d={d:.3}", (out_total - d).max(0.0)));
                }
                // Loudness is measured first (each track on its own), then applied as one gain.
                let measure_share = if req.normalize && na > 0 { 0.3 } else { 0.0 };
                for (k, (orig, s)) in audio.iter().enumerate() {
                    let mut chain = pre.clone();
                    if req.keep_pitch {
                        chain.extend(convert::atempo(speed));
                    } else if sped {
                        // Like a tape: play the samples faster or slower, then back to the rate.
                        let sr = s["sample_rate"].as_str().and_then(|r| r.parse::<f64>().ok()).unwrap_or(48000.0);
                        chain.push(format!("asetrate={},aresample={}", (sr * speed).round(), sr));
                    }
                    if req.normalize {
                        let share = measure_share / na as f64;
                        let tail = run_ffmpeg_tail(app, ffmpeg, &measure_args(&req.source, &segs, *orig, &pre), total, share * k as f64, share, "export", true)?;
                        // A silent track has no loudness to even out; leave it as it is.
                        if let Ok((i, tp)) = parse_loudness(&tail.join("\n")) {
                            chain.extend(loudness_filters(i, tp));
                        }
                    }
                    chain.extend(fades.iter().cloned());
                    if !chain.is_empty() {
                        fc = fc.replacen(&format!("[aout{k}]"), &format!("[aout{k}pre]"), 1);
                        fc.push_str(&format!(";[aout{k}pre]{}[aout{k}]", chain.join(",")));
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
                // Filtered tracks don't inherit their tags: carry each sound track's language and
                // title over (and the picture's, when it isn't a GIF).
                let mut tags = |kind: &str, n: usize, s: &Value| {
                    for key in ["language", "title"] {
                        if let Some(val) = s["tags"][key].as_str() {
                            a.extend([format!("-metadata:s:{kind}:{n}"), format!("{key}={val}")]);
                        }
                    }
                };
                if let (Some(v), false) = (v, gif) {
                    tags("v", 0, v);
                }
                for (k, (_, s)) in audio.iter().enumerate() {
                    tags("a", k, s);
                }
                if gif {
                    a.extend(["-loop".into(), "0".into()]);
                } else if mode == "smaller" {
                    let target_bits = req.size_mb.unwrap_or(10.0) * 8.0 * 1_000_000.0 * 0.94;
                    let audio_bps = if na > 0 { 96_000.0 * na as f64 } else { 0.0 };
                    let mut vbps = (target_bits / out_total - audio_bps).max(80_000.0);
                    // Never bigger than it was: a short clip that already fits keeps its bitrate.
                    if let Some(src) = v.and_then(|v| num(&v["bit_rate"])).or_else(|| num(&req.probe["format"]["bit_rate"])) {
                        vbps = vbps.min(src);
                    }
                    if nv == 1 {
                        // Fewer pixels when the bitrate is too low for the full size.
                        let (w, h) = (v.and_then(|v| v["width"].as_f64()).unwrap_or(1920.0), v.and_then(|v| v["height"].as_f64()).unwrap_or(1080.0));
                        let long = w.max(h);
                        let target_long: f64 = if vbps < 400_000.0 { 640.0 } else if vbps < 1_200_000.0 { 960.0 } else if vbps < 2_500_000.0 { 1280.0 } else { 1920.0 };
                        if long > target_long {
                            let idx = a.iter().position(|x| x == "-filter_complex").unwrap() + 1;
                            let t = target_long as u32;
                            // Fit the long side, whichever way the (turned, cropped) picture faces.
                            let scale = format!("scale='if(gte(iw,ih),min({t},iw),-2)':'if(gte(iw,ih),-2,min({t},ih))'");
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
                    if let Some(v) = v {
                        let codec = v["codec_name"].as_str().unwrap_or_default();
                        let other = (!same_container && !convert::video_fits(codec, &ext)).then(|| convert::video_fallback(&ext));
                        a.extend(video_settings(v, &req.probe, 0, other));
                    }
                    for (k, (_, s)) in audio.iter().enumerate() {
                        let codec = s["codec_name"].as_str().unwrap_or_default();
                        let settings = match &convert_to {
                            Some(e) => audio_for_ext(e, s, k),
                            None if !same_container && !convert::audio_fits(codec, &ext) => {
                                let (enc, br) = convert::audio_fallback(&ext);
                                vec![format!("-c:a:{k}"), enc.into(), format!("-b:a:{k}"), br.into()]
                            }
                            None => audio_settings(s, k),
                        };
                        a.extend(if has_video { settings } else { with_format(settings, k, &ext, req.sample_rate, req.bit_depth) });
                    }
                }
                a.extend(movflags.iter().cloned());
                a.push(tmp_out.to_string_lossy().into_owned());
                run_with_progress(app, ffmpeg, &a, out_total, measure_share, 1.0 - measure_share)
            }
        }
    })();
    if let Err(e) = result {
        cleanup();
        return Err(if e == "cancelled" { e } else { format!("Couldn’t save: {e}") });
    }

    // Subtitles that follow the edit: the source's text tracks when the picture was re-encoded
    // (lossless saves copied them already), and an added subtitle file.
    if video_out && !gif {
        if let Some(sub_codec) = convert::new_subtitle_codec(&ext) {
            let mut extra: Vec<ExtraSubs> = Vec::new();
            let mut add = |from: &Path, language: Option<String>, title: Option<String>, name: &str| -> Result<(), String> {
                let srt = work.join(format!("{name}.srt"));
                run_quiet(ffmpeg, &["-i".into(), from.to_string_lossy().into_owned(), "-map".into(), "0:s:0".into(), "-c:s".into(), "srt".into(), srt.to_string_lossy().into_owned()])?;
                let text = std::fs::read_to_string(&srt).map_err(|e| e.to_string())?;
                let cues = convert::retime(&convert::parse_srt(&text), &segs, speed);
                if !cues.is_empty() {
                    let out = work.join(format!("{name}-kept.srt"));
                    std::fs::write(&out, convert::write_srt(&cues)).map_err(|e| e.to_string())?;
                    extra.push(ExtraSubs { file: out, language, title });
                }
                Ok(())
            };
            let tag = |s: &Value, k: &str| s["tags"][k].as_str().map(str::to_owned);
            if mode != "lossless" {
                for (i, s) in &subs {
                    if convert::is_text_subtitle(s["codec_name"].as_str().unwrap_or_default()) {
                        // One track out of the source into its own file, then retimed.
                        let one = work.join(format!("src{i}.mkv"));
                        let pulled = run_quiet(ffmpeg, &["-i".into(), req.source.clone(), "-map".into(), format!("0:s:{i}"), "-c".into(), "copy".into(), one.to_string_lossy().into_owned()]);
                        if pulled.is_ok() {
                            let _ = add(&one, tag(s, "language"), tag(s, "title"), &format!("src{i}"));
                        }
                    }
                }
            }
            if let Some(f) = &req.subtitle_file {
                let title = Path::new(f).file_stem().map(|s| s.to_string_lossy().into_owned());
                if let Err(e) = add(Path::new(f), None, title, "added") {
                    cleanup();
                    return Err(format!("Couldn’t read the subtitle file: {e}"));
                }
            }
            if !extra.is_empty() {
                let mut a: Vec<String> = vec!["-i".into(), tmp_out.to_string_lossy().into_owned()];
                for x in &extra {
                    a.extend(["-i".into(), x.file.to_string_lossy().into_owned()]);
                }
                a.extend(["-map".into(), "0".into()]);
                for i in 0..extra.len() {
                    a.extend(["-map".into(), format!("{}:0", i + 1)]);
                }
                a.extend(["-c".into(), "copy".into()]);
                for (i, x) in extra.iter().enumerate() {
                    let n = subs_written + i;
                    a.extend([format!("-c:s:{n}"), sub_codec.into()]);
                    if let Some(l) = &x.language {
                        a.extend([format!("-metadata:s:s:{n}"), format!("language={l}")]);
                    }
                    if let Some(t) = &x.title {
                        a.extend([format!("-metadata:s:s:{n}"), format!("title={t}")]);
                    }
                }
                a.extend(movflags.iter().cloned());
                a.push(tmp_subs.to_string_lossy().into_owned());
                if let Err(e) = run_quiet(ffmpeg, &a).and_then(|_| std::fs::rename(&tmp_subs, &tmp_out).map_err(|e| e.to_string())) {
                    cleanup();
                    return Err(format!("Couldn’t add the subtitles: {e}"));
                }
            }
        } else if req.subtitle_file.is_some() {
            cleanup();
            return Err(format!("{} files can’t hold subtitles. Save as MP4 or MKV to add them.", ext.to_uppercase()));
        }
    }

    // Audio files: tags and cover art, in a quick copy of the result (the sound is not touched).
    if !has_video && req.output.is_empty() {
        let source_cover = streams(&req.probe, "video").iter().any(|v| v["disposition"]["attached_pic"] == 1);
        let holds_cover = matches!(ext.as_str(), "mp3" | "m4a" | "m4b" | "flac");
        let new_cover = req.cover.as_deref().filter(|c| !c.is_empty());
        let keep_cover = req.cover.is_none() && source_cover;
        if !req.tags.is_empty() || (holds_cover && (new_cover.is_some() || keep_cover)) {
            let mut a: Vec<String> = vec!["-i".into(), tmp_out.to_string_lossy().into_owned()];
            a.extend(["-map".into(), "0:a".into(), "-c".into(), "copy".into(), "-map_metadata".into(), "0".into()]);
            if holds_cover && (new_cover.is_some() || keep_cover) {
                let pic = new_cover.unwrap_or(&req.source);
                a.splice(2..2, ["-i".into(), pic.to_owned()]);
                a.extend(["-map".into(), "1:v:0".into(), "-disposition:v:0".into(), "attached_pic".into()]);
                // JPEG and PNG go in as they are; other pictures become JPEG.
                let as_is = keep_cover || matches!(ext_of(Path::new(pic)).as_str(), "jpg" | "jpeg" | "png");
                if !as_is {
                    a.extend(["-c:v".into(), "mjpeg".into(), "-q:v".into(), "2".into()]);
                }
            }
            for (k, v) in &req.tags {
                if matches!(k.as_str(), "title" | "artist" | "album" | "album_artist" | "date" | "track" | "genre" | "comment") {
                    a.extend(["-metadata".into(), format!("{k}={v}")]);
                }
            }
            a.extend(movflags.iter().cloned());
            a.push(tmp_subs.to_string_lossy().into_owned());
            if let Err(e) = run_quiet(ffmpeg, &a).and_then(|_| std::fs::rename(&tmp_subs, &tmp_out).map_err(|e| e.to_string())) {
                cleanup();
                return Err(format!("Couldn’t write the tags: {e}"));
            }
        }
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
    fn loudness_is_one_gain_with_a_limiter_only_when_needed() {
        // Quiet with headroom: plain gain to -16 LUFS.
        assert_eq!(loudness_gain(-26.0, -12.0), (10.0, false));
        // Quiet but peaky: the same gain, with the limiter catching the peaks.
        assert_eq!(loudness_gain(-31.3, -10.0), (15.3, true));
        assert_eq!(loudness_filters(-31.3, -10.0).len(), 2);
        // Too loud: turned down, never limited.
        assert_eq!(loudness_gain(-10.0, 0.0), (-6.0, false));
        assert_eq!(loudness_gain(-70.0, -60.0).0, 30.0);
        let report = "[Parsed_loudnorm_0 @ 0]\n{\n\t\"input_i\" : \"-21.75\",\n\t\"input_tp\" : \"-16.32\",\n\t\"input_lra\" : \"3.1\"\n}\n";
        assert_eq!(parse_loudness(report), Ok((-21.75, -16.32)));
        assert!(parse_loudness("{ \"input_i\" : \"-inf\" }").is_err());
    }

    #[test]
    fn sample_rate_and_bit_depth() {
        let s = |v: &[&str]| v.iter().map(|x| x.to_string()).collect::<Vec<_>>();
        let wav = s(&["-c:a:0", "pcm_s16le", "-ar:a:0", "44100", "-ac:a:0", "2"]);
        assert_eq!(with_format(wav.clone(), 0, "wav", Some(48000), Some(24)), s(&["-c:a:0", "pcm_s24le", "-ac:a:0", "2", "-ar:a:0", "48000"]));
        assert_eq!(with_format(wav.clone(), 0, "wav", None, None), wav);
        let flac = with_format(s(&["-c:a:0", "flac"]), 0, "flac", None, Some(24));
        assert_eq!(flac, s(&["-c:a:0", "flac", "-sample_fmt:a:0", "s32", "-bits_per_raw_sample:a:0", "24"]));
        // Lossy formats have no bit depth to set.
        assert_eq!(with_format(s(&["-c:a:0", "aac"]), 0, "m4a", None, Some(24)), s(&["-c:a:0", "aac"]));
    }

    #[test]
    fn measures_only_the_kept_parts_after_the_sound_chain() {
        let segs = [Segment { start: 0.0, end: 2.0 }, Segment { start: 5.0, end: 6.5 }];
        let chain = shaping_chain(3.0, [6.0, 0.0, 0.0], false, true);
        let a = measure_args("in.wav", &segs, 1, &chain).join(" ");
        assert!(a.contains("[0:a:1]atrim=start=5.000000:end=6.500000"), "{a}");
        assert!(a.contains("concat=n=2:v=0:a=1,volume=3.00dB,bass=g=6.0:f=200"), "{a}");
        assert!(a.contains("acompressor") && a.ends_with("print_format=json[m] -map [m] -f null -"), "{a}");
    }

    #[test]
    fn matched_video_settings() {
        let probe = serde_json::json!({ "format": { "bit_rate": "8500000" }, "streams": [
            { "codec_type": "video", "codec_name": "h264", "profile": "High", "pix_fmt": "yuv420p", "bit_rate": "8200000",
              "avg_frame_rate": "30000/1001", "color_primaries": "bt709", "color_transfer": "bt709", "color_space": "bt709" },
            { "codec_type": "audio", "codec_name": "aac", "bit_rate": "256000", "sample_rate": "48000", "channels": 2 } ] });
        let v = &probe["streams"][0];
        let a = video_settings(v, &probe, 0, None);
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
