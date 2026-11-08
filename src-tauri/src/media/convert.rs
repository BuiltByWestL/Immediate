//! Pure decisions for saving video and audio: which streams a container can hold, the picture and
//! speed filters, and subtitles retimed to the kept parts. No ffmpeg runs here, so it is all
//! unit-tested.

use super::export::Segment;
use serde::Deserialize;

/// A crop in the displayed frame (after rotation and flip), as fractions of its width and height.
#[derive(Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Crop {
    pub x: f64,
    pub y: f64,
    pub w: f64,
    pub h: f64,
}

impl Crop {
    /// None when the crop keeps (nearly) the whole frame.
    pub fn effective(self) -> Option<Crop> {
        let c = Crop { x: self.x.clamp(0.0, 1.0), y: self.y.clamp(0.0, 1.0), w: self.w.clamp(0.01, 1.0), h: self.h.clamp(0.01, 1.0) };
        (c.w < 0.999 || c.h < 0.999).then_some(c)
    }
}

/// Containers that carry rotation and flip as a display matrix every player honors, so turning
/// the picture needs no re-encode.
pub fn rotates_losslessly(ext: &str) -> bool {
    matches!(ext, "mp4" | "m4v" | "mov")
}

/// Whether a container can hold this video codec as it is.
pub fn video_fits(codec: &str, ext: &str) -> bool {
    match ext {
        "mkv" => true,
        "mp4" | "m4v" => matches!(codec, "h264" | "hevc" | "av1" | "vp9" | "mpeg4" | "mpeg2video" | "mpeg1video"),
        "mov" => matches!(codec, "h264" | "hevc" | "av1" | "vp9" | "mpeg4" | "mpeg2video" | "prores" | "mjpeg" | "dnxhd" | "png" | "qtrle"),
        "webm" => matches!(codec, "vp8" | "vp9" | "av1"),
        _ => false,
    }
}

/// Whether a container can hold this audio codec as it is.
pub fn audio_fits(codec: &str, ext: &str) -> bool {
    match ext {
        "mkv" | "mka" => true,
        "mp4" | "m4v" | "m4a" => matches!(codec, "aac" | "mp3" | "alac" | "ac3" | "eac3" | "opus" | "flac"),
        "mov" => matches!(codec, "aac" | "mp3" | "alac" | "ac3" | "eac3" | "opus" | "flac") || codec.starts_with("pcm_"),
        "webm" => matches!(codec, "opus" | "vorbis"),
        _ => false,
    }
}

/// The encoder for audio a container can't hold as it is.
pub fn audio_fallback(ext: &str) -> (&'static str, &'static str) {
    if ext == "webm" { ("libopus", "128k") } else { ("aac", "192k") }
}

/// The encoder for video a container can't hold as it is (frame-exact saves into another format).
pub fn video_fallback(ext: &str) -> &'static str {
    if ext == "webm" { "libvpx-vp9" } else { "libx264" }
}

pub fn is_text_subtitle(codec: &str) -> bool {
    matches!(codec, "subrip" | "srt" | "ass" | "ssa" | "webvtt" | "mov_text" | "text")
}

/// How to write a subtitle track into `ext`: "copy", a codec to convert to, or None when the
/// container can't hold it (picture-based subtitles outside MKV, or formats with no subtitles).
pub fn subtitle_codec(codec: &str, ext: &str) -> Option<&'static str> {
    let text = is_text_subtitle(codec);
    match ext {
        "mkv" if codec == "mov_text" => Some("srt"),
        "mkv" => Some("copy"),
        "mp4" | "m4v" | "mov" if text => Some(if codec == "mov_text" { "copy" } else { "mov_text" }),
        "webm" if text => Some(if codec == "webvtt" { "copy" } else { "webvtt" }),
        _ => None,
    }
}

/// The subtitle codec for a new text track (an added file, or one retimed from the source).
pub fn new_subtitle_codec(ext: &str) -> Option<&'static str> {
    match ext {
        "mkv" => Some("srt"),
        "mp4" | "m4v" | "mov" => Some("mov_text"),
        "webm" => Some("webvtt"),
        _ => None,
    }
}

/// The file type that holds an audio codec as it is ("Save audio only" without re-encoding).
pub fn natural_audio_ext(codec: &str) -> &'static str {
    match codec {
        "aac" | "alac" => "m4a",
        "mp3" => "mp3",
        "opus" => "opus",
        "vorbis" => "ogg",
        "flac" => "flac",
        "ac3" => "ac3",
        "eac3" => "eac3",
        c if c.starts_with("pcm_") => "wav",
        _ => "mka",
    }
}

