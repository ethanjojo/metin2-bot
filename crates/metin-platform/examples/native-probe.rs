//! Native acceptance fixture. Input is restricted to a child process created here.
#[cfg(windows)]
mod probe {
    use metin_core::{Action, ActionKind, Profile};
    use metin_platform::*;
    use std::{
        io::{BufRead, BufReader},
        mem::zeroed,
        path::Path,
        process::{Command, Stdio},
        ptr::{null, null_mut},
        time::{Duration, Instant},
    };
    use windows_sys::Win32::{
        Foundation::*,
        Graphics::Gdi::*,
        System::LibraryLoader::GetModuleHandleW,
        UI::{Input::KeyboardAndMouse::GetKeyState, WindowsAndMessaging::*},
    };
    struct State {
        cover: bool,
        events: Vec<serde_json::Value>,
    }
    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(Some(0)).collect()
    }
    unsafe extern "system" fn callback(
        window: HWND,
        message: u32,
        wparam: WPARAM,
        lparam: LPARAM,
    ) -> LRESULT {
        // SAFETY: the creating thread owns State until GetMessage exits. The
        // pointer is installed during WM_NCCREATE before any other use.
        unsafe {
            if message == WM_NCCREATE {
                let create = &*(lparam as *const CREATESTRUCTW);
                SetWindowLongPtrW(window, GWLP_USERDATA, create.lpCreateParams as isize);
            }
            let state = GetWindowLongPtrW(window, GWLP_USERDATA) as *mut State;
            if [
                WM_KEYDOWN,
                WM_KEYUP,
                WM_SYSKEYDOWN,
                WM_SYSKEYUP,
                WM_LBUTTONDOWN,
                WM_LBUTTONUP,
            ]
            .contains(&message)
                && !state.is_null()
            {
                let key = if [WM_LBUTTONDOWN, WM_LBUTTONUP].contains(&message) {
                    1
                } else {
                    wparam as i32
                };
                (*state).events.push(
                    serde_json::json!({"message":message,"key":wparam,"flags":lparam as u32,
                        "pressed":GetKeyState(key) < 0}),
                );
            }
            match message {
                WM_PAINT => {
                    let mut paint = zeroed();
                    let dc = BeginPaint(window, &mut paint);
                    let brush = CreateSolidBrush(40 | (70 << 8) | (100 << 16));
                    FillRect(dc, &paint.rcPaint, brush);
                    DeleteObject(brush);
                    let square = RECT {
                        left: 60,
                        top: 60,
                        right: 140,
                        bottom: 140,
                    };
                    let brush = CreateSolidBrush(if (*state).cover {
                        220 | (20 << 8) | (60 << 16)
                    } else {
                        220 | (170 << 8) | (30 << 16)
                    });
                    FillRect(dc, &square, brush);
                    DeleteObject(brush);
                    EndPaint(window, &paint);
                    0
                }
                WM_TIMER if wparam == 2 => {
                    InvalidateRect(window, std::ptr::null(), 0);
                    0
                }
                WM_TIMER | WM_CLOSE => {
                    DestroyWindow(window);
                    0
                }
                WM_DESTROY => {
                    PostQuitMessage(0);
                    0
                }
                _ => DefWindowProcW(window, message, wparam, lparam),
            }
        }
    }
    fn fixture(title: &str, report: &Path) -> std::result::Result<(), Box<dyn std::error::Error>> {
        enable_dpi();
        let mut state = State {
            events: vec![],
            cover: title.contains("OCCLUDER"),
        };
        let class = wide("Companion.NativeVerification");
        let title = wide(title);
        unsafe {
            let instance = GetModuleHandleW(null());
            let registration = WNDCLASSW {
                lpfnWndProc: Some(callback),
                hInstance: instance,
                lpszClassName: class.as_ptr(),
                ..zeroed()
            };
            if RegisterClassW(&registration) == 0 {
                return Err(std::io::Error::last_os_error().into());
            }
            let window = CreateWindowExW(
                0,
                class.as_ptr(),
                title.as_ptr(),
                WS_OVERLAPPEDWINDOW | WS_VISIBLE,
                100,
                100,
                600,
                420,
                null_mut(),
                null_mut(),
                instance,
                &mut state as *mut _ as *const _,
            );
            if window.is_null() {
                return Err(std::io::Error::last_os_error().into());
            }
            ShowWindow(window, SW_SHOW);
            SetForegroundWindow(window);
            SetTimer(window, 1, 60000, None);
            SetTimer(window, 2, 100, None);
            println!("ready");
            let mut message: MSG = zeroed();
            while GetMessageW(&mut message, null_mut(), 0, 0) > 0 {
                TranslateMessage(&message);
                DispatchMessageW(&message);
            }
        }
        std::fs::write(report, serde_json::to_vec_pretty(&state.events)?)?;
        Ok(())
    }
    fn run(report: &Path, input_only: bool) -> std::result::Result<(), Box<dyn std::error::Error>> {
        enable_dpi();
        let event_report = report.with_extension("events.json");
        let title = format!("Companion input test - CLICK HERE - {}", std::process::id());
        let mut child = Command::new(std::env::current_exe()?)
            .arg("--fixture")
            .arg(&title)
            .arg(&event_report)
            .stdout(Stdio::piped())
            .spawn()?;
        let mut ready = String::new();
        BufReader::new(child.stdout.take().ok_or("Missing child output")?).read_line(&mut ready)?;
        if ready.trim() != "ready" {
            return Err("Fixture did not create its window".into());
        }
        let mut backend = backend();
        let targets = backend.list_targets()?;
        let target = targets
            .into_iter()
            .find(|w| w.token.pid == child.id() && w.title == title)
            .ok_or("Fixture target was not found by exact PID and title")?;
        backend.attach(target.token)?;
        backend.request_activation()?;
        let focus_deadline = Instant::now() + Duration::from_secs(45);
        while !backend.inspect()?.foreground && Instant::now() < focus_deadline {
            std::thread::sleep(Duration::from_millis(50));
        }
        if !backend.inspect()?.foreground {
            return Err(
                "OS refused fixture foreground activation; native acceptance is inconclusive"
                    .into(),
            );
        }
        // Give the operator time to release the activation click and mouse.
        std::thread::sleep(Duration::from_secs(3));
        let stop = StopToken::default();
        let state = backend.inspect()?;
        let (width, height) = (state.width, state.height);
        if !input_only {
            let frame = backend.capture(Instant::now() + Duration::from_secs(2), &stop, 0)?;
            let offset = (80 * frame.frame.width + 80) as usize * 3;
            let pixel = &frame.frame.bgr[offset..offset + 3];
            if pixel != [30, 170, 220] {
                return Err(
                    format!("Captured pixels do not match rendered fixture: {pixel:?}").into(),
                );
            }
        }
        let mut profile = Profile::create("Isolated native acceptance");
        profile.frame_size = Some([width, height]);
        let mut visual_checks = 0;
        let mut verify = |backend: &mut dyn PlatformBackend| -> Result<()> {
            if input_only {
                return Ok(());
            }
            let fresh = backend.capture(Instant::now() + Duration::from_secs(2), &stop, 0)?;
            let offset = (80 * fresh.frame.width + 80) as usize * 3;
            if fresh.frame.bgr[offset..offset + 3] != [30, 170, 220] {
                return Err(PlatformError("Fixture changed before down".into()));
            }
            visual_checks += 1;
            Ok(())
        };
        let mut executor = InputExecutor::default();
        executor.execute_verified(
            &Action {
                id: 1,
                kind: ActionKind::Configured {
                    input: metin_core::ActionInput::Key {
                        shortcut: "Ctrl+G".into(),
                        hold_seconds: 0.1,
                    },
                },
            },
            &profile,
            backend.as_mut(),
            &stop,
            0,
            Some(&mut verify),
        )?;
        executor.execute_verified(
            &Action {
                id: 2,
                kind: ActionKind::Configured {
                    input: metin_core::ActionInput::Click {
                        region: Some(metin_core::Region {
                            left: 70. / f64::from(width),
                            top: 70. / f64::from(height),
                            right: 90. / f64::from(width),
                            bottom: 90. / f64::from(height),
                        }),
                    },
                },
            },
            &profile,
            backend.as_mut(),
            &stop,
            0,
            Some(&mut verify),
        )?;
        executor.execute(
            &Action {
                id: 3,
                kind: ActionKind::Attack {
                    shortcut: "Space".into(),
                    duration_seconds: 0.3,
                },
            },
            &profile,
            backend.as_mut(),
            &stop,
            0,
        )?;
        let mut held_captures = 0;
        while executor.attack_held() {
            executor.poll_attack(backend.as_mut(), &stop, 0)?;
            if let Some(budget) = executor
                .capture_budget()
                .filter(|_| !input_only && executor.attack_held())
            {
                if let Err(error) = backend.capture(Instant::now() + budget, &stop, 0) {
                    let _ = executor.release(backend.as_mut());
                    return Err(error.into());
                }
                held_captures += 1;
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        if !input_only && held_captures == 0 {
            return Err("No capture while native attack was held".into());
        }
        // The fixture is our own exact child PID/title; close it after every up
        // event has been sent. Never send WM_CLOSE to a selected game process.
        unsafe {
            PostMessageW(target.token.handle as HWND, WM_CLOSE, 0, 0);
        }
        if !child.wait()?.success() {
            return Err("Fixture exited unsuccessfully".into());
        }
        let events: Vec<serde_json::Value> =
            serde_json::from_slice(&std::fs::read(&event_report)?)?;
        let expected = [
            (WM_KEYDOWN, 0x11),
            (WM_KEYDOWN, 0x47),
            (WM_KEYUP, 0x47),
            (WM_KEYUP, 0x11),
            (WM_LBUTTONDOWN, 1),
            (WM_LBUTTONUP, 0),
            (WM_KEYDOWN, 0x20),
            (WM_KEYUP, 0x20),
        ];
        let actual: Vec<_> = events
            .iter()
            .map(|e| {
                (
                    e["message"].as_u64().unwrap_or(0) as u32,
                    e["key"].as_u64().unwrap_or(0) as usize,
                )
            })
            .collect();
        // The operator's activation click can precede the automated sequence.
        if !actual.ends_with(&expected) {
            return Err(format!("Native input order differs: {actual:?}").into());
        }
        let states: Vec<_> = events[events.len() - expected.len()..]
            .iter()
            .map(|event| event["pressed"].as_bool())
            .collect();
        if states != [true, true, false, false, true, false, true, false].map(Some) {
            return Err(format!(
                "Windows did not register actual pressed/released input: {states:?}"
            )
            .into());
        }
        std::fs::write(
            report,
            serde_json::to_vec_pretty(
                &serde_json::json!({"success":true,"capture_matches_rendered_pixels":!input_only,"captured_size":[width,height],"keyboard_and_mouse_order":true,"windows_input_state":true,"visual_pre_down_captures":visual_checks,"persistent_attack_and_release":true,"captures_during_attack":held_captures,"child_pid":child.id(),"game_input":false,"events":events}),
            )?,
        )?;
        Ok(())
    }

    fn capture_only(report: &Path) -> std::result::Result<(), Box<dyn std::error::Error>> {
        enable_dpi();
        let exe = std::env::current_exe()?;
        let title = format!("Companion capture fixture {}", std::process::id());
        let mut child = Command::new(&exe)
            .args(["--fixture", &title])
            .arg(report.with_extension("fixture.json"))
            .stdout(Stdio::piped())
            .spawn()?;
        let mut ready = String::new();
        BufReader::new(child.stdout.take().ok_or("Missing child output")?).read_line(&mut ready)?;
        let mut backend = backend();
        let target = backend
            .list_targets()?
            .into_iter()
            .find(|w| w.token.pid == child.id() && w.title == title)
            .ok_or("Capture fixture not found")?;
        backend.attach(target.token)?;
        // A second, different-colored exact child window fully covers the first.
        // No foreground activation, keyboard event or mouse input is required.
        let cover_title = format!("Companion OCCLUDER {}", std::process::id());
        let mut cover = Command::new(&exe)
            .args(["--fixture", &cover_title])
            .arg(report.with_extension("cover.json"))
            .stdout(Stdio::piped())
            .spawn()?;
        ready.clear();
        BufReader::new(cover.stdout.take().ok_or("Missing cover output")?).read_line(&mut ready)?;
        let cover_target = backend
            .list_targets()?
            .into_iter()
            .find(|w| w.token.pid == cover.id() && w.title == cover_title)
            .ok_or("Occluder not found")?;
        unsafe {
            SetWindowPos(
                cover_target.token.handle as HWND,
                HWND_TOPMOST,
                100,
                100,
                600,
                420,
                SWP_NOACTIVATE,
            );
        }
        std::thread::sleep(Duration::from_millis(400));
        let result = (|| -> std::result::Result<_, Box<dyn std::error::Error>> {
            let state = backend.inspect()?;
            // Establish the occlusion independently: client test point belongs
            // to the cover window in screen space, not to the captured target.
            let top = unsafe {
                WindowFromPoint(POINT {
                    x: state.left + 80,
                    y: state.top + 80,
                })
            };
            if top != cover_target.token.handle as HWND {
                return Err("Fixture was not actually occluded".into());
            }
            let stop = StopToken::default();
            let requested = Instant::now();
            let frame = backend.capture(Instant::now() + Duration::from_secs(3), &stop, 0)?;
            let offset = (80 * frame.frame.width + 80) as usize * 3;
            let pixel = &frame.frame.bgr[offset..offset + 3];
            if pixel != [30, 170, 220] {
                return Err(format!(
                    "Capture contains occluder or incorrect client offset: {pixel:?}"
                )
                .into());
            }
            if frame.captured_at < requested || frame.captured_at > Instant::now() {
                return Err("Initial frame source timestamp is not fresh".into());
            }
            std::thread::sleep(Duration::from_millis(450));
            let requested = Instant::now();
            let second = backend.capture(Instant::now() + Duration::from_secs(3), &stop, 0)?;
            if second.captured_at < requested {
                return Err("Old queued frame was falsely stamped as fresh".into());
            }
            unsafe {
                SetWindowPos(
                    target.token.handle as HWND,
                    std::ptr::null_mut(),
                    100,
                    100,
                    700,
                    460,
                    SWP_NOACTIVATE | SWP_NOZORDER,
                );
            }
            std::thread::sleep(Duration::from_millis(150));
            let resized = backend.capture(Instant::now() + Duration::from_secs(3), &stop, 0)?;
            let resized_offset = (80 * resized.frame.width + 80) as usize * 3;
            if resized.frame.width == frame.frame.width
                || resized.frame.bgr[resized_offset..resized_offset + 3] != [30, 170, 220]
            {
                return Err("Resized client capture differs from fixture".into());
            }
            let cancelled = StopToken::default();
            cancelled.stop();
            if backend
                .capture(Instant::now() + Duration::from_secs(1), &cancelled, 0)
                .is_ok()
            {
                return Err("Cancelled capture succeeded".into());
            }
            Ok(
                serde_json::json!({"success":true,"occluded_window_pixels_match":true,"source_time_fresh_after_queue_delay":true,"resize_matches_client":true,"client_size":[frame.frame.width,frame.frame.height],"cancellation":true,"game_input":false}),
            )
        })();
        unsafe {
            PostMessageW(target.token.handle as HWND, WM_CLOSE, 0, 0);
            PostMessageW(cover_target.token.handle as HWND, WM_CLOSE, 0, 0);
        }
        let _ = child.wait();
        let _ = cover.wait();
        std::fs::write(report, serde_json::to_vec_pretty(&result?)?)?;
        Ok(())
    }
    pub fn main() {
        let args: Vec<_> = std::env::args().collect();
        let result = if args.get(1).is_some_and(|a| a == "--fixture") {
            fixture(&args[2], Path::new(&args[3]))
        } else if args.get(1).is_some_and(|a| a == "--capture-only") {
            capture_only(Path::new(&args[2]))
        } else if args.get(1).is_some_and(|a| a == "--input-only") {
            run(Path::new(&args[2]), true)
        } else if let Some(path) = args.get(1) {
            run(Path::new(path), false)
        } else {
            Err("Usage: native-probe <report.json>".into())
        };
        if let Err(error) = result {
            let path = if matches!(
                args.get(1).map(String::as_str),
                Some("--input-only" | "--capture-only")
            ) {
                args.get(2)
            } else if args.get(1).is_some_and(|arg| !arg.starts_with("--")) {
                args.get(1)
            } else {
                None
            };
            if let Some(path) = path {
                let _ = std::fs::write(path, serde_json::to_vec_pretty(&serde_json::json!({"success":false,"error":error.to_string(),"game_input":false})).expect("error report serializes"));
            }
            eprintln!("Native verification failed: {error}");
            std::process::exit(1);
        }
    }
}
fn main() {
    #[cfg(windows)]
    probe::main();
    #[cfg(not(windows))]
    {
        eprintln!("Native probe currently verifies the Windows backend only.");
        std::process::exit(2);
    }
}
