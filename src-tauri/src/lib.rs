use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use tauri::webview::{PageLoadEvent, WebviewBuilder};
use tauri::window::{Color, WindowBuilder};
use tauri::{Manager, PhysicalPosition, WebviewUrl, Window};

/// Process start, used for the startup budget (`scripts/bench-startup.mjs`).
static START: OnceLock<(Instant, f64)> = OnceLock::new();
/// Milestones recorded on the Rust side: (name, ms since start).
static MARKS: Mutex<Vec<(&'static str, u64)>> = Mutex::new(Vec::new());

fn since_start() -> u64 {
    START.get().map_or(0, |(s, _)| s.elapsed().as_millis() as u64)
}

fn mark(name: &'static str) {
    if let Ok(mut m) = MARKS.lock() {
        m.push((name, since_start()));
    }
}

/// Frontend milestones as epoch milliseconds (performance.timeOrigin + offset).
#[derive(Deserialize, Default)]
#[serde(rename_all = "camelCase")]
struct WebMarks {
    nav_start: f64,
    dom_content_loaded: f64,
    script_start: f64,
    first_frame: f64,
}

/// Called by the frontend after its first frame is painted; reports time-to-content.
#[tauri::command]
fn app_ready(window: Window, marks: Option<WebMarks>) -> u64 {
    let ms = since_start();

    // Benchmark mode: write the measurements to the given file and quit.
    if let Some(path) = std::env::var_os("IMMEDIATE_BENCH_OUT") {
        let epoch0 = START.get().map_or(0.0, |(_, e)| *e);
        let rel = |t: f64| if t > 0.0 { (t - epoch0).round() as i64 } else { -1 };
        let w = marks.unwrap_or_default();
        let mut out = serde_json::Map::new();
        out.insert("total".into(), ms.into());
        if let Ok(m) = MARKS.lock() {
            for (k, v) in m.iter() {
                out.insert((*k).into(), (*v).into());
            }
        }
        out.insert("navStart".into(), rel(w.nav_start).into());
        out.insert("scriptStart".into(), rel(w.script_start).into());
        out.insert("domContentLoaded".into(), rel(w.dom_content_loaded).into());
        out.insert("firstFrame".into(), rel(w.first_frame).into());
        let _ = std::fs::write(path, serde_json::Value::Object(out).to_string());
        window.app_handle().exit(0);
    }
    ms
}

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    let epoch_ms = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_or(0.0, |d| d.as_secs_f64() * 1000.0);
    START.get_or_init(|| (Instant::now(), epoch_ms));

    tauri::Builder::default()
        .plugin(tauri_plugin_opener::init())
        .setup(|app| {
            mark("setup");
            // Show a plain native window right away, painted in the theme's background color,
            // then attach the webview. On Windows, WebView2 needs ~250 ms to start its browser
            // process; without this split nothing would be on screen during that time.
            let bg = Color(0x1b, 0x1b, 0x1c, 0xff);
            WindowBuilder::new(app, "main")
                .title("Immediate")
                .inner_size(1100.0, 720.0)
                .min_inner_size(560.0, 400.0)
                .center()
                .background_color(bg)
                .visible(true)
                .build()?;
            mark("windowShown");

            // Attach the webview once the event loop has had a chance to paint the window.
            let handle = app.handle().clone();
            handle.clone().run_on_main_thread(move || {
                let Some(window) = handle.get_window("main") else { return };
                let size = window.inner_size().unwrap_or_default();
                let webview = WebviewBuilder::new("main-view", WebviewUrl::default())
                    .background_color(bg)
                    .auto_resize();
                if let Err(e) = window.add_child(webview, PhysicalPosition::new(0, 0), size) {
                    eprintln!("failed to create webview: {e}");
                }
                mark("webviewBuilt");
            })?;
            Ok(())
        })
        .on_page_load(|_, payload| match payload.event() {
            PageLoadEvent::Started => mark("pageStarted"),
            PageLoadEvent::Finished => mark("pageFinished"),
        })
        .invoke_handler(tauri::generate_handler![app_ready])
        .run(tauri::generate_context!())
        .expect("error while running Immediate");
}
