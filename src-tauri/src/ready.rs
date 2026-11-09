//! "Keep Immediate ready in the background" (Settings → General): when the last window closes,
//! the process stays with one hidden, fully loaded window, so the next file opens instantly. An
//! icon in the notification area (menu bar on macOS) shows it's running and offers Quit.

use tauri::menu::{Menu, MenuItem};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::{AppHandle, Manager, Runtime};

use crate::settings::SettingsState;
use crate::wm;

const TRAY: &str = "ready";

fn wanted<R: Runtime>(app: &AppHandle<R>) -> bool {
    app.try_state::<SettingsState>().is_some_and(|s| s.get().keep_ready)
}

/// The last window closed. Returns true to keep running (with a hidden window ready).
pub fn on_last_window_closed<R: Runtime>(app: &AppHandle<R>) -> bool {
    if !wanted(app) {
        return false;
    }
    if let Err(e) = wm::open_spare(app) {
        eprintln!("couldn't prepare a window: {e}");
        return false;
    }
    show_tray(app);
    true
}

/// The hidden window was shown (or the setting turned off): no more icon.
pub fn hide_tray<R: Runtime>(app: &AppHandle<R>) {
    let _ = app.remove_tray_by_id(TRAY);
}

/// Settings changed: turning the option off while only the hidden window is left quits.
pub fn settings_changed<R: Runtime>(app: &AppHandle<R>) {
    if !wanted(app) && wm::only_spare_left(app) {
        hide_tray(app);
        app.exit(0);
    }
}

fn show_tray<R: Runtime>(app: &AppHandle<R>) {
    if app.tray_by_id(TRAY).is_some() {
        return;
    }
    let build = || -> tauri::Result<()> {
        let open = MenuItem::with_id(app, "ready-open", "Open Immediate", true, None::<&str>)?;
        let quit = MenuItem::with_id(app, "ready-quit", "Quit Immediate", true, None::<&str>)?;
        let menu = Menu::with_items(app, &[&open, &quit])?;
        let mut tray = TrayIconBuilder::with_id(TRAY)
            .tooltip("Immediate is ready")
            .menu(&menu)
            .show_menu_on_left_click(false)
            .on_menu_event(|app, e| match e.id().as_ref() {
                "ready-open" => wm::deliver(app, vec![]),
                "ready-quit" => {
                    hide_tray(app);
                    app.exit(0);
                }
                _ => {}
            })
            .on_tray_icon_event(|tray, e| {
                if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, .. } = e {
                    wm::deliver(tray.app_handle(), vec![]);
                }
            });
        if let Some(icon) = app.default_window_icon() {
            tray = tray.icon(icon.clone());
        }
        tray.build(app)?;
        Ok(())
    };
    if let Err(e) = build() {
        eprintln!("couldn't show the notification area icon: {e}");
    }
}
