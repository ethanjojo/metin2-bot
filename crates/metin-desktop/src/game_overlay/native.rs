//! Win32 boundary: this worker owns every HWND, GDI object and message pump.
use super::*;
mod drawing;
use super::selection::{SelectionHit, SelectionShape, color_candidates};
use drawing::draw_selection;
use std::{
    cell::{Cell, RefCell},
    collections::VecDeque,
    mem::{size_of, zeroed},
    ptr::{null, null_mut},
    sync::{
        Arc, Mutex,
        atomic::{AtomicBool, AtomicUsize, Ordering},
        mpsc::{self, Receiver, SyncSender},
    },
    thread::{self, JoinHandle},
    time::Duration,
};
use windows_sys::Win32::{
    Foundation::*,
    Graphics::Gdi::*,
    System::{LibraryLoader::GetModuleHandleW, Threading::*},
    UI::{HiDpi::*, Input::KeyboardAndMouse::*, WindowsAndMessaging::*},
};

const MAX_FRAME_AGE: Duration = Duration::from_millis(500);
const ANNOTATION_EX_STYLE: u32 =
    WS_EX_LAYERED | WS_EX_TRANSPARENT | WS_EX_NOACTIVATE | WS_EX_TOOLWINDOW;
const ANNOTATION_STYLE: u32 = WS_POPUP | WS_DISABLED;
const WHITE: u32 = 0x00edeeee;
const GOLD: u32 = 0x0069bbe4;
const RED: u32 = 0x00565be9;
const GREEN: u32 = 0x008cdd7c;

fn selection_has_foreground(capture: usize, foreground: usize) -> bool {
    capture != 0 && foreground == capture
}

fn annotation_redraw_due(last: Option<Instant>, now: Instant, interval: Duration) -> bool {
    last.is_none_or(|painted| now.saturating_duration_since(painted) >= interval)
}

/// HWNDs and all coordinate APIs on this thread use physical client pixels.
/// Restoring the previous context also makes direct fixture use composable.
struct ThreadDpiContext(DPI_AWARENESS_CONTEXT);
impl ThreadDpiContext {
    fn physical() -> Result<Self, String> {
        let previous =
            unsafe { SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2) };
        if previous.is_null() {
            Err(native_error(
                "Cannot select physical per-monitor DPI coordinates",
            ))
        } else {
            Ok(Self(previous))
        }
    }
}
impl Drop for ThreadDpiContext {
    fn drop(&mut self) {
        unsafe {
            SetThreadDpiAwarenessContext(self.0);
        }
    }
}

/// One cached Unicode UI font per native worker, rather than GDI's bitmap default.
struct UiFont(HFONT);
impl UiFont {
    fn new() -> Result<Self, String> {
        let font = unsafe {
            CreateFontW(
                -16,
                0,
                0,
                0,
                FW_NORMAL as i32,
                0,
                0,
                0,
                DEFAULT_CHARSET as u32,
                OUT_DEFAULT_PRECIS as u32,
                CLIP_DEFAULT_PRECIS as u32,
                CLEARTYPE_QUALITY as u32,
                DEFAULT_PITCH as u32,
                wide("Segoe UI").as_ptr(),
            )
        };
        if font.is_null() {
            Err(native_error("Cannot create native UI font"))
        } else {
            Ok(Self(font))
        }
    }
}
impl Drop for UiFont {
    fn drop(&mut self) {
        unsafe {
            DeleteObject(self.0);
        }
    }
}

enum Command {
    Annotations(OverlayScene),
    Select(SelectionRequest),
    ResetPanel(PanelIdentity),
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
struct PanelIdentity {
    window: usize,
    target: TargetToken,
}
impl From<ControlPanelLink> for PanelIdentity {
    fn from(link: ControlPanelLink) -> Self {
        Self {
            window: link.window,
            target: link.target,
        }
    }
}

#[derive(Default)]
struct PanelPositionState {
    latest: Option<(PanelIdentity, crate::placement::PanelOffset)>,
    pending_reset: Option<PanelIdentity>,
}
impl PanelPositionState {
    fn current(&self) -> Option<(TargetToken, crate::placement::PanelOffset)> {
        self.pending_reset
            .map(|identity| (identity.target, Default::default()))
            .or_else(|| {
                self.latest
                    .map(|(identity, offset)| (identity.target, offset))
            })
    }
    fn publish(&mut self, identity: PanelIdentity, offset: crate::placement::PanelOffset) {
        self.latest = Some((identity, offset));
        if self.pending_reset == Some(identity) && offset == Default::default() {
            self.pending_reset = None;
        }
    }
    fn relink(&mut self, identity: Option<PanelIdentity>) {
        if self.pending_reset != identity {
            self.pending_reset = None;
        }
    }
}

struct Shared {
    shutdown: AtomicBool,
    hidden: AtomicBool,
    selecting: AtomicBool,
    annotation: AtomicUsize,
    control_panel: Mutex<Option<ControlPanelLink>>,
    panel_position: Mutex<PanelPositionState>,
}

/// A bounded, nonblocking adapter. Dropping it closes all windows and joins its worker.
pub struct GameOverlay {
    commands: SyncSender<Command>,
    events: Receiver<OverlayEvent>,
    shared: Arc<Shared>,
    worker: Option<JoinHandle<()>>,
}

impl GameOverlay {
    /// Latest worker-owned preferred offset, including moves not yet debounced.
    pub fn panel_position(&self) -> Option<(TargetToken, crate::placement::PanelOffset)> {
        self.shared
            .panel_position
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .current()
    }
    pub fn new() -> Result<Self, String> {
        let (commands, incoming) = mpsc::sync_channel(2);
        let (outgoing, events) = mpsc::sync_channel(2);
        let (ready, initialized) = mpsc::sync_channel(1);
        let shared = Arc::new(Shared {
            shutdown: AtomicBool::new(false),
            hidden: AtomicBool::new(true),
            selecting: AtomicBool::new(false),
            annotation: AtomicUsize::new(0),
            control_panel: Mutex::new(None),
            panel_position: Mutex::new(Default::default()),
        });
        let state = shared.clone();
        let worker = thread::Builder::new()
            .name("game-overlay".into())
            .spawn(move || {
                // SAFETY: all window creation, dispatch and destruction remain on this thread.
                unsafe {
                    match Worker::new(state, outgoing) {
                        Ok(mut worker) => {
                            let _ = ready.send(Ok(()));
                            worker.run(incoming);
                        }
                        Err(error) => {
                            let _ = ready.send(Err(error));
                        }
                    }
                }
            })
            .map_err(|error| error.to_string())?;
        match initialized.recv() {
            Ok(Ok(())) => Ok(Self {
                commands,
                events,
                shared,
                worker: Some(worker),
            }),
            result => {
                shared.shutdown.store(true, Ordering::Release);
                let _ = worker.join();
                Err(match result {
                    Ok(Err(error)) => error,
                    Err(error) => error.to_string(),
                    _ => unreachable!(),
                })
            }
        }
    }

    pub fn show_annotations(&self, scene: OverlayScene) -> Result<(), String> {
        validate_frame(scene.geometry, &scene.frame)?;
        if self.shared.selecting.load(Ordering::Acquire) {
            return Err("Frozen capture selection is active.".into());
        }
        self.shared.hidden.store(false, Ordering::Release);
        if let Err(error) = self.commands.try_send(Command::Annotations(scene)) {
            self.hide();
            return Err(error.to_string());
        }
        Ok(())
    }

    /// The caller MUST stop automation and receive its input-release acknowledgement first.
    /// This method activates a native selection window; it never generates game input.
    pub fn begin_selection_after_release(&self, request: SelectionRequest) -> Result<(), String> {
        validate_frame(request.geometry, &request.frame)?;
        if let Some(region) = request.initial_region {
            region.validate().map_err(|error| error.to_string())?;
        }
        self.shared
            .selecting
            .compare_exchange(false, true, Ordering::AcqRel, Ordering::Acquire)
            .map_err(|_| "Finish or cancel the current capture selection first.".to_owned())?;
        self.shared.hidden.store(false, Ordering::Release);
        if let Err(error) = self.commands.try_send(Command::Select(request)) {
            self.shared.selecting.store(false, Ordering::Release);
            self.hide();
            return Err(error.to_string());
        }
        Ok(())
    }

    pub fn poll(&self) -> Option<OverlayEvent> {
        loop {
            let event = self.events.try_recv().ok()?;
            if let OverlayEvent::PanelMoved { target, .. } = event {
                // Events are only notifications; the latest worker-owned placement is
                // authoritative, including a Reset clicked after this event was queued.
                if let Some((latest_target, offset)) = self.panel_position()
                    && latest_target == target
                {
                    return Some(OverlayEvent::PanelMoved { target, offset });
                }
                continue;
            }
            self.shared.selecting.store(false, Ordering::Release);
            return Some(event);
        }
    }

    /// Also cancels selection. Independent of a saturated scene-command queue.
    pub fn hide(&self) {
        self.shared.hidden.store(true, Ordering::Release);
    }

    /// Link only this process's existing control window; the worker never owns/destroys it.
    pub fn set_control_panel(&self, link: Option<ControlPanelLink>) -> Result<(), String> {
        if let Some(link) = link
            && !control_panel_valid(link)
        {
            return Err("The control panel must be this application's own native window.".into());
        }
        let mut current = self
            .shared
            .control_panel
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        *current = link;
        self.shared
            .panel_position
            .lock()
            .unwrap_or_else(|error| error.into_inner())
            .relink(link.map(Into::into));
        Ok(())
    }

    /// Reset presentation offset only, scoped to the exact linked target and owned HWND.
    pub fn reset_panel_position(&self) -> Result<(), String> {
        let current = self
            .shared
            .control_panel
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let Some(link) = *current else {
            return Ok(());
        };
        let identity = PanelIdentity::from(link);
        let mut position = self
            .shared
            .panel_position
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        let previous = position.pending_reset.replace(identity);
        if let Err(error) = self.commands.try_send(Command::ResetPanel(identity)) {
            position.pending_reset = previous;
            return Err(format!("Cannot reset panel position: {error}"));
        }
        Ok(())
    }

