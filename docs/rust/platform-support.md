# Platform status

| Platform | GUI/core/vision | Capture and input | Acceptance remaining |
| --- | --- | --- | --- |
| Windows x64, Windows 10 2004 / build 19041 or newer | 98 tests, fmt, strict Clippy and debug executable self-test passed; native GPU screenshots in both themes | HWND Windows Graphics Capture; own-child probe verifies capture through occlusion, source freshness, resize and cancellation. Earlier input fixture verified Ctrl+G and mouse down/up | Real-client cycle and clean-machine installation/upgrade/uninstall; latest packaged UAC launch |
| Linux X11 | Native Ubuntu WSL2 build, 97 tests, fmt, strict Clippy and executable self-test passed; real WSLg GUI rendered with Vulkan llvmpipe | Unavailable; Start blocked by capabilities | X11 backend, Xvfb input fixture and Wine client acceptance (T10); hardware GPU/HiDPI verification |
| macOS | Portable source and native CI configuration prepared | Unavailable; Start blocked by capabilities | Native build/editor, permissions/Retina/backend and compatible-client acceptance (T11) |
| Linux Wayland | Same portable GUI | Unavailable; no implicit XWayland/root fallback | Establish all required identity/focus/obstruction/manual-input/stop guards before implementation (T12) |

An unavailable backend is intentional and visible; it is not a supported gameplay backend. The common controller, immutable BGR frames, UUID profiles and runtime worker do not depend on egui or native window handles. Native key transport currently uses Win32 virtual key codes inside the platform crate; introduce backend-specific translation when implementing T10/T11 rather than exporting those codes into core configuration.

Windows capture requires Windows Graphics Capture support and a D3D11 device. The installer enforces build 19041, matching the cursor-exclusion API used by this path. Capture never falls back to desktop pixels: overlapping applications must not enter recognition. Failure is reported visibly. Foreground/visibility guards still apply to automated game input. See Microsoft's [HWND capture API](https://learn.microsoft.com/windows/win32/api/windows.graphics.capture.interop/nf-windows-graphics-capture-interop-igraphicscaptureiteminterop-createforwindow) and [cursor capture property](https://learn.microsoft.com/uwp/api/windows.graphics.capture.graphicscapturesession.iscursorcaptureenabled).
