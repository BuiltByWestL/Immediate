//! Themes: built-in ones are compiled in from src/design/themes/*.json (the same files the UI
//! and the contrast test read); custom ones are JSON files in `<config>/themes/`.
//!
//! The Rust side needs themes for two things the webview can't do in time: painting the native
//! window and title bar in the theme color, and injecting the theme into the page before its
//! first paint (so there is never a flash of the wrong colors).

use std::collections::BTreeMap;
use std::path::PathBuf;

use serde::{Deserialize, Serialize};
use tauri::{AppHandle, Manager, Runtime};

use crate::settings::Settings;

macro_rules! builtin {
    ($($id:literal),* $(,)?) => {
        &[$(($id, include_str!(concat!("../../src/design/themes/", $id, ".json")))),*]
    };
}
const BUILTIN: &[(&str, &str)] = builtin![
    "paper", "graphite", "ink", "mist", "sand", "dusk", "sunset", "aurora", "midnight", "candy", "citrus",
];
const ACCENTS: &str = include_str!("../../src/design/accents.json");

#[derive(Serialize, Deserialize, Clone, Debug)]
pub struct Theme {
    pub name: String,
    #[serde(default)]
    pub group: String,
    pub scheme: String,
    pub tokens: BTreeMap<String, String>,
}

impl Theme {
    pub fn is_dark(&self) -> bool {
        self.scheme == "dark"
    }
    pub fn color(&self, token: &str) -> (u8, u8, u8) {
        self.tokens.get(token).and_then(|c| parse_hex(c)).unwrap_or((0x80, 0x80, 0x80))
    }
}

pub fn parse_hex(c: &str) -> Option<(u8, u8, u8)> {
    let h = c.strip_prefix('#')?;
    if h.len() != 6 {
        return None;
    }
    let n = u32::from_str_radix(h, 16).ok()?;
    Some(((n >> 16) as u8, (n >> 8) as u8, n as u8))
}

fn builtin_theme(id: &str) -> Option<Theme> {
    BUILTIN
        .iter()
        .find(|(k, _)| *k == id)
        .and_then(|(_, json)| serde_json::from_str(json).ok())
}

fn custom_dir<R: Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    app.path().app_config_dir().ok().map(|d| d.join("themes"))
}

/// Custom themes as (id, theme), id = "custom:<file stem>". Invalid files are skipped.
pub fn custom_themes<R: Runtime>(app: &AppHandle<R>) -> Vec<(String, Theme)> {
    let Some(dir) = custom_dir(app) else { return vec![] };
    let Ok(entries) = std::fs::read_dir(dir) else { return vec![] };
    let mut out: Vec<(String, Theme)> = entries
        .flatten()
        .filter(|e| e.path().extension().is_some_and(|x| x.eq_ignore_ascii_case("json")))
        .filter_map(|e| {
            let theme: Theme = serde_json::from_str(&std::fs::read_to_string(e.path()).ok()?).ok()?;
            let complete = ["bg", "text"].iter().all(|k| theme.tokens.get(*k).and_then(|c| parse_hex(c)).is_some());
            let stem = e.path().file_stem()?.to_string_lossy().into_owned();
            complete.then(|| (format!("custom:{stem}"), theme))
        })
        .collect();
    out.sort_by(|a, b| a.1.name.to_lowercase().cmp(&b.1.name.to_lowercase()));
    out
}

fn find<R: Runtime>(app: &AppHandle<R>, id: &str) -> Option<Theme> {
    match id.strip_prefix("custom:") {
        Some(_) => custom_themes(app).into_iter().find(|(k, _)| k == id).map(|(_, t)| t),
        None => builtin_theme(id),
    }
}

/// Applies an accent override (src/design/accents.json) for the theme's scheme.
fn with_accent(mut theme: Theme, accent: Option<&str>) -> Theme {
    let Some(accent) = accent else { return theme };
    let Ok(all) = serde_json::from_str::<serde_json::Value>(ACCENTS) else { return theme };
    let v = &all[accent][&theme.scheme];
    if let (Some(a), Some(on)) = (v["accent"].as_str(), v["onAccent"].as_str()) {
        theme.tokens.insert("accent".into(), a.into());
        theme.tokens.insert("focus".into(), a.into());
        theme.tokens.insert("onAccent".into(), on.into());
    }
    theme
}

