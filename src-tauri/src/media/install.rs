//! Installs media tools (ffmpeg + ffprobe) into Immediate's data folder: a published build from
//! the upstream publisher, checked against the publisher's SHA-256 before anything is unpacked.
//!   Windows / Linux: BtbN/FFmpeg-Builds (GPL, shared libraries), from one dated "autobuild"
//!                    release so the archive and its checksum always match.
//!   macOS:           Martin Riedl's static builds (each binary has its own .sha256).
//! ffmpeg runs as a separate program and is never linked into Immediate.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicBool, Ordering};

use serde::Serialize;
use sha2::{Digest, Sha256};
use tauri::{AppHandle, Emitter, Manager, Runtime};

static RUNNING: AtomicBool = AtomicBool::new(false);
const SERIES: &str = "9.0";

#[derive(Serialize, Clone)]
#[serde(rename_all = "camelCase")]
pub struct Progress {
    /// "download" | "verify" | "unpack" | "done" | "error"
    phase: &'static str,
    received: u64,
    total: u64,
    message: String,
}

pub fn tools_dir<R: Runtime>(app: &AppHandle<R>) -> Option<PathBuf> {
    app.path().app_local_data_dir().ok().map(|d| d.join("tools").join("ffmpeg"))
}

fn client() -> Result<reqwest::Client, String> {
    let _ = rustls::crypto::ring::default_provider().install_default();
    reqwest::Client::builder()
        .user_agent("Immediate media tools installer")
        .build()
        .map_err(|e| e.to_string())
}

struct Download {
    url: String,
    /// Expected SHA-256 (hex).
    sha256: String,
    name: String,
}

/// Finds the newest dated build with both the archive and its checksum.
async fn btbn(client: &reqwest::Client) -> Result<Download, String> {
    let plat = match (std::env::consts::OS, std::env::consts::ARCH) {
        ("windows", "x86_64") => "win64",
        ("windows", "aarch64") => "winarm64",
        ("linux", "x86_64") => "linux64",
        ("linux", "aarch64") => "linuxarm64",
        _ => return Err("No media tools build is published for this system.".into()),
    };
    let ext = if plat.starts_with("win") { "zip" } else { "tar.xz" };
    let suffix = format!("-{plat}-gpl-shared-{SERIES}.{ext}");
    let releases: serde_json::Value = client
        .get("https://api.github.com/repos/BtbN/FFmpeg-Builds/releases?per_page=6")
        .header("Accept", "application/vnd.github+json")
        .send()
        .await
        .map_err(net)?
        .error_for_status()
        .map_err(net)?
        .json()
        .await
        .map_err(net)?;
    for rel in releases.as_array().into_iter().flatten() {
        if !rel["tag_name"].as_str().unwrap_or_default().starts_with("autobuild-") {
            continue;
        }
        let assets = rel["assets"].as_array().cloned().unwrap_or_default();
        let find = |pred: &dyn Fn(&str) -> bool| {
            assets.iter().find(|a| pred(a["name"].as_str().unwrap_or_default())).and_then(|a| {
                Some((a["name"].as_str()?.to_owned(), a["browser_download_url"].as_str()?.to_owned()))
            })
        };
        let archive = find(&|n: &str| n.starts_with(&format!("ffmpeg-n{SERIES}")) && n.ends_with(&suffix));
        let sums = find(&|n: &str| n == "checksums.sha256");
        if let (Some((name, url)), Some((_, sums_url))) = (archive, sums) {
            let text = client.get(&sums_url).send().await.map_err(net)?.text().await.map_err(net)?;
            let sha256 = text
                .lines()
                .find_map(|l| {
                    let (hash, file) = l.split_once(char::is_whitespace)?;
                    (file.trim().trim_start_matches('*') == name).then(|| hash.to_lowercase())
                })
                .ok_or("The download's checksum is missing.")?;
            return Ok(Download { url, sha256, name });
        }
    }
    Err("Couldn’t find a media tools build to download. Try again later.".into())
}

fn net(e: reqwest::Error) -> String {
    if e.is_connect() || e.is_timeout() {
        "Couldn’t reach the download server. Check your internet connection; photos work without it.".into()
    } else {
        format!("Download failed: {e}")
    }
}

/// Downloads to `dest`, reporting progress, and returns the SHA-256 of what was received.
async fn fetch<R: Runtime>(app: &AppHandle<R>, client: &reqwest::Client, url: &str, dest: &Path, done_before: u64, total_all: u64) -> Result<(String, u64), String> {
    let mut resp = client.get(url).send().await.map_err(net)?.error_for_status().map_err(net)?;
    let total = resp.content_length().unwrap_or(0);
    let mut file = std::fs::File::create(dest).map_err(|e| e.to_string())?;
    let mut hasher = Sha256::new();
    let mut received = 0u64;
    let mut last_emit = 0u64;
    while let Some(chunk) = resp.chunk().await.map_err(net)? {
        file.write_all(&chunk).map_err(|e| e.to_string())?;
        hasher.update(&chunk);
        received += chunk.len() as u64;
        if received - last_emit > 512 * 1024 {
            last_emit = received;
            emit(app, "download", done_before + received, total_all.max(done_before + total), "");
        }
    }
    Ok((hex(&hasher.finalize()), received))
}

