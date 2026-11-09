//! Saving edited photos. The UI renders the edited pixels at full resolution and sends them here;
//! Rust encodes them in the original's format and quality and carries its metadata over.

pub mod decode;
pub mod meta;
pub mod thumb;

use std::io::Cursor;
use std::path::{Path, PathBuf};

use img_parts::jpeg::Jpeg;
use img_parts::{Bytes, DynImage, ImageEXIF, ImageICC};
use serde::{Deserialize, Serialize};
use tauri::ipc::{InvokeBody, Request};
use tauri::{AppHandle, Manager, Runtime};

use crate::files::unique_path;

/// The format Immediate writes for a source extension, if it can write that format.
fn writable(ext: &str) -> Option<&'static str> {
    Some(match ext {
        "jpg" | "jpeg" | "jfif" => "jpeg",
        "png" | "apng" => "png",
        "webp" => "webp",
        "gif" => "gif",
        "bmp" => "bmp",
        "tif" | "tiff" => "tiff",
        "tga" => "tga",
        "ico" => "ico",
        "pnm" | "ppm" | "pgm" | "pbm" => "pnm",
        "qoi" => "qoi",
        _ => return None,
    })
}

/// What Save writes: the same format, or JPEG for formats Immediate can't write
/// (HEIC, AVIF, JPEG XL, RAW, PSD…).
fn save_format(ext: &str) -> &'static str {
    writable(ext).unwrap_or("jpeg")
}

