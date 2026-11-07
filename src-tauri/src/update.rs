//! App self-update from GitHub (signed with the minisign key in tauri.conf.json).
//!
//! While the repo is private, both the manifest (raw.githubusercontent.com, `updates` branch)
//! and the installer (GitHub API asset URL) need a read-only GitHub token. It is read from
//! `IMMEDIATE_GITHUB_TOKEN`, or from a `github-token` file in the app config folder.
//! Once the repo is public: point `endpoints` at releases/latest/download/latest.json and
//! the token becomes unnecessary (it is simply not sent when absent).

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Manager, Runtime, State};
use tauri_plugin_updater::{Update, UpdaterExt};

#[derive(Default)]
pub struct PendingUpdate(Mutex<Option<Update>>);

#[derive(Serialize, Clone)]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    pub notes: Option<String>,
}

fn github_token<R: Runtime>(app: &AppHandle<R>) -> Option<String> {
    if let Ok(t) = std::env::var("IMMEDIATE_GITHUB_TOKEN") {
        let t = t.trim().to_owned();
        if !t.is_empty() {
            return Some(t);
        }
    }
    let path = app.path().app_config_dir().ok()?.join("github-token");
    let t = std::fs::read_to_string(path).ok()?.trim().to_owned();
    (!t.is_empty()).then_some(t)
}

async fn find_update<R: Runtime>(app: &AppHandle<R>) -> Result<Option<Update>, String> {
    let mut builder = app.updater_builder();
    if let Some(token) = github_token(app) {
        builder = builder
            .header("Authorization", format!("Bearer {token}"))
            .map_err(|e| e.to_string())?;
    }
    let updater = builder.build().map_err(|e| e.to_string())?;
    updater.check().await.map_err(|e| e.to_string())
}

/// Checks GitHub for a newer signed release. Returns None when up to date.
#[tauri::command]
pub async fn update_check<R: Runtime>(
    app: AppHandle<R>,
    pending: State<'_, PendingUpdate>,
) -> Result<Option<UpdateInfo>, String> {
    let update = find_update(&app).await?;
    let info = update.as_ref().map(|u| UpdateInfo {
        version: u.version.clone(),
        current_version: u.current_version.clone(),
        notes: u.body.clone(),
    });
    *pending.0.lock().map_err(|e| e.to_string())? = update;
    Ok(info)
}

/// Downloads, verifies the signature, installs, and restarts into the new version.
#[tauri::command]
pub async fn update_install<R: Runtime>(
    app: AppHandle<R>,
    pending: State<'_, PendingUpdate>,
) -> Result<(), String> {
    let update = pending.0.lock().map_err(|e| e.to_string())?.take();
    let update = match update {
        Some(u) => u,
        None => find_update(&app).await?.ok_or("Immediate is already up to date")?,
    };
    update
        .download_and_install(|_, _| {}, || {})
        .await
        .map_err(|e| e.to_string())?;
    app.restart();
}

/// Test hook used by the updater spike / CI: `IMMEDIATE_UPDATE_TEST=check|install` with
/// `IMMEDIATE_UPDATE_TEST_OUT=<file>` runs one check (and optionally the install) at
/// startup and writes the outcome to the file.
pub fn run_test_hook<R: Runtime>(app: &AppHandle<R>) {
    let Ok(mode) = std::env::var("IMMEDIATE_UPDATE_TEST") else { return };
    let out = std::env::var_os("IMMEDIATE_UPDATE_TEST_OUT");
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        let write = |s: String| {
            if let Some(p) = &out {
                let _ = std::fs::write(p, s);
            }
        };
        match find_update(&app).await {
            Ok(None) => write(format!("up-to-date {}", app.package_info().version)),
            Ok(Some(u)) if mode == "install" => {
                write(format!("installing {} -> {}", u.current_version, u.version));
                // On Windows this hands over to the installer, which relaunches the app;
                // the relaunched copy inherits this hook and reports "up-to-date <new version>".
                if let Err(e) = u.download_and_install(|_, _| {}, || {}).await {
                    write(format!("install-error {e}"));
                }
            }
            Ok(Some(u)) => write(format!("available {} -> {}", u.current_version, u.version)),
            Err(e) => write(format!("check-error {e}")),
        }
        app.exit(0);
    });
}
