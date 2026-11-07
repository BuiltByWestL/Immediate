//! Windows: one file per window, one process. Files arrive from the command line, a second
//! launch (forwarded by the single-instance plugin), macOS "open document" events, drag-drop
//! (handled in the UI) and the Open dialog.
//!
//! Each window has an inbox of file batches. The UI takes from it once it has loaded and again
//! whenever it is told new files arrived, so nothing is lost while a webview is still starting.

use std::collections::HashMap;
use std::path::Path;
#[cfg(target_os = "macos")]
use std::path::PathBuf;
use std::sync::Mutex;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use serde::Serialize;
use tauri::webview::WebviewBuilder;
use tauri::window::WindowBuilder;
use tauri::{AppHandle, Emitter, Manager, PhysicalPosition, Runtime, WebviewUrl, Window};

use crate::formats;
use crate::settings::SettingsState;

/// Files launched together from Explorer/Finder arrive as separate launches a few ms apart;
/// launches this close together are gathered into one window.
const GATHER: Duration = Duration::from_millis(1500);

#[derive(Serialize, Clone, Debug)]
pub struct Batch {
    pub files: Vec<String>,
    /// true: add to the window's current set of files instead of replacing it
    pub append: bool,
}

#[derive(Default)]
pub struct Wm {
    next: AtomicU32,
    inner: Mutex<Inner>,
}

#[derive(Default)]
struct Inner {
    has_file: HashMap<String, bool>,
    inbox: HashMap<String, Vec<Batch>>,
    last_open: Option<(Instant, String)>,
}

pub fn set_has_file<R: Runtime>(app: &AppHandle<R>, label: &str) {
    if let Ok(mut i) = app.state::<Wm>().inner.lock() {
        i.has_file.insert(label.to_owned(), true);
    }
}

pub fn forget<R: Runtime>(app: &AppHandle<R>, label: &str) {
    if let Ok(mut i) = app.state::<Wm>().inner.lock() {
        i.has_file.remove(label);
        i.inbox.remove(label);
    }
}

/// Command-line arguments that name files (relative paths resolve against `cwd`).
pub fn files_from_args<I: IntoIterator<Item = String>>(args: I, cwd: &Path) -> Vec<String> {
    args.into_iter()
        .filter(|a| !a.starts_with('-') || Path::new(a).exists())
        .map(|a| cwd.join(a))
        .filter(|p| p.is_file())
        .map(|p| std::path::absolute(&p).unwrap_or(p).to_string_lossy().into_owned())
        .collect()
}

/// Opens a new window, optionally with files. Returns its label.
pub fn open_window<R: Runtime>(app: &AppHandle<R>, files: Vec<String>) -> tauri::Result<String> {
    let wm = app.state::<Wm>();
    let label = format!("w{}", wm.next.fetch_add(1, Ordering::Relaxed) + 1);
    if let Ok(mut i) = wm.inner.lock() {
        i.has_file.insert(label.clone(), false);
        if !files.is_empty() {
            i.inbox.insert(label.clone(), vec![Batch { files, append: false }]);
        }
        i.last_open = Some((Instant::now(), label.clone()));
    }

    // Cascade from the focused window, otherwise center.
    let anchor = app.windows().values().find(|w| w.is_focused().unwrap_or(false)).cloned();
    let mut builder = WindowBuilder::new(app, &label)
        .title("Immediate")
        .inner_size(1100.0, 720.0)
        .min_inner_size(560.0, 400.0)
        .visible(false);
    builder = match anchor.as_ref().and_then(|w| Some((w.outer_position().ok()?, w.scale_factor().ok()?))) {
        Some((p, scale)) => {
            let p = p.to_logical::<f64>(scale);
            builder.position(p.x + 28.0, p.y + 28.0)
        }
        None => builder.center(),
    };
    let window = builder.build()?;
    // Paint the frame in the theme color before it is shown, so it never flashes white.
    let theme = crate::chrome::apply(app, &window);
    window.show()?;
    let _ = window.set_focus();
    crate::mark("windowShown");

    // Attach the webview once the event loop has had a chance to paint the window. On Windows,
    // WebView2 needs ~250 ms to start its browser process; the window is on screen meanwhile.
    let init = crate::themes::init_script(app, &app.state::<SettingsState>().get());
    let (r, g, b) = theme.color("bg");
    let handle = app.clone();
    let l = label.clone();
    app.run_on_main_thread(move || {
        let Some(window) = handle.get_window(&l) else { return };
        let size = window.inner_size().unwrap_or_default();
        let webview = WebviewBuilder::new(&l, WebviewUrl::default())
            .initialization_script(&init)
            .background_color(tauri::window::Color(r, g, b, 255))
            .auto_resize();
        if let Err(e) = window.add_child(webview, PhysicalPosition::new(0, 0), size) {
            eprintln!("failed to create webview: {e}");
        }
        crate::mark("webviewBuilt");
    })?;
    Ok(label)
}

