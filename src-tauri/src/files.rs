//! Opening files: validation, folder listing for ←/→ navigation, details for the info panel,
//! and paste-from-clipboard.

use std::cmp::Ordering;
use std::path::{Path, PathBuf};
use std::time::UNIX_EPOCH;

use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime, Window};

use crate::formats::{self, Kind};

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenedFile {
    path: String,
    name: String,
    dir: String,
    ext: String,
    kind: Kind,
    editable: bool,
    size: u64,
    modified: Option<f64>,
    /// Lives in Immediate's temp folder (e.g. a pasted image), so Save must ask where to put it.
    temporary: bool,
}

fn epoch_ms(t: std::io::Result<std::time::SystemTime>) -> Option<f64> {
    t.ok()?.duration_since(UNIX_EPOCH).ok().map(|d| d.as_millis() as f64)
}

pub fn temp_dir() -> PathBuf {
    std::env::temp_dir().join("Immediate")
}

/// Makes a file openable in the window that asked: checks it, and grants the webview read
/// access to exactly that file (the asset protocol scope starts empty).
#[tauri::command]
pub fn open_path<R: Runtime>(app: AppHandle<R>, window: Window<R>, path: String) -> Result<OpenedFile, String> {
    let path = std::path::absolute(&path).map_err(|e| e.to_string())?;
    let meta = std::fs::metadata(&path).map_err(|_| format!("“{}” doesn’t exist any more.", display_name(&path)))?;
    if !meta.is_file() {
        return Err(format!("“{}” is a folder, not a file.", display_name(&path)));
    }
    let ext = path.extension().map(|e| e.to_string_lossy().to_lowercase()).unwrap_or_default();
    let format = formats::lookup(&path).ok_or_else(|| {
        if ext.is_empty() {
            format!("Immediate can’t tell what kind of file “{}” is.", display_name(&path))
        } else {
            format!("Immediate can’t open .{ext} files.")
        }
    })?;
    app.asset_protocol_scope().allow_file(&path).map_err(|e| e.to_string())?;
    crate::wm::set_has_file(&app, window.label());
    Ok(OpenedFile {
        name: display_name(&path),
        dir: path.parent().map(|p| p.to_string_lossy().into_owned()).unwrap_or_default(),
        temporary: path.starts_with(temp_dir()),
        path: path.to_string_lossy().into_owned(),
        ext,
        kind: format.kind,
        editable: format.editable,
        size: meta.len(),
        modified: epoch_ms(meta.modified()),
    })
}

fn display_name(path: &Path) -> String {
    path.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default()
}

/// Shows the file selected in Explorer / Finder / the file manager.
#[tauri::command]
pub fn reveal(path: String) -> Result<(), String> {
    tauri_plugin_opener::reveal_item_in_dir(path).map_err(|e| e.to_string())
}

/// Every openable file in the same folder, in the order Explorer/Finder shows them by name.
/// The webview may read these (so the next and previous photo can load ahead of time).
#[tauri::command]
pub async fn list_folder<R: Runtime>(app: AppHandle<R>, path: String) -> Result<Vec<String>, String> {
    let dir = Path::new(&path).parent().ok_or("no folder")?;
    let mut files: Vec<PathBuf> = std::fs::read_dir(dir)
        .map_err(|e| e.to_string())?
        .flatten()
        .filter(|e| e.file_type().is_ok_and(|t| t.is_file()))
        .map(|e| e.path())
        .filter(|p| formats::lookup(p).is_some())
        .collect();
    files.sort_by(|a, b| natural_cmp(&display_name(a), &display_name(b)));
    let scope = app.asset_protocol_scope();
    for f in &files {
        let _ = scope.allow_file(f);
    }
    Ok(files.into_iter().map(|p| p.to_string_lossy().into_owned()).collect())
}

