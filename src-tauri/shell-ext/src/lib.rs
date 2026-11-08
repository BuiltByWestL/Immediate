//! "Edit with Immediate" at the top of the Windows 11 right-click menu.
//!
//! Explorer loads this DLL (through the sparse package's COM surrogate) and asks one
//! `IExplorerCommand` whether to show itself for the selected files. It shows when every selected
//! item is a file type Immediate registers a right-click entry for (the same list as the classic
//! entry, generated from src/formats.json), and launches immediate.exe from this DLL's folder
//! with all the selected paths.
#![cfg(windows)]
#![allow(non_snake_case)]

mod types;

use std::ffi::c_void;
use std::os::windows::ffi::OsStringExt;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicIsize, Ordering};
use std::sync::OnceLock;

use windows::core::{implement, Error, Interface, Ref, Result, BOOL, GUID, HRESULT, PWSTR};
use windows::Win32::Foundation::{CLASS_E_CLASSNOTAVAILABLE, CLASS_E_NOAGGREGATION, E_FAIL, E_NOTIMPL, E_POINTER, HMODULE, S_FALSE, S_OK};
use windows::Win32::System::Com::{CoTaskMemFree, IBindCtx, IClassFactory, IClassFactory_Impl};
use windows::Win32::System::LibraryLoader::{GetModuleFileNameW, GetModuleHandleExW, GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS, GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT};
use windows::Win32::UI::Shell::{
    IEnumExplorerCommand, IExplorerCommand, IExplorerCommand_Impl, IShellItemArray, SHStrDupW, ECF_DEFAULT, ECS_ENABLED, ECS_HIDDEN, SIGDN_FILESYSPATH,
};
use windows::Win32::UI::WindowsAndMessaging::{AllowSetForegroundWindow, ASFW_ANY};

/// Must match the Clsid in package/AppxManifest.xml.
pub const CLSID_MENU: GUID = GUID::from_u128(0xbc9ff4c3_61ff_4062_bd4c_58bff5520f67);

/// Live objects plus LockServer locks; COM may unload the DLL when it reaches zero.
static REFS: AtomicIsize = AtomicIsize::new(0);

/// The folder this DLL was loaded from (immediate.exe sits beside it).
fn install_dir() -> &'static Path {
    static DIR: OnceLock<PathBuf> = OnceLock::new();
    DIR.get_or_init(|| unsafe {
        let mut module = HMODULE::default();
        let anchor = install_dir as *const () as *const u16;
        let flags = GET_MODULE_HANDLE_EX_FLAG_FROM_ADDRESS | GET_MODULE_HANDLE_EX_FLAG_UNCHANGED_REFCOUNT;
        if GetModuleHandleExW(flags, windows::core::PCWSTR(anchor), &mut module).is_err() {
            return PathBuf::new();
        }
        let mut buf = vec![0u16; 1024];
        let n = GetModuleFileNameW(Some(module), &mut buf) as usize;
        let path = PathBuf::from(std::ffi::OsString::from_wide(&buf[..n]));
        path.parent().map(Path::to_path_buf).unwrap_or_default()
    })
}

fn exe() -> PathBuf {
    install_dir().join("immediate.exe")
}

/// The selected items' file-system paths; None if any item isn't a plain file on disk.
fn paths(items: Option<&IShellItemArray>) -> Option<Vec<PathBuf>> {
    let items = items?;
    unsafe {
        let n = items.GetCount().ok()?;
        let mut out = Vec::with_capacity(n as usize);
        for i in 0..n {
            let item = items.GetItemAt(i).ok()?;
            let name = item.GetDisplayName(SIGDN_FILESYSPATH).ok()?;
            let wide = name.as_wide().to_vec();
            CoTaskMemFree(Some(name.0 as *const c_void));
            out.push(PathBuf::from(std::ffi::OsString::from_wide(&wide)));
        }
        Some(out)
    }
}

#[derive(Clone, Copy, PartialEq, Debug)]
pub enum Verb {
    Hidden,
    /// Every item can be edited.
    Edit,
    /// Every item opens, but at least one is view-only.
    Open,
}

/// Which entry (if any) to show for these paths. Folders and unknown types hide it.
pub fn verb_for(paths: &[PathBuf]) -> Verb {
    if paths.is_empty() {
        return Verb::Hidden;
    }
    let mut all_edit = true;
    for p in paths {
        if p.is_dir() {
            return Verb::Hidden;
        }
        let ext = p.extension().and_then(|e| e.to_str()).unwrap_or("").to_ascii_lowercase();
        if types::EDIT.contains(&ext.as_str()) {
            continue;
        }
        if types::VIEW.contains(&ext.as_str()) {
            all_edit = false;
            continue;
        }
        return Verb::Hidden;
    }
    if all_edit {
        Verb::Edit
    } else {
        Verb::Open
    }
}

