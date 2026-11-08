//! App self-update from GitHub (signed with the minisign key in tauri.conf.json).
//!
//! The manifest is latest.json on the public repo's `updates` branch (raw.githubusercontent.com),
//! pointing at the release's public download URLs. No token: the repository is public.

use std::sync::Mutex;

use serde::Serialize;
use tauri::{AppHandle, Runtime, State};
use tauri_plugin_updater::{Update, UpdaterExt};

#[derive(Default)]
pub struct PendingUpdate(Mutex<Option<Update>>);

#[derive(Serialize, Clone)]
pub struct UpdateInfo {
    pub version: String,
    pub current_version: String,
    pub notes: Option<String>,
}

async fn find_update<R: Runtime>(app: &AppHandle<R>) -> Result<Option<Update>, String> {
    let updater = app.updater_builder().build().map_err(|e| e.to_string())?;
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