/// "IMG 2.jpg" before "IMG 10.jpg", case-insensitive: the order file managers use.
pub fn natural_cmp(a: &str, b: &str) -> Ordering {
    let (mut a, mut b) = (a.chars().peekable(), b.chars().peekable());
    loop {
        match (a.peek().copied(), b.peek().copied()) {
            (None, None) => return Ordering::Equal,
            (None, Some(_)) => return Ordering::Less,
            (Some(_), None) => return Ordering::Greater,
            (Some(x), Some(y)) if x.is_ascii_digit() && y.is_ascii_digit() => {
                let take = |it: &mut std::iter::Peekable<std::str::Chars>| {
                    let mut s = String::new();
                    while let Some(c) = it.peek().copied().filter(char::is_ascii_digit) {
                        s.push(c);
                        it.next();
                    }
                    s
                };
                let (na, nb) = (take(&mut a), take(&mut b));
                let (ta, tb) = (na.trim_start_matches('0'), nb.trim_start_matches('0'));
                let ord = ta.len().cmp(&tb.len()).then_with(|| ta.cmp(tb)).then_with(|| na.len().cmp(&nb.len()));
                if ord != Ordering::Equal {
                    return ord;
                }
            }
            (Some(x), Some(y)) => {
                let ord = x.to_lowercase().cmp(y.to_lowercase());
                if ord != Ordering::Equal {
                    return ord;
                }
                a.next();
                b.next();
            }
        }
    }
}

#[derive(Serialize, Default)]
#[serde(rename_all = "camelCase")]
pub struct Details {
    created: Option<f64>,
    /// Camera / capture details as (label, value), in display order.
    exif: Vec<(String, String)>,
    /// Where the photo was taken (latitude, longitude), if it says.
    location: Option<(f64, f64)>,
}

/// Extra details for the info panel (EXIF for photos). Media details come from ffprobe later.
#[tauri::command]
pub async fn file_details(path: String) -> Result<Details, String> {
    let path = PathBuf::from(path);
    let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
    let mut d = Details { created: epoch_ms(meta.created()), ..Default::default() };
    if formats::lookup(&path).is_some_and(|f| f.kind == Kind::Image) {
        if let Ok(file) = std::fs::File::open(&path) {
            if let Ok(exif) = exif::Reader::new().read_from_container(&mut std::io::BufReader::new(file)) {
                (d.exif, d.location) = describe_exif(&exif);
            }
        }
    }
    Ok(d)
}

fn describe_exif(exif: &exif::Exif) -> (Vec<(String, String)>, Option<(f64, f64)>) {
    use exif::{In, Tag, Value};
    let field = |t: Tag| exif.get_field(t, In::PRIMARY);
    let text = |t: Tag| {
        field(t).and_then(|f| match &f.value {
            Value::Ascii(v) => v.first().map(|s| String::from_utf8_lossy(s).trim().trim_end_matches('\0').to_owned()),
            _ => Some(f.display_value().to_string()),
        })
        .filter(|s| !s.is_empty())
    };
    let num = |t: Tag| {
        field(t).and_then(|f| match &f.value {
            Value::Rational(v) => v.first().map(|r| r.to_f64()),
            Value::SRational(v) => v.first().map(|r| r.to_f64()),
            Value::Double(v) => v.first().copied(),
            Value::Float(v) => v.first().map(|n| f64::from(*n)),
            Value::Short(v) => v.first().map(|n| f64::from(*n)),
            Value::Long(v) => v.first().map(|n| f64::from(*n)),
            _ => None,
        })
        .filter(|n| n.is_finite() && *n > 0.0)
    };
    let trim = |n: f64| {
        let s = format!("{n:.1}");
        s.strip_suffix(".0").map(str::to_owned).unwrap_or(s)
    };

    let mut rows = Vec::new();
    let make = text(Tag::Make).unwrap_or_default();
    if let Some(model) = text(Tag::Model) {
        let camera = if model.to_lowercase().starts_with(&make.to_lowercase()) || make.is_empty() {
            model
        } else {
            format!("{make} {model}")
        };
        rows.push(("Camera".into(), camera));
    }
    if let Some(lens) = text(Tag::LensModel) {
        rows.push(("Lens".into(), lens));
    }
    if let Some(f) = num(Tag::FocalLength) {
        let equiv = num(Tag::FocalLengthIn35mmFilm).map(|e| format!(" ({} mm equiv.)", trim(e))).unwrap_or_default();
        rows.push(("Focal length".into(), format!("{} mm{equiv}", trim(f))));
    }
    if let Some(a) = num(Tag::FNumber) {
        rows.push(("Aperture".into(), format!("ƒ/{}", trim(a))));
    }
    if let Some(t) = num(Tag::ExposureTime) {
        let s = if t >= 1.0 { format!("{} s", trim(t)) } else { format!("1/{} s", (1.0 / t).round()) };
        rows.push(("Shutter".into(), s));
    }
    if let Some(iso) = num(Tag::PhotographicSensitivity) {
        rows.push(("ISO".into(), format!("{iso}")));
    }
    if let Some(taken) = text(Tag::DateTimeOriginal).or_else(|| text(Tag::DateTime)) {
        // EXIF writes "2024:05:01 12:34:56"; show "2024-05-01 12:34".
        let t = taken.replacen(':', "-", 2);
        rows.push(("Taken".into(), t.get(..16).unwrap_or(&t).to_owned()));
    }
    if let Some(sw) = text(Tag::Software) {
        rows.push(("Software".into(), sw));
    }

    let coord = |t: Tag, r: Tag, neg: &str| {
        let f = field(t)?;
        let Value::Rational(v) = &f.value else { return None };
        let deg = v.first()?.to_f64() + v.get(1).map_or(0.0, |m| m.to_f64() / 60.0) + v.get(2).map_or(0.0, |s| s.to_f64() / 3600.0);
        let sign = if text(r).is_some_and(|s| s.eq_ignore_ascii_case(neg)) { -1.0 } else { 1.0 };
        deg.is_finite().then_some(deg * sign)
    };
    let location = coord(Tag::GPSLatitude, Tag::GPSLatitudeRef, "S")
        .zip(coord(Tag::GPSLongitude, Tag::GPSLongitudeRef, "W"));
    (rows, location)
}