/// The theme a window should show: (id, theme with accent applied).
/// `os_dark` matters only for the "system" theme.
pub fn resolve<R: Runtime>(app: &AppHandle<R>, s: &Settings, os_dark: bool) -> (String, Theme) {
    let id = match s.theme.as_str() {
        "system" if os_dark => s.system_dark.clone(),
        "system" => s.system_light.clone(),
        other => other.to_owned(),
    };
    let fallback = if os_dark { "graphite" } else { "paper" };
    let (id, theme) = match find(app, &id) {
        Some(t) => (id, t),
        None => (fallback.to_owned(), builtin_theme(fallback).expect("built-in theme")),
    };
    (id, with_accent(theme, s.accent.as_deref()))
}

#[derive(Serialize)]
struct Resolved {
    id: String,
    scheme: String,
    tokens: BTreeMap<String, String>,
}

/// Script injected into each page before it loads: applies the theme to <html> before the
/// first paint and hands the settings to the UI. src/design/theme.ts does the same at runtime.
pub fn init_script<R: Runtime>(app: &AppHandle<R>, s: &Settings) -> String {
    let pack = |dark: bool| {
        let (id, t) = resolve(app, s, dark);
        Resolved { id, scheme: t.scheme, tokens: t.tokens }
    };
    let payload = serde_json::json!({
        "settings": s,
        "light": pack(false),
        "dark": pack(true),
        "platform": std::env::consts::OS,
    });
    format!(
        r#"(function () {{
  var d = {payload};
  window.__IMMEDIATE__ = d;
  function apply() {{
    var el = document.documentElement, s = d.settings;
    var osDark = window.matchMedia && matchMedia("(prefers-color-scheme: dark)").matches;
    var t = s.theme === "system" ? (osDark ? d.dark : d.light) : d.light;
    el.setAttribute("data-theme", t.id);
    el.setAttribute("data-scheme", t.scheme);
    el.setAttribute("data-font", s.font);
    el.setAttribute("data-platform", d.platform);
    if (s.reduceMotion) el.setAttribute("data-reduce-motion", "");
    if (s.tintCanvas) el.setAttribute("data-tint-canvas", "");
    for (var k in t.tokens) el.style.setProperty("--" + k.replace(/[A-Z]/g, function (c) {{ return "-" + c.toLowerCase(); }}), t.tokens[k]);
  }}
  if (document.documentElement) apply();
  else new MutationObserver(function (_, o) {{ if (document.documentElement) {{ o.disconnect(); apply(); }} }}).observe(document, {{ childList: true }});
}})();"#
    )
}

#[derive(Serialize)]
pub struct CustomTheme {
    id: String,
    #[serde(flatten)]
    theme: Theme,
}

/// Custom themes for the Settings gallery.
#[tauri::command]
pub fn themes_custom<R: Runtime>(app: AppHandle<R>) -> Vec<CustomTheme> {
    custom_themes(&app).into_iter().map(|(id, theme)| CustomTheme { id, theme }).collect()
}

/// Opens (creating if needed) the folder for custom themes.
#[tauri::command]
pub fn themes_folder<R: Runtime>(app: AppHandle<R>) -> Result<String, String> {
    let dir = custom_dir(&app).ok_or("no config folder")?;
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let readme = dir.join("README.txt");
    if !readme.exists() {
        let example = BUILTIN[0].1.replacen("\"Paper\"", "\"My theme\"", 1).replacen("\"Monochrome\"", "\"Custom\"", 1);
        let text = format!(
            "Custom themes for Immediate

             Each .json file in this folder is a theme. Copy the example below into a new file such as
             \"my theme.json\", change the name and colors (#rrggbb), and reopen Settings > Appearance.
             \"scheme\" is \"light\" or \"dark\". Keep \"canvas\" a neutral grey so photos look true.

{example}"
        );
        let _ = std::fs::write(&readme, text);
    }
    Ok(dir.to_string_lossy().into_owned())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_builtin_theme_parses_and_is_complete() {
        for (id, json) in BUILTIN {
            let t: Theme = serde_json::from_str(json).unwrap_or_else(|e| panic!("{id}: {e}"));
            assert!(matches!(t.scheme.as_str(), "light" | "dark"), "{id}");
            for k in ["bg", "panel", "raised", "hover", "line", "lineStrong", "text", "textDim", "accent", "onAccent", "focus", "danger", "canvas"] {
                assert!(t.tokens.get(k).and_then(|c| parse_hex(c)).is_some(), "{id}: {k}");
            }
        }
    }

    #[test]
    fn accent_override_uses_the_scheme_variant() {
        let dark = with_accent(builtin_theme("graphite").unwrap(), Some("blue"));
        assert_eq!(dark.tokens["accent"], "#4d9bff");
        let light = with_accent(builtin_theme("paper").unwrap(), Some("blue"));
        assert_eq!(light.tokens["accent"], "#0a66d8");
        assert_eq!(light.tokens["focus"], "#0a66d8");
    }
}
