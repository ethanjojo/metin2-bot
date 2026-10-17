//! One bounded worker; OS notification services never block the input/controller thread.
use std::sync::{
    Arc, Mutex,
    mpsc::{self, SyncSender},
};
#[derive(Debug, Clone)]
pub struct Notice {
    pub title: String,
    pub body: String,
    pub sound: bool,
    pub system: bool,
}
pub struct NotificationService {
    sender: SyncSender<Notice>,
    status: Arc<Mutex<String>>,
}
impl Default for NotificationService {
    fn default() -> Self {
        Self::with_delivery(deliver)
    }
}
impl NotificationService {
    fn with_delivery(deliver: impl Fn(Notice) -> Result<(), String> + Send + 'static) -> Self {
        let (sender, receiver) = mpsc::sync_channel::<Notice>(4);
        let status = Arc::new(Mutex::new("No notifications sent.".into()));
        let output = status.clone();
        std::thread::Builder::new()
            .name("metin-notifications".into())
            .spawn(move || {
                while let Ok(notice) = receiver.recv() {
                    let result = deliver(notice);
                    let message = match result {
                        Ok(()) => "Notification submitted to the operating system.".into(),
                        Err(error) => format!("Notification failed: {error}"),
                    };
                    tracing::info!(%message,"Local notification result");
                    if let Ok(mut status) = output.lock() {
                        *status = message;
                    }
                }
            })
            .expect("notification worker");
        Self { sender, status }
    }
    pub fn send(&self, notice: Notice) -> Result<(), String> {
        if !notice.sound && !notice.system {
            return Err("Choose sound or a system notification first.".into());
        }
        self.sender.try_send(notice).map_err(|_| {
            let message = "Notification queue is busy or unavailable.".to_owned();
            if let Ok(mut status) = self.status.lock() {
                *status = message.clone();
            }
            message
        })
    }
    pub fn status(&self) -> String {
        self.status
            .lock()
            .map(|s| s.clone())
            .unwrap_or_else(|_| "Notification status unavailable.".into())
    }
}
fn deliver(notice: Notice) -> Result<(), String> {
    let mut errors = vec![];
    if notice.system {
        #[cfg(windows)]
        register_windows_identity()?;
        let mut n = notify_rust::Notification::new();
        n.summary(&notice.title).body(&notice.body).timeout(6000);
        #[cfg(windows)]
        n.app_id(crate::APP_ID);
        #[cfg(target_os = "linux")]
        {
            n.appname("MetinBot")
                .hint(notify_rust::Hint::SuppressSound(!notice.sound));
            if notice.sound {
                n.sound_name("dialog-warning");
            }
        }
        #[cfg(target_os = "macos")]
        if notice.sound {
            n.sound_name("Glass");
        }
        if let Err(error) = n.show() {
            errors.push(error.to_string());
        }
    }
    #[cfg(windows)]
    if notice.sound
        && unsafe { windows_sys::Win32::System::Diagnostics::Debug::MessageBeep(0x30) } == 0
    {
        errors.push("Windows could not play the notification sound.".into());
    }
    #[cfg(any(target_os = "linux", target_os = "macos"))]
    if notice.sound && !notice.system {
        #[cfg(target_os = "linux")]
        let result = std::process::Command::new("canberra-gtk-play")
            .args(["--id", "dialog-warning"])
            .output();
        #[cfg(target_os = "macos")]
        let result = std::process::Command::new("/usr/bin/afplay")
            .arg("/System/Library/Sounds/Glass.aiff")
            .output();
        match result {
            Ok(output) if output.status.success() => {}
            Ok(_) => errors.push("The operating system could not play the sound.".into()),
            Err(error) => errors.push(format!("Sound service unavailable: {error}")),
        }
    }
    if errors.is_empty() {
        Ok(())
    } else {
        Err(errors.join("; "))
    }
}
#[cfg(windows)]
fn register_windows_identity() -> Result<(), String> {
    use windows_sys::Win32::System::Registry::*;
    let wide = |s: &str| s.encode_utf16().chain(Some(0)).collect::<Vec<_>>();
    let path = wide(&format!(
        "Software\\Classes\\AppUserModelId\\{}",
        crate::APP_ID
    ));
    let mut key = std::ptr::null_mut();
    let code = unsafe { RegCreateKeyW(HKEY_CURRENT_USER, path.as_ptr(), &mut key) };
    if code != 0 {
        return Err(format!(
            "Cannot register notification identity (Windows error {code})."
        ));
    }
    let result = (|| {
        let executable = std::env::current_exe().map_err(|e| e.to_string())?;
        for (name, value) in [
            ("DisplayName", "MetinBot".to_owned()),
            ("IconUri", executable.to_string_lossy().into_owned()),
        ] {
            let name = wide(name);
            let value = wide(&value);
            let code = unsafe {
                RegSetValueExW(
                    key,
                    name.as_ptr(),
                    0,
                    REG_SZ,
                    value.as_ptr().cast(),
                    (value.len() * 2) as u32,
                )
            };
            if code != 0 {
                return Err(format!(
                    "Cannot set notification identity (Windows error {code})."
                ));
            }
        }
        Ok(())
    })();
    unsafe { RegCloseKey(key) };
    result
}
#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn delivery_failure_is_reported_without_panicking_or_blocking_sender() {
        let service = NotificationService::with_delivery(|_| Err("denied".into()));
        service
            .send(Notice {
                title: "Test".into(),
                body: "Test".into(),
                sound: false,
                system: true,
            })
            .unwrap();
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(2);
        while !service.status().contains("denied") && std::time::Instant::now() < deadline {
            std::thread::yield_now();
        }
        assert!(service.status().contains("denied"));
    }
}
