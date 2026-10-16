# Rust desktop migration

The Rust application is implemented in `crates/metin-desktop` (egui/eframe), with a platform-independent controller in `metin-core`, image processing in `metin-vision`, and native integrations in `metin-platform`.

Rust is the only current application/release path. This is a Windows prerelease pending the checks in [../release-readiness.md](../release-readiness.md). The Python application, launcher and PyInstaller build have been removed; legacy profile import remains supported. Historical source is preserved in Git.

## Windows development

Application branding, the shared publisher identity, icon regeneration and the UAC signing limitation are documented in [app-branding.md](app-branding.md).

Latest UI follow-up: [calibration toolbar and buff capture](ui-calibration-toolbar.md), including the independent pixel-size toggle, aligned trash buttons, explicit capture return and Windows verification evidence.

Install Rust 1.99.0 (the workspace pins its rustfmt/clippy components), Visual Studio C++ tools/Windows SDK, and 7-Zip. From the repository root in PowerShell:

```powershell
./tools/prepare-rust-native.ps1
. ./tools/rust-env.ps1
cargo test --workspace --all-targets --locked
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo run -p metin-desktop -- --data-dir build/rust-dev-data
```

The explicit data directory isolates development from existing profiles. Omit it only when deliberately using canonical application data. `--import-data <old-root>` copies validated profiles/settings into an empty destination and preserves originals. A legacy `config.json` in the canonical data folder is converted idempotently, with its backup retained.

For a release build, use `cargo build --workspace --release --locked`. Release Windows binaries request administrator rights, matching the existing application. The platform also verifies the selected process's access before input. Diagnostics: `metinbot --self-test --self-test-report <output.json>`; this exercises storage, vision and headless egui, without game input. It does not replace GPU/window or real-client acceptance.

Package from a clean committed checkout with `tools/package-rust-windows.ps1 -Version 0.1.0-rc.1 -BuildNumber 1 -CrtDirectory <VS redistributable CRT folder> -CompilerPath <ISCC.exe> -PythonPath .venv/Scripts/python.exe`. The CRT directory must contain `msvcp140.dll`, `vcruntime140.dll`, `vcruntime140_1.dll`, and `concrt140.dll` (required by the vendor OpenCV build). The script builds the locked release, stages a fresh self-contained folder, records file hashes and dependency notices, and produces a ZIP, installer, and SHA256SUMS. Inno Setup version validation and the existing installer AppId/data-retention contract are shared with the current distribution.

## Linux and macOS preparation

`tools/prepare-rust-native.sh` builds pinned OpenCV source locally. Install the native C++/Clang/CMake/pkg-config and GUI dependencies listed in `.github/workflows/rust-ci.yml`; export `PKG_CONFIG_PATH` to the resulting OpenCV `lib/pkgconfig`. On macOS also set `LIBCLANG_PATH` to LLVM's `lib` directory. Build and test with the same Cargo commands.

CI has a native Windows/Linux/macOS matrix. Creating the workflow is not proof that it passed: no remote execution is claimed yet. Linux release build, current regression tests, fmt, strict Clippy and an actual editor window have been verified locally under Ubuntu 24.04 WSL2/WSLg (software Vulkan llvmpipe); macOS remains unverified. Linux/macOS currently expose profile editing and fixture-based vision; their automation capability report explicitly disables Start because capture/input backends have not passed native-host acceptance. T10–T12 cover X11, macOS and conditional Wayland implementations.

## Regression evidence

The immutable fixtures under `tests/parity` come from the historical Python baseline. Regeneration/export tools are retained in that baseline commit, not in the current release path. Rust tests replay 648 controller events, 84 buff observations, image/tracker calls, and 327 schema boundary cases. These supplement direct Rust tests for input guards, storage, migration, editor gestures, UI forms and runtime cancellation. They do not assert that every original UI/packaging test has been ported.

`cargo run --release -p metin-vision --example benchmark` measures fixture detection and panel matching, not capture latency or a complete gameplay cycle. Historical Python comparison results remain in the earlier performance report.

## Desktop interface

For Linux launcher/icon installation and macOS `.app` packaging, see [native desktop integration](unix-desktop-integration.md), including platform-specific verification limits.

The application now separates Run, Calibration, Buffs and Settings. Run shows activity and actionable Start readiness; Calibration owns the shared image editor; Buffs presents ordered skills with a selected detail and an explicit return from icon capture; Settings groups the existing fields. Painter-drawn icons do not require an icon font. See [UI verification and review scenarios](ui-verification.md).