    /// Diagnostic identity only: disabled annotation windows are ignored by WindowFromPoint.
    pub fn annotation_window_handle(&self) -> usize {
        self.shared.annotation.load(Ordering::Acquire)
    }
}

impl Drop for GameOverlay {
    fn drop(&mut self) {
        self.shared.shutdown.store(true, Ordering::Release);
        if let Some(worker) = self.worker.take() {
            let _ = worker.join();
        }
    }
}

#[derive(Clone, Copy)]
enum WindowEvent {
    Dirty,
    Down([i32; 2]),
    Move([i32; 2]),
    Up([i32; 2]),
    Confirm,
    Cancel,
    ToggleControls,
}

struct WindowContext {
    annotation: bool,
    events: RefCell<VecDeque<WindowEvent>>,
    cursor: Cell<HCURSOR>,
}

unsafe extern "system" fn window_proc(hwnd: HWND, message: u32, w: WPARAM, l: LPARAM) -> LRESULT {
    // SAFETY: context is allocated before CreateWindowExW and lives until DestroyWindow returns.
    unsafe {
        if message == WM_NCCREATE {
            let create = &*(l as *const CREATESTRUCTW);
            SetWindowLongPtrW(hwnd, GWLP_USERDATA, create.lpCreateParams as isize);
        }
        let pointer = GetWindowLongPtrW(hwnd, GWLP_USERDATA) as *const WindowContext;
        if pointer.is_null() {
            return DefWindowProcW(hwnd, message, w, l);
        }
        let context = &*pointer;
        if message == WM_SETCURSOR && !context.annotation && (l as u32 & 0xffff) == HTCLIENT {
            SetCursor(context.cursor.get());
            return 1;
        }
        if context.annotation {
            if message == WM_NCHITTEST {
                return HTTRANSPARENT as isize;
            }
            if message == WM_MOUSEACTIVATE {
                return MA_NOACTIVATE as isize;
            }
        }
        let point = || [(l as u16 as i16) as i32, ((l >> 16) as u16 as i16) as i32];
        let event = match message {
            WM_PAINT => {
                let mut paint = zeroed();
                BeginPaint(hwnd, &mut paint);
                EndPaint(hwnd, &paint);
                Some(WindowEvent::Dirty)
            }
            WM_ERASEBKGND => return 1,
            WM_LBUTTONDOWN if !context.annotation => {
                SetCapture(hwnd);
                Some(WindowEvent::Down(point()))
            }
            WM_MOUSEMOVE if !context.annotation => Some(WindowEvent::Move(point())),
            WM_LBUTTONUP if !context.annotation => {
                ReleaseCapture();
                Some(WindowEvent::Up(point()))
            }
            WM_RBUTTONDOWN | WM_CLOSE if !context.annotation => Some(WindowEvent::Cancel),
            WM_KEYDOWN
                if !context.annotation && (w == VK_ESCAPE as usize || w == VK_F8 as usize) =>
            {
                Some(WindowEvent::Cancel)
            }
            WM_KEYDOWN if !context.annotation && w == VK_RETURN as usize => {
                Some(WindowEvent::Confirm)
            }
            WM_KEYDOWN if !context.annotation && w == VK_TAB as usize => {
                Some(WindowEvent::ToggleControls)
            }
            WM_NCDESTROY => {
                SetWindowLongPtrW(hwnd, GWLP_USERDATA, 0);
                None
            }
            _ => return DefWindowProcW(hwnd, message, w, l),
        };
        if let Some(event) = event {
            let mut events = context.events.borrow_mut();
            // Motion is coalesced; button/key edges retain ordering without unbounded growth.
            if matches!(event, WindowEvent::Move(_) | WindowEvent::Dirty) && events.len() >= 32 {
                return 0;
            }
            if events.len() < 64 {
                events.push_back(event);
            }
        }
        0
    }
}

struct Selection {
    request: SelectionRequest,
    dib: Vec<u8>,
    dimmed: Vec<u8>,
    shape: SelectionShape,
    point: Option<[u32; 2]>,
    candidates: Vec<Candidate>,
    error: String,
    controls_visible: bool,
}

impl Selection {
    fn new(request: SelectionRequest) -> Self {
        let dib: Vec<u8> = request
            .frame
            .bgr
            .as_chunks::<3>()
            .0
            .iter()
            .flat_map(|p| [p[0], p[1], p[2], 0])
            .collect();
        let dimmed = dib
            .as_chunks::<4>()
            .0
            .iter()
            .flat_map(|p| [p[0] / 2, p[1] / 2, p[2] / 2, 0])
            .collect();
        let rect = request
            .initial_region
            .map(|region| region.pixels(request.frame.width, request.frame.height));
        let shape = SelectionShape::new([request.frame.width, request.frame.height], rect);
        Self {
            request,
            dib,
            dimmed,
            shape,
            point: None,
            candidates: Vec::new(),
            error: String::new(),
            controls_visible: true,
        }
    }

    fn picked(&self) -> [u32; 2] {
        if self.request.kind == SelectionKind::Region
            && !self.shape.dragging()
            && self.shape.hit(self.shape.pointer) == SelectionHit::Draw
            && let Some([_, _, right, bottom]) = self.shape.bounds()
        {
            return [right, bottom];
        }
        self.point.unwrap_or(self.shape.pointer)
    }

    fn preview_color(&mut self) {
        if let (Some(profile), Some(point)) = (&self.request.color_preview, self.point) {
            match color_candidates(&self.request.frame, profile, point) {
                Ok(candidates) => {
                    self.candidates = candidates;
                    self.error.clear();
                }
                Err(error) => {
                    self.candidates.clear();
                    self.error = error;
                }
            }
        }
    }
}

struct PinnedPanel {
    link: ControlPanelLink,
    offset: crate::placement::PanelOffset,
    saved_offset: crate::placement::PanelOffset,
    changed_at: Option<Instant>,
    work_area: Option<[i32; 4]>,
    size: Option<[u32; 2]>,
    dpi: Option<u32>,
    geometry: Option<(i32, i32, u32, u32)>,
    position: Option<[i32; 2]>,
    pending_position_until: Option<Instant>,
    hidden_by_us: bool,
    topmost: Option<bool>,
}

fn control_panel_valid(link: ControlPanelLink) -> bool {
    unsafe {
        let window = link.window as HWND;
        let mut pid = 0;
        window != link.target.handle as HWND
            && IsWindow(window) != 0
            && GetWindowThreadProcessId(window, &mut pid) != 0
            && pid == GetCurrentProcessId()
    }
}

impl PinnedPanel {
    fn new(link: ControlPanelLink) -> Self {
        Self {
            link,
            offset: link.offset.unwrap_or_default(),
            saved_offset: link.offset.unwrap_or_default(),
            changed_at: None,
            work_area: None,
            size: None,
            dpi: None,
            geometry: None,
            position: None,
            pending_position_until: None,
            hidden_by_us: false,
            topmost: None,
        }
    }

    fn reset(&mut self, identity: PanelIdentity) -> bool {
        if PanelIdentity::from(self.link) != identity {
            return false;
        }
        self.offset = Default::default();
        self.geometry = None;
        self.changed_at = Some(Instant::now());
        true
    }

    unsafe fn set_level(&mut self, topmost: bool) {
        if self.topmost == Some(topmost) {
            return;
        }
        unsafe {
            SetWindowPos(
                self.link.window as HWND,
                if topmost {
                    HWND_TOPMOST
                } else {
                    HWND_NOTOPMOST
                },
                0,
                0,
                0,
                0,
                SWP_NOMOVE | SWP_NOSIZE | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS,
            );
        }
        self.topmost = Some(topmost);
    }

    unsafe fn restore(mut self) {
        if !control_panel_valid(self.link) {
            return;
        }
        unsafe {
            self.restore_visibility();
            self.set_level(false);
        }
    }

    unsafe fn restore_visibility(&mut self) {
        if !std::mem::take(&mut self.hidden_by_us) {
            return;
        }
        unsafe {
            // SW_SHOWNOACTIVATE restores a minimized window. A user's later
            // minimization supersedes the visibility ownership we acquired.
            let window = self.link.window as HWND;
            if IsIconic(window) == 0 {
                ShowWindowAsync(window, SW_SHOWNOACTIVATE);
            }
        }
    }

    unsafe fn sync(&mut self) {
        unsafe {
            let window = self.link.window as HWND;
            let foreground = GetForegroundWindow();
            let Some(target) = inspect_target(self.link.target) else {
                self.set_level(false);
                let target_window = self.link.target.handle as HWND;
                let minimized_or_hidden = IsWindow(target_window) != 0
                    && (IsIconic(target_window) != 0 || IsWindowVisible(target_window) == 0);
                // A disappeared target must leave a usable panel for re-selection.
                // Keep an active panel visible even when its target is minimized.
                if minimized_or_hidden && foreground != window {
                    if !self.hidden_by_us && IsWindowVisible(window) != 0 && IsIconic(window) == 0 {
                        ShowWindowAsync(window, SW_HIDE);
                        self.hidden_by_us = true;
                    }
                } else {
                    self.restore_visibility();
                }
                return;
            };
            self.restore_visibility();
            self.set_level(target.foreground || foreground == window);
            // User minimization of the panel itself remains a user choice.
            if IsIconic(window) != 0 || IsWindowVisible(window) == 0 {
                return;
            }
            let mut rect = zeroed();
            if GetWindowRect(window, &mut rect) == 0 {
                return;
            }
            let bounds = (target.left, target.top, target.width, target.height);
            let target_rect = [
                target.left,
                target.top,
                (i64::from(target.left) + i64::from(target.width))
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
                (i64::from(target.top) + i64::from(target.height))
                    .clamp(i64::from(i32::MIN), i64::from(i32::MAX)) as i32,
            ];
            let size = [
                (rect.right - rect.left).max(1) as u32,
                (rect.bottom - rect.top).max(1) as u32,
            ];
            let dpi = GetDpiForWindow(window).max(96);
            let monitor_area = |bounds: RECT| {
                let monitor = MonitorFromRect(&bounds, MONITOR_DEFAULTTONEAREST);
                let mut info: MONITORINFO = zeroed();
                info.cbSize = size_of::<MONITORINFO>() as u32;
                (GetMonitorInfoW(monitor, &mut info) != 0).then_some([
                    info.rcWork.left,
                    info.rcWork.top,
                    info.rcWork.right,
                    info.rcWork.bottom,
                ])
            };
            if self.geometry == Some(bounds) && self.dpi == Some(dpi) {
                let size_changed = self.size != Some(size);
                if self.position == Some([rect.left, rect.top]) {
                    self.pending_position_until = None;
                } else if self
                    .pending_position_until
                    .is_some_and(|until| Instant::now() < until)
                {
                    // The control panel belongs to egui's thread. Do not interpret an
                    // asynchronous position request still in its queue as a user drag.
                    return;
                }
                if self
                    .position
                    .is_some_and(|position| position != [rect.left, rect.top])
                {
                    self.offset = crate::placement::panel_offset(
                        target_rect,
                        [rect.left, rect.top, rect.right, rect.bottom],
                        dpi,
                    );
                    self.changed_at = Some(Instant::now());
                    self.position = Some([rect.left, rect.top]);
                }
                let work = monitor_area(rect);
                if !size_changed && work == self.work_area {
                    return;
                }
                self.work_area = work;
            }
            let unbounded = crate::placement::place_panel(
                target_rect,
                size,
                self.offset,
                dpi,
                [i32::MIN, i32::MIN, i32::MAX, i32::MAX],
            );
            let desired_rect = RECT {
                left: unbounded[0],
                top: unbounded[1],
                right: unbounded[0].saturating_add(size[0] as i32),
                bottom: unbounded[1].saturating_add(size[1] as i32),
            };
            let Some(work) = monitor_area(desired_rect) else {
                return;
            };
            let [x, y] = crate::placement::place_panel(target_rect, size, self.offset, dpi, work);
            SetWindowPos(
                window,
                null_mut(),
                x,
                y,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE | SWP_ASYNCWINDOWPOS,
            );
            self.position = Some([x, y]);
            self.size = Some(size);
            self.work_area = Some(work);
            self.pending_position_until = Some(Instant::now() + Duration::from_millis(200));
            self.geometry = Some(bounds);
            self.dpi = Some(dpi);
        }
    }
}

struct Worker {
    shared: Arc<Shared>,
    events: SyncSender<OverlayEvent>,
    class: Vec<u16>,
    annotation_context: Box<WindowContext>,
    selection_context: Box<WindowContext>,
    annotation: HWND,
    capture: HWND,
    scene: Option<OverlayScene>,
    selection: Option<Selection>,
    dirty: bool,
    annotation_painted: Option<Instant>,
    control_panel: Option<PinnedPanel>,
    font: UiFont,
    _dpi: ThreadDpiContext,
}

fn wide(text: &str) -> Vec<u16> {
    text.encode_utf16().chain(Some(0)).collect()
}
fn native_error(message: &str) -> String {
    format!("{message}: {}", std::io::Error::last_os_error())
}

impl Worker {
    unsafe fn sync_control_panel(&mut self) {
        let desired = *self
            .shared
            .control_panel
            .lock()
            .unwrap_or_else(|error| error.into_inner());
        if self.control_panel.as_ref().map(|panel| panel.link) != desired {
            if let Some(panel) = self.control_panel.take() {
                unsafe {
                    panel.restore();
                }
            }
            self.control_panel = desired
                .filter(|link| control_panel_valid(*link))
                .map(PinnedPanel::new);
        }
        if let Some(panel) = self.control_panel.as_mut()
            && control_panel_valid(panel.link)
        {
            unsafe {
                panel.sync();
            }
            self.shared
                .panel_position
                .lock()
                .unwrap_or_else(|error| error.into_inner())
                .publish(panel.link.into(), panel.offset);
            if panel
                .changed_at
                .is_some_and(|time| time.elapsed() >= Duration::from_millis(350))
                && panel.offset != panel.saved_offset
                && self
                    .events
                    .try_send(OverlayEvent::PanelMoved {
                        target: panel.link.target,
                        offset: panel.offset,
                    })
                    .is_ok()
            {
                panel.saved_offset = panel.offset;
                panel.changed_at = None;
            }
        }
    }

