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
fn confirm_input_state(mut read: impl FnMut() -> bool, down: bool) -> Result<()> {
    // HID delivery acknowledges the class-driver queue before Win32's async
    // key state necessarily changes. Confirm it before the next action tests
    // for manual input, otherwise our own release looks like a physical hold.
    let end = Instant::now() + std::time::Duration::from_millis(200);
    while read() != down {
        if Instant::now() >= end {
            return Err(PlatformError(
                "Windows did not confirm the virtual HID pressed/released state.".into(),
            ));
        }
        std::thread::sleep(std::time::Duration::from_millis(1));
    }
    Ok(())
}
fn hwnd(token: TargetToken) -> HWND {
    token.handle as HWND
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum CaptureProblem {
    Cancelled,
    WindowChanged,
    FocusChanged,
    TimedOut,
    StaleSource,
}
impl CaptureProblem {
    fn message(self) -> &'static str {
        match self {
            Self::Cancelled => "Capture cancelled; no game input was sent.",
            Self::WindowChanged => {
                "Window geometry changed during capture. Check its position, size and visibility before starting."
            }
            Self::FocusChanged => {
                "Game focus changed during capture. Activate the selected game and start again."
            }
            Self::TimedOut => {
                "Window capture timed out before a valid image was available. Check that the selected game is visible and responding."
            }
            Self::StaleSource => {
                "The captured game image is too old. A fresh image is required before input."
            }
        }
    }
}
fn capture_problem(
    before: TargetState,
    after: TargetState,
    cancelled: bool,
    timed_out: bool,
    stale: bool,
) -> Option<CaptureProblem> {
    if cancelled {
        Some(CaptureProblem::Cancelled)
    } else if before.target != after.target
        || before.left != after.left
        || before.top != after.top
        || before.width != after.width
        || before.height != after.height
        || before.visible != after.visible
    {
        Some(CaptureProblem::WindowChanged)
    } else if before.foreground != after.foreground {
        Some(CaptureProblem::FocusChanged)
    } else if timed_out {
        Some(CaptureProblem::TimedOut)
    } else if stale {
        Some(CaptureProblem::StaleSource)
    } else {
        None
    }
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
fn verify_target(target: TargetToken) -> Result<()> {
    unsafe {
        let mut pid = 0;
        if IsWindow(hwnd(target)) == 0
            || GetWindowThreadProcessId(hwnd(target), &mut pid) == 0
            || pid != target.pid
            || process_identity(pid)? != target.created
        {
            return Err(PlatformError(
                "The original window no longer exists.".into(),
            ));
        }
    }
    Ok(())
}
fn inspect_target(target: TargetToken) -> Result<TargetState> {
    verify_target(target)?;
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

/// Read-only chooser thumbnail source. Uses the same HWND-only WGC implementation
/// as runtime capture, without creating HID sessions or registering hotkeys.
pub fn capture_window_preview(
    target: TargetToken,
    deadline: Instant,
    stop: &StopToken,
    generation: u64,
) -> Result<Frame> {
    if stop.cancelled(generation) || Instant::now() >= deadline {
        return Err(PlatformError("Preview cancelled.".into()));
    }
    let before = inspect_target(target)?;
    if !before.visible || before.width == 0 || before.height == 0 {
        return Err(PlatformError(
            "Restore this window to see its preview.".into(),
        ));
    }
    if u64::from(before.width) * u64::from(before.height) > 32_000_000 {
        return Err(PlatformError(
            "Preview unavailable for this window size.".into(),
        ));
    }
    let requested = Instant::now();
    let mut capture = window_capture::WindowCapture::new(target)?;
    let (pixels, captured_at) = capture.frame(before, requested, deadline, stop, generation)?;
    if inspect_target(target)? != before
        || stop.cancelled(generation)
        || Instant::now() >= deadline
        || captured_at.elapsed() > window_capture::MAX_FRAME_AGE
    {
        return Err(PlatformError(
            "Window changed while making its preview.".into(),
        ));
    }
    Frame::new(before.width, before.height, pixels)
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
    input: Option<crate::hid_bridge::Session>,
    input_error: Option<String>,
}
impl WindowsBackend {
    pub fn new() -> Self {
        let input_error = crate::hid_bridge::Bridge::load()
            .and_then(|bridge| bridge.manage(0))
            .err()
            .map(|error| error.to_string());
        Self {
            input: None,
            input_error,
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
    fn input(&mut self) -> Result<&mut crate::hid_bridge::Session> {
        if self.input.is_none() {
            match crate::hid_bridge::Bridge::load().and_then(|bridge| bridge.session()) {
                Ok(input) => {
                    self.input = Some(input);
                    self.input_error = None;
                }
                Err(error) => {
                    self.input_error = Some(error.to_string());
                    return Err(error);
                }
            }
        }
        Ok(self.input.as_mut().expect("initialized input session"))
    }
    fn verify(&self, target: TargetToken) -> Result<()> {
        verify_target(target)
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
            input: self.input_error.is_none(),
            emergency_stop: self.registered,
            reason: self
                .input_error
                .clone()
                .or_else(|| self.registration_error.clone())
                .unwrap_or_default(),
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
        inspect_target(self.target()?)
    }
    fn stop_capture(&mut self) {
        // Drop closes both the WGC session and frame pool immediately.
        self.capture = None;
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
        if stop.cancelled(generation) {
            return Err(PlatformError(CaptureProblem::Cancelled.message().into()));
        }
        if Instant::now() > deadline {
            return Err(PlatformError(CaptureProblem::TimedOut.message().into()));
        }
        let requested = Instant::now();
        let before = self.inspect()?;
        if !before.visible {
            return Err(PlatformError(
                "The selected game window is not visible. Restore it before capturing.".into(),
            ));
        }
        if before.width == 0 || before.height == 0 || before.width > 16384 || before.height > 16384
        {
            return Err(PlatformError(format!(
                "The selected window has invalid capture dimensions: {} × {} pixels.",
                before.width, before.height
            )));
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
        if let Some(problem) = capture_problem(
            before,
            after,
            stop.cancelled(generation),
            Instant::now() > deadline,
            captured_at.elapsed() > window_capture::MAX_FRAME_AGE,
        ) {
            return Err(PlatformError(problem.message().into()));
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
                "The game is elevated. Run Companion with the same privileges.".into(),
            ));
        }
        self.input()?;
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
        let state = self.inspect()?;
        if !state.visible || !state.foreground {
            return Err(PlatformError(
                "The game must be visible and in the foreground.".into(),
            ));
        }
        self.input()?.mouse(delta, None)
    }
    fn key(&mut self, key: u16, down: bool, _alt: bool) -> Result<()> {
        if down {
            let state = self.inspect()?;
            if !state.visible || !state.foreground {
                return Err(PlatformError(
                    "The game must be visible and in the foreground.".into(),
                ));
            }
        }
        // An owned release must work even after target destruction/focus loss.
        let scan = unsafe { MapVirtualKeyW(u32::from(key), MAPVK_VK_TO_VSC_EX) };
        if scan == 0 && down {
            return Err(PlatformError("Key has no valid scan code.".into()));
        }
        if !down && self.input.is_none() {
            return Ok(());
        }
        self.input()?.key(key, scan, down)?;
        confirm_input_state(|| self.key_held(key), down)?;
        if !down {
            self.input()?.confirm_key_release(key);
        }
        Ok(())
    }
    fn mouse(&mut self, point: [u32; 2], down: bool) -> Result<()> {
        if down {
            let state = self.inspect()?;
            if !state.visible
                || !state.foreground
                || point[0] >= state.width
                || point[1] >= state.height
            {
                return Err(PlatformError("Invalid game click target.".into()));
            }
            let expected = [state.left + point[0] as i32, state.top + point[1] as i32];
            let actual = self.cursor()?;
            if actual[0].abs_diff(expected[0]) > 3
                || actual[1].abs_diff(expected[1]) > 3
                || !self.point_clear(actual)
            {
                return Err(PlatformError(
                    "The cursor is not over the game click target.".into(),
                ));
            }
        }
        if !down && self.input.is_none() {
            return Ok(());
        }
        self.input()?.mouse([0, 0], Some(down))?;
        confirm_input_state(|| self.key_held(1), down)
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

#[cfg(test)]
mod input_confirmation_tests {
    use super::*;
    #[test]
    fn capture_failure_reports_the_actual_guard_in_priority_order() {
        let before = TargetState {
            target: TargetToken {
                handle: 1,
                pid: 2,
                created: 3,
            },
            left: -100,
            top: 50,
            width: 200,
            height: 300,
            visible: true,
            foreground: true,
        };
        let focus = TargetState {
            foreground: false,
            ..before
        };
        let geometry = TargetState {
            width: 201,
            ..focus
        };
        assert_eq!(capture_problem(before, before, false, false, false), None);
        assert_eq!(
            capture_problem(before, geometry, true, true, true),
            Some(CaptureProblem::Cancelled)
        );
        assert_eq!(
            capture_problem(before, geometry, false, true, true),
            Some(CaptureProblem::WindowChanged)
        );
        assert_eq!(
            capture_problem(before, focus, false, true, true),
            Some(CaptureProblem::FocusChanged)
        );
        assert_eq!(
            capture_problem(before, before, false, true, true),
            Some(CaptureProblem::TimedOut)
        );
        assert_eq!(
            capture_problem(before, before, false, false, true),
            Some(CaptureProblem::StaleSource)
        );
        for problem in [
            CaptureProblem::Cancelled,
            CaptureProblem::WindowChanged,
            CaptureProblem::FocusChanged,
            CaptureProblem::TimedOut,
            CaptureProblem::StaleSource,
        ] {
            assert!(!problem.message().contains(" or "), "{}", problem.message());
        }
    }
    #[test]
    fn release_waits_for_win32_after_hid_class_acknowledgement() {
        let mut reads = 0;
        confirm_input_state(
            || {
                reads += 1;
                reads < 3
            },
            false,
        )
        .unwrap();
        assert_eq!(reads, 3);
    }
    #[test]
    fn press_waits_before_starting_the_hold_duration() {
        let mut reads = 0;
        confirm_input_state(
            || {
                reads += 1;
                reads >= 3
            },
            true,
        )
        .unwrap();
        assert_eq!(reads, 3);
    }
}
