//! Win32 FFI boundary. Native handles stay on the runtime worker thread.
use crate::*;
use std::{
    ffi::c_void,
    mem::{size_of, zeroed},
    ptr::null_mut,
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::ClientToScreen,
    Security::*,
    System::Threading::*,
    UI::{HiDpi::*, Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};

fn os_error(message: &str) -> PlatformError {
    PlatformError(format!("{message}: {}", std::io::Error::last_os_error()))
}
fn hwnd(token: TargetToken) -> HWND {
    token.handle as HWND
}
struct OwnedHandle(HANDLE);
impl Drop for OwnedHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                CloseHandle(self.0);
            }
        }
    }
}
fn process_identity(pid: u32) -> Result<u64> {
    // SAFETY: OpenProcess returns an owned handle; all FILETIME pointers are valid.
    unsafe {
        let process = OwnedHandle(OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid));
        if process.0.is_null() {
            return Err(os_error("Cannot inspect target process"));
        }
        let (mut created, mut exit, mut kernel, mut user) =
            (zeroed(), zeroed(), zeroed(), zeroed());
        if GetProcessTimes(process.0, &mut created, &mut exit, &mut kernel, &mut user) == 0 {
            return Err(os_error("Cannot verify process identity"));
        }
        Ok((u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime))
    }
}
fn elevated(pid: u32) -> Result<bool> {
    unsafe {
        let process = OwnedHandle(OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid));
        if process.0.is_null() {
            return Err(os_error("Cannot check privileges"));
        }
        let mut token = null_mut();
        if OpenProcessToken(process.0, TOKEN_QUERY, &mut token) == 0 {
            return Err(os_error("Cannot inspect process token"));
        }
        let token = OwnedHandle(token);
        let mut value: TOKEN_ELEVATION = zeroed();
        let mut length = 0;
        if GetTokenInformation(
            token.0,
            TokenElevation,
            &mut value as *mut _ as *mut c_void,
            size_of::<TOKEN_ELEVATION>() as u32,
            &mut length,
        ) == 0
        {
            return Err(os_error("Cannot read elevation"));
        }
        Ok(value.TokenIsElevated != 0)
    }
}

