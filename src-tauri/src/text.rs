//! Text documents (txt, md, csv, json, log…): read and write keeping the file's encoding and
//! line endings, so a quick edit changes only what was edited.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};

const MAX: u64 = 32 * 1024 * 1024;

#[derive(Serialize)]
pub struct TextFile {
    /// Text with "\n" line endings.
    text: String,
    /// "utf-8" | "utf-8-bom" | "utf-16le" | "utf-16be" | "windows-1252"
    encoding: &'static str,
    /// "\r\n" or "\n"
    eol: &'static str,
}

/// Windows-1252 code points for bytes 0x80..=0x9F (the rest map to the same Unicode value).
const CP1252: [u16; 32] = [
    0x20AC, 0x81, 0x201A, 0x0192, 0x201E, 0x2026, 0x2020, 0x2021, 0x02C6, 0x2030, 0x0160, 0x2039, 0x0152, 0x8D, 0x017D, 0x8F,
    0x90, 0x2018, 0x2019, 0x201C, 0x201D, 0x2022, 0x2013, 0x2014, 0x02DC, 0x2122, 0x0161, 0x203A, 0x0153, 0x9D, 0x017E, 0x0178,
];

pub fn decode(bytes: &[u8]) -> (String, &'static str) {
    if let Some(rest) = bytes.strip_prefix(&[0xEF, 0xBB, 0xBF]) {
        return (String::from_utf8_lossy(rest).into_owned(), "utf-8-bom");
    }
    let utf16 = |rest: &[u8], le: bool| {
        let units: Vec<u16> = rest.chunks_exact(2).map(|c| if le { u16::from_le_bytes([c[0], c[1]]) } else { u16::from_be_bytes([c[0], c[1]]) }).collect();
        String::from_utf16_lossy(&units)
    };
    if let Some(rest) = bytes.strip_prefix(&[0xFF, 0xFE]) {
        return (utf16(rest, true), "utf-16le");
    }
    if let Some(rest) = bytes.strip_prefix(&[0xFE, 0xFF]) {
        return (utf16(rest, false), "utf-16be");
    }
    match std::str::from_utf8(bytes) {
        Ok(s) => (s.to_owned(), "utf-8"),
        Err(_) => (
            bytes
                .iter()
                .map(|&b| match b {
                    0x80..=0x9F => char::from_u32(CP1252[(b - 0x80) as usize] as u32).unwrap_or('\u{FFFD}'),
                    _ => b as char,
                })
                .collect(),
            "windows-1252",
        ),
    }
}

pub fn encode(text: &str, encoding: &str) -> Vec<u8> {
    match encoding {
        "utf-8-bom" => [&[0xEF, 0xBB, 0xBF][..], text.as_bytes()].concat(),
        "utf-16le" => [0xFF, 0xFE].into_iter().chain(text.encode_utf16().flat_map(u16::to_le_bytes)).collect(),
        "utf-16be" => [0xFE, 0xFF].into_iter().chain(text.encode_utf16().flat_map(u16::to_be_bytes)).collect(),
        "windows-1252" => text
            .chars()
            .map(|c| {
                let u = c as u32;
                if u < 0x80 || (0xA0..=0xFF).contains(&u) {
                    u as u8
                } else {
                    CP1252.iter().position(|&x| x as u32 == u).map_or(b'?', |i| 0x80 + i as u8)
                }
            })
            .collect(),
        _ => text.as_bytes().to_vec(),
    }
}

#[tauri::command]
pub async fn read_text(path: String) -> Result<TextFile, String> {
    let meta = std::fs::metadata(&path).map_err(|e| e.to_string())?;
    if meta.len() > MAX {
        return Err("This file is too big to open as text (over 32 MB).".into());
    }
    let bytes = std::fs::read(&path).map_err(|e| e.to_string())?;
    let (text, encoding) = decode(&bytes);
    let eol = if text.contains("\r\n") { "\r\n" } else { "\n" };
    Ok(TextFile { text: text.replace("\r\n", "\n"), encoding, eol })
}

#[derive(Deserialize)]
pub struct SaveText {
    source: String,
    text: String,
    encoding: String,
    eol: String,
    /// "copy" | "replace" | "path"
    target: String,
    dest: Option<String>,
    suffix: String,
}

/// Saves text (a copy next to the original by default), in the original's encoding and line endings.
#[tauri::command]
pub async fn save_text<R: Runtime>(app: AppHandle<R>, req: SaveText) -> Result<String, String> {
    let source = PathBuf::from(&req.source);
    let ext = source.extension().map(|e| e.to_string_lossy().into_owned()).unwrap_or_else(|| "txt".into());
    let dest = match req.target.as_str() {
        "copy" => {
            let dir = source.parent().ok_or("no folder")?;
            let stem = source.file_stem().map(|s| s.to_string_lossy().into_owned()).unwrap_or_default();
            crate::files::unique_path(dir, &format!("{stem}{}", req.suffix), &ext)
        }
        "replace" => source.clone(),
        _ => PathBuf::from(req.dest.as_deref().ok_or("no destination")?),
    };
    let text = if req.eol == "\r\n" { req.text.replace('\n', "\r\n") } else { req.text };
    let bytes = encode(&text, &req.encoding);
    write_atomic(&dest, &bytes, req.target == "replace")?;
    let _ = app.asset_protocol_scope().allow_file(&dest);
    Ok(dest.to_string_lossy().into_owned())
}

/// Writes beside the destination first; for a replace, the old file goes to the trash.
pub fn write_atomic(dest: &Path, bytes: &[u8], replacing: bool) -> Result<(), String> {
    let dir = dest.parent().ok_or("no folder")?;
    let tmp = dir.join(format!(".{}.immediate-tmp", dest.file_name().map(|n| n.to_string_lossy()).unwrap_or_default()));
    std::fs::write(&tmp, bytes).map_err(|e| e.to_string())?;
    if replacing && dest.exists() {
        if let Err(e) = trash::delete(dest) {
            let _ = std::fs::remove_file(&tmp);
            return Err(format!("Couldn’t move the original to the trash: {e}"));
        }
    }
    std::fs::rename(&tmp, dest).map_err(|e| {
        let _ = std::fs::remove_file(&tmp);
        e.to_string()
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn encodings_round_trip() {
        for enc in ["utf-8", "utf-8-bom", "utf-16le", "utf-16be", "windows-1252"] {
            let text = "Café – “quoted” €5";
            let (back, detected) = decode(&encode(text, enc));
            assert_eq!(back, text, "{enc}");
            if enc != "utf-8" {
                assert_eq!(detected, enc);
            }
        }
    }

    #[test]
    fn windows_1252_is_detected_when_not_utf8() {
        let (t, e) = decode(&[b'C', b'a', b'f', 0xE9, b' ', 0x80]);
        assert_eq!((t.as_str(), e), ("Café €", "windows-1252"));
    }
}
