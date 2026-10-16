fn main() {
    #[cfg(all(target_os = "macos", feature = "ui-smoke"))]
    for dark in [false, true, false] {
        metin_desktop::dock_icon::set(dark).expect("set Dock icon on main thread");
        assert!(metin_desktop::dock_icon::matches(dark).expect("read actual Dock icon"));
        assert!(!metin_desktop::dock_icon::matches(!dark).expect("reject opposite theme"));
    }
}
