//! Loads the built DLL the way Explorer's surrogate does (DllGetClassObject -> IClassFactory ->
//! IExplorerCommand) and asks it about real files. Usage: cargo run --release --example probe -- <dll> <files...>
#[cfg(windows)]
fn main() -> windows::core::Result<()> {
    use std::ffi::c_void;
    use windows::core::{Interface, GUID, HRESULT, HSTRING, PCSTR};
    use windows::Win32::System::Com::{CoInitializeEx, IClassFactory, COINIT_APARTMENTTHREADED};
    use windows::Win32::System::LibraryLoader::{GetProcAddress, LoadLibraryW};
    use windows::Win32::UI::Shell::{IExplorerCommand, IShellItem, SHCreateItemFromParsingName, IShellItemArray};
    let args: Vec<String> = std::env::args().skip(1).collect();
    unsafe {
        CoInitializeEx(None, COINIT_APARTMENTTHREADED).ok()?;
        let dll = LoadLibraryW(&HSTRING::from(&args[0]))?;
        type GetClassObject = unsafe extern "system" fn(*const GUID, *const GUID, *mut *mut c_void) -> HRESULT;
        let f: GetClassObject = std::mem::transmute(GetProcAddress(dll, PCSTR(b"DllGetClassObject\0".as_ptr())).unwrap());
        let mut p = std::ptr::null_mut();
        f(&immediate_menu::CLSID_MENU, &IClassFactory::IID, &mut p).ok()?;
        let factory = IClassFactory::from_raw(p);
        let cmd: IExplorerCommand = factory.CreateInstance(None)?;
        let items: Vec<IShellItem> = args[1..].iter().map(|a| SHCreateItemFromParsingName(&HSTRING::from(a), None)).collect::<Result<_, _>>()?;
        // Build an IShellItemArray from the items via their PIDLs.
        use windows::Win32::UI::Shell::{SHGetIDListFromObject, Common::ITEMIDLIST};
        let pidls: Vec<*const ITEMIDLIST> = items.iter().map(|i| SHGetIDListFromObject(i).map(|p| p as *const _)).collect::<Result<_, _>>()?;
        let arr: IShellItemArray = windows::Win32::UI::Shell::SHCreateShellItemArrayFromIDLists(&pidls)?;
        let title = cmd.GetTitle(&arr)?;
        let icon = cmd.GetIcon(&arr)?;
        let state = cmd.GetState(&arr, false)?;
        println!("title={} icon={} state={}", title.to_string().unwrap(), icon.to_string().unwrap(), state);
    }
    Ok(())
}
#[cfg(not(windows))]
fn main() {}
