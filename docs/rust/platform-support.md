# Platform status

| Platform | GUI/core/vision | Capture and input | Acceptance remaining |
| --- | --- | --- | --- |
| Windows x64 | Built and tested locally; GPU screenshots in both themes; packaged UAC self-test passed | Native backend; own-child fixture verified actual pixels, Ctrl+G down/up and mouse down/up | Real-client cycle and clean-machine installation/upgrade/uninstall |
| Linux X11 | Native Ubuntu WSL2 build, 61 tests, fmt, strict Clippy and executable self-test passed; real WSLg GUI rendered with Vulkan llvmpipe | Unavailable; Start blocked by capabilities | X11 backend, Xvfb input fixture and Wine client acceptance (T10); hardware GPU/HiDPI verification |
| macOS | Portable source and native CI configuration prepared | Unavailable; Start blocked by capabilities | Native build/editor, permissions/Retina/backend and compatible-client acceptance (T11) |
| Linux Wayland | Same portable GUI | Unavailable; no implicit XWayland/root fallback | Establish all required identity/focus/obstruction/manual-input/stop guards before implementation (T12) |

An unavailable backend is intentional and visible; it is not a supported gameplay backend. The common controller, immutable BGR frames, UUID profiles and runtime worker do not depend on egui or native window handles. Native key transport currently uses Win32 virtual key codes inside the platform crate; introduce backend-specific translation when implementing T10/T11 rather than exporting those codes into core configuration.
