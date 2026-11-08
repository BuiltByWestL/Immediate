mod chrome;
mod files;
mod formats;
mod image;
mod media;
mod settings;
mod shellmenu;
mod text;
mod themes;
mod tools;
mod update;
mod wm;

use std::sync::{Mutex, OnceLock};
use std::time::{Instant, SystemTime, UNIX_EPOCH};

use serde::Deserialize;
use tauri::webview::PageLoadEvent;
use tauri::{Manager, Window, WindowEvent};

/// Process start, used for the startup budget (`scripts/bench-startup.mjs`).
static START: OnceLock<(Instant, f64)> = OnceLock::new();
/// Milestones recorded on the Rust side: (name, ms since start).
static MARKS: Mutex<Vec<(&'static str, u64)>> = Mutex::new(Vec::new());

fn since_start() -> u64 {
    START.get().map_or(0, |(s, _)| s.elapsed().as_millis() as u64)
}

pub(crate) fn mark(name: &'static str) {
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

    let mut builder = tauri::Builder::default();
    // A second launch (double-click, "Open with", right-click) hands its files to this process,
    // which opens them in a window right away. Benchmarks and tests run standalone.
    let standalone = std::env::var_os("IMMEDIATE_BENCH_OUT").is_some() || std::env::var_os("IMMEDIATE_UPDATE_TEST").is_some();
    if !standalone {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, argv, cwd| {
            wm::deliver(app, wm::files_from_args(argv.into_iter().skip(1), std::path::Path::new(&cwd)));
        }));
    }

    let app = builder
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .manage(update::PendingUpdate::default())
        .manage(wm::Wm::default())
        .setup(|app| {
            mark("setup");
            app.manage(settings::SettingsState::load(app.handle()));
            update::run_test_hook(app.handle());
            let cwd = std::env::current_dir().unwrap_or_default();
            wm::open_window(app.handle(), wm::files_from_args(std::env::args().skip(1), &cwd))?;
            Ok(())
        })
        .on_window_event(|window, event| match event {
            WindowEvent::ThemeChanged(_) => {
                chrome::apply(window.app_handle(), window);
            }
            WindowEvent::Destroyed => wm::forget(window.app_handle(), window.label()),
            _ => {}
        })
        .on_page_load(|_, payload| match payload.event() {
            PageLoadEvent::Started => mark("pageStarted"),
            PageLoadEvent::Finished => mark("pageFinished"),
        })
        .invoke_handler(tauri::generate_handler![
            app_ready,
            files::open_path,
            files::list_folder,
            files::file_details,
            files::clipboard_paste,
            files::reveal,
            image::image_info,
            image::decode_image,
            image::copy_image_file,
            image::copy_pixels,
            image::thumbnail,
            image::start_drag,
            image::image_save,
            settings::settings_get,
            settings::settings_set,
            text::read_text,
            text::save_text,
            themes::themes_custom,
            themes::themes_folder,
            tools::tools_status,
            media::media_tools,
            media::media_probe,
            media::media_keyframes,
            media::media_waveform,
            media::media_loudness,
            media::media_frame,
            media::media_proxy,
            media::record::rec_devices,
            media::record::rec_start,
            media::record::rec_pause,
            media::record::rec_stop,
            media::install::tools_install,
            media::export::media_export,
            media::export::media_cancel,
            media::export::media_grab_frame,
            wm::inbox_take,
            wm::open_in_new_window,
            update::update_check,
            update::update_install,
            shellmenu::shell_menu_status,
            shellmenu::shell_menu_set
        ])
        .build(tauri::generate_context!())
        .expect("error while starting Immediate");

    app.run(|_app, _event| {
        // macOS delivers "Open with" / double-clicked files as an event instead of arguments.
        #[cfg(target_os = "macos")]
        if let tauri::RunEvent::Opened { urls } = _event {
            let files = urls.into_iter().filter_map(|u| u.to_file_path().ok());
            wm::deliver(_app, wm::path_list(files));
        }
    });
}
