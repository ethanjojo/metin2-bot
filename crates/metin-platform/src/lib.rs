//! OS integration with explicit capabilities and immutable BGR frames.
use std::sync::{
    Arc,
    atomic::{AtomicU64, Ordering},
};
use std::time::Instant;
#[derive(Debug, Clone, thiserror::Error)]
#[error("{0}")]
pub struct PlatformError(pub String);
pub type Result<T> = std::result::Result<T, PlatformError>;

#[derive(Debug, Clone, PartialEq)]
pub struct Frame {
    pub width: u32,
    pub height: u32,
    pub bgr: Arc<[u8]>,
}
impl Frame {
    pub fn new(width: u32, height: u32, bgr: Vec<u8>) -> Result<Self> {
        if width == 0
            || height == 0
            || width > 16384
            || height > 16384
            || bgr.len() != width as usize * height as usize * 3
        {
            return Err(PlatformError(
                "Invalid BGR frame dimensions or buffer length.".into(),
            ));
        }
        Ok(Self {
            width,
            height,
            bgr: bgr.into(),
        })
    }
}
#[derive(Debug, Clone, Default)]
pub struct StopToken(Arc<AtomicU64>);
impl StopToken {
    pub fn generation(&self) -> u64 {
        self.0.load(Ordering::SeqCst)
    }
    pub fn stop(&self) {
        self.0.fetch_add(1, Ordering::SeqCst);
    }
    /// Cancel input only while the caller's start intent is still current.
    /// A concurrent emergency stop wins instead of being hidden by this stop.
    pub fn stop_if_current(&self, expected: u64) -> Option<u64> {
        let next = expected.checked_add(1)?;
        self.0
            .compare_exchange(expected, next, Ordering::SeqCst, Ordering::SeqCst)
            .ok()
            .map(|_| next)
    }
    pub fn cancelled(&self, generation: u64) -> bool {
        self.generation() != generation
    }
}