/// Routes files from outside (second launch, macOS open event) to a window: a window opened
/// moments ago gathers them, an empty window takes them, otherwise a new window opens.
pub fn deliver<R: Runtime>(app: &AppHandle<R>, files: Vec<String>) {
    let files: Vec<String> = files.into_iter().filter(|f| formats::lookup(Path::new(f)).is_some()).collect();
    let target = {
        let wm = app.state::<Wm>();
        let Ok(mut i) = wm.inner.lock() else { return };
        let alive = |l: &str| app.get_window(l).is_some();
        let recent = i
            .last_open
            .as_ref()
            .filter(|(t, l)| t.elapsed() < GATHER && alive(l) && !files.is_empty())
            .map(|(_, l)| (l.clone(), true));
        let empty = || {
            i.has_file
                .iter()
                .find(|(l, has)| !**has && alive(l) && !i.inbox.get(*l).is_some_and(|b| !b.is_empty()))
                .map(|(l, _)| (l.clone(), false))
        };
        let target = recent.or_else(empty);
        if let Some((label, append)) = &target {
            if !files.is_empty() {
                i.inbox.entry(label.clone()).or_default().push(Batch { files: files.clone(), append: *append });
            }
            if !append {
                i.last_open = Some((Instant::now(), label.clone()));
            }
        }
        target
    };
    match target {
        Some((label, _)) => {
            let _ = app.emit_to(&label, "inbox", ());
            if let Some(w) = app.get_window(&label) {
                let _ = w.unminimize();
                let _ = w.set_focus();
            }
        }
        None => {
            if let Err(e) = open_window(app, files) {
                eprintln!("failed to open window: {e}");
            }
        }
    }
}

/// The UI takes the files waiting for its window.
#[tauri::command]
pub fn inbox_take<R: Runtime>(app: AppHandle<R>, window: Window<R>) -> Vec<Batch> {
    app.state::<Wm>()
        .inner
        .lock()
        .ok()
        .and_then(|mut i| i.inbox.remove(window.label()))
        .unwrap_or_default()
}

/// Opens files in new windows (or one empty window), e.g. files dropped on a window that is busy.
#[tauri::command]
pub async fn open_in_new_window<R: Runtime>(app: AppHandle<R>, files: Vec<String>) -> Result<(), String> {
    open_window(&app, files).map(|_| ()).map_err(|e| e.to_string())
}

#[cfg(target_os = "macos")]
pub fn path_list(paths: impl IntoIterator<Item = PathBuf>) -> Vec<String> {
    paths.into_iter().map(|p| p.to_string_lossy().into_owned()).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn args_keep_existing_files_only() {
        let dir = std::env::temp_dir().join(format!("immediate-args-{}", std::process::id()));
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("a.jpg"), b"x").unwrap();
        let got = files_from_args(["a.jpg".into(), "--flag".into(), "missing.png".into()], &dir);
        assert_eq!(got.len(), 1);
        assert!(got[0].ends_with("a.jpg"));
        std::fs::remove_dir_all(dir).unwrap();
    }
}