    unsafe fn new(shared: Arc<Shared>, events: SyncSender<OverlayEvent>) -> Result<Self, String> {
        unsafe {
            let dpi = ThreadDpiContext::physical()?;
            let font = UiFont::new()?;
            let class = wide(&format!("MetinCompanionOverlay-{}", GetCurrentThreadId()));
            let module = GetModuleHandleW(null());
            let descriptor = WNDCLASSW {
                lpfnWndProc: Some(window_proc),
                hInstance: module,
                lpszClassName: class.as_ptr(),
                hCursor: LoadCursorW(null_mut(), IDC_CROSS),
                ..zeroed()
            };
            if RegisterClassW(&descriptor) == 0 {
                return Err(native_error("Cannot register overlay window"));
            }
            let mut worker = Self {
                shared,
                events,
                class,
                annotation_context: Box::new(WindowContext {
                    annotation: true,
                    events: RefCell::new(VecDeque::new()),
                    cursor: Cell::new(LoadCursorW(null_mut(), IDC_ARROW)),
                }),
                selection_context: Box::new(WindowContext {
                    annotation: false,
                    events: RefCell::new(VecDeque::new()),
                    cursor: Cell::new(LoadCursorW(null_mut(), IDC_CROSS)),
                }),
                annotation: null_mut(),
                capture: null_mut(),
                scene: None,
                selection: None,
                dirty: true,
                annotation_painted: None,
                control_panel: None,
                font,
                _dpi: dpi,
            };
            worker.annotation = CreateWindowExW(
                ANNOTATION_EX_STYLE,
                worker.class.as_ptr(),
                wide("Companion annotations").as_ptr(),
                ANNOTATION_STYLE,
                0,
                0,
                1,
                1,
                null_mut(),
                null_mut(),
                module,
                &*worker.annotation_context as *const WindowContext as _,
            );
            if worker.annotation.is_null() {
                return Err(native_error("Cannot create annotations"));
            }
            if SetLayeredWindowAttributes(worker.annotation, 0, 255, LWA_COLORKEY) == 0 {
                return Err(native_error("Cannot configure transparent annotations"));
            }
            // Explicit desktop QA fixtures need a discoverable application
            // window; the production selection remains a taskbar-free tool.
            #[cfg(test)]
            let capture_style =
                if std::env::var("METIN_OVERLAY_FIXTURE_MANUAL").as_deref() == Ok("1") {
                    WS_EX_APPWINDOW
                } else {
                    WS_EX_TOOLWINDOW
                };
            #[cfg(not(test))]
            let capture_style = WS_EX_TOOLWINDOW;
            worker.capture = CreateWindowExW(
                capture_style,
                worker.class.as_ptr(),
                wide("Companion frozen capture").as_ptr(),
                WS_POPUP,
                0,
                0,
                1,
                1,
                null_mut(),
                null_mut(),
                module,
                &*worker.selection_context as *const WindowContext as _,
            );
            if worker.capture.is_null() {
                return Err(native_error("Cannot create capture selection"));
            }
            Ok(worker)
        }
    }

    unsafe fn run(&mut self, commands: Receiver<Command>) {
        unsafe {
            while !self.shared.shutdown.load(Ordering::Acquire) {
                self.sync_control_panel();
                while let Ok(command) = commands.try_recv() {
                    match command {
                        Command::Annotations(scene) => {
                            if self
                                .scene
                                .as_ref()
                                .is_none_or(|old| old.geometry != scene.geometry)
                            {
                                self.annotation_painted = None;
                            }
                            self.scene = Some(scene);
                            self.dirty = true;
                        }
                        Command::Select(request) => self.begin(request),
                        Command::ResetPanel(identity) => {
                            if let Some(panel) = self.control_panel.as_mut() {
                                panel.reset(identity);
                            }
                        }
                    }
                }
                let mut message = zeroed();
                // Bound dispatch so a busy pointer cannot starve shutdown/target validation.
                for _ in 0..128 {
                    if PeekMessageW(&mut message, null_mut(), 0, 0, PM_REMOVE) == 0 {
                        break;
                    }
                    TranslateMessage(&message);
                    DispatchMessageW(&message);
                }
                self.process_events();
                if self.shared.hidden.load(Ordering::Acquire) {
                    self.scene = None;
                    self.finish(None);
                    self.hide_annotations();
                } else if self.selection.is_some() {
                    self.hide_annotations();
                    let selection = self.selection.as_ref().unwrap();
                    let fg = GetForegroundWindow();
                    if inspect(selection.request.geometry).is_none()
                        || !selection_has_foreground(self.capture as usize, fg as usize)
                        || GetAsyncKeyState(VK_F8 as i32) < 0
                        || GetAsyncKeyState(VK_ESCAPE as i32) < 0
                    {
                        self.finish(None);
                    }
                } else if let Some(scene) = self.scene.as_ref() {
                    if scene.captured_at.elapsed() > MAX_FRAME_AGE
                        || !scene.geometry.foreground
                        || GetForegroundWindow() != scene.geometry.target.handle as HWND
                        || GetAsyncKeyState(VK_F8 as i32) < 0
                    {
                        self.scene = None;
                        self.hide_annotations();
                    } else if let Some(geometry) = inspect(scene.geometry) {
                        SetWindowPos(
                            self.annotation,
                            HWND_TOPMOST,
                            geometry.left,
                            geometry.top,
                            geometry.width as i32,
                            geometry.height as i32,
                            SWP_NOACTIVATE | SWP_SHOWWINDOW,
                        );
                        self.shared
                            .annotation
                            .store(self.annotation as usize, Ordering::Release);
                    } else {
                        self.scene = None;
                        self.hide_annotations();
                    }
                } else {
                    self.hide_annotations();
                }
                let paint_due = self.selection.is_some()
                    || self.scene.as_ref().is_none_or(|scene| {
                        annotation_redraw_due(
                            self.annotation_painted,
                            Instant::now(),
                            scene.redraw_interval,
                        )
                    });
                if self.dirty && paint_due {
                    self.draw();
                    if self.selection.is_none() && self.scene.is_some() {
                        self.annotation_painted = Some(Instant::now());
                    }
                    self.dirty = false;
                }
                thread::sleep(Duration::from_millis(16));
            }
            self.finish(None);
            self.hide_annotations();
        }
    }

    unsafe fn begin(&mut self, request: SelectionRequest) {
        unsafe {
            self.scene = None;
            self.hide_annotations();
            if self.shared.hidden.load(Ordering::Acquire) {
                let _ = self
                    .events
                    .try_send(OverlayEvent::Cancelled { id: request.id });
                return;
            }
            if inspect(request.geometry).is_none() {
                let _ = self.events.try_send(OverlayEvent::Failed {
                    id: Some(request.id),
                    message: "The original game client changed before selection opened.".into(),
                });
                return;
            }
            let geometry = request.geometry;
            self.selection_context.events.borrow_mut().clear();
            // BGRA avoids row padding at odd client widths; dim once per frozen source.
            self.selection = Some(Selection::new(request));
            SetWindowPos(
                self.capture,
                HWND_TOPMOST,
                geometry.left,
                geometry.top,
                geometry.width as i32,
                geometry.height as i32,
                SWP_SHOWWINDOW,
            );
            ShowWindow(self.capture, SW_SHOW);
            SetForegroundWindow(self.capture);
            SetFocus(self.capture);
            if !selection_has_foreground(self.capture as usize, GetForegroundWindow() as usize)
                || GetFocus() != self.capture
            {
                let id = self
                    .selection
                    .take()
                    .expect("selection initialized")
                    .request
                    .id;
                ReleaseCapture();
                ShowWindow(self.capture, SW_HIDE);
                let _ = self.events.try_send(OverlayEvent::Failed {
                    id: Some(id),
                    message: "Windows did not activate the frozen capture. Activate Companion and retry the capture tool.".into(),
                });
                return;
            }
            #[cfg(test)]
            if std::env::var("METIN_OVERLAY_FIXTURE_MANUAL").as_deref() == Ok("1") {
                log_fixture_window_geometry("active frozen capture", self.capture);
            }
            self.dirty = true;
        }
    }

    unsafe fn hide_annotations(&mut self) {
        self.annotation_painted = None;
        unsafe {
            ShowWindow(self.annotation, SW_HIDE);
        }
        self.shared.annotation.store(0, Ordering::Release);
    }

    unsafe fn finish(&mut self, selected: Option<(Option<Region>, [u32; 2])>) {
        if let Some(selection) = self.selection.take() {
            unsafe {
                ReleaseCapture();
                ShowWindow(self.capture, SW_HIDE);
            }
            let id = selection.request.id;
            let event = selected.map_or(OverlayEvent::Cancelled { id }, |(region, point)| {
                OverlayEvent::Selected { id, region, point }
            });
            // Only one request is admitted until its terminal event is polled, so capacity two cannot overflow.
            let _ = self.events.try_send(event);
        }
    }

    unsafe fn process_events(&mut self) {
        if !self.annotation_context.events.borrow_mut().is_empty() {
            self.annotation_context.events.borrow_mut().clear();
            self.dirty = true;
        }
        let events: Vec<_> = self
            .selection_context
            .events
            .borrow_mut()
            .drain(..)
            .collect();
        for event in events {
            let Some(selection) = self.selection.as_mut() else {
                break;
            };
            let size = [
                selection.request.frame.width,
                selection.request.frame.height,
            ];
            let toolbar = selection_toolbar(selection);
            match event {
                WindowEvent::Cancel => unsafe {
                    self.finish(None);
                },
                WindowEvent::Confirm => unsafe {
                    self.confirm();
                },
                WindowEvent::Down(point) => {
                    if let Some(toolbar) = toolbar.filter(|rect| in_rect(point, *rect)) {
                        if point[0] >= toolbar.right - 140 && point[0] < toolbar.right - 70 {
                            unsafe {
                                self.confirm();
                            }
                        } else if point[0] >= toolbar.right - 70 {
                            unsafe {
                                self.finish(None);
                            }
                        }
                    } else {
                        selection.error.clear();
                        let point = client_point(point, [0, 0], size);
                        if selection.request.kind == SelectionKind::Region {
                            selection.shape.down(point);
                        } else {
                            selection.shape.motion(point);
                            selection.point = Some(point);
                            selection.preview_color();
                        }
                    }
                }
                WindowEvent::Move(point) => {
                    selection.shape.motion(client_point(point, [0, 0], size));
                }
                WindowEvent::Up(point) => {
                    if selection.shape.dragging() {
                        selection.shape.up(client_point(point, [0, 0], size));
                    }
                }
                WindowEvent::Dirty => {}
                WindowEvent::ToggleControls => {
                    selection.controls_visible = !selection.controls_visible;
                }
            }
            self.dirty = true;
        }
        if let Some(selection) = self.selection.as_ref() {
            let pointer = selection.shape.pointer.map(|p| p as i32);
            let id = if selection_toolbar(selection).is_some_and(|rect| in_rect(pointer, rect)) {
                IDC_ARROW
            } else if selection.request.kind == SelectionKind::Point {
                IDC_CROSS
            } else {
                match selection.shape.cursor_hit() {
                    SelectionHit::Draw => IDC_CROSS,
                    SelectionHit::Move => IDC_SIZEALL,
                    SelectionHit::Resize([0, _]) => IDC_SIZENS,
                    SelectionHit::Resize([_, 0]) => IDC_SIZEWE,
                    SelectionHit::Resize([x, y]) if x == y => IDC_SIZENWSE,
                    SelectionHit::Resize(_) => IDC_SIZENESW,
                }
            };
            unsafe {
                let cursor = LoadCursorW(null_mut(), id);
                self.selection_context.cursor.set(cursor);
                SetCursor(cursor);
            }
        }
    }

    unsafe fn confirm(&mut self) {
        if self.shared.hidden.load(Ordering::Acquire) {
            unsafe {
                self.finish(None);
            }
            return;
        }
        let Some(selection) = self.selection.as_mut() else {
            return;
        };
        // Recheck at the confirmation edge as well as in the timer: messages may
        // arrive after the target was moved, destroyed or replaced.
        unsafe {
            let foreground = GetForegroundWindow();
            if inspect(selection.request.geometry).is_none()
                || !selection_has_foreground(self.capture as usize, foreground as usize)
                || GetAsyncKeyState(VK_F8 as i32) < 0
                || GetAsyncKeyState(VK_ESCAPE as i32) < 0
            {
                self.finish(None);
                return;
            }
        }
        let mut point = selection.picked();
        let region = if selection.request.kind == SelectionKind::Region {
            let Some([left, top, right, bottom]) = selection.shape.bounds() else {
                selection.error = "Drag a rectangle first.".into();
                return;
            };
            // Hover keeps changing for resize cursors after mouse-up; the result
            // must describe the confirmed rectangle rather than that hover.
            point = [right, bottom];
            match selection_region(
                [left, top],
                [right, bottom],
                selection.request.frame.width,
                selection.request.frame.height,
            ) {
                Ok(region) => Some(region),
                Err(error) => {
                    selection.error = error;
                    return;
                }
            }
        } else {
            if selection.point.is_none() {
                selection.error = "Pick a pixel first.".into();
                return;
            }
            None
        };
        unsafe {
            self.finish(Some((region, point)));
        }
    }

