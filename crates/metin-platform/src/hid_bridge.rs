//! Versioned NativeAOT C ABI. Load only the DLL shipped beside this executable.
use crate::{HidKeyboard, PlatformError, Result, scan_to_hid};
use std::{collections::BTreeMap, ffi::CStr, os::windows::ffi::OsStrExt, sync::OnceLock};
use windows_sys::Win32::{Foundation::HMODULE, System::LibraryLoader::*};

type Manage = unsafe extern "C" fn(u32, *mut u8, u32) -> i32;
type Open = unsafe extern "C" fn(*mut usize, *mut u8, u32) -> i32;
type Keyboard = unsafe extern "C" fn(usize, u8, *const u8, u32, *mut u8, u32) -> i32;
type Mouse = unsafe extern "C" fn(usize, i32, i32, u32, *mut u8, u32) -> i32;
type Close = unsafe extern "C" fn(usize, *mut u8, u32) -> i32;

#[derive(Clone, Copy)]
pub struct Bridge {
    manage: Manage,
    open: Open,
    keyboard: Keyboard,
    mouse: Mouse,
    close: Close,
}
impl Bridge {
    pub fn load() -> Result<Self> {
        static BRIDGE: OnceLock<Result<Bridge>> = OnceLock::new();
        BRIDGE.get_or_init(Self::load_once).clone()
    }
    fn load_once() -> Result<Self> {
        let path = std::env::current_exe()
            .map_err(|e| PlatformError(e.to_string()))?
            .with_file_name("MetinInput.dll");
        let path: Vec<_> = path.as_os_str().encode_wide().chain(Some(0)).collect();
        // SAFETY: absolute NUL-terminated path; dependencies are restricted to
        // the DLL's own directory and System32. NativeAOT cannot be unloaded:
        // keep this module loaded for the process lifetime, including on error.
        unsafe {
            let module = LoadLibraryExW(
                path.as_ptr(),
                std::ptr::null_mut(),
                LOAD_LIBRARY_SEARCH_DLL_LOAD_DIR | LOAD_LIBRARY_SEARCH_SYSTEM32,
            );
            if module.is_null() {
                return Err(PlatformError("MetinInput.dll is missing or could not load. Repair the Companion Windows installation and restart.".into()));
            }
            let abi: unsafe extern "C" fn() -> u32 = symbol(module, c"mi_abi_version")?;
            if abi() != 1 {
                return Err(PlatformError(
                    "MetinInput.dll ABI mismatch. Repair the Companion Windows installation."
                        .into(),
                ));
            }
            Ok(Self {
                manage: symbol(module, c"mi_manage")?,
                open: symbol(module, c"mi_open")?,
                keyboard: symbol(module, c"mi_keyboard")?,
                mouse: symbol(module, c"mi_mouse")?,
                close: symbol(module, c"mi_close")?,
            })
        }
    }
    pub fn manage(&self, operation: u32) -> Result<String> {
        call(|buffer, capacity| unsafe { (self.manage)(operation, buffer, capacity) })
    }
    pub fn session(self) -> Result<Session> {
        let mut handle = 0;
        call(|buffer, capacity| unsafe { (self.open)(&mut handle, buffer, capacity) })?;
        if handle == 0 {
            return Err(PlatformError(
                "Input bridge returned an invalid session.".into(),
            ));
        }
        Ok(Session {
            bridge: self,
            handle,
            keyboard: HidKeyboard::default(),
            mouse_down: false,
            scans: BTreeMap::new(),
        })
    }
}
unsafe fn symbol<T: Copy>(module: HMODULE, name: &CStr) -> Result<T> {
    // SAFETY: caller chooses a function pointer with the pinned ABI signature.
    let address = unsafe { GetProcAddress(module, name.as_ptr().cast()) }.ok_or_else(|| {
        PlatformError(format!(
            "Input bridge export {} is missing.",
            name.to_string_lossy()
        ))
    })?;
    assert_eq!(std::mem::size_of::<T>(), std::mem::size_of_val(&address));
    Ok(unsafe { std::mem::transmute_copy(&address) })
}
fn call(function: impl FnOnce(*mut u8, u32) -> i32) -> Result<String> {
    let mut buffer = vec![0u8; 65536];
    let code = function(buffer.as_mut_ptr(), buffer.len() as u32);
    let end = buffer
        .iter()
        .position(|byte| *byte == 0)
        .unwrap_or(buffer.len());
    let detail = String::from_utf8_lossy(&buffer[..end]).into_owned();
    if code == 0 {
        Ok(detail)
    } else {
        Err(PlatformError(format!("Virtual HID input: {detail}")))
    }
}
pub struct Session {
    bridge: Bridge,
    handle: usize,
    keyboard: HidKeyboard,
    mouse_down: bool,
    scans: BTreeMap<u16, u32>,
}
impl Session {
    pub fn key(&mut self, virtual_key: u16, scan: u32, down: bool) -> Result<()> {
        // A layout change during a hold must release the original physical key.
        let scan = self.scans.get(&virtual_key).copied().unwrap_or(scan);
        self.keyboard.set(scan_to_hid(scan)?, down)?;
        if down {
            self.scans.insert(virtual_key, scan);
        }
        // Keep desired state even on an uncertain delivery. A subsequent
        // release retries the complete report, including all other held keys.
        call(|buffer, capacity| unsafe {
            (self.bridge.keyboard)(
                self.handle,
                self.keyboard.modifiers,
                self.keyboard.keys.as_ptr(),
                self.keyboard.keys.len() as u32,
                buffer,
                capacity,
            )
        })?;
        Ok(())
    }
    pub fn confirm_key_release(&mut self, virtual_key: u16) {
        self.scans.remove(&virtual_key);
    }
    pub fn mouse(&mut self, delta: [i32; 2], down: Option<bool>) -> Result<()> {
        if let Some(down) = down {
            self.mouse_down = down;
        }
        call(|buffer, capacity| unsafe {
            (self.bridge.mouse)(
                self.handle,
                delta[0],
                delta[1],
                u32::from(self.mouse_down),
                buffer,
                capacity,
            )
        })
        .map(|_| ())
    }
}
impl Drop for Session {
    fn drop(&mut self) {
        // Release snapshots separately so keyboard failure cannot prevent a
        // mouse release, then dispose both devices. Driver also watches exit.
        let _ = call(|b, n| unsafe {
            (self.bridge.keyboard)(self.handle, 0, std::ptr::null(), 0, b, n)
        });
        let _ = call(|b, n| unsafe { (self.bridge.mouse)(self.handle, 0, 0, 0, b, n) });
        let _ = call(|b, n| unsafe { (self.bridge.close)(self.handle, b, n) });
    }
}

/// Explicit installer/admin command; never called by automation startup.
pub fn manage_input_driver(operation: u32) -> Result<String> {
    Bridge::load()?.manage(operation)
}
