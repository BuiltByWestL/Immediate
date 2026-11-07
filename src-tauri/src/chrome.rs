//! Paints the native window frame in the theme's colors so the title bar and the app read as one
//! surface. Windows 11 lets us color the caption directly; elsewhere the frame follows light/dark.

use tauri::window::Color;
use tauri::{AppHandle, Manager, Runtime, Theme as OsTheme, Window};

use crate::settings::SettingsState;
use crate::themes::{self, Theme};

/// Whether the OS is in dark mode, as seen by this window (only meaningful while the window's
/// theme is not forced, i.e. for the "system" theme).
fn os_dark<R: Runtime>(window: &Window<R>) -> bool {
    matches!(window.theme(), Ok(OsTheme::Dark))
}

/// Resolves the window's theme from the settings and applies it to the native frame.
/// Returns the theme so callers can reuse its colors.
pub fn apply<R: Runtime>(app: &AppHandle<R>, window: &Window<R>) -> Theme {
    let s = app.state::<SettingsState>().get();
    let follow_os = s.theme == "system";
    if follow_os {
        let _ = window.set_theme(None);
    }
    let (_, theme) = themes::resolve(app, &s, os_dark(window));
    if !follow_os {
        let _ = window.set_theme(Some(if theme.is_dark() { OsTheme::Dark } else { OsTheme::Light }));
    }
    let (r, g, b) = theme.color("bg");
    let _ = window.set_background_color(Some(Color(r, g, b, 255)));
    for webview in window.webviews() {
        let _ = webview.set_background_color(Some(Color(r, g, b, 255)));
    }
    #[cfg(windows)]
    caption_colors(window, theme.color("bg"), theme.color("text"), theme.color("line"));
    theme
}

/// Re-applies the frame colors on every window (after a settings change).
pub fn apply_all<R: Runtime>(app: &AppHandle<R>) {
    for window in app.windows().values() {
        apply(app, window);
    }
}

#[cfg(windows)]
fn caption_colors<R: Runtime>(window: &Window<R>, bg: (u8, u8, u8), text: (u8, u8, u8), border: (u8, u8, u8)) {
    use windows_sys::Win32::Graphics::Dwm::{
        DWMWA_BORDER_COLOR, DWMWA_CAPTION_COLOR, DWMWA_TEXT_COLOR, DwmSetWindowAttribute,
    };
    let Ok(hwnd) = window.hwnd() else { return };
    let colorref = |(r, g, b): (u8, u8, u8)| u32::from(r) | u32::from(g) << 8 | u32::from(b) << 16;
    for (attr, color) in [(DWMWA_CAPTION_COLOR, bg), (DWMWA_TEXT_COLOR, text), (DWMWA_BORDER_COLOR, border)] {
        let value = colorref(color);
        // Fails harmlessly before Windows 11 (build 22000), which keeps the default frame.
        unsafe {
            DwmSetWindowAttribute(hwnd.0 as _, attr as u32, (&raw const value).cast(), size_of::<u32>() as u32);
        }
    }
}
