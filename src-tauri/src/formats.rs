//! File types Immediate opens, from src/formats.json (shared with the UI and the installer).

use std::collections::HashMap;
use std::path::Path;
use std::sync::OnceLock;

use serde::Serialize;

#[derive(Serialize, Clone, Copy, PartialEq, Eq, Debug)]
#[serde(rename_all = "lowercase")]
pub enum Kind {
    Image,
    Video,
    Audio,
    Document,
}

#[derive(Clone, Copy, Debug)]
pub struct Format {
    pub kind: Kind,
    /// false = view only (develops/converts on save, see the plan's file-type table)
    pub editable: bool,
}

fn table() -> &'static HashMap<String, Format> {
    static TABLE: OnceLock<HashMap<String, Format>> = OnceLock::new();
    TABLE.get_or_init(|| {
        let json: serde_json::Value =
            serde_json::from_str(include_str!("../../src/formats.json")).expect("formats.json");
        let mut map = HashMap::new();
        for (key, kind) in [("image", Kind::Image), ("video", Kind::Video), ("audio", Kind::Audio), ("document", Kind::Document)] {
            for (group, editable) in [("edit", true), ("view", false)] {
                if let Some(exts) = json[key][group].as_object() {
                    for ext in exts.keys() {
                        map.insert(ext.clone(), Format { kind, editable });
                    }
                }
            }
        }
        map
    })
}

pub fn lookup(path: &Path) -> Option<Format> {
    let ext = path.extension()?.to_str()?.to_ascii_lowercase();
    table().get(&ext).copied()
}

/// The format of a file with a missing or unknown extension, from its first bytes. Returns the
/// extension it should have.
pub fn sniff(path: &Path) -> Option<(&'static str, Format)> {
    use std::io::Read;
    let mut head = [0u8; 64];
    let n = std::fs::File::open(path).ok()?.read(&mut head).ok()?;
    let ext = sniff_bytes(&head[..n])?;
    Some((ext, *table().get(ext)?))
}

fn sniff_bytes(b: &[u8]) -> Option<&'static str> {
    let at = |i: usize, sig: &[u8]| b.len() >= i + sig.len() && &b[i..i + sig.len()] == sig;
    Some(match () {
        _ if at(0, &[0xFF, 0xD8, 0xFF]) => "jpg",
        _ if at(0, b"\x89PNG") => "png",
        _ if at(0, b"GIF8") => "gif",
        _ if at(0, b"RIFF") && at(8, b"WEBP") => "webp",
        _ if at(0, b"RIFF") && at(8, b"WAVE") => "wav",
        _ if at(0, b"RIFF") && at(8, b"AVI ") => "avi",
        _ if at(0, b"II*\0") || at(0, b"MM\0*") => "tif",
        _ if at(0, b"8BPS") => "psd",
        _ if at(0, &[0xFF, 0x0A]) || at(4, b"JXL ") => "jxl",
        _ if at(4, b"ftyp") => match b.get(8..12)? {
            b"heic" | b"heix" | b"mif1" | b"msf1" | b"hevc" => "heic",
            b"avif" | b"avis" => "avif",
            b"qt  " => "mov",
            b"M4A " | b"M4B " => "m4a",
            b"3gp4" | b"3gp5" | b"3gp6" | b"3g2a" => "3gp",
            _ => "mp4",
        },
        _ if at(0, &[0x1A, 0x45, 0xDF, 0xA3]) => {
            if b.windows(4).any(|w| w == b"webm") { "webm" } else { "mkv" }
        }
        _ if at(0, b"fLaC") => "flac",
        _ if at(0, b"OggS") => "ogg",
        _ if at(0, b"ID3") || (b.len() > 1 && b[0] == 0xFF && b[1] & 0xE6 == 0xE2) => "mp3",
        _ if at(0, b"FORM") && (at(8, b"AIFF") || at(8, b"AIFC")) => "aiff",
        _ if at(0, b"caff") => "caf",
        _ if at(0, &[0x30, 0x26, 0xB2, 0x75]) => "wmv",
        _ if at(0, &[0x00, 0x00, 0x01, 0xBA]) => "mpg",
        _ if at(0, b"FLV") => "flv",
        _ if at(0, b"%PDF") => "pdf",
        _ if at(0, b"BM") && b.len() > 14 => "bmp",
        _ => return None,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn looks_up_by_extension_case_insensitively() {
        assert_eq!(lookup(Path::new("a/IMG_0001.JPG")).unwrap().kind, Kind::Image);
        assert_eq!(lookup(Path::new("clip.mkv")).unwrap().kind, Kind::Video);
        assert_eq!(lookup(Path::new("memo.opus")).unwrap().kind, Kind::Audio);
        assert!(!lookup(Path::new("raw.CR3")).unwrap().editable);
        assert_eq!(lookup(Path::new("notes.txt")).unwrap().kind, Kind::Document);
        assert!(!lookup(Path::new("sheet.xlsx")).unwrap().editable);
        assert!(lookup(Path::new("archive.zip")).is_none());
        assert!(lookup(Path::new("no_extension")).is_none());
    }

    #[test]
    fn recognizes_files_by_their_first_bytes() {
        assert_eq!(sniff_bytes(&[0xFF, 0xD8, 0xFF, 0xE1, 0, 0]), Some("jpg"));
        assert_eq!(sniff_bytes(b"\x89PNG\r\n\x1a\n"), Some("png"));
        assert_eq!(sniff_bytes(b"\0\0\0\x18ftypheic\0\0\0\0"), Some("heic"));
        assert_eq!(sniff_bytes(b"\0\0\0\x20ftypisom\0\0\x02\0"), Some("mp4"));
        assert_eq!(sniff_bytes(b"\0\0\0\x14ftypqt  \0\0\0\0"), Some("mov"));
        assert_eq!(sniff_bytes(b"RIFF\0\0\0\0WAVEfmt "), Some("wav"));
        assert_eq!(sniff_bytes(&[0x1A, 0x45, 0xDF, 0xA3, 0x9F, 0x42, 0x82, 0x84, b'w', b'e', b'b', b'm']), Some("webm"));
        assert_eq!(sniff_bytes(b"ID3\x04\0"), Some("mp3"));
        assert_eq!(sniff_bytes(b"%PDF-1.7"), Some("pdf"));
        assert_eq!(sniff_bytes(b"PK\x03\x04"), None);
        assert_eq!(sniff_bytes(b"hello"), None);
    }
}
