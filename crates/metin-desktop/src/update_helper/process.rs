use std::{
    path::{Path, PathBuf},
    process::{Child, Command},
    time::{Duration, Instant},
};

#[cfg(windows)]
mod native {
    use windows_sys::Win32::{
        Foundation::{CloseHandle, FILETIME, HANDLE, WAIT_OBJECT_0},
        System::Threading::{
            GetProcessTimes, OpenProcess, PROCESS_QUERY_LIMITED_INFORMATION, PROCESS_SYNCHRONIZE,
            WaitForSingleObject,
        },
    };
    pub(super) struct Parent(HANDLE);
    unsafe impl Send for Parent {}
    impl Drop for Parent {
        fn drop(&mut self) {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
    impl Parent {
        pub(super) fn open(pid: u32) -> Result<Self, String> {
            let handle = unsafe {
                OpenProcess(
                    PROCESS_QUERY_LIMITED_INFORMATION | PROCESS_SYNCHRONIZE,
                    0,
                    pid,
                )
            };
            if handle.is_null() {
                Err("The original Companion process is unavailable.".into())
            } else {
                Ok(Self(handle))
            }
        }
        pub(super) fn created(&self) -> Result<u64, String> {
            let mut created = FILETIME {
                dwLowDateTime: 0,
                dwHighDateTime: 0,
            };
            let mut exit = created;
            let mut kernel = created;
            let mut user = created;
            if unsafe { GetProcessTimes(self.0, &mut created, &mut exit, &mut kernel, &mut user) }
                == 0
            {
                return Err("The original Companion process identity cannot be checked.".into());
            }
            Ok(((created.dwHighDateTime as u64) << 32) | created.dwLowDateTime as u64)
        }
        pub(super) fn wait(&self) -> Result<(), String> {
            if unsafe { WaitForSingleObject(self.0, 60000) } != WAIT_OBJECT_0 {
                Err(
                    "Companion did not close. Close the original window, then retry the update."
                        .into(),
                )
            } else {
                Ok(())
            }
        }
    }
    pub(super) fn registered() -> Result<std::path::PathBuf, String> {
        use windows_sys::Win32::System::Registry::*;
        let key:Vec<u16>="Software\\Microsoft\\Windows\\CurrentVersion\\Uninstall\\{78F36AC5-1367-4498-B486-50E3326B9426}_is1\0".encode_utf16().collect();
        let name: Vec<u16> = "InstallLocation\0".encode_utf16().collect();
        let mut buffer = vec![0u16; 32768];
        let mut size = (buffer.len() * 2) as u32;
        let result = unsafe {
            RegGetValueW(
                HKEY_LOCAL_MACHINE,
                key.as_ptr(),
                name.as_ptr(),
                RRF_RT_REG_SZ | RRF_SUBKEY_WOW6464KEY,
                std::ptr::null_mut(),
                buffer.as_mut_ptr().cast(),
                &mut size,
            )
        };
        if result != 0 || size < 2 || size as usize > buffer.len() * 2 {
            return Err("The registered Companion installation could not be found.".into());
        }
        let count = buffer
            .iter()
            .position(|v| *v == 0)
            .ok_or("Invalid registered installation directory.")?;
        Ok(std::path::PathBuf::from(
            String::from_utf16(&buffer[..count])
                .map_err(|_| "Invalid registered installation directory.")?,
        ))
    }
}
#[cfg(windows)]
pub(super) fn creation_time(pid: u32) -> Result<u64, String> {
    native::Parent::open(pid)?.created()
}
#[cfg(not(windows))]
pub(super) fn creation_time(_: u32) -> Result<u64, String> {
    Err("Updates require Windows.".into())
}

pub(super) struct Parent {
    #[cfg(windows)]
    native: native::Parent,
}
impl Parent {
    pub(super) fn open(pid: u32, expected: u64) -> Result<Self, String> {
        if pid == std::process::id() {
            return Err("An updater cannot wait for itself.".into());
        }
        #[cfg(windows)]
        {
            let native = native::Parent::open(pid)?;
            if native.created() != Ok(expected) {
                return Err("The original Companion process identity changed.".into());
            }
            Ok(Self { native })
        }
        #[cfg(not(windows))]
        {
            let _ = (pid, expected);
            Err("Updates require Windows.".into())
        }
    }
    pub(super) fn wait(&self) -> Result<(), String> {
        #[cfg(windows)]
        {
            self.native.wait()
        }
        #[cfg(not(windows))]
        {
            Err("Updates require Windows.".into())
        }
    }
}

pub(super) fn registered_installation() -> Result<PathBuf, String> {
    #[cfg(windows)]
    {
        native::registered()
    }
    #[cfg(not(windows))]
    {
        Err("Updates require Windows.".into())
    }
}
pub(super) fn spawn(path: &Path, args: &[std::ffi::OsString]) -> Result<Child, String> {
    let mut command = Command::new(path);
    command.args(args);
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000);
    }
    command
        .spawn()
        .map_err(|e| format!("Could not start {}: {e}", path.display()))
}
pub(super) fn wait(
    child: &mut Child,
    timeout: Duration,
) -> Result<std::process::ExitStatus, String> {
    let deadline = Instant::now() + timeout;
    loop {
        if let Some(exit) = child.try_wait().map_err(|e| e.to_string())? {
            return Ok(exit);
        }
        if Instant::now() >= deadline {
            return Err("The update operation is still running. No process was terminated. Inspect the setup log before retrying.".into());
        }
        std::thread::sleep(Duration::from_millis(100));
    }
}