fn ext_for(format: &str, source_ext: &str) -> String {
    if writable(source_ext) == Some(format) {
        return source_ext.to_owned();
    }
    match format {
        "jpeg" => "jpg",
        "pnm" => "ppm",
        "tiff" => "tif",
        f => f,
    }
    .to_owned()
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImageInfo {
    /// Name of the embedded color profile, if any.
    icc_name: Option<String>,
    /// How the UI should read the pixels: "srgb" (untagged or sRGB), "display-p3", or "other"
    /// (any other profile: the UI converts to sRGB and the profile isn't kept).
    color_space: &'static str,
    /// The format Save writes, and its extension.
    save_format: &'static str,
    save_ext: String,
    /// Estimated JPEG quality of the original (Save uses the same).
    jpeg_quality: Option<u8>,
    /// Lossless WebP original.
    lossless: bool,
    /// The photo carries an HDR gain map (iPhone, Pixel, Ultra HDR), which re-saving drops.
    hdr: bool,
}

/// Signs of an HDR gain map: Adobe/Google "hdrgm" XMP, Apple's gain map auxiliary image, the
/// ISO 21496-1 gain map metadata.
fn has_gain_map(bytes: &[u8]) -> bool {
    const SIGNS: [&[u8]; 5] = [b"hdrgm:", b"HDRGainMap", b"hdrgainmap", b"urn:iso:std:iso:ts:21496", b"GainMapVersion"];
    // Metadata sits near the start; the second (gain map) picture may follow the first.
    let head = &bytes[..bytes.len().min(4 << 20)];
    SIGNS.iter().any(|s| head.windows(s.len()).any(|w| w == *s))
}

fn ext_of(path: &Path) -> String {
    path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default()
}

#[tauri::command]
pub async fn image_info(path: String) -> Result<ImageInfo, String> {
    let path = PathBuf::from(path);
    let ext = ext_of(&path);
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let hdr = has_gain_map(&bytes);
    let parsed = DynImage::from_bytes(Bytes::from(bytes)).ok().flatten();
    let icc_name = parsed.as_ref().and_then(|d| d.icc_profile()).and_then(|icc| meta::icc_description(&icc));
    let color_space = match icc_name.as_deref().map(str::to_lowercase) {
        None => "srgb",
        Some(n) if n.contains("srgb") => "srgb",
        Some(n) if n.contains("p3") => "display-p3",
        Some(_) => "other",
    };
    let (mut jpeg_quality, mut lossless) = (None, false);
    match &parsed {
        Some(DynImage::Jpeg(j)) => {
            let dqt: Vec<&[u8]> = j.segments_by_marker(img_parts::jpeg::markers::DQT).map(|s| s.contents().as_ref()).collect();
            jpeg_quality = meta::jpeg_quality(&dqt);
        }
        Some(DynImage::WebP(w)) => lossless = matches!(w.kind(), img_parts::vp8::VP8Kind::VP8L),
        _ => {}
    }
    let fmt = save_format(&ext);
    Ok(ImageInfo { icc_name, color_space, save_format: fmt, save_ext: ext_for(fmt, &ext), jpeg_quality, lossless, hdr })
}

/// Decodes a picture the webview can't show itself. Returns width and height (u32 LE each),
/// then upright sRGB RGBA pixels.
#[tauri::command]
pub async fn decode_image(path: String) -> Result<tauri::ipc::Response, String> {
    let img = tauri::async_runtime::spawn_blocking(move || decode::decode(Path::new(&path)))
        .await
        .map_err(|e| e.to_string())??;
    let mut out = Vec::with_capacity(8 + img.data.len());
    out.extend(img.w.to_le_bytes());
    out.extend(img.h.to_le_bytes());
    out.extend(img.data);
    Ok(tauri::ipc::Response::new(out))
}

/// A small PNG preview of any file (picture, video…) for the filmstrip.
#[tauri::command]
pub async fn thumbnail(path: String, size: u32) -> Result<tauri::ipc::Response, String> {
    let png = tauri::async_runtime::spawn_blocking(move || thumb::thumbnail(Path::new(&path), size.clamp(16, 512)))
        .await
        .map_err(|e| e.to_string())??;
    Ok(tauri::ipc::Response::new(png))
}

/// Starts dragging a file out of the window (into Explorer, Finder, a chat, an email…).
#[tauri::command]
pub async fn start_drag<R: Runtime>(app: AppHandle<R>, window: tauri::Window<R>, path: String) -> Result<(), String> {
    let p = PathBuf::from(&path);
    let icon = tauri::async_runtime::spawn_blocking({
        let p = p.clone();
        move || thumb::thumbnail(&p, 96).ok()
    })
    .await
    .ok()
    .flatten()
    .unwrap_or_default();
    app.run_on_main_thread(move || {
        let item = drag::DragItem::Files(vec![p]);
        let image = drag::Image::Raw(icon);
        #[cfg(target_os = "linux")]
        if let Ok(gw) = window.gtk_window() {
            let _ = drag::start_drag(&gw, item, image, |_, _| {}, drag::Options::default());
        }
        #[cfg(not(target_os = "linux"))]
        let _ = drag::start_drag(&window, item, image, |_, _| {}, drag::Options::default());
    })
    .map_err(|e| e.to_string())
}

/// The clipboard stays open for the life of the app: on Linux, copied data lives only as long
/// as its owner.
static CLIPBOARD: std::sync::Mutex<Option<arboard::Clipboard>> = std::sync::Mutex::new(None);

fn set_clipboard_image(w: u32, h: u32, rgba: Vec<u8>) -> Result<(), String> {
    let mut guard = CLIPBOARD.lock().map_err(|e| e.to_string())?;
    if guard.is_none() {
        *guard = Some(arboard::Clipboard::new().map_err(|e| e.to_string())?);
    }
    let cb = guard.as_mut().unwrap();
    cb.set_image(arboard::ImageData { width: w as usize, height: h as usize, bytes: rgba.into() })
        .map_err(|e| format!("Couldn’t copy the picture: {e}"))
}

/// Copies a picture file's pixels to the clipboard (for pasting into chats, documents…).
#[tauri::command]
pub async fn copy_image_file(path: String) -> Result<(), String> {
    tauri::async_runtime::spawn_blocking(move || {
        let img = decode::decode(Path::new(&path))?;
        set_clipboard_image(img.w, img.h, img.data)
    })
    .await
    .map_err(|e| e.to_string())?
}

/// Copies edited pixels (request body, RGBA; header "x-size": "w,h") to the clipboard.
#[tauri::command]
pub async fn copy_pixels(request: Request<'_>) -> Result<(), String> {
    let (w, h) = request
        .headers()
        .get("x-size")
        .and_then(|v| v.to_str().ok())
        .and_then(|s| s.split_once(','))
        .and_then(|(a, b)| Some((a.parse::<u32>().ok()?, b.parse::<u32>().ok()?)))
        .ok_or("missing size")?;
    let InvokeBody::Raw(rgba) = request.body() else { return Err("missing pixels".into()) };
    if rgba.len() != w as usize * h as usize * 4 {
        return Err("the image data is incomplete".into());
    }
    let rgba = rgba.clone();
    tauri::async_runtime::spawn_blocking(move || set_clipboard_image(w, h, rgba)).await.map_err(|e| e.to_string())?
}

#[derive(Deserialize, Clone)]
#[serde(rename_all = "camelCase")]
struct SaveOptions {
    source: String,
    width: u32,
    height: u32,
    /// "copy" (next to the original, with the suffix), "replace" (original to the trash),
    /// or "path" (to `dest`, chosen in a Save As dialog).
    target: String,
    dest: Option<String>,
    suffix: String,
    format: Option<String>,
    quality: Option<u8>,
    strip_location: bool,
    strip_metadata: bool,
    keep_icc: bool,
    /// Shrink so the long side is at most this many pixels (never enlarges).
    #[serde(default)]
    resize_long: Option<u32>,
    /// JPEG/WebP: lower the quality (and if needed the size) until the file fits in this many bytes.
    #[serde(default)]
    target_bytes: Option<u64>,
}

/// Scales RGBA pixels so the long side is `long` pixels (Lanczos), keeping the shape.
fn shrink(rgba: &[u8], w: u32, h: u32, long: u32) -> Result<(Vec<u8>, u32, u32), String> {
    let k = long as f64 / w.max(h) as f64;
    let (nw, nh) = (((w as f64 * k).round() as u32).max(1), ((h as f64 * k).round() as u32).max(1));
    let img = image::RgbaImage::from_raw(w, h, rgba.to_vec()).ok_or("bad image size")?;
    let out = image::imageops::resize(&img, nw, nh, image::imageops::FilterType::Lanczos3);
    Ok((out.into_raw(), nw, nh))
}

/// Saves edited pixels (the request body, RGBA, top row first). Returns the saved file's path.
#[tauri::command]
pub async fn image_save<R: Runtime>(app: AppHandle<R>, request: Request<'_>) -> Result<String, String> {
    let opts: SaveOptions = request
        .headers()
        .get("x-save")
        .and_then(|h| h.to_str().ok())
        .and_then(|s| serde_json::from_str(&urlencoding_decode(s)).ok())
        .ok_or("missing save options")?;
    let InvokeBody::Raw(rgba) = request.body() else { return Err("missing pixels".into()) };
    if rgba.len() != opts.width as usize * opts.height as usize * 4 {
        return Err("the image data is incomplete".into());
    }
    let rgba = rgba.clone();
    let saved = tauri::async_runtime::spawn_blocking(move || save(&opts, &rgba))
        .await
        .map_err(|e| e.to_string())??;
    let _ = app.asset_protocol_scope().allow_file(&saved);
    Ok(saved.to_string_lossy().into_owned())
}

/// Header values must be ASCII, so the UI percent-encodes the JSON.
fn urlencoding_decode(s: &str) -> String {
    let b = s.as_bytes();
    let mut out = Vec::with_capacity(b.len());
    let mut i = 0;
    while i < b.len() {
        if b[i] == b'%' && i + 2 < b.len() {
            if let Ok(v) = u8::from_str_radix(&s[i + 1..i + 3], 16) {
                out.push(v);
                i += 3;
                continue;
            }
        }
        out.push(b[i]);
        i += 1;
    }
    String::from_utf8_lossy(&out).into_owned()
}

fn save(o: &SaveOptions, rgba: &[u8]) -> Result<PathBuf, String> {
    // A smaller size first: everything after (metadata, encoding) sees the new dimensions.
    let resized;
    let (o, rgba): (&SaveOptions, &[u8]) = match o.resize_long.filter(|l| *l >= 16 && *l < o.width.max(o.height)) {
        Some(long) => {
            let (px, w, h) = shrink(rgba, o.width, o.height, long)?;
            resized = (SaveOptions { width: w, height: h, ..o.clone() }, px);
            (&resized.0, &resized.1)
        }
        None => (o, rgba),
    };
    let source = PathBuf::from(&o.source);
    let src_ext = ext_of(&source);
    let format = o.format.clone().unwrap_or_else(|| save_format(&src_ext).to_owned());
    let src_bytes = std::fs::read(&source).ok();
    let src_parsed = src_bytes.clone().and_then(|b| DynImage::from_bytes(Bytes::from(b)).ok().flatten());

    // Quality: the original's for JPEG, else a high default.
    let jpeg_q = match &src_parsed {
        Some(DynImage::Jpeg(j)) => {
            let dqt: Vec<&[u8]> = j.segments_by_marker(img_parts::jpeg::markers::DQT).map(|s| s.contents().as_ref()).collect();
            meta::jpeg_quality(&dqt)
        }
        _ => None,
    };
    let quality = o.quality.or(jpeg_q).unwrap_or(92).clamp(1, 100);
    let sampling = match &src_parsed {
        Some(DynImage::Jpeg(j)) => [img_parts::jpeg::markers::SOF0, img_parts::jpeg::markers::SOF1, img_parts::jpeg::markers::SOF2]
            .iter()
            .find_map(|m| j.segment_by_marker(*m))
            .and_then(|s| meta::jpeg_sampling(s.contents())),
        _ => None,
    };
    let lossless = matches!(&src_parsed, Some(DynImage::WebP(w)) if matches!(w.kind(), img_parts::vp8::VP8Kind::VP8L));

    let encoded = match o.target_bytes.filter(|_| matches!(format.as_str(), "jpeg" | "webp")) {
        Some(limit) => fit_in(&format, rgba, o, quality, sampling, src_parsed.as_ref(), limit)?,
        None => with_metadata(encode(&format, rgba, o.width, o.height, quality, sampling, lossless)?, src_parsed.as_ref(), o)?,
    };

    let ext = ext_for(&format, &src_ext);
    let dest = match o.target.as_str() {
        "copy" => {
            let dir = source.parent().ok_or("no folder")?;
            let stem = source.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            unique_path(dir, &format!("{stem}{}", o.suffix), &ext)
        }
        "replace" => {
            if ext != src_ext {
                return Err("Replace keeps the original’s format; use Save As for a different one.".into());
            }
            source.clone()
        }
        _ => PathBuf::from(o.dest.as_deref().ok_or("no destination")?),
    };

    // Write next to the destination first, so a failure never leaves a half-written file.
    let dir = dest.parent().ok_or("no folder")?;
    let tmp = dir.join(format!(".{}.immediate-tmp", dest.file_name().map(|n| n.to_string_lossy()).unwrap_or_default()));
    std::fs::write(&tmp, &encoded).map_err(|e| friendly_io(e, dir))?;
    if o.target == "replace" {
        if let Err(e) = trash::delete(&source) {
            let _ = std::fs::remove_file(&tmp);
            return Err(format!("Couldn’t move the original to the trash: {e}"));
        }
    }
    std::fs::rename(&tmp, &dest).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        friendly_io(e, dir)
    })?;
    Ok(dest)
}

