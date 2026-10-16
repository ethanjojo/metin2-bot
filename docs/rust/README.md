# Rust desktop migration

The Rust application is implemented in `crates/metin-desktop` (egui/eframe), with a platform-independent controller in `metin-core`, image processing in `metin-vision`, and native integrations in `metin-platform`.

This remains a migration preview until the acceptance gates in [acceptance.md](acceptance.md) pass. The existing Python launcher and release pipeline remain the production path during this explicitly temporary migration period. Remove them under T9 of the approved plan after Windows upgrade/game acceptance and native Linux/macOS build/editor verification; do not maintain both implementations after cutover.

## Windows development

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

Package with `tools/package-rust-windows.ps1 -CrtDirectory <VS redistributable CRT folder> -CompilerPath <ISCC.exe>`. The CRT directory must contain `msvcp140.dll`, `vcruntime140.dll`, `vcruntime140_1.dll`, and `concrt140.dll` (required by the vendor OpenCV build). The script builds the locked release, stages a fresh self-contained folder, records file hashes and dependency notices, and produces a ZIP, installer, and SHA256SUMS. `-SkipBuild` is only for packaging a release already verified in the same checkout. Inno Setup version validation and the existing installer AppId/data-retention contract are shared with the current distribution.

## Linux and macOS preparation

`tools/prepare-rust-native.sh` builds pinned OpenCV source locally. Install the native C++/Clang/CMake/pkg-config and GUI dependencies listed in `.github/workflows/rust-ci.yml`; export `PKG_CONFIG_PATH` to the resulting OpenCV `lib/pkgconfig`. On macOS also set `LIBCLANG_PATH` to LLVM's `lib` directory. Build and test with the same Cargo commands.

CI has a native Windows/Linux/macOS matrix. Creating the workflow is not proof that it passed: no remote execution is claimed yet. Linux release build, 61 tests, fmt, strict Clippy and an actual editor window have been verified locally under Ubuntu 24.04 WSL2/WSLg (software Vulkan llvmpipe); macOS remains unverified. Linux/macOS currently expose profile editing and fixture-based vision; their automation capability report explicitly disables Start because capture/input backends have not passed native-host acceptance. T10–T12 cover X11, macOS and conditional Wayland implementations.

## Regression evidence

The immutable fixtures under `tests/parity` come from the Python baseline. Temporary `tools/export_*_parity.py` scripts regenerate them while Python is still available. Rust tests replay 648 controller events, 84 buff observations, image/tracker calls, and 327 schema boundary cases. These supplement direct Rust tests for input guards, storage, migration, editor gestures, UI forms and runtime cancellation. They do not assert that every original UI/packaging test has been ported.

`tools/benchmark-python-reference.py` and `cargo run --release -p metin-vision --example benchmark` use the same two fixture workloads, with ten warmups and three runs of 100 samples. They measure detection and panel matching, not capture latency or a complete gameplay cycle.

## Desktop interface

The application now separates Run, Calibration, Buffs and Settings. Run shows activity and actionable Start readiness; Calibration owns the shared image editor; Buffs presents ordered skills with a selected detail and an explicit return from icon capture; Settings groups the existing fields. Painter-drawn icons do not require an icon font. See [UI verification and review scenarios](ui-verification.md).
