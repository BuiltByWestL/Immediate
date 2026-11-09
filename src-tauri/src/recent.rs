//! Recently opened files (not projects): a short list in `<config>/recent.json`, newest first,
//! shown on the start screen. Temporary files (pasted pictures) aren't remembered.

use std::path::{Path, PathBuf};
use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime};

use crate::formats::{self, Kind};

const KEEP: usize = 12;

/// Serializes reads and writes of the list across windows.
static LOCK: Mutex<()> = Mutex::new(());

fn file<R: Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("recent.json"))
}

fn load(path: &Path) -> Vec<String> {
    std::fs::read_to_string(path).ok().and_then(|s| serde_json::from_str(&s).ok()).unwrap_or_default()
}

/// The list with `path` moved to the front.
fn with_front(mut list: Vec<String>, path: &str) -> Vec<String> {
    list.retain(|p| !p.eq_ignore_ascii_case(path));
    list.insert(0, path.to_owned());
    list.truncate(KEEP);
    list
}

/// Remembers a file the user opened (not ones reached with the arrow keys).
#[tauri::command]
pub fn recent_add<R: Runtime>(app: AppHandle<R>, path: String) {
    note(&app, Path::new(&path));
}

fn note<R: Runtime>(app: &AppHandle<R>, path: &Path) {
    if path.starts_with(crate::files::temp_dir()) {
        return;
    }
    let Some(store) = file(app) else { return };
    let Ok(_guard) = LOCK.lock() else { return };
    let list = with_front(load(&store), &path.to_string_lossy());
    if let Ok(json) = serde_json::to_string_pretty(&list) {
        let _ = crate::settings::write_atomic(&store, &json);
    }
}

#[derive(Serialize)]
pub struct RecentFile {
    path: String,
    name: String,
    dir: String,
    kind: Option<Kind>,
}

/// The recent files that still exist, newest first.
#[tauri::command]
pub fn recent_list<R: Runtime>(app: AppHandle<R>) -> Vec<RecentFile> {
    let Some(store) = file(&app) else { return vec![] };
    let _guard = LOCK.lock();
    load(&store)
        .into_iter()
        .map(PathBuf::from)
        .filter(|p| p.is_file())
        .map(|p| RecentFile {
            name: p.file_name().map(|n| n.to_string_lossy().into_owned()).unwrap_or_default(),
            dir: p.parent().map(|d| d.to_string_lossy().into_owned()).unwrap_or_default(),
            kind: formats::lookup(&p).or_else(|| formats::sniff(&p).map(|(_, f)| f)).map(|f| f.kind),
            path: p.to_string_lossy().into_owned(),
        })
        .collect()
}

#[tauri::command]
pub fn recent_clear<R: Runtime>(app: AppHandle<R>) {
    let Some(store) = file(&app) else { return };
    let _guard = LOCK.lock();
    let _ = std::fs::remove_file(store);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn newest_first_without_repeats_and_short() {
        let mut list = Vec::new();
        for i in 0..20 {
            list = with_front(list, &format!("C:\\a\\{i}.jpg"));
        }
        assert_eq!(list.len(), KEEP);
        assert_eq!(list[0], "C:\\a\\19.jpg");
        let list = with_front(list, "c:\\A\\15.JPG");
        assert_eq!(list[0], "c:\\A\\15.JPG");
        assert_eq!(list.iter().filter(|p| p.to_lowercase().ends_with("15.jpg")).count(), 1);
    }
}
