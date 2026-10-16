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
        Foundation::*, Graphics::Gdi::*, System::LibraryLoader::GetModuleHandleW,
        UI::WindowsAndMessaging::*,
    };
    struct State {
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
                (*state).events.push(
                    serde_json::json!({"message":message,"key":wparam,"flags":lparam as u32}),
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
                    let brush = CreateSolidBrush(220 | (170 << 8) | (30 << 16));
                    FillRect(dc, &square, brush);
                    DeleteObject(brush);
                    EndPaint(window, &paint);
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
        let mut state = State { events: vec![] };
        let class = wide("MetinBot.NativeVerification");
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
    fn run(report: &Path) -> std::result::Result<(), Box<dyn std::error::Error>> {
        enable_dpi();
        let event_report = report.with_extension("events.json");
        let title = format!("MetinBot input test - CLICK HERE - {}", std::process::id());
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
        let frame = backend.capture(Instant::now() + Duration::from_secs(2), &stop, 0)?;
        let offset = (80 * frame.frame.width + 80) as usize * 3;
        let pixel = &frame.frame.bgr[offset..offset + 3];
        if pixel != [30, 170, 220] {
            return Err(format!("Captured pixels do not match rendered fixture: {pixel:?}").into());
        }
        let profile = Profile::create("Isolated native acceptance");
        let mut executor = InputExecutor::default();
        executor.execute(
            &Action {
                id: 1,
                kind: ActionKind::Skill {
                    shortcut: "Ctrl+G".into(),
                    index: 0,
                },
            },
            &profile,
            backend.as_mut(),
            &stop,
            0,
        )?;
        executor.execute(
            &Action {
                id: 2,
                kind: ActionKind::Click { point: [80, 80] },
            },
            &profile,
            backend.as_mut(),
            &stop,
            0,
        )?;
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
        std::fs::write(
            report,
            serde_json::to_vec_pretty(
                &serde_json::json!({"success":true,"capture_matches_rendered_pixels":true,"captured_size":[frame.frame.width,frame.frame.height],"keyboard_and_mouse_order":true,"child_pid":child.id(),"game_input":false,"events":events}),
            )?,
        )?;
        Ok(())
    }
    pub fn main() {
        let args: Vec<_> = std::env::args().collect();
        let result = if args.get(1).is_some_and(|a| a == "--fixture") {
            fixture(&args[2], Path::new(&args[3]))
        } else if let Some(path) = args.get(1) {
            run(Path::new(path))
        } else {
            Err("Usage: native-probe <report.json>".into())
        };
        if let Err(error) = result {
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