fn hex(b: &[u8]) -> String {
    b.iter().map(|x| format!("{x:02x}")).collect()
}

fn emit<R: Runtime>(app: &AppHandle<R>, phase: &'static str, received: u64, total: u64, message: &str) {
    let _ = app.emit("tools", Progress { phase, received, total, message: message.to_owned() });
}

/// Unpacks with the system's `tar` (built into Windows 10+, macOS and Linux; it reads zip too).
fn untar(archive: &Path, into: &Path) -> Result<(), String> {
    std::fs::create_dir_all(into).map_err(|e| e.to_string())?;
    let tar = if cfg!(windows) {
        std::env::var_os("SystemRoot").map(|r| PathBuf::from(r).join("System32").join("tar.exe")).unwrap_or_else(|| "tar".into())
    } else {
        "tar".into()
    };
    let out = crate::tools::quiet_command(&tar).arg("-xf").arg(archive).arg("-C").arg(into).output().map_err(|e| e.to_string())?;
    if !out.status.success() {
        return Err(format!("Couldn’t unpack the media tools: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(())
}

/// Installs (or updates) the media tools. Progress arrives as "tools" events.
#[tauri::command]
pub async fn tools_install<R: Runtime>(app: AppHandle<R>) -> Result<(), String> {
    if RUNNING.swap(true, Ordering::SeqCst) {
        return Ok(()); // already installing; progress events keep coming
    }
    let result = install(&app).await;
    RUNNING.store(false, Ordering::SeqCst);
    match &result {
        Ok(()) => emit(&app, "done", 0, 0, ""),
        Err(e) => emit(&app, "error", 0, 0, e),
    }
    result
}

async fn install<R: Runtime>(app: &AppHandle<R>) -> Result<(), String> {
    let dir = tools_dir(app).ok_or("no data folder")?;
    let parent = dir.parent().ok_or("no data folder")?.to_path_buf();
    std::fs::create_dir_all(&parent).map_err(|e| e.to_string())?;
    let staging = parent.join("ffmpeg.new");
    let _ = std::fs::remove_dir_all(&staging);
    std::fs::create_dir_all(staging.join("bin")).map_err(|e| e.to_string())?;
    let client = client()?;
    emit(app, "download", 0, 0, "");

    if cfg!(target_os = "macos") {
        let arch = if cfg!(target_arch = "aarch64") { "arm64" } else { "amd64" };
        let mut done = 0u64;
        for tool in ["ffmpeg", "ffprobe"] {
            let base = format!("https://ffmpeg.martin-riedl.de/redirect/latest/macos/{arch}/release/{tool}.zip");
            let expected = client.get(format!("{base}.sha256")).send().await.map_err(net)?.text().await.map_err(net)?;
            let expected = expected.split_whitespace().next().unwrap_or_default().to_lowercase();
            let zip = parent.join(format!("{tool}.zip"));
            let (got, n) = fetch(app, &client, &base, &zip, done, 0).await?;
            done += n;
            emit(app, "verify", done, done, "");
            if got != expected {
                let _ = std::fs::remove_file(&zip);
                return Err("The download didn’t match its published checksum, so it wasn’t installed.".into());
            }
            emit(app, "unpack", done, done, "");
            untar(&zip, &staging.join("bin"))?;
            let _ = std::fs::remove_file(&zip);
        }
    } else {
        let d = btbn(&client).await?;
        let archive = parent.join(&d.name);
        let (got, n) = fetch(app, &client, &d.url, &archive, 0, 0).await?;
        emit(app, "verify", n, n, "");
        if got != d.sha256 {
            let _ = std::fs::remove_file(&archive);
            return Err("The download didn’t match its published checksum, so it wasn’t installed.".into());
        }
        emit(app, "unpack", n, n, "");
        let unpacked = parent.join("ffmpeg.unpack");
        let _ = std::fs::remove_dir_all(&unpacked);
        untar(&archive, &unpacked)?;
        let _ = std::fs::remove_file(&archive);
        // The archive holds one folder named like itself, with bin/ (and lib/ for shared builds).
        let inner = std::fs::read_dir(&unpacked)
            .map_err(|e| e.to_string())?
            .flatten()
            .map(|e| e.path())
            .find(|p| p.join("bin").is_dir())
            .ok_or("The media tools archive looks wrong.")?;
        let _ = std::fs::remove_dir_all(&staging);
        std::fs::rename(&inner, &staging).map_err(|e| e.to_string())?;
        let _ = std::fs::remove_dir_all(&unpacked);
    }

    #[cfg(unix)]
    for tool in ["ffmpeg", "ffprobe"] {
        use std::os::unix::fs::PermissionsExt;
        let p = staging.join("bin").join(tool);
        let _ = std::fs::set_permissions(&p, std::fs::Permissions::from_mode(0o755));
    }
    // Swap in the new copy only once it is complete.
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::rename(&staging, &dir).map_err(|e| e.to_string())?;
    super::forget_tools();
    Ok(())
}