    unsafe fn draw(&self) {
        unsafe {
            if let Some(selection) = self.selection.as_ref() {
                draw_window(
                    self.capture,
                    selection.request.frame.width,
                    selection.request.frame.height,
                    self.font.0,
                    |dc| draw_selection(dc, selection),
                );
            } else if let Some(scene) = self.scene.as_ref() {
                draw_window(
                    self.annotation,
                    scene.frame.width,
                    scene.frame.height,
                    self.font.0,
                    |dc| draw_annotations(dc, scene),
                );
            }
        }
    }
}

impl Drop for Worker {
    fn drop(&mut self) {
        // SAFETY: worker and HWND destruction happen on their creating thread before contexts drop.
        unsafe {
            if let Some(panel) = self.control_panel.take() {
                panel.restore();
            }
            self.shared.annotation.store(0, Ordering::Release);
            if !self.capture.is_null() {
                DestroyWindow(self.capture);
            }
            if !self.annotation.is_null() {
                DestroyWindow(self.annotation);
            }
            UnregisterClassW(self.class.as_ptr(), GetModuleHandleW(null()));
        }
    }
}

/// Reject recycled HWND/PID, process restart, changed client size/origin, minimize and destruction.
fn inspect(expected: TargetState) -> Option<TargetState> {
    let actual = inspect_target(expected.target)?;
    (actual.left == expected.left
        && actual.top == expected.top
        && actual.width == expected.width
        && actual.height == expected.height)
        .then_some(actual)
}

/// Read-only live geometry for this exact original process/window identity.
/// Hidden, minimized, destroyed or recycled targets return `None`.
pub fn inspect_target(target: TargetToken) -> Option<TargetState> {
    let _dpi = ThreadDpiContext::physical().ok()?;
    unsafe {
        let hwnd = target.handle as HWND;
        let mut pid = 0;
        if IsWindow(hwnd) == 0
            || IsWindowVisible(hwnd) == 0
            || IsIconic(hwnd) != 0
            || GetWindowThreadProcessId(hwnd, &mut pid) == 0
            || pid != target.pid
        {
            return None;
        }
        let process = OpenProcess(PROCESS_QUERY_LIMITED_INFORMATION, 0, pid);
        if process.is_null() {
            return None;
        }
        let (mut created, mut exit, mut kernel, mut user) =
            (zeroed(), zeroed(), zeroed(), zeroed());
        let valid = GetProcessTimes(process, &mut created, &mut exit, &mut kernel, &mut user) != 0;
        CloseHandle(process);
        if !valid
            || (u64::from(created.dwHighDateTime) << 32) | u64::from(created.dwLowDateTime)
                != target.created
        {
            return None;
        }
        let mut rect = zeroed();
        let mut origin = POINT { x: 0, y: 0 };
        if GetClientRect(hwnd, &mut rect) == 0
            || ClientToScreen(hwnd, &mut origin) == 0
            || rect.right <= rect.left
            || rect.bottom <= rect.top
        {
            return None;
        }
        Some(TargetState {
            target,
            left: origin.x,
            top: origin.y,
            width: (rect.right - rect.left) as u32,
            height: (rect.bottom - rect.top) as u32,
            visible: true,
            foreground: GetForegroundWindow() == hwnd,
        })
    }
}

fn intersects(a: RECT, b: RECT) -> bool {
    a.left < b.right && a.right > b.left && a.top < b.bottom && a.bottom > b.top
}

fn selection_toolbar(selection: &Selection) -> Option<RECT> {
    if !selection.controls_visible || selection.shape.dragging() {
        return None;
    }
    let frame = &selection.request.frame;
    let width = frame.width as i32;
    let height = frame.height as i32;
    if width < 260 || height < 90 {
        return None;
    }
    let selected = if selection.request.kind == SelectionKind::Region {
        let [l, t, r, b] = selection.shape.bounds()?;
        RECT {
            left: l as i32 - 9,
            top: t as i32 - 9,
            right: r as i32 + 10,
            bottom: b as i32 + 10,
        }
    } else {
        let [x, y] = selection.point?;
        RECT {
            left: x as i32 - 18,
            top: y as i32 - 18,
            right: x as i32 + 19,
            bottom: y as i32 + 19,
        }
    };
    let bar_width = 360.min(width - 16);
    let bar_height = 58;
    let x = selected.left.clamp(8, width - bar_width - 8);
    let positions = [
        [x, selected.bottom + 8],
        [x, selected.top - bar_height - 8],
        [width - bar_width - 8, 8],
        [8, 8],
        [width - bar_width - 8, height - bar_height - 8],
        [8, height - bar_height - 8],
    ];
    positions
        .into_iter()
        .map(|[left, top]| RECT {
            left,
            top,
            right: left + bar_width,
            bottom: top + bar_height,
        })
        .find(|r| {
            r.left >= 8
                && r.top >= 8
                && r.right <= width - 8
                && r.bottom <= height - 8
                && !intersects(*r, selected)
        })
}

fn loupe_rect(selection: &Selection) -> Option<RECT> {
    let [x, y] = selection.picked().map(|v| v as i32);
    let size = [
        selection.request.frame.width as i32,
        selection.request.frame.height as i32,
    ];
    let toolbar = selection_toolbar(selection);
    let bounds = selection.shape.bounds().map(|[l, t, r, b]| RECT {
        left: l as i32 - 7,
        top: t as i32 - 7,
        right: r as i32 + 8,
        bottom: b as i32 + 8,
    });
    [
        [x + 24, y + 24],
        [x - 122, y + 24],
        [x + 24, y - 122],
        [x - 122, y - 122],
        [size[0] - 106, 8],
        [size[0] - 106, size[1] - 106],
        [8, 8],
    ]
    .into_iter()
    .map(|[left, top]| RECT {
        left,
        top,
        right: left + 98,
        bottom: top + 98,
    })
    .find(|r| {
        r.left >= 0
            && r.top >= 0
            && r.right <= size[0]
            && r.bottom <= size[1]
            && toolbar.is_none_or(|bar| !intersects(*r, bar))
            && bounds.is_none_or(|area| !intersects(*r, area))
    })
}

#[cfg(test)]
unsafe fn log_fixture_window_geometry(label: &str, window: HWND) {
    unsafe {
        let mut bounds = zeroed();
        let mut client = zeroed();
        let mut origin = POINT { x: 0, y: 0 };
        GetWindowRect(window, &mut bounds);
        GetClientRect(window, &mut client);
        ClientToScreen(window, &mut origin);
        eprintln!(
            "Native QA {label}: PMv2={} DPI={} window=({}, {})..({}, {}) client={}x{} physical_origin=({}, {})",
            AreDpiAwarenessContextsEqual(
                GetThreadDpiAwarenessContext(),
                DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2
            ) != 0,
            GetDpiForWindow(window),
            bounds.left,
            bounds.top,
            bounds.right,
            bounds.bottom,
            client.right - client.left,
            client.bottom - client.top,
            origin.x,
            origin.y
        );
    }
}
fn in_rect(point: [i32; 2], rect: RECT) -> bool {
    point[0] >= rect.left && point[0] < rect.right && point[1] >= rect.top && point[1] < rect.bottom
}

unsafe fn text(dc: HDC, position: [i32; 2], value: &str, color: u32) {
    unsafe {
        let value: Vec<u16> = value.encode_utf16().collect();
        SetTextColor(dc, color);
        SetBkMode(dc, TRANSPARENT as i32);
        TextOutW(
            dc,
            position[0],
            position[1],
            value.as_ptr(),
            value.len() as i32,
        );
    }
}

unsafe fn outline(dc: HDC, rect: RECT, color: u32, width: i32) {
    unsafe {
        let pen = CreatePen(PS_SOLID, width, color);
        let old_pen = SelectObject(dc, pen);
        let old_brush = SelectObject(dc, GetStockObject(NULL_BRUSH));
        Rectangle(dc, rect.left, rect.top, rect.right, rect.bottom);
        SelectObject(dc, old_pen);
        SelectObject(dc, old_brush);
        DeleteObject(pen);
    }
}

unsafe fn crosshair(dc: HDC, point: [u32; 2], color: u32) {
    unsafe {
        let (x, y) = (point[0] as i32, point[1] as i32);
        let pen = CreatePen(PS_SOLID, 2, color);
        let old = SelectObject(dc, pen);
        MoveToEx(dc, x - 10, y, null_mut());
        LineTo(dc, x + 11, y);
        MoveToEx(dc, x, y - 10, null_mut());
        LineTo(dc, x, y + 11);
        SelectObject(dc, old);
        DeleteObject(pen);
    }
}

unsafe fn draw_window(hwnd: HWND, width: u32, height: u32, font: HFONT, render: impl FnOnce(HDC)) {
    unsafe {
        let destination = GetDC(hwnd);
        if destination.is_null() {
            return;
        }
        let dc = CreateCompatibleDC(destination);
        let bitmap = CreateCompatibleBitmap(destination, width as i32, height as i32);
        if !dc.is_null() && !bitmap.is_null() {
            let old = SelectObject(dc, bitmap);
            let old_font = SelectObject(dc, font);
            PatBlt(dc, 0, 0, width as i32, height as i32, BLACKNESS);
            render(dc);
            BitBlt(
                destination,
                0,
                0,
                width as i32,
                height as i32,
                dc,
                0,
                0,
                SRCCOPY,
            );
            SelectObject(dc, old);
            SelectObject(dc, old_font);
        }
        if !bitmap.is_null() {
            DeleteObject(bitmap);
        }
        if !dc.is_null() {
            DeleteDC(dc);
        }
        ReleaseDC(hwnd, destination);
    }
}

unsafe fn draw_annotations(dc: HDC, scene: &OverlayScene) {
    unsafe {
        for candidate in &scene.candidates {
            let color = if scene.current == Some(candidate.point) {
                GREEN
            } else if scene.next == Some(candidate.point) {
                GOLD
            } else {
                WHITE
            };
            let rect = RECT {
                left: candidate.x as i32,
                top: candidate.y as i32,
                right: candidate.x.saturating_add(candidate.width) as i32,
                bottom: candidate.y.saturating_add(candidate.height) as i32,
            };
            outline(dc, rect, color, 2);
            if scene.show_sizes {
                text(
                    dc,
                    [rect.left, rect.top - 18],
                    &format!("{} x {}", candidate.width, candidate.height),
                    color,
                );
            }
        }
        if let Some(point) = scene.next {
            crosshair(dc, point, GOLD);
        }
        draw_region_annotations(
            dc,
            &scene.regions,
            scene.frame.width,
            scene.frame.height,
            scene.show_sizes,
        );
    }
}

/// Both live annotations and frozen review use the same safe presentation path.
unsafe fn draw_region_annotations(
    dc: HDC,
    regions: &[(Region, String)],
    width: u32,
    height: u32,
    show_sizes: bool,
) {
    unsafe {
        for (region, label) in regions {
            let Some(annotation) = region_annotation(*region, width, height) else {
                continue;
            };
            let [left, top, right, bottom] = annotation.pixels;
            let color = if annotation.outside_image { RED } else { GOLD };
            outline(
                dc,
                RECT {
                    left: left as i32,
                    top: top as i32,
                    right: right as i32 + 1,
                    bottom: bottom as i32 + 1,
                },
                color,
                1,
            );
            let mut label = label.clone();
            if annotation.outside_image {
                label.push_str(" · outside image");
            }
            if show_sizes {
                label.push_str(&format!("  {} x {}", right - left + 1, bottom - top + 1));
            }
            // Keep an edge-clipped invalid draft's warning inside the image.
            let value: Vec<u16> = label.encode_utf16().collect();
            let mut extent = SIZE { cx: 0, cy: 0 };
            GetTextExtentPoint32W(dc, value.as_ptr(), value.len() as i32, &mut extent);
            let label_width = extent.cx.max(0) as u32;
            let label_x = left
                .saturating_add(3)
                .min(width.saturating_sub(label_width));
            let label_y = top
                .saturating_add(3)
                .min(height.saturating_sub(extent.cy.max(0) as u32));
            text(dc, [label_x as i32, label_y as i32], &label, color);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn queued_move_notifications_read_reset_preference_without_finishing_selection() {
        let link = ControlPanelLink {
            window: 11,
            target: TargetToken {
                handle: 12,
                pid: 13,
                created: 14,
            },
            offset: Some(crate::placement::PanelOffset { right: 80, top: 90 }),
        };
        let mut position = PanelPositionState::default();
        position.publish(link.into(), link.offset.unwrap());
        let (commands, incoming) = mpsc::sync_channel(2);
        let (outgoing, events) = mpsc::sync_channel(2);
        let shared = Arc::new(Shared {
            shutdown: AtomicBool::new(false),
            hidden: AtomicBool::new(true),
            selecting: AtomicBool::new(true),
            annotation: AtomicUsize::new(0),
            control_panel: Mutex::new(Some(link)),
            panel_position: Mutex::new(position),
        });
        let overlay = GameOverlay {
            commands,
            events,
            shared,
            worker: None,
        };
        outgoing
            .try_send(OverlayEvent::PanelMoved {
                target: link.target,
                offset: link.offset.unwrap(),
            })
            .unwrap();
        overlay.reset_panel_position().unwrap();
        assert_eq!(
            overlay.poll(),
            Some(OverlayEvent::PanelMoved {
                target: link.target,
                offset: Default::default()
            })
        );
        assert!(
            overlay.shared.selecting.load(Ordering::Acquire),
            "Placement notifications do not acknowledge capture completion"
        );
        let Command::ResetPanel(identity) = incoming.try_recv().unwrap() else {
            panic!("Expected scoped Reset");
        };
        assert_eq!(identity, link.into());
    }

    #[test]
    fn pending_panel_reset_masks_old_publication_until_acknowledged_and_never_retargets() {
        let a = ControlPanelLink {
            window: 11,
            target: TargetToken {
                handle: 12,
                pid: 13,
                created: 14,
            },
            offset: Some(crate::placement::PanelOffset { right: 80, top: 90 }),
        };
        let identity = PanelIdentity::from(a);
        let old = a.offset.unwrap();
        let mut position = PanelPositionState::default();
        position.publish(identity, old);
        position.pending_reset = Some(identity);
        assert_eq!(
            position.current(),
            Some((a.target, Default::default())),
            "Immediate shutdown must persist Reset, not the old worker offset"
        );
        position.publish(identity, old);
        assert_eq!(
            position.current(),
            Some((a.target, Default::default())),
            "A pre-command publication cannot undo Reset"
        );
        assert_eq!(position.pending_reset, Some(identity));
        position.publish(identity, Default::default());
        assert_eq!(position.pending_reset, None);
        assert_eq!(position.current(), Some((a.target, Default::default())));
        let mut panel = PinnedPanel::new(a);
        assert!(panel.reset(identity));
        for b in [
            ControlPanelLink { window: 21, ..a },
            ControlPanelLink {
                target: TargetToken {
                    handle: 22,
                    ..a.target
                },
                ..a
            },
        ] {
            let mut next = PinnedPanel::new(b);
            assert!(
                !next.reset(identity),
                "Queued Reset cannot target a replacement panel or selected game"
            );
            assert_eq!(next.offset, old);
            position.pending_reset = Some(identity);
            position.relink(Some(b.into()));
            assert_eq!(position.pending_reset, None);
        }
    }

    fn synthetic_frame() -> Frame {
        // Asymmetric RGB gradient, 40px grid and three solid swatches expose color,
        // orientation, row-padding and pixel-coordinate mistakes in the actual native tool.
        let (width, height) = (803u32, 560u32);
        let mut bgr = Vec::with_capacity(width as usize * height as usize * 3);
        for y in 0..height {
            for x in 0..width {
                let color = if (80..180).contains(&y) && (80..180).contains(&x) {
                    [0, 0, 255]
                } else if (80..180).contains(&y) && (200..300).contains(&x) {
                    [0, 255, 0]
                } else if (80..180).contains(&y) && (320..420).contains(&x) {
                    [255, 0, 0]
                } else if x % 40 == 0 || y % 40 == 0 {
                    [210, 210, 210]
                } else {
                    [(x * 255 / width) as u8, (y * 255 / height) as u8, 45]
                };
                bgr.extend_from_slice(&color);
            }
        }
        Frame::new(width, height, bgr).unwrap()
    }

    fn selection_fixture(kind: SelectionKind) -> Selection {
        Selection::new(SelectionRequest {
            id: 42,
            label: "Fixed reference".into(),
            geometry: TargetState {
                target: TargetToken {
                    handle: 0,
                    pid: 0,
                    created: 0,
                },
                left: 0,
                top: 0,
                width: 803,
                height: 560,
                visible: true,
                foreground: true,
            },
            frame: Frame::new(803, 560, vec![120; 803 * 560 * 3]).unwrap(),
            kind,
            initial_region: None,
            context_regions: Vec::new(),
            color_preview: None,
        })
    }

    #[test]
    fn selection_controls_never_cover_the_selected_pixels_and_vanish_during_drag() {
        let mut selection = selection_fixture(SelectionKind::Region);
        assert!(
            selection_toolbar(&selection).is_none(),
            "no header/control can intercept the initial draw"
        );
        selection.shape.down([20, 20]);
        selection.shape.up([100, 100]);
        let toolbar = selection_toolbar(&selection).unwrap();
        assert!(!intersects(
            toolbar,
            RECT {
                left: 20,
                top: 20,
                right: 101,
                bottom: 101
            }
        ));
        selection.shape.down([50, 50]);
        assert!(
            selection_toolbar(&selection).is_none(),
            "all controls hide while moving/resizing"
        );
        selection.shape.up([60, 60]);
        assert!(selection_toolbar(&selection).is_some());
        selection.shape = SelectionShape::new([803, 560], Some([0, 0, 802, 559]));
        assert!(
            selection_toolbar(&selection).is_none(),
            "full-frame selection reserves every source pixel; keyboard applies it"
        );
    }

    #[test]
    fn dimming_preserves_the_frozen_source_and_existing_rect_is_editable() {
        let mut selection = selection_fixture(SelectionKind::Region);
        assert_eq!(&selection.dib[..4], &[120, 120, 120, 0]);
        assert_eq!(&selection.dimmed[..4], &[60, 60, 60, 0]);
        assert_eq!(&selection.request.frame.bgr[..3], &[120, 120, 120]);
        selection.request.initial_region =
            Some(selection_region([20, 30], [80, 90], 803, 560).unwrap());
        let mut selection = Selection::new(selection.request);
        assert_eq!(selection.shape.bounds(), Some([20, 30, 80, 90]));
        selection.shape.down([50, 60]);
        selection.shape.up([60, 70]);
        assert_eq!(selection.shape.bounds(), Some([30, 40, 90, 100]));
    }

    #[test]
    fn redraw_throttle_does_not_own_freshness_or_accumulate_frame_updates() {
        let now = Instant::now();
        assert!(annotation_redraw_due(None, now, Duration::from_secs(1)));
        assert!(!annotation_redraw_due(
            Some(now),
            now + Duration::from_millis(999),
            Duration::from_secs(1)
        ));
        assert!(annotation_redraw_due(
            Some(now),
            now + Duration::from_secs(1),
            Duration::from_secs(1)
        ));
        assert!(annotation_redraw_due(Some(now), now, Duration::ZERO));
    }

    #[test]
    fn magnifier_center_reads_the_exact_frozen_pixel_for_rgb_swatches_and_gradient() {
        unsafe {
            let dc = CreateCompatibleDC(null_mut());
            assert!(!dc.is_null());
            let mut pixels = null_mut();
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: 803,
                    biHeight: -560,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB,
                    ..zeroed()
                },
                ..zeroed()
            };
            let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut pixels, null_mut(), 0);
            assert!(!bitmap.is_null());
            let old = SelectObject(dc, bitmap);
            let mut selection = selection_fixture(SelectionKind::Point);
            selection.request.frame = synthetic_frame();
            selection.dib = selection
                .request
                .frame
                .bgr
                .as_chunks::<3>()
                .0
                .iter()
                .flat_map(|p| [p[0], p[1], p[2], 0])
                .collect();
            for point in [
                [120, 120],
                [250, 130],
                [350, 130],
                [600, 400],
                [0, 0],
                [802, 559],
            ] {
                selection.point = Some(point);
                selection.shape.motion(point);
                draw_selection(dc, &selection);
                let loupe = loupe_rect(&selection).expect("fixture has space for magnifier");
                let sx = (point[0] as i32 - 8).clamp(0, 803 - 16);
                let sy = (point[1] as i32 - 8).clamp(0, 560 - 16);
                let x = loupe.left + 1 + (point[0] as i32 - sx) * 6 + 3;
                let y = loupe.top + 1 + (point[1] as i32 - sy) * 6 + 3;
                let actual = GetPixel(dc, x, y);
                let offset = (point[1] * 803 + point[0]) as usize * 3;
                let bgr = &selection.request.frame.bgr[offset..offset + 3];
                let expected =
                    u32::from(bgr[2]) | (u32::from(bgr[1]) << 8) | (u32::from(bgr[0]) << 16);
                assert_eq!(actual, expected, "magnifier source at {point:?}");
            }
            SelectObject(dc, old);
            DeleteObject(bitmap);
            DeleteDC(dc);
        }
    }