fn dup(s: &str) -> Result<PWSTR> {
    let wide: Vec<u16> = s.encode_utf16().chain(Some(0)).collect();
    unsafe { SHStrDupW(windows::core::PCWSTR(wide.as_ptr())) }
}

#[implement(IExplorerCommand)]
pub struct MenuCommand;

impl MenuCommand {
    pub fn new() -> Self {
        REFS.fetch_add(1, Ordering::SeqCst);
        MenuCommand
    }
}

impl Drop for MenuCommand {
    fn drop(&mut self) {
        REFS.fetch_sub(1, Ordering::SeqCst);
    }
}

impl IExplorerCommand_Impl for MenuCommand_Impl {
    fn GetTitle(&self, items: Ref<'_, IShellItemArray>) -> Result<PWSTR> {
        let verb = paths(items.as_ref()).map(|p| verb_for(&p)).unwrap_or(Verb::Edit);
        dup(if verb == Verb::Open { "Open in Immediate" } else { "Edit with Immediate" })
    }

    fn GetIcon(&self, _items: Ref<'_, IShellItemArray>) -> Result<PWSTR> {
        dup(&format!("{},0", exe().display()))
    }

    fn GetToolTip(&self, _items: Ref<'_, IShellItemArray>) -> Result<PWSTR> {
        Err(E_NOTIMPL.into())
    }

    fn GetCanonicalName(&self) -> Result<GUID> {
        Ok(CLSID_MENU)
    }

    fn GetState(&self, items: Ref<'_, IShellItemArray>, _ok_to_be_slow: BOOL) -> Result<u32> {
        let show = paths(items.as_ref()).is_some_and(|p| verb_for(&p) != Verb::Hidden);
        Ok(if show { ECS_ENABLED.0 as u32 } else { ECS_HIDDEN.0 as u32 })
    }

    fn Invoke(&self, items: Ref<'_, IShellItemArray>, _bind: Ref<'_, IBindCtx>) -> Result<()> {
        let files = paths(items.as_ref()).ok_or_else(|| Error::from(E_FAIL))?;
        unsafe {
            // Let Immediate (or the instance it forwards to) bring its window to the front.
            let _ = AllowSetForegroundWindow(ASFW_ANY);
        }
        std::process::Command::new(exe()).args(&files).current_dir(install_dir()).spawn().map_err(|_| Error::from(E_FAIL))?;
        Ok(())
    }

    fn GetFlags(&self) -> Result<u32> {
        Ok(ECF_DEFAULT.0 as u32)
    }

    fn EnumSubCommands(&self) -> Result<IEnumExplorerCommand> {
        Err(E_NOTIMPL.into())
    }
}

#[implement(IClassFactory)]
struct Factory;

impl IClassFactory_Impl for Factory_Impl {
    fn CreateInstance(&self, outer: Ref<'_, windows::core::IUnknown>, iid: *const GUID, object: *mut *mut c_void) -> Result<()> {
        if object.is_null() {
            return Err(E_POINTER.into());
        }
        unsafe { *object = std::ptr::null_mut() };
        if outer.is_some() {
            return Err(CLASS_E_NOAGGREGATION.into());
        }
        let command: IExplorerCommand = MenuCommand::new().into();
        unsafe { command.query(iid, object).ok() }
    }

    fn LockServer(&self, lock: BOOL) -> Result<()> {
        REFS.fetch_add(if lock.as_bool() { 1 } else { -1 }, Ordering::SeqCst);
        Ok(())
    }
}

#[unsafe(no_mangle)]
extern "system" fn DllGetClassObject(clsid: *const GUID, iid: *const GUID, out: *mut *mut c_void) -> HRESULT {
    if out.is_null() || clsid.is_null() || iid.is_null() {
        return E_POINTER;
    }
    unsafe {
        *out = std::ptr::null_mut();
        if *clsid != CLSID_MENU {
            return CLASS_E_CLASSNOTAVAILABLE;
        }
        let factory: IClassFactory = Factory.into();
        factory.query(iid, out)
    }
}

#[unsafe(no_mangle)]
extern "system" fn DllCanUnloadNow() -> HRESULT {
    if REFS.load(Ordering::SeqCst) <= 0 {
        S_OK
    } else {
        S_FALSE
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn verbs() {
        let p = |s: &[&str]| s.iter().map(PathBuf::from).collect::<Vec<_>>();
        assert_eq!(verb_for(&p(&["C:\\a\\b.JPG", "c.mp4", "d.wav"])), Verb::Edit);
        assert_eq!(verb_for(&p(&["a.jpg", "b.cr2"])), Verb::Open);
        assert_eq!(verb_for(&p(&["a.jpg", "notes.docx"])), Verb::Hidden); // documents: no right-click entry
        assert_eq!(verb_for(&p(&["a.ts"])), Verb::Hidden); // shared with TypeScript
        assert_eq!(verb_for(&p(&["README"])), Verb::Hidden);
        assert_eq!(verb_for(&[]), Verb::Hidden);
    }
}