/// Opens what's on the clipboard: copied files (from Explorer/Finder), an image (saved as a
/// PNG in Immediate's temp folder), or a copied path. Returns the files to open.
#[tauri::command]
pub async fn clipboard_paste() -> Result<Vec<String>, String> {
    let mut cb = arboard::Clipboard::new().map_err(|e| e.to_string())?;
    if let Ok(files) = cb.get().file_list() {
        let files: Vec<String> = files
            .into_iter()
            .filter(|p| formats::lookup(p).is_some())
            .map(|p| p.to_string_lossy().into_owned())
            .collect();
        if !files.is_empty() {
            return Ok(files);
        }
    }
    if let Ok(img) = cb.get_image() {
        let path = unique_path(&temp_dir(), "Pasted image", "png");
        save_png(&path, img.width as u32, img.height as u32, img.bytes.into_owned())?;
        return Ok(vec![path.to_string_lossy().into_owned()]);
    }
    if let Ok(text) = cb.get_text() {
        let candidate = PathBuf::from(text.trim().trim_matches('"'));
        if candidate.is_file() && formats::lookup(&candidate).is_some() {
            return Ok(vec![candidate.to_string_lossy().into_owned()]);
        }
    }
    Err("There’s no picture or media file on the clipboard.".into())
}

fn save_png(path: &Path, w: u32, h: u32, rgba: Vec<u8>) -> Result<(), String> {
    use image::ImageEncoder;
    use image::codecs::png::{CompressionType, FilterType, PngEncoder};
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let file = std::io::BufWriter::new(std::fs::File::create(path).map_err(|e| e.to_string())?);
    PngEncoder::new_with_quality(file, CompressionType::Fast, FilterType::Adaptive)
        .write_image(&rgba, w, h, image::ExtendedColorType::Rgba8)
        .map_err(|e| e.to_string())
}

/// "name.ext", or "name 2.ext", "name 3.ext"… if taken.
pub fn unique_path(dir: &Path, stem: &str, ext: &str) -> PathBuf {
    let first = dir.join(format!("{stem}.{ext}"));
    if !first.exists() {
        return first;
    }
    (2..).map(|n| dir.join(format!("{stem} {n}.{ext}"))).find(|p| !p.exists()).expect("a free name")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn natural_order() {
        let mut v = vec!["IMG 10.jpg", "img 2.jpg", "IMG 1.jpg", "a.png", "IMG 02.jpg", "B.png"];
        v.sort_by(|a, b| natural_cmp(a, b));
        assert_eq!(v, ["a.png", "B.png", "IMG 1.jpg", "img 2.jpg", "IMG 02.jpg", "IMG 10.jpg"]);
    }

    #[test]
    fn unique_names() {
        let dir = std::env::temp_dir().join(format!("immediate-test-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        let a = unique_path(&dir, "Pasted image", "png");
        std::fs::write(&a, b"x").unwrap();
        let b = unique_path(&dir, "Pasted image", "png");
        assert_eq!(b.file_name().unwrap(), "Pasted image 2.png");
        std::fs::remove_dir_all(dir).unwrap();
    }
}