pub struct WindowsBackend {
    target: Option<TargetToken>,
    frame_id: u64,
    registered: bool,
    registration_error: Option<String>,
    pending: HotkeyEvents,
    capture: Option<window_capture::WindowCapture>,
}
impl WindowsBackend {
    pub fn new() -> Self {
        Self {
            target: None,
            capture: None,
            frame_id: 0,
            registered: false,
            registration_error: None,
            pending: HotkeyEvents {
                toggle: false,
                stop: false,
            },
        }
    }
    fn target(&self) -> Result<TargetToken> {
        self.target
            .ok_or_else(|| PlatformError("Select a game window.".into()))
    }
    fn verify(&self, target: TargetToken) -> Result<()> {
        unsafe {
            let mut pid = 0;
            if IsWindow(hwnd(target)) == 0
                || GetWindowThreadProcessId(hwnd(target), &mut pid) == 0
                || pid != target.pid
                || process_identity(pid)? != target.created
            {
                return Err(PlatformError(
                    "The original game window no longer exists.".into(),
                ));
            }
        }
        Ok(())
    }
    fn register(&mut self) -> Result<()> {
        if self.registered {
            return Ok(());
        }
        unsafe {
            if RegisterHotKey(null_mut(), 7, MOD_NOREPEAT, 0x76) == 0 {
                return Err(os_error("F7 is unavailable"));
            }
            if RegisterHotKey(null_mut(), 8, MOD_NOREPEAT, 0x77) == 0 {
                UnregisterHotKey(null_mut(), 7);
                return Err(os_error("F8 is unavailable"));
            }
        }
        self.registered = true;
        Ok(())
    }
    fn pump_hotkeys(&mut self) -> Result<()> {
        self.register()?;
        unsafe {
            let mut message: MSG = zeroed();
            while PeekMessageW(&mut message, null_mut(), WM_HOTKEY, WM_HOTKEY, PM_REMOVE) != 0 {
                match message.wParam {
                    7 => self.pending.toggle = true,
                    8 => self.pending.stop = true,
                    _ => {}
                }
            }
        }
        Ok(())
    }
}
impl PlatformBackend for WindowsBackend {
    fn capabilities(&self) -> CapabilityReport {
        CapabilityReport {
            capture: true,
            identity: true,
            foreground: true,
            obstruction: true,
            manual_input: true,
            input: true,
            emergency_stop: self.registered,
            reason: self.registration_error.clone().unwrap_or_default(),
        }
    }
    fn list_targets(&mut self) -> Result<Vec<TargetInfo>> {
        unsafe extern "system" fn visit(window: HWND, data: LPARAM) -> i32 {
            // SAFETY: EnumWindows synchronously calls us with a pointer to this Vec.
            unsafe {
                let result = &mut *(data as *mut Vec<TargetInfo>);
                if IsWindowVisible(window) == 0 || GetWindowTextLengthW(window) == 0 {
                    return 1;
                }
                let mut title = vec![0u16; (GetWindowTextLengthW(window) + 1) as usize];
                let n = GetWindowTextW(window, title.as_mut_ptr(), title.len() as i32);
                let mut pid = 0;
                GetWindowThreadProcessId(window, &mut pid);
                if pid == GetCurrentProcessId() {
                    return 1;
                }
                if let Ok(created) = process_identity(pid) {
                    let process =
                        OwnedHandle(OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid));
                    let mut path = vec![0u16; 32768];
                    let mut len = path.len() as u32;
                    let name = if !process.0.is_null()
                        && QueryFullProcessImageNameW(process.0, 0, path.as_mut_ptr(), &mut len)
                            != 0
                    {
                        String::from_utf16_lossy(&path[..len as usize])
                            .rsplit('\\')
                            .next()
                            .unwrap_or("")
                            .to_owned()
                    } else {
                        String::new()
                    };
                    result.push(TargetInfo {
                        token: TargetToken {
                            handle: window as usize,
                            pid,
                            created,
                        },
                        title: String::from_utf16_lossy(&title[..n as usize]),
                        process_name: name,
                    });
                }
                1
            }
        }
        let mut result: Vec<TargetInfo> = vec![];
        unsafe {
            if EnumWindows(Some(visit), &mut result as *mut _ as LPARAM) == 0 {
                return Err(os_error("Window enumeration failed"));
            }
        }
        Ok(result)
    }
    fn attach(&mut self, target: TargetToken) -> Result<()> {
        self.verify(target)?;
        self.capture = None;
        self.target = Some(target);
        self.register()?;
        Ok(())
    }
    fn inspect(&mut self) -> Result<TargetState> {
        let target = self.target()?;
        self.verify(target)?;
        unsafe {
            let mut rect: RECT = zeroed();
            let mut origin = POINT { x: 0, y: 0 };
            if GetClientRect(hwnd(target), &mut rect) == 0
                || ClientToScreen(hwnd(target), &mut origin) == 0
            {
                return Err(os_error("Cannot inspect client area"));
            }
            Ok(TargetState {
                target,
                left: origin.x,
                top: origin.y,
                width: (rect.right - rect.left).max(0) as u32,
                height: (rect.bottom - rect.top).max(0) as u32,
                visible: IsWindowVisible(hwnd(target)) != 0 && IsIconic(hwnd(target)) == 0,
                foreground: GetForegroundWindow() == hwnd(target),
            })
        }
    }
    fn request_activation(&mut self) -> Result<bool> {
        let target = self.target()?;
        self.verify(target)?;
        self.check_input_access()?;
        unsafe {
            if IsIconic(hwnd(target)) != 0 {
                ShowWindow(hwnd(target), SW_RESTORE);
            }
            Ok(SetForegroundWindow(hwnd(target)) != 0)
        }
    }
    fn capture(
        &mut self,
        deadline: Instant,
        stop: &StopToken,
        generation: u64,
    ) -> Result<CapturedFrame> {
        if stop.cancelled(generation) || Instant::now() > deadline {
            return Err(PlatformError("Capture cancelled.".into()));
        }
        let requested = Instant::now();
        let before = self.inspect()?;
        if !before.visible
            || before.width == 0
            || before.height == 0
            || before.width > 16384
            || before.height > 16384
        {
            return Err(PlatformError(
                "The selected window is hidden, minimized or has invalid dimensions.".into(),
            ));
        }
        if self.capture.is_none() {
            self.capture = Some(window_capture::WindowCapture::new(before.target)?);
        }
        let captured = self
            .capture
            .as_mut()
            .expect("capture initialized")
            .frame(before, requested, deadline, stop, generation);
        let (bytes, captured_at) = match captured {
            Ok(value) => value,
            Err(error) => {
                self.capture = None;
                return Err(error);
            }
        };
        let after = self.inspect()?;
        if before != after
            || stop.cancelled(generation)
            || Instant::now() > deadline
            || captured_at.elapsed() > window_capture::MAX_FRAME_AGE
        {
            return Err(PlatformError(
                "Window geometry/focus changed, the source frame became stale, or capture timed out.".into(),
            ));
        }
        if bytes.iter().copied().max().unwrap_or(0) < 8 {
            return Err(PlatformError("The game frame is black.".into()));
        }
        self.frame_id += 1;
        Ok(CapturedFrame {
            frame: Frame::new(before.width, before.height, bytes)?,
            target: before.target,
            captured_at,
            id: self.frame_id,
            geometry: before,
        })
    }
    fn check_input_access(&mut self) -> Result<()> {
        let target = self.target()?;
        self.verify(target)?;
        if elevated(target.pid)? && !elevated(unsafe { GetCurrentProcessId() })? {
            return Err(PlatformError(
                "The game is elevated. Run MetinBot with the same privileges.".into(),
            ));
        }
        Ok(())
    }
    fn key_held(&self, key: u16) -> bool {
        unsafe { GetAsyncKeyState(i32::from(key)) < 0 }
    }
    fn point_clear(&self, point: [i32; 2]) -> bool {
        self.target.is_some_and(|target| unsafe {
            let hit = WindowFromPoint(POINT {
                x: point[0],
                y: point[1],
            });
            !hit.is_null() && GetAncestor(hit, GA_ROOT) == hwnd(target)
        })
    }
    fn cursor(&self) -> Result<[i32; 2]> {
        unsafe {
            let mut p = POINT { x: 0, y: 0 };
            if GetCursorPos(&mut p) == 0 {
                return Err(os_error("GetCursorPos failed"));
            }
            Ok([p.x, p.y])
        }
    }
    fn move_relative(&mut self, delta: [i32; 2]) -> Result<()> {
        unsafe {
            let event = INPUT {
                r#type: INPUT_MOUSE,
                Anonymous: INPUT_0 {
                    mi: MOUSEINPUT {
                        dx: delta[0],
                        dy: delta[1],
                        mouseData: 0,
                        dwFlags: MOUSEEVENTF_MOVE,
                        time: 0,
                        dwExtraInfo: 0,
                    },
                },
            };
            if SendInput(1, &event, size_of::<INPUT>() as i32) != 1 {
                return Err(os_error("SendInput cursor movement rejected"));
            }
        }
        Ok(())
    }
    fn key(&mut self, key: u16, down: bool, alt: bool) -> Result<()> {
        let target = self.target()?;
        if let Err(error) = self.verify(target) {
            return if down { Err(error) } else { Ok(()) };
        }
        if down {
            let state = self.inspect()?;
            if !state.visible || !state.foreground {
                return Err(PlatformError(
                    "The game must be visible and in the foreground.".into(),
                ));
            }
        }
        unsafe {
            let scan = MapVirtualKeyW(u32::from(key), MAPVK_VK_TO_VSC);
            if scan == 0 {
                return Err(PlatformError("Key has no valid scan code.".into()));
            }
            let packet = keyboard_packet(key, scan, down, alt);
            if PostMessageW(
                hwnd(target),
                packet.message,
                usize::from(key),
                packet.flags as isize,
            ) == 0
            {
                return Err(os_error("PostMessage key rejected"));
            }
        }
        Ok(())
    }
    fn mouse(&mut self, point: [u32; 2], down: bool) -> Result<()> {
        let target = self.target()?;
        if let Err(error) = self.verify(target) {
            return if down { Err(error) } else { Ok(()) };
        }
        if point.iter().any(|v| *v > 32767) {
            return Err(PlatformError("Mouse coordinates out of range.".into()));
        }
        if down {
            let state = self.inspect()?;
            if !state.visible
                || !state.foreground
                || point[0] >= state.width
                || point[1] >= state.height
            {
                return Err(PlatformError("Invalid game click target.".into()));
            }
        }
        unsafe {
            if PostMessageW(
                hwnd(target),
                if down { WM_LBUTTONDOWN } else { WM_LBUTTONUP },
                usize::from(down),
                (point[0] | (point[1] << 16)) as isize,
            ) == 0
            {
                return Err(os_error("PostMessage mouse rejected"));
            }
        }
        Ok(())
    }
    fn hotkeys(&mut self) -> Result<HotkeyEvents> {
        if let Err(error) = self.pump_hotkeys() {
            self.registration_error = Some(error.to_string());
            return Err(error);
        }
        let mut events = std::mem::replace(
            &mut self.pending,
            HotkeyEvents {
                toggle: false,
                stop: false,
            },
        );
        events.stop |= self.key_held(0x77);
        Ok(events)
    }
    fn emergency_pressed(&mut self) -> Result<bool> {
        self.pump_hotkeys()?;
        Ok(self.pending.stop || self.key_held(0x77))
    }
}
impl Drop for WindowsBackend {
    fn drop(&mut self) {
        if self.registered {
            unsafe {
                UnregisterHotKey(null_mut(), 7);
                UnregisterHotKey(null_mut(), 8);
            }
        }
    }
}
pub fn enable_dpi() {
    unsafe {
        SetProcessDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2);
    }
}

#[path = "window_capture.rs"]
mod window_capture;