    #[test]
    fn native_worker_uses_physical_dpi_and_restores_the_callers_context() {
        unsafe {
            let previous = SetThreadDpiAwarenessContext(DPI_AWARENESS_CONTEXT_UNAWARE);
            assert!(!previous.is_null());
            let (tx, _rx) = mpsc::sync_channel(2);
            let shared = Arc::new(Shared {
                shutdown: AtomicBool::new(false),
                hidden: AtomicBool::new(false),
                selecting: AtomicBool::new(false),
                annotation: AtomicUsize::new(0),
                control_panel: Mutex::new(None),
                panel_position: Mutex::new(Default::default()),
            });
            let worker = Worker::new(shared, tx).unwrap();
            let during = AreDpiAwarenessContextsEqual(
                GetThreadDpiAwarenessContext(),
                DPI_AWARENESS_CONTEXT_PER_MONITOR_AWARE_V2,
            ) != 0;
            drop(worker);
            let restored = AreDpiAwarenessContextsEqual(
                GetThreadDpiAwarenessContext(),
                DPI_AWARENESS_CONTEXT_UNAWARE,
            ) != 0;
            SetThreadDpiAwarenessContext(previous);
            assert!(
                during,
                "native HWND creation and messages must use physical PMv2 coordinates"
            );
            assert!(
                restored,
                "worker teardown must restore its caller's DPI context"
            );
        }
    }

    #[test]
    fn selection_requires_its_own_foreground_and_never_the_game() {
        assert!(selection_has_foreground(100, 100));
        assert!(!selection_has_foreground(100, 200));
        assert!(!selection_has_foreground(100, 0));
        assert!(!selection_has_foreground(0, 0));
    }

    struct FixtureWindow(HWND);
    impl Drop for FixtureWindow {
        fn drop(&mut self) {
            unsafe {
                DestroyWindow(self.0);
            }
        }
    }

