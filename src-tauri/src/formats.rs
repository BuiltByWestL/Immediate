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
}