/// The best JPEG/WebP that fits in `limit` bytes: the highest quality that fits (down to 40),
/// then smaller sizes if even that is too big.
fn fit_in(format: &str, rgba: &[u8], o: &SaveOptions, start_q: u8, sampling: Option<(u8, u8)>, src: Option<&DynImage>, limit: u64) -> Result<Vec<u8>, String> {
    const MIN_Q: u8 = 40;
    let mut px = rgba.to_vec();
    let mut opts = o.clone();
    for _ in 0..6 {
        let at = |q: u8, px: &[u8], opts: &SaveOptions| -> Result<Vec<u8>, String> {
            with_metadata(encode(format, px, opts.width, opts.height, q, sampling, false)?, src, opts)
        };
        let best = at(start_q, &px, &opts)?;
        if best.len() as u64 <= limit {
            return Ok(best);
        }
        let floor = at(MIN_Q, &px, &opts)?;
        if floor.len() as u64 <= limit {
            // Binary search for the highest quality that fits.
            let (mut lo, mut hi, mut found) = (MIN_Q, start_q, floor);
            while hi - lo > 1 {
                let mid = (lo + hi) / 2;
                let e = at(mid, &px, &opts)?;
                if e.len() as u64 <= limit {
                    lo = mid;
                    found = e;
                } else {
                    hi = mid;
                }
            }
            return Ok(found);
        }
        // Still too big at the lowest quality: fewer pixels (file size follows the area).
        let k = ((limit as f64 / floor.len() as f64).sqrt() * 0.95).clamp(0.3, 0.95);
        let long = ((opts.width.max(opts.height) as f64) * k) as u32;
        let (p, w, h) = shrink(&px, opts.width, opts.height, long.max(16))?;
        px = p;
        opts.width = w;
        opts.height = h;
    }
    Err("Couldn’t make the picture that small. Pick a larger size limit.".into())
}