    /// Internal messages are restricted to this process's own capture HWND. This
    /// exercises the real Win32 callback and canonical result without game input.
    #[test]
    #[ignore = "activates synthetic native target/capture windows for exact coordinate QA"]
    fn owned_capture_messages_return_exact_inclusive_physical_selections() {
        let (tx, rx) = mpsc::sync_channel(2);
        let shared = Arc::new(Shared {
            shutdown: AtomicBool::new(false),
            hidden: AtomicBool::new(false),
            selecting: AtomicBool::new(false),
            annotation: AtomicUsize::new(0),
            control_panel: Mutex::new(None),
            panel_position: Mutex::new(Default::default()),
        });
        unsafe {
            let mut worker = Worker::new(shared, tx).unwrap();
            let manual = std::env::var("METIN_OVERLAY_FIXTURE_MANUAL").as_deref() == Ok("1");
            let target = FixtureWindow(CreateWindowExW(
                if manual {
                    WS_EX_APPWINDOW
                } else {
                    WS_EX_TOOLWINDOW
                },
                worker.class.as_ptr(),
                wide("Companion exact-coordinate QA - activate this synthetic target").as_ptr(),
                WS_POPUP,
                80,
                80,
                803,
                560,
                null_mut(),
                null_mut(),
                GetModuleHandleW(null()),
                &*worker.selection_context as *const WindowContext as _,
            ));
            assert!(!target.0.is_null());
            ShowWindow(target.0, SW_SHOWNOACTIVATE);
            let (mut created, mut exit, mut kernel, mut user) =
                (zeroed(), zeroed(), zeroed(), zeroed());
            assert_ne!(
                GetProcessTimes(
                    GetCurrentProcess(),
                    &mut created,
                    &mut exit,
                    &mut kernel,
                    &mut user
                ),
                0
            );
            let token = TargetToken {
                handle: target.0 as usize,
                pid: GetCurrentProcessId(),
                created: (u64::from(created.dwHighDateTime) << 32)
                    | u64::from(created.dwLowDateTime),
            };
            let geometry = inspect_target(token).unwrap();
            assert_eq!(
                [
                    geometry.left,
                    geometry.top,
                    geometry.width as i32,
                    geometry.height as i32
                ],
                [80, 80, 803, 560]
            );
            let frame = synthetic_frame();
            SetForegroundWindow(target.0);
            if manual {
                eprintln!(
                    "Activate the synthetic exact-coordinate QA target. The test posts only its own HWND messages after activation/button release."
                );
                let deadline = Instant::now() + Duration::from_secs(90);
                loop {
                    pump_fixture_messages();
                    draw_window(target.0, 803, 560, worker.font.0, |dc| {
                        let brush = CreateSolidBrush(0x005a5a5a);
                        FillRect(
                            dc,
                            &RECT {
                                left: 0,
                                top: 0,
                                right: 803,
                                bottom: 560,
                            },
                            brush,
                        );
                        DeleteObject(brush);
                        text(
                            dc,
                            [20, 20],
                            "SYNTHETIC EXACT-COORDINATE QA - activate / click here to run",
                            WHITE,
                        );
                    });
                    if GetForegroundWindow() == target.0
                        && GetAsyncKeyState(VK_LBUTTON as i32) >= 0
                        && GetCapture() != target.0
                    {
                        break;
                    }
                    assert!(
                        Instant::now() < deadline,
                        "owned-window activation needs an explicit operator gesture"
                    );
                    thread::sleep(Duration::from_millis(16));
                }
            }
            // The window chooser uses this same read-only WGC source. Verify an
            // actual visible source without constructing a backend, HID device,
            // global hotkeys or any input executor.
            pump_fixture_messages();
            draw_window(target.0, 803, 560, worker.font.0, |dc| {
                let background = CreateSolidBrush(0x005a5a5a);
                FillRect(
                    dc,
                    &RECT {
                        left: 0,
                        top: 0,
                        right: 803,
                        bottom: 560,
                    },
                    background,
                );
                DeleteObject(background);
                let marker = CreateSolidBrush(0x0000ff00);
                FillRect(
                    dc,
                    &RECT {
                        left: 200,
                        top: 200,
                        right: 240,
                        bottom: 240,
                    },
                    marker,
                );
                DeleteObject(marker);
            });
            let stop = metin_platform::StopToken::default();
            let preview = metin_platform::capture_window_preview(
                token,
                Instant::now() + Duration::from_secs(2),
                &stop,
                stop.generation(),
            )
            .expect("own visible chooser source must produce a real WGC preview");
            assert_eq!([preview.width, preview.height], [803, 560]);
            assert!(preview.bgr.iter().any(|value| *value != 0));
            let marker = (210 * preview.width as usize + 210) * 3;
            assert_eq!(&preview.bgr[marker..marker + 3], &[0, 255, 0]);
            let background = (100 * preview.width as usize + 100) * 3;
            assert_eq!(&preview.bgr[background..background + 3], &[90, 90, 90]);
            eprintln!(
                "Read-only chooser WGC preview: 803x560, exact green marker and gray background verified"
            );
            for (id, start, end) in [
                (901, [90u32, 90u32], [94u32, 94u32]),
                (902, [90, 90], [160, 150]),
            ] {
                worker.begin(SelectionRequest {
                    id,
                    label: "Internal exact-coordinate fixture".into(),
                    geometry,
                    frame: frame.clone(),
                    kind: SelectionKind::Region,
                    initial_region: None,
                    context_regions: Vec::new(),
                    color_preview: None,
                });
                assert!(
                    worker.selection.is_some(),
                    "strict capture activation must succeed in the owned-window fixture: {:?}",
                    rx.try_recv()
                );
                let mut origin = POINT { x: 0, y: 0 };
                let mut bounds = zeroed();
                assert_ne!(ClientToScreen(worker.capture, &mut origin), 0);
                assert_ne!(GetClientRect(worker.capture, &mut bounds), 0);
                eprintln!(
                    "Owned capture DPI={} physical origin=({}, {}) size={}x{} expected={start:?}..{end:?}",
                    GetDpiForWindow(worker.capture),
                    origin.x,
                    origin.y,
                    bounds.right,
                    bounds.bottom
                );
                assert_eq!(
                    [origin.x, origin.y, bounds.right, bounds.bottom],
                    [80, 80, 803, 560]
                );
                worker.draw();
                save_owned_fixture(worker.capture, "region-before.png");
                post_owned_capture(
                    worker.capture,
                    WM_LBUTTONDOWN,
                    1,
                    packed_client_point(start),
                );
                post_owned_capture(worker.capture, WM_MOUSEMOVE, 1, packed_client_point(end));
                post_owned_capture(worker.capture, WM_LBUTTONUP, 0, packed_client_point(end));
                pump_fixture_messages();
                worker.process_events();
                worker.draw();
                save_owned_fixture(
                    worker.capture,
                    if id == 901 {
                        "region-five-pixels.png"
                    } else {
                        "region-drawn.png"
                    },
                );
                post_owned_capture(worker.capture, WM_KEYDOWN, VK_RETURN as usize, 0);
                pump_fixture_messages();
                worker.process_events();
                let event = rx
                    .try_recv()
                    .expect("owned capture must return its terminal event");
                let OverlayEvent::Selected {
                    id: returned,
                    region: Some(region),
                    point,
                } = event
                else {
                    panic!("unexpected native result: {event:?}")
                };
                assert_eq!(returned, id);
                assert_eq!(point, end);
                assert_eq!(
                    region.pixels(803, 560),
                    [start[0], start[1], end[0], end[1]]
                );
                assert!(rx.try_recv().is_err(), "one selection produces one result");
            }
            worker.begin(SelectionRequest {
                id: 904,
                label: "Move and resize saved reference".into(),
                geometry,
                frame: frame.clone(),
                kind: SelectionKind::Region,
                initial_region: Some(selection_region([20, 30], [80, 90], 803, 560).unwrap()),
                context_regions: Vec::new(),
                color_preview: None,
            });
            assert!(worker.selection.is_some());
            for (start, end) in [([50, 60], [60, 70]), ([90, 100], [100, 110])] {
                post_owned_capture(
                    worker.capture,
                    WM_LBUTTONDOWN,
                    1,
                    packed_client_point(start),
                );
                post_owned_capture(worker.capture, WM_MOUSEMOVE, 1, packed_client_point(end));
                post_owned_capture(worker.capture, WM_LBUTTONUP, 0, packed_client_point(end));
            }
            pump_fixture_messages();
            worker.process_events();
            worker.draw();
            save_owned_fixture(worker.capture, "region-moved-resized.png");
            post_owned_capture(worker.capture, WM_KEYDOWN, VK_RETURN as usize, 0);
            pump_fixture_messages();
            worker.process_events();
            let OverlayEvent::Selected {
                id: 904,
                region: Some(region),
                point: [100, 110],
            } = rx.try_recv().unwrap()
            else {
                panic!("native move/resize did not return its edited shape")
            };
            assert_eq!(region.pixels(803, 560), [30, 40, 100, 110]);
            worker.begin(SelectionRequest {
                id: 903,
                label: "Metin color · synthetic preview".into(),
                geometry,
                frame,
                kind: SelectionKind::Point,
                initial_region: None,
                context_regions: Vec::new(),
                color_preview: Some(Box::new({
                    let mut profile = metin_core::Profile::create("Synthetic color preview");
                    profile.region = Some(Region {
                        left: 0.,
                        top: 0.,
                        right: 1.,
                        bottom: 1.,
                    });
                    profile
                })),
            });
            assert!(worker.selection.is_some());
            post_owned_capture(
                worker.capture,
                WM_LBUTTONDOWN,
                1,
                packed_client_point([250, 130]),
            );
            post_owned_capture(
                worker.capture,
                WM_LBUTTONUP,
                0,
                packed_client_point([250, 130]),
            );
            pump_fixture_messages();
            worker.process_events();
            worker.draw();
            save_owned_fixture(worker.capture, "color-preview.png");
            let selection = worker.selection.as_ref().unwrap();
            assert_eq!(selection.picked(), [250, 130]);
            assert!(
                selection.candidates.iter().any(|c| c.x <= 250
                    && c.x + c.width > 250
                    && c.y <= 130
                    && c.y + c.height > 130)
            );
            post_owned_capture(worker.capture, WM_KEYDOWN, VK_RETURN as usize, 0);
            pump_fixture_messages();
            worker.process_events();
            assert_eq!(
                rx.try_recv().unwrap(),
                OverlayEvent::Selected {
                    id: 903,
                    region: None,
                    point: [250, 130]
                }
            );
        }
    }

    fn packed_client_point(point: [u32; 2]) -> LPARAM {
        (point[0] | (point[1] << 16)) as LPARAM
    }

    unsafe fn post_owned_capture(window: HWND, message: u32, w: WPARAM, l: LPARAM) {
        unsafe {
            let mut pid = 0;
            assert_ne!(GetWindowThreadProcessId(window, &mut pid), 0);
            assert_eq!(
                pid,
                GetCurrentProcessId(),
                "test messages may only reach our own fixture"
            );
            assert_ne!(PostMessageW(window, message, w, l), 0);
        }
    }

