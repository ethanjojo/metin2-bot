//! Explicit test of the production HID backend. F7 arms one input; F8 cancels.
#[cfg(windows)]
fn main() {
    use metin_platform::*;
    use std::time::{Duration, Instant};
    let args: Vec<_> = std::env::args().skip(1).collect();
    if args.len() != 3 || !["--key", "--cursor-click"].contains(&args[2].as_str()) {
        eprintln!("Usage: input-diagnostics <exact-pid> <report.json> --key|--cursor-click");
        std::process::exit(2);
    }
    let click = args[2] == "--cursor-click";
    let mut report = serde_json::json!({"success":false,"game_response_verified":false,"transport":"hidmaestro","click":click});
    let result = (|| -> std::result::Result<(), Box<dyn std::error::Error>> {
        enable_dpi();
        let pid = args[0].parse::<u32>()?;
        let mut backend = backend();
        let targets: Vec<_> = backend
            .list_targets()?
            .into_iter()
            .filter(|t| t.token.pid == pid)
            .collect();
        if targets.len() != 1 {
            return Err("Expected one visible window for the exact PID".into());
        }
        let target = &targets[0];
        report["pid"] = pid.into();
        report["created"] = target.token.created.into();
        backend.attach(target.token)?;
        backend.check_input_access()?;
        // Never programmatically activate the game: the operator selects the
        // actual window/control and arms the diagnostic with a physical F7.
        println!("Activate the target, hover its X for a click, then press F7. F8 cancels.");
        let timeout = Instant::now() + Duration::from_secs(120);
        loop {
            let hotkeys = backend.hotkeys()?;
            if hotkeys.stop {
                return Err("Cancelled by F8".into());
            }
            if hotkeys.toggle && backend.inspect()?.foreground {
                break;
            }
            if Instant::now() >= timeout {
                return Err("No F7 in the target before timeout".into());
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        let baseline = backend.inspect()?;
        let cursor = backend.cursor()?;
        let point = [cursor[0] - baseline.left, cursor[1] - baseline.top];
        if click
            && (point[0] < 0
                || point[1] < 0
                || point[0] >= baseline.width as i32
                || point[1] >= baseline.height as i32
                || !backend.point_clear(cursor))
        {
            return Err("Cursor is outside or obstructing the selected client".into());
        }
        let guard = |backend: &mut dyn PlatformBackend| -> Result<()> {
            if backend.emergency_pressed()? {
                return Err(PlatformError("Cancelled by F8".into()));
            }
            if backend.inspect()? != baseline || !baseline.foreground || !baseline.visible {
                return Err(PlatformError("Target/focus/geometry changed".into()));
            }
            if click && (backend.cursor()? != cursor || !backend.point_clear(cursor)) {
                return Err(PlatformError("Cursor moved or became obstructed".into()));
            }
            Ok(())
        };
        let settle = Instant::now() + Duration::from_secs(3);
        while Instant::now() < settle {
            guard(backend.as_mut())?;
            std::thread::sleep(Duration::from_millis(10));
        }
        let point = [point[0].max(0) as u32, point[1].max(0) as u32];
        let down = if click {
            backend.mouse(point, true)
        } else {
            backend.key(0x1b, true, false)
        };
        let held = (|| -> Result<()> {
            down?;
            let until = Instant::now() + Duration::from_millis(250);
            while Instant::now() < until {
                guard(backend.as_mut())?;
                std::thread::sleep(Duration::from_millis(10));
            }
            report["windows_pressed"] = backend.key_held(if click { 1 } else { 0x1b }).into();
            Ok(())
        })();
        // Release even when press, hold or target validation failed.
        let released = if click {
            backend.mouse(point, false)
        } else {
            backend.key(0x1b, false, false)
        };
        held?;
        released?;
        std::thread::sleep(Duration::from_millis(100));
        report["windows_released"] = (!backend.key_held(if click { 1 } else { 0x1b })).into();
        Ok(())
    })();
    match &result {
        Ok(()) => report["success"] = true.into(),
        Err(error) => report["error"] = error.to_string().into(),
    }
    if let Err(error) = std::fs::write(
        &args[1],
        serde_json::to_vec_pretty(&report).expect("report serializes"),
    ) {
        eprintln!("{error}");
        std::process::exit(1);
    }
    println!("{report}");
    if result.is_err() {
        std::process::exit(1);
    }
}
#[cfg(not(windows))]
fn main() {
    eprintln!("Windows only");
    std::process::exit(2);
}
