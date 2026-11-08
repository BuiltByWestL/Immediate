//! The Windows 11 top-level right-click entry ("Edit with Immediate").
//!
//! Windows 11's compact menu only lists handlers that belong to a package, so the installer ships
//! a small COM DLL (src-tauri/shell-ext) and a signed sparse package (immediate-menu.msix) beside
//! immediate.exe. Turning the entry on registers that package for the current user. The package
//! is signed with Immediate's own self-signed certificate, which Windows must trust first; that's
//! one administrator prompt (certutil adding immediate-menu.cer to Trusted People), only the
//! first time.

use serde::Serialize;

#[derive(Serialize)]
pub struct MenuStatus {
    /// Windows 11 and the package is installed beside the app.
    available: bool,
    /// Registered for this user.
    enabled: bool,
}

#[cfg(windows)]
mod imp {
    use std::path::{Path, PathBuf};

    use windows::core::{w, HSTRING, PCWSTR};
    use windows::Win32::Foundation::{CloseHandle, ERROR_CANCELLED};
    use windows::Win32::System::Registry::{RegGetValueW, HKEY_LOCAL_MACHINE, RRF_RT_REG_SZ};
    use windows::Win32::System::Threading::{GetExitCodeProcess, WaitForSingleObject, INFINITE};
    use windows::Win32::UI::Shell::{ShellExecuteExW, SEE_MASK_NOASYNC, SEE_MASK_NOCLOSEPROCESS, SHELLEXECUTEINFOW};
    use windows::Win32::UI::WindowsAndMessaging::SW_HIDE;

    use crate::tools::quiet_command;

    pub const PACKAGE: &str = "BuiltByWestL.Immediate.Menu";

    pub fn dir() -> Option<PathBuf> {
        std::env::current_exe().ok()?.parent().map(Path::to_path_buf)
    }