    /// Copies only our own rendered fixture client, never the desktop or a game.
    unsafe fn save_owned_fixture(window: HWND, name: &str) {
        let Ok(directory) = std::env::var("METIN_OVERLAY_FIXTURE_SCREENSHOTS") else {
            return;
        };
        unsafe {
            let mut pid = 0;
            GetWindowThreadProcessId(window, &mut pid);
            assert_eq!(pid, GetCurrentProcessId());
            let mut rect = zeroed();
            assert_ne!(GetClientRect(window, &mut rect), 0);
            let width = rect.right as u32;
            let height = rect.bottom as u32;
            let source = GetDC(window);
            assert!(!source.is_null());
            let dc = CreateCompatibleDC(source);
            let bitmap = CreateCompatibleBitmap(source, width as i32, height as i32);
            assert!(!dc.is_null() && !bitmap.is_null());
            let old = SelectObject(dc, bitmap);
            assert_ne!(
                BitBlt(dc, 0, 0, width as i32, height as i32, source, 0, 0, SRCCOPY),
                0
            );
            SelectObject(dc, old);
            let mut pixels = vec![0u8; width as usize * height as usize * 4];
            let mut info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: width as i32,
                    biHeight: -(height as i32),
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB,
                    ..zeroed()
                },
                ..zeroed()
            };
            assert_ne!(
                GetDIBits(
                    dc,
                    bitmap,
                    0,
                    height,
                    pixels.as_mut_ptr().cast(),
                    &mut info,
                    DIB_RGB_COLORS
                ),
                0
            );
            DeleteObject(bitmap);
            DeleteDC(dc);
            ReleaseDC(window, source);
            let bgr = pixels
                .as_chunks::<4>()
                .0
                .iter()
                .flat_map(|p| [p[0], p[1], p[2]])
                .collect();
            let frame = Frame::new(width, height, bgr).unwrap();
            std::fs::create_dir_all(&directory).unwrap();
            std::fs::write(
                std::path::Path::new(&directory).join(name),
                metin_vision::encode_png(&frame).unwrap(),
            )
            .unwrap();
        }
    }

    #[test]
    fn dropping_adapter_joins_worker_and_releases_its_shared_state() {
        let overlay = GameOverlay::new().unwrap();
        let shared = overlay.shared.clone();
        drop(overlay);
        assert_eq!(
            Arc::strong_count(&shared),
            1,
            "native worker must be joined"
        );
        assert_eq!(shared.annotation.load(Ordering::Acquire), 0);
    }

    #[test]
    fn pending_selection_is_bounded_until_its_terminal_event_is_polled() {
        let overlay = GameOverlay::new().unwrap();
        let request = SelectionRequest {
            id: 123,
            label: "Target-loss fixture".into(),
            geometry: TargetState {
                target: metin_platform::TargetToken {
                    handle: 0,
                    pid: 0,
                    created: 0,
                },
                left: -100,
                top: -100,
                width: 2,
                height: 2,
                visible: true,
                foreground: false,
            },
            frame: Frame::new(2, 2, vec![0; 12]).unwrap(),
            kind: SelectionKind::Point,
            initial_region: None,
            context_regions: Vec::new(),
            color_preview: None,
        };
        overlay
            .begin_selection_after_release(request.clone())
            .unwrap();
        assert!(overlay.begin_selection_after_release(request).is_err());
        let deadline = Instant::now() + Duration::from_secs(2);
        let result = loop {
            if let Some(event) = overlay.poll() {
                break event;
            }
            assert!(
                Instant::now() < deadline,
                "native worker failed to return target-loss event"
            );
            thread::sleep(Duration::from_millis(10));
        };
        assert!(matches!(result, OverlayEvent::Failed { id: Some(123), .. }));
        assert!(overlay.poll().is_none());
        assert!(!overlay.shared.selecting.load(Ordering::Acquire));
    }

    #[test]
    fn cancellation_returns_exactly_one_terminal_result() {
        let (tx, rx) = mpsc::sync_channel(2);
        let shared = Arc::new(Shared {
            shutdown: AtomicBool::new(false),
            hidden: AtomicBool::new(false),
            selecting: AtomicBool::new(true),
            annotation: AtomicUsize::new(0),
            control_panel: Mutex::new(None),
            panel_position: Mutex::new(Default::default()),
        });
        unsafe {
            let mut worker = Worker::new(shared, tx).unwrap();
            worker.selection = Some(Selection::new(SelectionRequest {
                id: 55,
                label: "Cancellation fixture".into(),
                geometry: TargetState {
                    target: metin_platform::TargetToken {
                        handle: 0,
                        pid: 0,
                        created: 0,
                    },
                    left: 0,
                    top: 0,
                    width: 2,
                    height: 2,
                    visible: true,
                    foreground: false,
                },
                frame: Frame::new(2, 2, vec![0; 12]).unwrap(),
                kind: SelectionKind::Point,
                initial_region: None,
                context_regions: Vec::new(),
                color_preview: None,
            }));
            worker.finish(None);
            worker.finish(None);
        }
        assert_eq!(rx.try_recv().unwrap(), OverlayEvent::Cancelled { id: 55 });
        assert!(rx.try_recv().is_err());
    }

    #[test]
    #[ignore = "briefly displays two native fixture windows; run explicitly during desktop QA"]
    fn annotation_is_ignored_by_window_from_point_without_any_input_exception() {
        metin_platform::enable_dpi();
        let (tx, _rx) = mpsc::sync_channel(2);
        let shared = Arc::new(Shared {
            shutdown: AtomicBool::new(false),
            hidden: AtomicBool::new(false),
            selecting: AtomicBool::new(false),
            annotation: AtomicUsize::new(0),
            control_panel: Mutex::new(None),
            panel_position: Mutex::new(Default::default()),
        });
        unsafe {
            let worker = Worker::new(shared, tx).unwrap();
            let foreground = GetForegroundWindow();
            let target = CreateWindowExW(
                WS_EX_TOOLWINDOW,
                worker.class.as_ptr(),
                wide("Overlay hit-test fixture").as_ptr(),
                WS_POPUP,
                120,
                120,
                120,
                120,
                null_mut(),
                null_mut(),
                GetModuleHandleW(null()),
                &*worker.selection_context as *const WindowContext as _,
            );
            assert!(!target.is_null());
            SetWindowPos(
                target,
                HWND_TOPMOST,
                120,
                120,
                120,
                120,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
            SetWindowPos(
                worker.annotation,
                HWND_TOPMOST,
                120,
                120,
                120,
                120,
                SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
            let dc = GetDC(worker.annotation);
            let brush = CreateSolidBrush(GOLD);
            FillRect(
                dc,
                &RECT {
                    left: 20,
                    top: 20,
                    right: 100,
                    bottom: 100,
                },
                brush,
            );
            DeleteObject(brush);
            ReleaseDC(worker.annotation, dc);
            let disabled = IsWindowEnabled(worker.annotation) == 0;
            let hit = WindowFromPoint(POINT { x: 160, y: 160 });
            // Clean up the fixture before assertions, including on a failed hit-test.
            DestroyWindow(target);
            assert!(disabled);
            assert_eq!(
                hit, target,
                "annotation HWND must never bypass the guarded target hit-test"
            );
            assert_eq!(
                GetForegroundWindow(),
                foreground,
                "annotations must never activate"
            );
        }
    }

    #[test]
    #[ignore = "moves/minimizes synthetic native windows; run explicitly during desktop QA"]
    fn pin_follows_live_geometry_hides_on_minimize_and_restores_on_unlink() {
        metin_platform::enable_dpi();
        let (tx, _rx) = mpsc::sync_channel(2);
        let shared = Arc::new(Shared {
            shutdown: AtomicBool::new(false),
            hidden: AtomicBool::new(false),
            selecting: AtomicBool::new(false),
            annotation: AtomicUsize::new(0),
            control_panel: Mutex::new(None),
            panel_position: Mutex::new(Default::default()),
        });
        unsafe {
            let fixture = Worker::new(shared, tx).unwrap();
            let target = fixture.capture;
            let panel = CreateWindowExW(
                WS_EX_TOOLWINDOW,
                fixture.class.as_ptr(),
                wide("Synthetic pin panel").as_ptr(),
                WS_POPUP,
                100,
                100,
                200,
                180,
                null_mut(),
                null_mut(),
                GetModuleHandleW(null()),
                &*fixture.annotation_context as *const WindowContext as _,
            );
            assert!(!panel.is_null());
            SetWindowPos(
                target,
                null_mut(),
                120,
                120,
                500,
                400,
                SWP_NOZORDER | SWP_NOACTIVATE | SWP_SHOWWINDOW,
            );
            ShowWindow(panel, SW_SHOWNOACTIVATE);
            let (mut created, mut exit, mut kernel, mut user) =
                (zeroed(), zeroed(), zeroed(), zeroed());
            assert_ne!(
                GetProcessTimes(
                    GetCurrentProcess(),
                    &mut created,
                    &mut exit,
                    &mut kernel,
                    &mut user
                ),
                0
            );
            let token = TargetToken {
                handle: target as usize,
                pid: GetCurrentProcessId(),
                created: (u64::from(created.dwHighDateTime) << 32)
                    | u64::from(created.dwLowDateTime),
            };
            let overlay = GameOverlay::new().unwrap();
            overlay
                .set_control_panel(Some(ControlPanelLink {
                    window: panel as usize,
                    target: token,
                    offset: None,
                }))
                .unwrap();
            wait_fixture(|| {
                let mut rect = zeroed();
                GetWindowRect(panel, &mut rect) != 0 && [rect.left, rect.top] == [408, 132]
            });
            SetWindowPos(
                target,
                null_mut(),
                240,
                200,
                600,
                450,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            wait_fixture(|| {
                let mut rect = zeroed();
                GetWindowRect(panel, &mut rect) != 0 && [rect.left, rect.top] == [628, 212]
            });
            // Record a user reposition while the game's geometry stays fixed.
            SetWindowPos(
                panel,
                null_mut(),
                400,
                250,
                0,
                0,
                SWP_NOSIZE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
            let settle = Instant::now() + Duration::from_millis(250);
            while Instant::now() < settle {
                pump_fixture_messages();
                thread::sleep(Duration::from_millis(10));
            }
            SetWindowPos(
                target,
                null_mut(),
                300,
                220,
                600,
                450,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            wait_fixture(|| {
                let mut rect = zeroed();
                GetWindowRect(panel, &mut rect) != 0 && [rect.left, rect.top] == [460, 270]
            });
            // Expanding a strip near the bottom must keep every control reachable.
            let mut monitor: MONITORINFO = zeroed();
            monitor.cbSize = size_of::<MONITORINFO>() as u32;
            assert_ne!(
                GetMonitorInfoW(
                    MonitorFromWindow(panel, MONITOR_DEFAULTTONEAREST),
                    &mut monitor
                ),
                0
            );
            SetWindowPos(
                panel,
                null_mut(),
                460,
                monitor.rcWork.bottom - 48,
                200,
                42,
                SWP_NOZORDER | SWP_NOACTIVATE,
            );
            pump_fixture_for(Duration::from_millis(400));
            let preferred_before_expand = overlay.panel_position().expect("preferred strip offset");
            SetWindowPos(
                panel,
                null_mut(),
                0,
                0,
                200,
                480,
                SWP_NOMOVE | SWP_NOZORDER | SWP_NOACTIVATE,
            );
            pump_fixture_for(Duration::from_millis(400));
            let mut expanded: RECT = zeroed();
            assert_ne!(GetWindowRect(panel, &mut expanded), 0);
            assert!(
                expanded.bottom <= monitor.rcWork.bottom,
                "Expanded panel must be re-clamped into the monitor work area"
            );
            assert_eq!(
                overlay.panel_position(),
                Some(preferred_before_expand),
                "Automatic reflow must not overwrite the preferred position used on shutdown"
            );
            ShowWindow(target, SW_MINIMIZE);
            wait_fixture(|| IsWindowVisible(panel) == 0);
            ShowWindow(target, SW_SHOWNOACTIVATE);
            wait_fixture(|| IsWindowVisible(panel) != 0);
            ShowWindow(target, SW_MINIMIZE);
            wait_fixture(|| IsWindowVisible(panel) == 0);
            overlay.set_control_panel(None).unwrap();
            wait_fixture(|| {
                IsWindowVisible(panel) != 0
                    && GetWindowLongPtrW(panel, GWL_EXSTYLE) & WS_EX_TOPMOST as isize == 0
            });
            // The user, rather than the pin watcher, owns an already minimized panel.
            ShowWindow(target, SW_SHOWNOACTIVATE);
            overlay
                .set_control_panel(Some(ControlPanelLink {
                    window: panel as usize,
                    target: token,
                    offset: None,
                }))
                .unwrap();
            pump_fixture_for(Duration::from_millis(100));
            ShowWindow(panel, SW_MINIMIZE);
            assert_ne!(IsIconic(panel), 0);
            ShowWindow(target, SW_MINIMIZE);
            pump_fixture_for(Duration::from_millis(100));
            ShowWindow(target, SW_SHOWNOACTIVATE);
            pump_fixture_for(Duration::from_millis(100));
            let stayed_minimized_on_game_restore = IsIconic(panel) != 0;
            ShowWindow(panel, SW_MINIMIZE);
            ShowWindow(target, SW_MINIMIZE);
            pump_fixture_for(Duration::from_millis(100));
            overlay.set_control_panel(None).unwrap();
            pump_fixture_for(Duration::from_millis(100));
            let stayed_minimized_on_unlink = IsIconic(panel) != 0;
            // A panel hidden before linking is likewise never ours to restore.
            ShowWindow(target, SW_SHOWNOACTIVATE);
            ShowWindow(panel, SW_SHOWNOACTIVATE);
            ShowWindow(panel, SW_HIDE);
            overlay
                .set_control_panel(Some(ControlPanelLink {
                    window: panel as usize,
                    target: token,
                    offset: None,
                }))
                .unwrap();
            pump_fixture_for(Duration::from_millis(100));
            ShowWindow(target, SW_MINIMIZE);
            pump_fixture_for(Duration::from_millis(100));
            ShowWindow(target, SW_SHOWNOACTIVATE);
            pump_fixture_for(Duration::from_millis(100));
            overlay.set_control_panel(None).unwrap();
            pump_fixture_for(Duration::from_millis(100));
            let stayed_hidden_on_unlink = IsWindowVisible(panel) == 0;
            drop(overlay);
            assert_ne!(
                IsWindow(panel),
                0,
                "adapter must never destroy the control panel"
            );
            DestroyWindow(panel);
            assert!(
                stayed_minimized_on_game_restore,
                "game restore must preserve the user's minimized panel"
            );
            assert!(
                stayed_minimized_on_unlink,
                "unlink must preserve the user's minimized panel"
            );
            assert!(
                stayed_hidden_on_unlink,
                "unlink must preserve the user's hidden panel"
            );
        }
    }

    unsafe fn wait_fixture(mut condition: impl FnMut() -> bool) {
        let deadline = Instant::now() + Duration::from_secs(3);
        loop {
            unsafe {
                pump_fixture_messages();
            }
            if condition() {
                return;
            }
            assert!(
                Instant::now() < deadline,
                "native fixture state did not converge"
            );
            thread::sleep(Duration::from_millis(10));
        }
    }

    unsafe fn pump_fixture_for(duration: Duration) {
        let deadline = Instant::now() + duration;
        while Instant::now() < deadline {
            unsafe {
                pump_fixture_messages();
            }
            thread::sleep(Duration::from_millis(10));
        }
    }

    /// Explicit operator fixture; all interaction is confined to our own synthetic windows.
    /// METIN_OVERLAY_FIXTURE_KIND=point|region, METIN_OVERLAY_FIXTURE_SECONDS=1..120,
    /// METIN_OVERLAY_FIXTURE_REPORT=<json path>, METIN_OVERLAY_FIXTURE_SOURCE=<bmp path>.
    #[test]
    #[ignore = "interactive native calibration fixture, never attaches to a game"]
    fn frozen_selection_visual_fixture() {
        metin_platform::enable_dpi();
        let (tx, _rx) = mpsc::sync_channel(2);
        let shared = Arc::new(Shared {
            shutdown: AtomicBool::new(false),
            hidden: AtomicBool::new(false),
            selecting: AtomicBool::new(false),
            annotation: AtomicUsize::new(0),
            control_panel: Mutex::new(None),
            panel_position: Mutex::new(Default::default()),
        });
        let seconds = std::env::var("METIN_OVERLAY_FIXTURE_SECONDS")
            .ok()
            .and_then(|value| value.parse::<u64>().ok())
            .unwrap_or(45)
            .clamp(1, 120);
        let kind = if std::env::var("METIN_OVERLAY_FIXTURE_KIND").as_deref() == Ok("point") {
            SelectionKind::Point
        } else {
            SelectionKind::Region
        };
        let frame = synthetic_frame();
        let (width, height) = (frame.width, frame.height);
        if let Ok(path) = std::env::var("METIN_OVERLAY_FIXTURE_SOURCE") {
            let pixels: Vec<u8> = frame
                .bgr
                .as_chunks::<3>()
                .0
                .iter()
                .flat_map(|p| [p[0], p[1], p[2], 0])
                .collect();
            let mut bmp = vec![0u8; 54];
            bmp[..2].copy_from_slice(b"BM");
            bmp[2..6].copy_from_slice(&(54 + pixels.len() as u32).to_le_bytes());
            bmp[10..14].copy_from_slice(&54u32.to_le_bytes());
            bmp[14..18].copy_from_slice(&40u32.to_le_bytes());
            bmp[18..22].copy_from_slice(&(width as i32).to_le_bytes());
            bmp[22..26].copy_from_slice(&(-(height as i32)).to_le_bytes());
            bmp[26..28].copy_from_slice(&1u16.to_le_bytes());
            bmp[28..30].copy_from_slice(&32u16.to_le_bytes());
            bmp.extend_from_slice(&pixels);
            std::fs::write(path, bmp).unwrap();
        }
        unsafe {
            let fixture = Worker::new(shared, tx).unwrap();
            SetWindowTextW(
                fixture.capture,
                wide("Companion native overlay QA target (synthetic pixels)").as_ptr(),
            );
            SetWindowPos(
                fixture.capture,
                HWND_TOPMOST,
                80,
                80,
                width as i32,
                height as i32,
                SWP_SHOWWINDOW,
            );
            if SetForegroundWindow(fixture.capture) == 0 {
                eprintln!(
                    "Windows declined fixture activation; activate the synthetic QA target manually."
                );
            }
            let activation_deadline = Instant::now() + Duration::from_secs(15);
            while GetForegroundWindow() != fixture.capture {
                pump_fixture_messages();
                assert!(
                    Instant::now() < activation_deadline,
                    "Synthetic QA target needs manual activation; no selection was started."
                );
                thread::sleep(Duration::from_millis(16));
            }
            let (mut created, mut exit, mut kernel, mut user) =
                (zeroed(), zeroed(), zeroed(), zeroed());
            assert_ne!(
                GetProcessTimes(
                    GetCurrentProcess(),
                    &mut created,
                    &mut exit,
                    &mut kernel,
                    &mut user
                ),
                0
            );
            let geometry = TargetState {
                target: metin_platform::TargetToken {
                    handle: fixture.capture as usize,
                    pid: GetCurrentProcessId(),
                    created: (u64::from(created.dwHighDateTime) << 32)
                        | u64::from(created.dwLowDateTime),
                },
                left: 80,
                top: 80,
                width,
                height,
                visible: true,
                foreground: true,
            };
            if std::env::var("METIN_OVERLAY_FIXTURE_MANUAL").as_deref() == Ok("1") {
                log_fixture_window_geometry("synthetic source before arm", fixture.capture);
                eprintln!("Click inside the synthetic QA target to open the frozen selection.");
                let mut preview = Selection::new(SelectionRequest {
                    id: 0,
                    label: "Synthetic QA image".into(),
                    geometry,
                    frame: frame.clone(),
                    kind,
                    initial_region: None,
                    context_regions: Vec::new(),
                    color_preview: None,
                });
                preview.controls_visible = false;
                let armed_until = Instant::now() + Duration::from_secs(90);
                loop {
                    pump_fixture_messages();
                    draw_window(fixture.capture, width, height, fixture.font.0, |dc| {
                        draw_selection(dc, &preview);
                        text(
                            dc,
                            [20, 20],
                            "SYNTHETIC QA IMAGE - click here to start selection",
                            WHITE,
                        );
                    });
                    let clicked = fixture
                        .selection_context
                        .events
                        .borrow_mut()
                        .drain(..)
                        .any(|event| matches!(event, WindowEvent::Up(_)));
                    if clicked && GetForegroundWindow() == fixture.capture {
                        break;
                    }
                    assert!(
                        Instant::now() < armed_until,
                        "Synthetic target was not armed."
                    );
                    thread::sleep(Duration::from_millis(16));
                }
            }
            // The fixture has no runtime or input executor. Its released state is vacuous.
            let overlay = GameOverlay::new().unwrap();
            let color_preview = if kind == SelectionKind::Point {
                let mut profile = metin_core::Profile::create("Synthetic color preview");
                profile.region = Some(Region {
                    left: 0.,
                    top: 0.,
                    right: 1.,
                    bottom: 1.,
                });
                Some(Box::new(profile))
            } else {
                None
            };
            overlay
                .begin_selection_after_release(SelectionRequest {
                    id: 700,
                    label: if kind == SelectionKind::Point {
                        "Metin color (synthetic QA fixture)"
                    } else {
                        "Aura — Active icon (synthetic QA fixture)"
                    }
                    .into(),
                    geometry,
                    frame,
                    kind,
                    initial_region: None,
                    context_regions: Vec::new(),
                    color_preview,
                })
                .unwrap();
            eprintln!(
                "Synthetic native {kind:?} fixture: drag/click, Enter or visible Confirm; Escape/F8 cancels; Tab hides controls. Timeout {seconds}s."
            );
            let deadline = Instant::now() + Duration::from_secs(seconds);
            let event = loop {
                pump_fixture_messages();
                if let Some(event) = overlay.poll() {
                    break event;
                }
                if Instant::now() >= deadline {
                    overlay.hide();
                }
                assert!(
                    Instant::now() < deadline + Duration::from_secs(3),
                    "Fixture failed to close after timeout"
                );
                thread::sleep(Duration::from_millis(16));
            };
            let report = match &event {
                OverlayEvent::PanelMoved { target, offset } => {
                    serde_json::json!({"result":"panel_moved","target":format!("{target:?}"),"offset":offset})
                }
                OverlayEvent::Selected { id, region, point } => {
                    serde_json::json!({"result":"selected","id":id,"kind":format!("{kind:?}"),"region":region,"point":point,"pixels":region.map(|r|r.pixels(width,height)),"width":width,"height":height})
                }
                OverlayEvent::Cancelled { id } => {
                    serde_json::json!({"result":"cancelled","id":id,"kind":format!("{kind:?}")})
                }
                OverlayEvent::Failed { id, message } => {
                    serde_json::json!({"result":"failed","id":id,"message":message})
                }
            };
            if let Ok(path) = std::env::var("METIN_OVERLAY_FIXTURE_REPORT") {
                std::fs::write(path, serde_json::to_vec_pretty(&report).unwrap()).unwrap();
            }
            eprintln!("Native fixture result: {report}");
            assert!(!matches!(event, OverlayEvent::Failed { .. }));
        }
    }

    unsafe fn pump_fixture_messages() {
        unsafe {
            let mut message = zeroed();
            for _ in 0..128 {
                if PeekMessageW(&mut message, null_mut(), 0, 0, PM_REMOVE) == 0 {
                    break;
                }
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
    }
    #[test]
    fn frozen_review_context_is_painted_over_restored_pixels_and_marks_outside_regions_red() {
        unsafe {
            let dc = CreateCompatibleDC(null_mut());
            assert!(!dc.is_null());
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: 803,
                    biHeight: -560,
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB,
                    ..zeroed()
                },
                ..zeroed()
            };
            let mut pixels = null_mut();
            let bitmap = CreateDIBSection(dc, &info, DIB_RGB_COLORS, &mut pixels, null_mut(), 0);
            assert!(!bitmap.is_null());
            let previous = SelectObject(dc, bitmap);
            let mut request = selection_fixture(SelectionKind::Region).request;
            request.initial_region =
                Some(selection_region([200, 150], [600, 400], 803, 560).unwrap());
            request.context_regions = vec![
                (
                    selection_region([240, 200], [360, 300], 803, 560).unwrap(),
                    "Preserved area".into(),
                ),
                (
                    Region {
                        left: 0.9,
                        top: 0.8,
                        right: 1.1,
                        bottom: 1.1,
                    },
                    "Outside area".into(),
                ),
            ];
            let mut selection = Selection::new(request);
            selection.controls_visible = false;
            draw_selection(dc, &selection);
            GdiFlush();
            assert_eq!(
                GetPixel(dc, 240, 200),
                GOLD,
                "Context inside the primary selection must survive restored source pixels"
            );
            assert_eq!(
                GetPixel(dc, 802, 500),
                RED,
                "The clipped out-of-bounds marker is actually painted red"
            );
            let offset = (350 * 803 + 500) * 3;
            let bgr = &selection.request.frame.bgr[offset..offset + 3];
            let original = u32::from(bgr[2]) | (u32::from(bgr[1]) << 8) | (u32::from(bgr[0]) << 16);
            assert_eq!(
                GetPixel(dc, 500, 350),
                original,
                "The selected source remains bright and unchanged"
            );
            assert_ne!(
                GetPixel(dc, 50, 50),
                original,
                "Pixels outside the selection are dimmed"
            );
            if let Some(path) = std::env::var_os("COMPANION_CONTEXT_RENDER_BMP") {
                let bytes = std::slice::from_raw_parts(pixels.cast::<u8>(), 803 * 560 * 4);
                let mut bmp = vec![0u8; 54];
                bmp[..2].copy_from_slice(b"BM");
                bmp[2..6].copy_from_slice(&(54 + bytes.len() as u32).to_le_bytes());
                bmp[10..14].copy_from_slice(&54u32.to_le_bytes());
                bmp[14..18].copy_from_slice(&40u32.to_le_bytes());
                bmp[18..22].copy_from_slice(&803i32.to_le_bytes());
                bmp[22..26].copy_from_slice(&(-560i32).to_le_bytes());
                bmp[26..28].copy_from_slice(&1u16.to_le_bytes());
                bmp[28..30].copy_from_slice(&32u16.to_le_bytes());
                bmp.extend_from_slice(bytes);
                std::fs::write(path, bmp).unwrap();
            }
            SelectObject(dc, previous);
            DeleteObject(bitmap);
            DeleteDC(dc);
        }
    }
}
