# Platform support

| Platform | Current status | Remaining acceptance |
| --- | --- | --- |
| Windows x64, Windows 10 2004/build 19041 or newer | Rust prerelease; native HWND capture/input and guards implemented, 122 regression tests; GPU UI and isolated child-process transport verified | Real-client cycle, exact-release packaged self-test and hosted clean install/upgrade/uninstall |
| Linux X11 | Editor/vision preview; 120 regression tests, native build/self-test, X11 icon pixel property probe and WSLg UI verified | Capture/input backend, compatible-client acceptance and hardware/HiDPI testing |
| Linux Wayland | Editor/vision preview; WSLg GUI and app_id protocol verified; installed desktop/hicolor icon integration | Capture/input capability design/backend and native shell appearance acceptance |
| macOS | Portable editor source, .app/ICNS packaging and AppKit Dock icon implementation; icon module cross-checks for Intel/Apple Silicon | Whole native build/runtime, Dock/Finder/Retina/permissions and capture/input backend |

Linux/macOS Start remains blocked by capabilities. Building an editor or installing a logo does not establish gameplay support. Linux WSLg rendering uses llvmpipe software Vulkan and is not hardware performance evidence.

Windows capture requires Windows Graphics Capture and D3D11. It captures the selected HWND and never falls back to desktop pixels. PID/creation identity, foreground/geometry, privileges, obstruction/manual movement and emergency-stop guards protect automated input. Invalid or unavailable capture is reported visibly.

Current release gates: [release readiness](../release-readiness.md). Native icon/package implementation and evidence: [desktop integration](unix-desktop-integration.md). Earlier reports contain historical counts/artifact identities.