    pub fn windows_build() -> u32 {
        let mut buf = [0u16; 32];
        let mut len = (buf.len() * 2) as u32;
        let ok = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                w!("SOFTWARE\\Microsoft\\Windows NT\\CurrentVersion"),
                w!("CurrentBuildNumber"),
                RRF_RT_REG_SZ,
                None,
                Some(buf.as_mut_ptr().cast()),
                Some(&mut len),
            )
        };
        if ok.is_err() {
            return 0;
        }
        let s = String::from_utf16_lossy(&buf[..(len as usize / 2).saturating_sub(1)]);
        s.trim().parse().unwrap_or(0)
    }

    /// PowerShell in single quotes.
    fn q(p: &Path) -> String {
        format!("'{}'", p.display().to_string().replace('\'', "''"))
    }

    /// Runs a PowerShell script; Ok(exit code) or the first error line.
    pub fn powershell(script: &str) -> Result<i32, String> {
        let exe = std::env::var_os("SystemRoot")
            .map(|r| PathBuf::from(r).join("System32\\WindowsPowerShell\\v1.0\\powershell.exe"))
            .unwrap_or_else(|| PathBuf::from("powershell.exe"));
        let out = quiet_command(&exe)
            .args(["-NoProfile", "-NonInteractive", "-ExecutionPolicy", "Bypass", "-Command", script])
            .output()
            .map_err(|e| format!("Couldn’t run PowerShell: {e}"))?;
        let code = out.status.code().unwrap_or(-1);
        if code == 0 || code == 3 {
            return Ok(code);
        }
        let err = String::from_utf8_lossy(&out.stderr);
        let line = err.lines().map(str::trim).find(|l| !l.is_empty()).unwrap_or("unknown error");
        Err(format!("Windows couldn’t add the right-click entry: {line}"))
    }

    pub fn registered() -> bool {
        matches!(powershell(&format!("if (Get-AppxPackage -Name {PACKAGE}) {{ exit 0 }} else {{ exit 3 }}")), Ok(0))
    }

    /// Exit 3 = the certificate isn't trusted yet.
    pub fn add(dir: &Path) -> Result<i32, String> {
        let script = format!(
            "$ErrorActionPreference = 'Stop'; \
             $c = New-Object Security.Cryptography.X509Certificates.X509Certificate2 {cer}; \
             if (-not (Test-Path ('Cert:\\LocalMachine\\TrustedPeople\\' + $c.Thumbprint))) {{ exit 3 }}; \
             Add-AppxPackage -Path {msix} -ExternalLocation {dir}",
            cer = q(&dir.join("immediate-menu.cer")),
            msix = q(&dir.join("immediate-menu.msix")),
            dir = q(dir),
        );
        powershell(&script)
    }

    pub fn remove() -> Result<i32, String> {
        powershell(&format!("$ErrorActionPreference = 'Stop'; Get-AppxPackage -Name {PACKAGE} | Remove-AppxPackage"))
    }

    /// Adds Immediate's certificate to the machine's Trusted People store (one UAC prompt).
    pub fn trust(cer: &Path) -> Result<(), String> {
        let params = HSTRING::from(format!("-addstore TrustedPeople \"{}\"", cer.display()));
        let mut info = SHELLEXECUTEINFOW {
            cbSize: std::mem::size_of::<SHELLEXECUTEINFOW>() as u32,
            fMask: SEE_MASK_NOCLOSEPROCESS | SEE_MASK_NOASYNC,
            lpVerb: w!("runas"),
            lpFile: w!("certutil.exe"),
            lpParameters: PCWSTR(params.as_ptr()),
            nShow: SW_HIDE.0,
            ..Default::default()
        };
        unsafe {
            if let Err(e) = ShellExecuteExW(&mut info) {
                return Err(if e.code() == ERROR_CANCELLED.to_hresult() {
                    "The right-click entry needs your permission once. Turn it on again and choose Yes.".into()
                } else {
                    format!("Couldn’t ask Windows for permission: {}", e.message())
                });
            }
            let mut code = 1u32;
            if !info.hProcess.is_invalid() {
                WaitForSingleObject(info.hProcess, INFINITE);
                let _ = GetExitCodeProcess(info.hProcess, &mut code);
                let _ = CloseHandle(info.hProcess);
            }
            if code != 0 {
                return Err(format!("Windows couldn’t trust Immediate’s certificate (certutil exit {code})."));
            }
        }
        Ok(())
    }
}

#[tauri::command]
pub async fn shell_menu_status() -> MenuStatus {
    #[cfg(windows)]
    {
        tauri::async_runtime::spawn_blocking(|| {
            let available = imp::windows_build() >= 22000 && imp::dir().is_some_and(|d| d.join("immediate-menu.msix").is_file() && d.join("immediate_menu.dll").is_file());
            MenuStatus { available, enabled: available && imp::registered() }
        })
        .await
        .unwrap_or(MenuStatus { available: false, enabled: false })
    }
    #[cfg(not(windows))]
    MenuStatus { available: false, enabled: false }
}

#[tauri::command]
pub async fn shell_menu_set(enabled: bool) -> Result<MenuStatus, String> {
    #[cfg(windows)]
    {
        tauri::async_runtime::spawn_blocking(move || {
            let dir = imp::dir().ok_or("Couldn’t find Immediate’s folder.")?;
            if enabled {
                if imp::add(&dir)? == 3 {
                    imp::trust(&dir.join("immediate-menu.cer"))?;
                    if imp::add(&dir)? == 3 {
                        return Err("Windows still doesn’t trust Immediate’s certificate.".into());
                    }
                }
            } else {
                imp::remove()?;
            }
            Ok(MenuStatus { available: true, enabled: imp::registered() })
        })
        .await
        .map_err(|e| e.to_string())?
    }
    #[cfg(not(windows))]
    {
        let _ = enabled;
        Err("Only on Windows 11.".into())
    }
}