/// Speed change for audio, keeping the pitch. atempo takes 0.5–100 per step, so slower speeds
/// chain steps (0.25 = 0.5 × 0.5).
pub fn atempo(speed: f64) -> Vec<String> {
    let mut out = Vec::new();
    let mut s = speed;
    while s < 0.5 {
        out.push("atempo=0.5".to_owned());
        s /= 0.5;
    }
    if (s - 1.0).abs() > 1e-6 {
        out.push(format!("atempo={s:.4}"));
    }
    out
}

/// Turning (clockwise degrees), mirroring and cropping, in the order the preview shows them.
pub fn picture_filters(rotate: u32, flip: bool, crop: Option<Crop>) -> Vec<String> {
    let mut f: Vec<String> = Vec::new();
    match rotate % 360 {
        90 => f.push("transpose=1".into()),
        180 => f.push("hflip,vflip".into()),
        270 => f.push("transpose=2".into()),
        _ => {}
    }
    if flip {
        f.push("hflip".into());
    }
    if let Some(c) = crop.and_then(Crop::effective) {
        // Even sizes and offsets keep 4:2:0 video valid.
        f.push(format!(
            "crop=trunc(iw*{w:.5}/2)*2:trunc(ih*{h:.5}/2)*2:trunc(iw*{x:.5}/2)*2:trunc(ih*{y:.5}/2)*2",
            w = c.w,
            h = c.h,
            x = c.x,
            y = c.y
        ));
    }
    f
}

/// The source's own rotation (clockwise degrees) from ffprobe's display matrix.
pub fn source_rotation(stream: &serde_json::Value) -> u32 {
    let r = stream["side_data_list"]
        .as_array()
        .and_then(|l| l.iter().find_map(|d| d["rotation"].as_f64()))
        .or_else(|| stream["tags"]["rotate"].as_str().and_then(|s| s.parse::<f64>().ok()).map(|cw| -cw))
        .unwrap_or(0.0);
    // ffprobe reports counter-clockwise degrees.
    (((-r).round() as i64).rem_euclid(360)) as u32
}

/// Input options that turn the picture without re-encoding: a display matrix with the source's
/// rotation plus ours, then the mirror (ffmpeg applies the flip after the rotation).
pub fn display_args(source_cw: u32, rotate: u32, flip: bool) -> Vec<String> {
    if rotate.is_multiple_of(360) && !flip {
        return Vec::new();
    }
    let total = (source_cw + rotate) % 360;
    let mut a = vec!["-display_rotation:v:0".to_owned(), format!("{}", -(total as i64))];
    if flip {
        a.push("-display_hflip:v:0".into());
    }
    a
}

#[derive(Debug, Clone, PartialEq)]
pub struct Cue {
    pub start: f64,
    pub end: f64,
    pub text: String,
}

fn srt_time(s: &str) -> Option<f64> {
    let s = s.trim().replace(',', ".");
    let mut parts = s.split(':').rev();
    let sec: f64 = parts.next()?.parse().ok()?;
    let min: f64 = parts.next().unwrap_or("0").parse().ok()?;
    let hour: f64 = parts.next().unwrap_or("0").parse().ok()?;
    Some(hour * 3600.0 + min * 60.0 + sec)
}

/// Reads SubRip cues (lenient: any numbering, CRLF, a BOM, positions after the times).
pub fn parse_srt(text: &str) -> Vec<Cue> {
    let text = text.trim_start_matches('\u{feff}').replace("\r\n", "\n");
    let mut cues = Vec::new();
    for block in text.split("\n\n") {
        let mut lines = block.lines().skip_while(|l| !l.contains("-->"));
        let Some(timing) = lines.next() else { continue };
        let Some((a, b)) = timing.split_once("-->") else { continue };
        let b = b.split_whitespace().next().unwrap_or("");
        let (Some(start), Some(end)) = (srt_time(a), srt_time(b)) else { continue };
        let body: Vec<&str> = lines.collect();
        if end > start && !body.is_empty() {
            cues.push(Cue { start, end, text: body.join("\n") });
        }
    }
    cues
}

fn fmt_srt_time(t: f64) -> String {
    let ms = (t.max(0.0) * 1000.0).round() as u64;
    format!("{:02}:{:02}:{:02},{:03}", ms / 3_600_000, ms / 60_000 % 60, ms / 1000 % 60, ms % 1000)
}