fn friendly_io(e: std::io::Error, dir: &Path) -> String {
    match e.kind() {
        std::io::ErrorKind::PermissionDenied => format!("Immediate isn’t allowed to save in {}. Use Save As to pick another folder.", dir.display()),
        _ => e.to_string(),
    }
}

fn encode(format: &str, rgba: &[u8], w: u32, h: u32, quality: u8, sampling: Option<(u8, u8)>, lossless: bool) -> Result<Vec<u8>, String> {
    let opaque = rgba.chunks_exact(4).all(|p| p[3] == 255);
    let mut out = Vec::new();
    match format {
        "jpeg" => {
            if w > 65535 || h > 65535 {
                return Err("JPEG can’t be wider or taller than 65,535 pixels.".into());
            }
            use jpeg_encoder::{ColorType, Encoder, SamplingFactor};
            let mut enc = Encoder::new(&mut out, quality);
            enc.set_sampling_factor(match sampling {
                Some((1, 1)) => SamplingFactor::R_4_4_4,
                Some((2, 1)) => SamplingFactor::R_4_2_2,
                Some((1, 2)) => SamplingFactor::R_4_4_0,
                Some((2, 2)) => SamplingFactor::R_4_2_0,
                // No original JPEG to match: 4:4:4 at high quality, 4:2:0 otherwise (like most cameras).
                _ if quality >= 90 => SamplingFactor::R_4_4_4,
                _ => SamplingFactor::R_4_2_0,
            });
            enc.encode(rgba, w as u16, h as u16, ColorType::Rgba).map_err(|e| e.to_string())?;
        }
        "png" => {
            use image::ImageEncoder;
            use image::codecs::png::{CompressionType, FilterType, PngEncoder};
            let enc = PngEncoder::new_with_quality(&mut out, CompressionType::Default, FilterType::Adaptive);
            if opaque {
                let rgb: Vec<u8> = rgba.chunks_exact(4).flat_map(|p| [p[0], p[1], p[2]]).collect();
                enc.write_image(&rgb, w, h, image::ExtendedColorType::Rgb8)
            } else {
                enc.write_image(rgba, w, h, image::ExtendedColorType::Rgba8)
            }
            .map_err(|e| e.to_string())?;
        }
        "webp" => {
            let enc = webp::Encoder::from_rgba(rgba, w, h);
            let mem = if lossless { enc.encode_lossless() } else { enc.encode(quality as f32) };
            out = mem.to_vec();
        }
        other => {
            let fmt = match other {
                "gif" => image::ImageFormat::Gif,
                "bmp" => image::ImageFormat::Bmp,
                "tiff" => image::ImageFormat::Tiff,
                "tga" => image::ImageFormat::Tga,
                "ico" => image::ImageFormat::Ico,
                "pnm" => image::ImageFormat::Pnm,
                "qoi" => image::ImageFormat::Qoi,
                _ => return Err(format!("Immediate can’t save {other} files.")),
            };
            if fmt == image::ImageFormat::Ico && (w > 256 || h > 256) {
                return Err("Icons can be at most 256 × 256 pixels. Crop or use Save As with another format.".into());
            }
            let img = image::RgbaImage::from_raw(w, h, rgba.to_vec()).ok_or("bad image size")?;
            let dynimg = image::DynamicImage::ImageRgba8(img);
            let dynimg = if fmt == image::ImageFormat::Pnm || (opaque && fmt != image::ImageFormat::Ico) {
                image::DynamicImage::ImageRgb8(dynimg.to_rgb8())
            } else {
                dynimg
            };
            dynimg.write_to(&mut Cursor::new(&mut out), fmt).map_err(|e| e.to_string())?;
        }
    }
    Ok(out)
}

