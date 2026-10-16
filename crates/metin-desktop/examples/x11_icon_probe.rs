#[cfg(target_os = "linux")]
fn main() {
    use winit::event_loop::EventLoop;
    use winit::platform::x11::{EventLoopBuilderExtX11, WindowAttributesExtX11};
    use winit::raw_window_handle::{HasWindowHandle, RawWindowHandle};
    use winit::window::{Icon, Window};
    let event_loop = EventLoop::builder().with_x11().build().unwrap();
    #[allow(deprecated)] // A standalone property probe, no application event loop is needed.
    let window = event_loop
        .create_window(
            Window::default_attributes()
                .with_visible(false)
                .with_name(metin_desktop::APP_ID, ""),
        )
        .unwrap();
    let id = match window.window_handle().unwrap().as_raw() {
        RawWindowHandle::Xlib(handle) => handle.window.to_string(),
        RawWindowHandle::Xcb(handle) => handle.window.get().to_string(),
        _ => panic!("The X11 probe requires an X11 window"),
    };
    let property = |name: &str| {
        let mut command = std::process::Command::new("xprop");
        command.args(["-id", &id, "-len", "1048576"]);
        if name == "_NET_WM_ICON" {
            command.args(["-f", name, "32c"]);
        }
        let output = command.args(["-notype", name]).output().unwrap();
        assert!(output.status.success());
        String::from_utf8(output.stdout).unwrap()
    };
    assert!(property("WM_CLASS").contains(metin_desktop::APP_ID));
    for dark in [false, true, false] {
        let icon = metin_desktop::app_icon(dark).unwrap();
        let expected: Vec<u32> = [icon.width, icon.height]
            .into_iter()
            .chain(
                icon.rgba
                    .as_chunks::<4>()
                    .0
                    .iter()
                    .map(|p| u32::from_be_bytes([p[3], p[0], p[1], p[2]])),
            )
            .collect();
        window.set_window_icon(Some(
            Icon::from_rgba(icon.rgba, icon.width, icon.height).unwrap(),
        ));
        let mut matched = false;
        for _ in 0..50 {
            let value = property("_NET_WM_ICON");
            let actual: Vec<u32> = value
                .split_once('=')
                .map(|(_, values)| {
                    values
                        .split(',')
                        .filter_map(|v| v.trim().parse().ok())
                        .collect()
                })
                .unwrap_or_default();
            if actual == expected {
                matched = true;
                break;
            }
            std::thread::sleep(std::time::Duration::from_millis(10));
        }
        assert!(matched, "X11 _NET_WM_ICON did not match the selected theme");
    }
}

#[cfg(not(target_os = "linux"))]
fn main() {}