/// Subtitles for the saved file: each cue clipped to the kept parts and moved to where that part
/// lands in the result, at the new speed.
pub fn retime(cues: &[Cue], segs: &[Segment], speed: f64) -> Vec<Cue> {
    let mut out = Vec::new();
    let mut offset = 0.0;
    for s in segs {
        for c in cues {
            let (a, b) = (c.start.max(s.start), c.end.min(s.end));
            if b - a > 0.05 {
                out.push(Cue { start: (a - s.start + offset) / speed, end: (b - s.start + offset) / speed, text: c.text.clone() });
            }
        }
        offset += s.end - s.start;
    }
    out
}

pub fn write_srt(cues: &[Cue]) -> String {
    cues.iter()
        .enumerate()
        .map(|(i, c)| format!("{}\n{} --> {}\n{}\n", i + 1, fmt_srt_time(c.start), fmt_srt_time(c.end), c.text))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn containers() {
        assert!(video_fits("h264", "mp4") && video_fits("prores", "mov") && !video_fits("prores", "mp4"));
        assert!(video_fits("vp9", "webm") && !video_fits("h264", "webm") && video_fits("anything", "mkv"));
        assert!(audio_fits("aac", "mp4") && !audio_fits("pcm_s16le", "mp4") && audio_fits("pcm_s16le", "mov"));
        assert!(!audio_fits("aac", "webm") && audio_fits("opus", "webm"));
        assert_eq!(subtitle_codec("subrip", "mp4"), Some("mov_text"));
        assert_eq!(subtitle_codec("mov_text", "mkv"), Some("srt"));
        assert_eq!(subtitle_codec("hdmv_pgs_subtitle", "mp4"), None);
        assert_eq!(subtitle_codec("hdmv_pgs_subtitle", "mkv"), Some("copy"));
        assert_eq!(subtitle_codec("ass", "webm"), Some("webvtt"));
        assert_eq!(natural_audio_ext("aac"), "m4a");
        assert_eq!(natural_audio_ext("pcm_s24le"), "wav");
        assert_eq!(natural_audio_ext("dts"), "mka");
    }

    #[test]
    fn speed_and_picture() {
        assert_eq!(atempo(1.0), Vec::<String>::new());
        assert_eq!(atempo(2.0), vec!["atempo=2.0000"]);
        assert_eq!(atempo(0.25), vec!["atempo=0.5", "atempo=0.5000"]);
        assert_eq!(atempo(0.5), vec!["atempo=0.5000"]);
        let f = picture_filters(90, true, Some(Crop { x: 0.1, y: 0.0, w: 0.5, h: 1.0 }));
        assert_eq!(f[0], "transpose=1");
        assert_eq!(f[1], "hflip");
        assert!(f[2].starts_with("crop=trunc(iw*0.50000/2)*2:trunc(ih*1.00000/2)*2:trunc(iw*0.10000/2)*2"), "{}", f[2]);
        assert!(picture_filters(0, false, Some(Crop { x: 0.0, y: 0.0, w: 1.0, h: 1.0 })).is_empty());
        // A phone video (-90 counter-clockwise = 90 clockwise) turned another 90 clockwise.
        let phone = serde_json::json!({ "side_data_list": [{ "rotation": -90 }] });
        assert_eq!(source_rotation(&phone), 90);
        assert_eq!(display_args(90, 90, false), vec!["-display_rotation:v:0", "-180"]);
        assert_eq!(display_args(0, 270, true), vec!["-display_rotation:v:0", "-270", "-display_hflip:v:0"]);
        assert!(display_args(90, 0, false).is_empty());
    }

    #[test]
    fn subtitles_follow_the_cuts() {
        let srt = "\u{feff}1\r\n00:00:01,000 --> 00:00:03,000\r\nHello\r\n\r\n2\r\n00:00:05,500 --> 00:00:07,000 X1:10\r\nSecond\r\nline\r\n";
        let cues = parse_srt(srt);
        assert_eq!(cues.len(), 2);
        assert_eq!(cues[1].text, "Second\nline");
        // Keep 0–2 s and 6–8 s: the first cue is clipped to 1–2, the second to 6–7 → 2–3 in the result.
        let segs = [Segment { start: 0.0, end: 2.0 }, Segment { start: 6.0, end: 8.0 }];
        let out = retime(&cues, &segs, 1.0);
        assert_eq!(out.len(), 2);
        assert_eq!((out[0].start, out[0].end), (1.0, 2.0));
        assert_eq!((out[1].start, out[1].end), (2.0, 3.0));
        // At double speed everything lands at half the time.
        assert_eq!(retime(&cues, &segs, 2.0)[1].start, 1.0);
        let text = write_srt(&out);
        assert!(text.starts_with("1\n00:00:01,000 --> 00:00:02,000\nHello\n"), "{text}");
        assert_eq!(parse_srt(&text), out);
    }
}
