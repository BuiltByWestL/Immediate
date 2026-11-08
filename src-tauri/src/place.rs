//! Window placement: the next window opens where and how big the last one was (maximized too),
//! as long as that spot is still on a connected screen. Kept in `<config>/window.json`.

use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::Mutex;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, PhysicalPosition, PhysicalSize, Runtime, Window};

#[derive(Serialize, Deserialize, Clone, Copy, Debug, PartialEq)]
pub struct Place {
    /// Outer position and inner size in physical pixels, from when the window was last neither
    /// maximized nor minimized.
    pub x: i32,
    pub y: i32,
    pub w: u32,
    pub h: u32,
    pub maximized: bool,
}

/// Each open window's last normal (not maximized or minimized) placement, and a count of its
/// move/resize events (see `track`).
#[derive(Default)]
pub struct Places(Mutex<HashMap<String, Place>>, Mutex<HashMap<String, u64>>);

fn file<R: Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("window.json"))
}

pub fn load<R: Runtime>(app: &AppHandle<R>) -> Option<Place> {
    let text = std::fs::read_to_string(file(app)?).ok()?;
    serde_json::from_str(&text).ok()
}

/// Notes a window's placement once it has stopped moving or resizing for a moment. Maximizing
/// sends move/resize events before the window reports itself maximized, so checking straight
/// away would store the maximized size as the normal one.
pub fn track<R: Runtime>(window: &Window<R>) {
    let label = window.label().to_owned();
    {
        let places = window.app_handle().state::<Places>();
        let Ok(mut gens) = places.1.lock() else { return };
        // Above zero, a waiter is already running and will see this event too.
        let g = gens.entry(label.clone()).or_insert(0);
        let waiting = *g > 0;
        *g += 1;
        if waiting {
            return;
        }
    }
    // One waiter per window, until it has been still for 400 ms.
    let window = window.clone();
    std::thread::spawn(move || {
        let app = window.app_handle().clone();
        let places = app.state::<Places>();
        loop {
            let before = places.1.lock().ok().and_then(|g| g.get(&label).copied()).unwrap_or(0);
            std::thread::sleep(std::time::Duration::from_millis(400));
            let Ok(mut gens) = places.1.lock() else { return };
            if gens.get(&label).copied().unwrap_or(0) == before {
                gens.remove(&label);
                break;
            }
        }
        note(&window);
    });
}

fn note<R: Runtime>(window: &Window<R>) {
    if window.is_maximized().unwrap_or(false) || window.is_minimized().unwrap_or(false) {
        return;
    }
    let (Ok(p), Ok(s)) = (window.outer_position(), window.inner_size()) else { return };
    if s.width < 100 || s.height < 100 {
        return;
    }
    if let Ok(mut m) = window.app_handle().state::<Places>().0.lock() {
        m.insert(window.label().to_owned(), Place { x: p.x, y: p.y, w: s.width, h: s.height, maximized: false });
    }
}

/// Saves a closing window's placement for the next window.
pub fn save<R: Runtime>(window: &Window<R>) {
    let app = window.app_handle();
    // A move or resize just before closing may still be waiting to be noted.
    note(window);
    // A window that was only maximized (never moved) keeps the normal placement it opened with.
    let tracked = app.state::<Places>().0.lock().ok().and_then(|m| m.get(window.label()).copied());
    let Some(mut place) = tracked.or_else(|| load(app)) else { return };
    place.maximized = window.is_maximized().unwrap_or(false);
    if let (Some(path), Ok(json)) = (file(app), serde_json::to_string(&place)) {
        if let Some(dir) = path.parent() {
            let _ = std::fs::create_dir_all(dir);
        }
        let _ = std::fs::write(path, json);
    }
}

/// Whether a window placed at `place` would have its title bar on one of `screens` (each an
/// (x, y, w, h) rectangle in physical pixels).
pub fn visible_on(place: &Place, screens: &[(i32, i32, u32, u32)]) -> bool {
    // A point near the top-left of the title bar must be on a screen, so it can be dragged.
    let (px, py) = (place.x + 60, place.y + 12);
    screens.iter().any(|&(x, y, w, h)| px >= x && py >= y && px < x + w as i32 && py < y + h as i32)
}

/// Applies the last saved placement to a new (still hidden) window. With `cascade`, only the
/// size is used: the caller has already placed it next to the window it came from.
pub fn restore<R: Runtime>(window: &Window<R>, cascade: bool) {
    let Some(place) = load(window.app_handle()) else { return };
    let _ = window.set_size(PhysicalSize::new(place.w, place.h));
    if !cascade {
        let screens: Vec<(i32, i32, u32, u32)> = window
            .available_monitors()
            .unwrap_or_default()
            .iter()
            .map(|m| (m.position().x, m.position().y, m.size().width, m.size().height))
            .collect();
        if visible_on(&place, &screens) {
            let _ = window.set_position(PhysicalPosition::new(place.x, place.y));
        } else {
            let _ = window.center();
        }
    }
    if place.maximized {
        let _ = window.maximize();
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn only_restores_onto_a_connected_screen() {
        let screens = [(0, 0, 1920, 1080), (1920, 0, 2560, 1440)];
        let at = |x, y| Place { x, y, w: 1100, h: 720, maximized: false };
        assert!(visible_on(&at(100, 100), &screens));
        assert!(visible_on(&at(3000, 200), &screens)); // second screen
        assert!(!visible_on(&at(5000, 200), &screens)); // a screen that was unplugged
        assert!(!visible_on(&at(100, -400), &screens)); // title bar above every screen
        assert!(visible_on(&at(-50, 0), &screens)); // hanging a little off the left edge
    }
}