#[cfg(test)]
mod stop_token_tests {
    use super::*;
    #[test]
    fn competing_start_intents_cannot_both_claim_the_same_generation() {
        let stop = StopToken::default();
        let barrier = Arc::new(std::sync::Barrier::new(3));
        let results = std::thread::scope(|scope| {
            let first = scope.spawn(|| {
                barrier.wait();
                stop.stop_if_current(0)
            });
            let second = scope.spawn(|| {
                barrier.wait();
                stop.stop_if_current(0)
            });
            barrier.wait();
            [first.join().unwrap(), second.join().unwrap()]
        });
        assert_eq!(results.iter().filter(|ticket| ticket.is_some()).count(), 1);
        assert_eq!(stop.generation(), 1);
    }
    #[test]
    fn conditional_stop_cannot_overwrite_a_concurrent_pause_and_returns_its_own_ticket() {
        let stop = StopToken::default();
        let before = stop.generation();
        stop.stop();
        assert_eq!(stop.stop_if_current(before), None);
        assert_eq!(stop.generation(), before + 1);
        let ticket = stop.stop_if_current(before + 1).unwrap();
        assert_eq!(ticket, before + 2);
        assert_eq!(stop.stop_if_current(before + 1), None);
        stop.clone().stop();
        assert!(stop.cancelled(ticket));
        assert_eq!(stop.stop_if_current(ticket), None);
        stop.0.store(u64::MAX, Ordering::SeqCst);
        assert_eq!(stop.stop_if_current(u64::MAX), None);
        assert_eq!(stop.generation(), u64::MAX);
    }
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct TargetToken {
    pub handle: usize,
    pub pid: u32,
    pub created: u64,
}
#[derive(Debug, Clone, PartialEq)]
pub struct TargetInfo {
    pub token: TargetToken,
    pub title: String,
    pub process_name: String,
}
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TargetState {
    pub target: TargetToken,
    pub left: i32,
    pub top: i32,
    pub width: u32,
    pub height: u32,
    pub visible: bool,
    pub foreground: bool,
}
#[derive(Debug, Clone)]
pub struct CapturedFrame {
    pub frame: Frame,
    pub target: TargetToken,
    pub captured_at: Instant,
    pub id: u64,
    pub geometry: TargetState,
}
#[derive(Debug, Clone, Default, PartialEq)]
pub struct CapabilityReport {
    pub capture: bool,
    pub identity: bool,
    pub foreground: bool,
    pub obstruction: bool,
    pub manual_input: bool,
    pub input: bool,
    pub emergency_stop: bool,
    pub reason: String,
}
impl CapabilityReport {
    pub fn automation_ready(&self) -> bool {
        self.capture
            && self.identity
            && self.foreground
            && self.obstruction
            && self.manual_input
            && self.input
            && self.emergency_stop
    }
}
#[derive(Debug, Clone, Copy)]
pub struct HotkeyEvents {
    pub toggle: bool,
    pub stop: bool,
}

/// One selected, verified target. Implementations must retain the original target
/// for releases and must never redirect an up event to a reused native handle.
pub trait PlatformBackend: Send {
    fn capabilities(&self) -> CapabilityReport;
    fn list_targets(&mut self) -> Result<Vec<TargetInfo>>;
    fn attach(&mut self, target: TargetToken) -> Result<()>;
    /// Release presentation capture resources without detaching input or hotkeys.
    fn stop_capture(&mut self);
    fn inspect(&mut self) -> Result<TargetState>;
    fn request_activation(&mut self) -> Result<bool>;
    fn capture(
        &mut self,
        deadline: Instant,
        stop: &StopToken,
        generation: u64,
    ) -> Result<CapturedFrame>;
    fn check_input_access(&mut self) -> Result<()>;
    fn key_held(&self, key: u16) -> bool;
    fn point_clear(&self, point: [i32; 2]) -> bool;
    fn cursor(&self) -> Result<[i32; 2]>;
    fn move_relative(&mut self, delta: [i32; 2]) -> Result<()>;
    fn key(&mut self, key: u16, down: bool, alt: bool) -> Result<()>;
    fn mouse(&mut self, point: [u32; 2], down: bool) -> Result<()>;
    fn hotkeys(&mut self) -> Result<HotkeyEvents>;
    fn emergency_pressed(&mut self) -> Result<bool> {
        Ok(self.key_held(0x77))
    }
}
mod guarded_input;
pub use guarded_input::{InputExecutor, InputVerification};
mod hid_report;
pub use hid_report::{HidKey, HidKeyboard, scan_to_hid};

#[cfg(windows)]
mod hid_bridge;
#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use hid_bridge::manage_input_driver;
#[cfg(windows)]
pub use windows::{capture_window_preview, enable_dpi};
#[cfg(not(windows))]
pub fn capture_window_preview(
    _target: TargetToken,
    _deadline: Instant,
    _stop: &StopToken,
    _generation: u64,
) -> Result<Frame> {
    Err(PlatformError("Window previews require Windows.".into()))
}
pub fn backend() -> Box<dyn PlatformBackend> {
    #[cfg(windows)]
    {
        Box::new(windows::WindowsBackend::new())
    }
    #[cfg(not(windows))]
    {
        Box::new(UnavailableBackend)
    }
}
#[cfg(not(windows))]
struct UnavailableBackend;
#[cfg(not(windows))]
impl PlatformBackend for UnavailableBackend {
    fn capabilities(&self) -> CapabilityReport {
        CapabilityReport{reason:"This platform's capture/input backend has not been verified. Calibration and profiles remain available.".into(),..Default::default()}
    }
    fn list_targets(&mut self) -> Result<Vec<TargetInfo>> {
        Ok(vec![])
    }
    fn attach(&mut self, _: TargetToken) -> Result<()> {
        Err(PlatformError(self.capabilities().reason))
    }
    fn stop_capture(&mut self) {}
    fn inspect(&mut self) -> Result<TargetState> {
        Err(PlatformError(self.capabilities().reason))
    }
    fn request_activation(&mut self) -> Result<bool> {
        Err(PlatformError(self.capabilities().reason))
    }
    fn capture(&mut self, _: Instant, _: &StopToken, _: u64) -> Result<CapturedFrame> {
        Err(PlatformError(self.capabilities().reason))
    }
    fn check_input_access(&mut self) -> Result<()> {
        Err(PlatformError(self.capabilities().reason))
    }
    fn key_held(&self, _: u16) -> bool {
        false
    }
    fn point_clear(&self, _: [i32; 2]) -> bool {
        false
    }
    fn cursor(&self) -> Result<[i32; 2]> {
        Err(PlatformError(self.capabilities().reason))
    }
    fn move_relative(&mut self, _: [i32; 2]) -> Result<()> {
        Err(PlatformError(self.capabilities().reason))
    }
    fn key(&mut self, _: u16, _: bool, _: bool) -> Result<()> {
        Err(PlatformError(self.capabilities().reason))
    }
    fn mouse(&mut self, _: [u32; 2], _: bool) -> Result<()> {
        Err(PlatformError(self.capabilities().reason))
    }
    fn hotkeys(&mut self) -> Result<HotkeyEvents> {
        Ok(HotkeyEvents {
            toggle: false,
            stop: false,
        })
    }
}