const XMP_PREFIX: &[u8] = b"http://ns.adobe.com/xap/1.0/\0";

/// Copies EXIF (patched), the color profile and XMP from the original onto the new file.
fn with_metadata(encoded: Vec<u8>, src: Option<&DynImage>, o: &SaveOptions) -> Result<Vec<u8>, String> {
    let Some(src) = src else { return Ok(encoded) };
    let Ok(Some(mut out)) = DynImage::from_bytes(Bytes::from(encoded.clone())) else { return Ok(encoded) };
    if !o.strip_metadata {
        let patch = meta::ExifPatch { width: o.width, height: o.height, strip_location: o.strip_location };
        out.set_exif(src.exif().and_then(|e| meta::patch_exif(&e, &patch)).map(Bytes::from));
    }
    if o.keep_icc {
        out.set_icc_profile(src.icc_profile());
    }
    // XMP (JPEG → JPEG): keep, with orientation reset. Dropped when removing location, since
    // XMP can hold GPS too.
    if let (DynImage::Jpeg(s), DynImage::Jpeg(d)) = (src, &mut out) {
        if !o.strip_metadata {
            copy_xmp(s, d, o.strip_location);
        }
    }
    let mut buf = Vec::new();
    out.encoder().write_to(&mut buf).map_err(|e| e.to_string())?;
    Ok(buf)
}

