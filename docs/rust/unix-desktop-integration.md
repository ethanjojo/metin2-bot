# Linux and macOS application identity

`packaging/app-identity.json` is the source of the application ID (`app.jojo.MetinBot`), product and publisher. The build validates and embeds the ID; packaging reads the same file. It is also the Wayland app_id, Linux desktop filename/icon name and macOS CFBundleIdentifier. With both Linux backends enabled, egui-winit's Wayland-named with_name call writes the shared winit platform ApplicationName, so it also sets the X11 WM_CLASS class to the app_id (instance is empty). StartupWMClass therefore uses the app_id too. No native window-builder hook or executable-name fallback is needed.

## Runtime icons

- Windows uses the existing shared themed PNG for both window and taskbar APIs.
- X11 uses the same themed window icon through egui/winit; WM_CLASS associates the window with the installed launcher.
- Wayland shell icons come from the installed desktop entry and hicolor icons. The compositor controls shell appearance. A high-contrast static icon is installed, rather than promising arbitrary runtime black/white shell icon changes unsupported by the pinned winit. The app UI still follows the selected theme.
- macOS Finder/Launchpad use the bundle ICNS. At native GUI creation and theme changes, the shared theme application path also sets NSApplication.applicationIconImage to the same embedded themed PNG. AppKit owns the retained image. Calls require the main thread. Headless egui tests do not initialize AppKit.

Static launcher/file icons retain their contrast contour on every platform. The pure black/white variants are the running application's theme icons.

## Build and install

Build on the destination OS after preparing OpenCV as described in README:

```sh
cargo build --workspace --release --locked
```

Linux:

```sh
python3 tools/package-rust-unix.py --binary target/release/metinbot \
  --native-lib build/rust-native/opencv/lib --output build/linux-package
sh build/linux-package/install.sh "$HOME/.local"
```

Ensure `$HOME/.local/bin` is on PATH (standard desktop setup); then launch MetinBot from the application menu. For a custom prefix its `share` directory must also be in XDG_DATA_DIRS. Starting an uninstalled executable on Wayland cannot provide its own launcher icon. The package contains bin/lib/share, native OpenCV shared libraries and an installer. Its executable wrapper resolves its own installation prefix, preserves arguments and sets a package-local library search path. Linux system libraries remain distribution prerequisites; this is a native distribution package, not a universal static binary. Test/ship on the oldest supported distribution. Existing desktop/icon cache utilities are called after installation when available.

macOS:

```sh
python3 tools/package-rust-unix.py --binary target/release/metinbot \
  --native-lib build/rust-native/opencv/lib --output build/MetinBot.app
```

Move `MetinBot.app` to Applications. The bundle includes Info.plist, executable, ICNS and recursively resolved non-system dylibs. Load commands are relocated to bundle-local paths. Missing libraries/name collisions fail packaging. Build separately for Intel and Apple Silicon; no universal binary is implied. After relocation, libraries and the bundle receive ad-hoc signatures for local execution on Apple Silicon. This is not Developer ID signing/notarization, and does not establish trusted publisher identity or remove Gatekeeper restrictions on downloaded apps.

Output must be a fresh directory; the packager never deletes an existing app or user data. Python is only a build dependency.

## Verification and remaining native acceptance

Local Windows verification: 122 Rust tests (including four branding/asset tests and 55 UI tests), strict Clippy, formatting, native build, executable self-test and five existing Python metadata tests. The AppKit production module and native readback helper were cross-checked for both `aarch64-apple-darwin` and `x86_64-apple-darwin`, including strict Clippy on ARM, in an isolated harness without OpenCV. This proves compilation of that module, not execution on macOS or a full macOS application build.

Linux Ubuntu WSL verification: 120 native Rust workspace tests, formatting, strict Clippy and the actual X11 property probe passed using the existing isolated environment in `build/rust-linux`. Five Python packaging tests passed, including executing the generated installer/launcher in a prefix containing spaces and building the system hicolor cache. A full debug package was also staged and installed into `build/unix-icons-linux-installed app`; desktop-file-validate and its real executable self-test passed with development LD_LIBRARY_PATH removed. Native WSLg GPU screenshots from that package were saved at `build/unix-icons-linux-gui/{light,dark,x11}.png`. Light/dark Wayland screenshots were visually inspected and protocol logs confirm `set_app_id("app.jojo.MetinBot")` for both. WSLg uses llvmpipe software Vulkan, not a hardware performance test. These screenshots contain application content, not shell/taskbar chrome.

The actual packaged main X11 window was also opened briefly with synthetic data: xprop read `WM_CLASS(STRING) = "", "app.jojo.MetinBot"` (`build/rust-linux/unix-icons-main-class.log`). Its X11 GPU screenshot was inspected too. The probe uses that same class/empty-instance combination. Earlier probe attempts exposed the removed winit xlib_window method and an invalid assumption that eframe's window_builder hook receives native attributes; both were corrected using the pinned sources and native execution. A transient executable-name launcher match was replaced with the verified app_id before delivery.

Asset/plist tests also run on Windows. Mocked macOS tool tests exercise recursive dylib resolution, relocation, signature ordering and missing-dependency failure; they do not substitute for macOS execution. Logs: `build/rust-linux/unix-icons-{tests,clippy,x11,gui-build,light,dark}.log` and `unix-icons-packaged-selftest.json`; Windows/module logs: `build/rust-migration/unix-icons-{tests,clippy,build,macos-check,macos-intel-check,macos-clippy}.log`, `unix-icons-selftest.json`. Windows temp-directory permissions required an explicit project-local TEMP/TMP for tests/self-test; the known post-Clippy rlib cache issue required cleaning only metin-desktop before building. No application workaround was added.

CI now builds on Windows/Linux/macOS and adds:

- X11 native winit probe reads WM_CLASS and the complete `_NET_WM_ICON` property for light → dark → light, comparing all ARGB pixels.
- macOS main-thread AppKit probe sets light → dark → light, reads back applicationIconImage as TIFF and rejects the opposite theme.
- Linux desktop-file validation, installation into a prefix with spaces and packaged self-test without development LD_LIBRARY_PATH.
- macOS plist/signature verification and packaged self-test without development DYLD_LIBRARY_PATH.
- Wayland headless Weston fixture rendering, protocol assertion for set_app_id and screenshot artifact, in addition to the existing native GPU screenshot.

These new CI jobs have not run for the local uncommitted tree. No native macOS machine is attached; local Wayland tests used WSLg rather than Weston. Actual Dock/Finder appearance, X11 taskbar appearance and Wayland launcher appearance still require native shell visual acceptance. Protocol/property tests establish integration contracts, not a screenshot of every desktop shell. Do not label all-platform visual acceptance complete until the remaining native checks have executed and been inspected. No release has been pushed/published by this change.
