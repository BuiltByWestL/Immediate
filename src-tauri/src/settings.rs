//! User settings: one JSON file in the app config folder. No accounts, no telemetry.
//!
//! Unknown or missing fields fall back to defaults, so older and newer settings files both load.

use std::path::PathBuf;
use std::sync::RwLock;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Emitter, Manager, Runtime, State};

#[derive(Serialize, Deserialize, Clone, Debug, PartialEq)]
#[serde(default, rename_all = "camelCase")]
pub struct Settings {
    /// "system" (follows the OS, using `system_light` / `system_dark`), a built-in theme id,
    /// or "custom:<file stem>" for a theme in `<config>/themes/`.
    pub theme: String,
    pub system_light: String,
    pub system_dark: String,
    /// Accent override (see src/design/accents.json); None keeps the theme's own accent.
    pub accent: Option<String>,
    /// "simple" (default) or "advanced".
    pub mode: String,
    /// "immediate" (bundled), "helvetica" (system Helvetica Neue, if installed) or "system".
    pub font: String,
    pub reduce_motion: bool,
    /// Tint the canvas behind photos and video with the theme. Off keeps it neutral grey.
    pub tint_canvas: bool,
    pub check_updates: bool,
    /// Appended to the file name by Save ("clip edited.mp4").
    pub save_suffix: String,
    pub info_open: bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            theme: "system".into(),
            system_light: "paper".into(),
            system_dark: "graphite".into(),
            accent: None,
            mode: "simple".into(),
            font: "immediate".into(),
            reduce_motion: false,
            tint_canvas: false,
            check_updates: true,
            save_suffix: " edited".into(),
            info_open: false,
        }
    }
}

impl Settings {
    /// Repairs values a hand-edited or older file might have wrong.
    fn sanitized(mut self) -> Self {
        let d = Settings::default();
        if !matches!(self.mode.as_str(), "simple" | "advanced") {
            self.mode = d.mode;
        }
        if !matches!(self.font.as_str(), "immediate" | "helvetica" | "system") {
            self.font = d.font;
        }
        if self.theme.is_empty() {
            self.theme = d.theme;
        }
        self
    }
}

pub struct SettingsState {
    path: Option<PathBuf>,
    current: RwLock<Settings>,
}

impl SettingsState {
    pub fn load<R: Runtime>(app: &AppHandle<R>) -> Self {
        let path = app.path().app_config_dir().ok().map(|d| d.join("settings.json"));
        let current = path
            .as_ref()
            .and_then(|p| std::fs::read_to_string(p).ok())
            .map(|s| parse(&s))
            .unwrap_or_default();
        Self { path, current: RwLock::new(current) }
    }

    pub fn get(&self) -> Settings {
        self.current.read().map(|s| s.clone()).unwrap_or_default()
    }

    fn set(&self, s: Settings) -> Result<(), String> {
        if let Some(path) = &self.path {
            write_atomic(path, &serde_json::to_string_pretty(&s).map_err(|e| e.to_string())?)?;
        }
        *self.current.write().map_err(|e| e.to_string())? = s;
        Ok(())
    }
}

fn parse(json: &str) -> Settings {
    serde_json::from_str::<Settings>(json).unwrap_or_default().sanitized()
}

/// Writes via a temp file + rename so a crash never leaves a half-written file.
pub fn write_atomic(path: &std::path::Path, contents: &str) -> Result<(), String> {
    if let Some(dir) = path.parent() {
        std::fs::create_dir_all(dir).map_err(|e| e.to_string())?;
    }
    let tmp = path.with_extension("json.tmp");
    std::fs::write(&tmp, contents).map_err(|e| e.to_string())?;
    std::fs::rename(&tmp, path).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn settings_get(state: State<'_, SettingsState>) -> Settings {
    state.get()
}

/// Saves the settings and tells every window, so a theme change applies everywhere at once.
#[tauri::command]
pub fn settings_set<R: Runtime>(
    app: AppHandle<R>,
    state: State<'_, SettingsState>,
    settings: Settings,
) -> Result<Settings, String> {
    let s = settings.sanitized();
    state.set(s.clone())?;
    crate::chrome::apply_all(&app);
    let _ = app.emit("settings", &s);
    Ok(s)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_and_unknown_fields_fall_back_to_defaults() {
        let s = parse(r#"{ "theme": "sunset", "someFutureField": 1 }"#);
        assert_eq!(s.theme, "sunset");
        assert_eq!(s.mode, "simple");
        assert!(s.check_updates);
    }

    #[test]
    fn invalid_values_are_repaired() {
        let s = parse(r#"{ "mode": "expert", "font": "comic", "theme": "" }"#);
        assert_eq!(s, Settings::default());
        assert_eq!(parse("not json"), Settings::default());
    }

    #[test]
    fn round_trips() {
        let s = Settings { theme: "ink".into(), accent: Some("pink".into()), mode: "advanced".into(), ..Default::default() };
        assert_eq!(parse(&serde_json::to_string(&s).unwrap()), s);
    }
}