fn copy_xmp(src: &Jpeg, dst: &mut Jpeg, strip_location: bool) {
    use img_parts::jpeg::{JpegSegment, markers};
    let Some(seg) = src.segments_by_marker(markers::APP1).find(|s| s.contents().starts_with(XMP_PREFIX)) else { return };
    let mut xml = String::from_utf8_lossy(&seg.contents()[XMP_PREFIX.len()..]).into_owned();
    if strip_location && xml.contains("GPS") {
        return;
    }
    for n in 2..=8 {
        xml = xml.replace(&format!("tiff:Orientation=\"{n}\""), "tiff:Orientation=\"1\"");
        xml = xml.replace(&format!("<tiff:Orientation>{n}</tiff:Orientation>"), "<tiff:Orientation>1</tiff:Orientation>");
    }
    let mut contents = XMP_PREFIX.to_vec();
    contents.extend(xml.as_bytes());
    if contents.len() > 65533 {
        return;
    }
    // Right after the EXIF segment (or the start), where readers expect it.
    let at = dst.segments().iter().position(|s| s.marker() == markers::APP1).map_or(0, |i| i + 1);
    dst.segments_mut().insert(at, JpegSegment::new_with_contents(markers::APP1, Bytes::from(contents)));
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn save_formats_and_extensions() {
        assert_eq!(save_format("jpeg"), "jpeg");
        assert_eq!(ext_for("jpeg", "jpeg"), "jpeg");
        assert_eq!(ext_for("jpeg", "heic"), "jpg");
        assert_eq!(ext_for("png", "png"), "png");
        assert_eq!(save_format("cr3"), "jpeg");
    }

    #[test]
    fn header_decoding() {
        assert_eq!(urlencoding_decode("%7B%22a%22%3A1%7D"), r#"{"a":1}"#);
        assert_eq!(urlencoding_decode("C%3A%5Cx%20y"), r"C:\x y");
    }

    #[test]
    fn jpeg_round_trip_keeps_quality_and_metadata() {
        // Encode a small JPEG at q85 with EXIF orientation 6, then save "edited" pixels from it.
        let dir = std::env::temp_dir().join(format!("immediate-save-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let (w, h) = (64u32, 48u32);
        let px: Vec<u8> = (0..w * h).flat_map(|i| [(i % 255) as u8, 120, 200, 255]).collect();
        let mut src = Vec::new();
        jpeg_encoder::Encoder::new(&mut src, 85).encode(&px, w as u16, h as u16, jpeg_encoder::ColorType::Rgba).unwrap();
        let mut exif = vec![b'I', b'I', 42, 0, 8, 0, 0, 0, 1, 0];
        exif.extend([0x12, 0x01, 3, 0, 1, 0, 0, 0, 6, 0, 0, 0, 0, 0, 0, 0]);
        let mut j = DynImage::from_bytes(Bytes::from(src)).unwrap().unwrap();
        j.set_exif(Some(Bytes::from(exif)));
        let mut srcbytes = Vec::new();
        j.encoder().write_to(&mut srcbytes).unwrap();
        let source = dir.join("photo.jpg");
        std::fs::write(&source, &srcbytes).unwrap();

        let o = SaveOptions {
            source: source.to_string_lossy().into(),
            width: w,
            height: h,
            target: "copy".into(),
            dest: None,
            suffix: " edited".into(),
            format: None,
            quality: None,
            strip_location: false,
            strip_metadata: false,
            resize_long: None,
            target_bytes: None,
            keep_icc: true,
        };
        let saved = save(&o, &px).unwrap();
        assert_eq!(saved.file_name().unwrap(), "photo edited.jpg");
        let out = DynImage::from_bytes(Bytes::from(std::fs::read(&saved).unwrap())).unwrap().unwrap();
        let DynImage::Jpeg(oj) = &out else { panic!() };
        let dqt: Vec<&[u8]> = oj.segments_by_marker(img_parts::jpeg::markers::DQT).map(|s| s.contents().as_ref()).collect();
        assert_eq!(meta::jpeg_quality(&dqt), Some(85));
        let e = out.exif().unwrap();
        assert_eq!(&e[e.len() - 8..e.len() - 6], &[1, 0], "orientation reset to 1");
        // A second save doesn't overwrite the first.
        assert_eq!(save(&o, &px).unwrap().file_name().unwrap(), "photo edited 2.jpg");
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn resize_and_fit_under_a_size() {
        let dir = std::env::temp_dir().join(format!("immediate-fit-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        // Noisy pixels compress badly, so the size limit has real work to do.
        let (w, h) = (1200u32, 800u32);
        let mut seed = 7u32;
        let px: Vec<u8> = (0..w * h)
            .flat_map(|_| {
                seed = seed.wrapping_mul(1_103_515_245).wrapping_add(12345);
                let v = (seed >> 16) as u8;
                [v, v.wrapping_mul(3), 255 - v, 255]
            })
            .collect();
        let source = dir.join("noise.png");
        image::RgbaImage::from_raw(w, h, px.clone()).unwrap().save(&source).unwrap();
        let base = SaveOptions {
            source: source.to_string_lossy().into(),
            width: w,
            height: h,
            target: "copy".into(),
            dest: None,
            suffix: " edited".into(),
            format: Some("jpeg".into()),
            quality: Some(92),
            strip_location: false,
            strip_metadata: false,
            keep_icc: true,
            resize_long: None,
            target_bytes: None,
        };
        // Long side 600: 600 x 400.
        let small = save(&SaveOptions { resize_long: Some(600), ..base.clone() }, &px).unwrap();
        assert_eq!(image::image_dimensions(&small).unwrap(), (600, 400));
        // Never enlarged.
        let same = save(&SaveOptions { resize_long: Some(5000), ..base.clone() }, &px).unwrap();
        assert_eq!(image::image_dimensions(&same).unwrap(), (1200, 800));
        // Fits under 150 KB (quality first, then fewer pixels).
        let fit = save(&SaveOptions { target_bytes: Some(150_000), ..base.clone() }, &px).unwrap();
        assert!(std::fs::metadata(&fit).unwrap().len() <= 150_000);
        std::fs::remove_dir_all(dir).unwrap();
    }

    #[test]
    fn spots_hdr_gain_maps() {
        assert!(has_gain_map(b"....<x:xmpmeta> hdrgm:Version=\"1.0\" ..."));
        assert!(has_gain_map(b"urn:com:apple:photo:2020:aux:hdrgainmap"));
        assert!(!has_gain_map(b"an ordinary JPEG with Exif"));
    }
}
