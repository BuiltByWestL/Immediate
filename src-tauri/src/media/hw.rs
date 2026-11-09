//! Graphics-card video encoders. ffmpeg lists encoders it was built with, not ones this machine
//! can use (NVENC is listed without an NVIDIA card), so each candidate is tried once with a tiny
//! encode and the answer is kept for the rest of the run.

use std::collections::HashMap;
use std::path::{Path, PathBuf};
use std::sync::Mutex;

/// For one ffmpeg: codec → the encoder that works (None: none does).
type Found = Option<(PathBuf, HashMap<&'static str, Option<&'static str>>)>;
static WORKING: Mutex<Found> = Mutex::new(None);

/// Candidates for a codec, best first, for this OS.
fn candidates(codec: &str) -> &'static [&'static str] {
    match codec {
        #[cfg(target_os = "macos")]
        "h264" => &["h264_videotoolbox"],
        #[cfg(target_os = "macos")]
        "hevc" => &["hevc_videotoolbox"],
        #[cfg(not(target_os = "macos"))]
        "h264" => &["h264_nvenc", "h264_qsv", "h264_amf"],
        #[cfg(not(target_os = "macos"))]
        "hevc" => &["hevc_nvenc", "hevc_qsv", "hevc_amf"],
        _ => &[],
    }
}

fn works(ffmpeg: &Path, enc: &str) -> bool {
    crate::tools::quiet_command(ffmpeg)
        .args(["-hide_banner", "-v", "error", "-f", "lavfi", "-i", "color=c=black:s=320x240:r=30:d=0.2", "-pix_fmt", "yuv420p", "-c:v", enc, "-f", "null", "-"])
        .output()
        .map(|o| o.status.success())
        .unwrap_or(false)
}

/// The first graphics-card encoder for `codec` ("h264" or "hevc") that works here.
pub fn encoder(ffmpeg: &Path, codec: &str) -> Option<&'static str> {
    let mut cache = WORKING.lock().ok()?;
    if cache.as_ref().is_none_or(|(p, _)| p != ffmpeg) {
        *cache = Some((ffmpeg.to_path_buf(), HashMap::new()));
    }
    let (_, map) = cache.as_mut()?;
    let key: &'static str = match codec {
        "h264" => "h264",
        "hevc" => "hevc",
        _ => return None,
    };
    *map.entry(key).or_insert_with(|| candidates(codec).iter().copied().find(|e| works(ffmpeg, e)))
}

/// Settings for a graphics-card encoder at the source's bitrate.
pub fn settings(enc: &str, out_index: usize, bitrate: Option<u64>) -> Vec<String> {
    let mut a = vec![format!("-c:v:{out_index}"), enc.into()];
    if let Some(br) = bitrate {
        a.extend(["-b:v".into(), br.to_string(), "-maxrate".into(), (br * 3 / 2).to_string(), "-bufsize".into(), (br * 2).to_string()]);
    }
    a.extend(["-pix_fmt".into(), "yuv420p".into()]);
    if enc.ends_with("_nvenc") {
        a.extend(["-preset".into(), "p5".into()]);
    } else if enc.ends_with("_qsv") {
        a.extend(["-preset".into(), "medium".into()]);
    } else if enc.ends_with("_amf") {
        a.extend(["-quality".into(), "balanced".into()]);
    }
    a
}

/// Graphics-card encoders take 8-bit 4:2:0 here; anything else stays on the processor.
pub fn suits(pix_fmt: &str) -> bool {
    matches!(pix_fmt, "yuv420p" | "yuvj420p" | "nv12")
}

/// What Settings shows: the encoders that work, by codec.
#[tauri::command]
pub async fn media_hw_encoders<R: tauri::Runtime>(app: tauri::AppHandle<R>) -> Vec<String> {
    let Some(tools) = super::locate(&app) else { return vec![] };
    tauri::async_runtime::spawn_blocking(move || ["h264", "hevc"].iter().filter_map(|c| encoder(&tools.ffmpeg, c)).map(str::to_owned).collect())
        .await
        .unwrap_or_default()
}
