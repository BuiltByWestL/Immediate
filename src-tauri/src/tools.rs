//! Media tools (ffmpeg / ffprobe). P1 only detects a system copy for Settings → Tools; the
//! managed download (SHA-256 verified, see the plan's P0 findings) arrives with video in P3.

use std::path::PathBuf;
use std::process::Command;

use serde::Serialize;

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ToolsStatus {
    ffmpeg: Option<Tool>,
    ffprobe: Option<Tool>,
}

#[derive(Serialize)]
pub struct Tool {
    path: String,
    version: String,
}

/// Finds an executable on PATH, like `which`.
pub fn which(name: &str) -> Option<PathBuf> {
    let exe = if cfg!(windows) { format!("{name}.exe") } else { name.to_owned() };
    std::env::split_paths(&std::env::var_os("PATH")?).map(|d| d.join(&exe)).find(|p| p.is_file())
}

/// A command that never flashes a console window on Windows.
pub fn quiet_command(program: &std::path::Path) -> Command {
    #[allow(unused_mut)]
    let mut cmd = Command::new(program);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}

fn probe(name: &str) -> Option<Tool> {
    let path = which(name)?;
    let out = quiet_command(&path).args(["-hide_banner", "-version"]).output().ok()?;
    let first = String::from_utf8_lossy(&out.stdout).lines().next()?.to_owned();
    // "ffmpeg version 8.1.2-full_build-www.gyan.dev Copyright (c) ..."
    let version = first.split_whitespace().nth(2)?.to_owned();
    Some(Tool { path: path.to_string_lossy().into_owned(), version })
}

#[tauri::command]
pub async fn tools_status() -> ToolsStatus {
    ToolsStatus { ffmpeg: probe("ffmpeg"), ffprobe: probe("ffprobe") }
}
