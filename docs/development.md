# Development

MetinBot is a Rust workspace with an egui desktop interface and OpenCV image processing. The toolchain is pinned in `rust-toolchain.toml`; dependency versions are recorded in `Cargo.lock`.

## Windows

Install Rust 1.99.0, Visual Studio C++ build tools and the Windows SDK, PowerShell 7 and 7-Zip. Prepare native dependencies, then build with the configured environment:

```powershell
./tools/prepare-rust-native.ps1
. ./tools/rust-env.ps1
cargo build --workspace --locked
cargo run -p metin-desktop -- --data-dir build/dev-data
```

Always use a separate development data directory. `run.cmd` launches an existing release build with the normal administrator prompt.

## Linux and macOS

Use a native Rust 1.99.0 toolchain, C++ compiler, CMake, pkg-config and Clang/libclang. Linux additionally needs X11/Wayland, xkbcommon, EGL and PNG development libraries; macOS needs Xcode command-line tools. The precise CI prerequisites are in [rust-ci.yml](../.github/workflows/rust-ci.yml).

```sh
bash tools/prepare-rust-native.sh
export PKG_CONFIG_PATH="$PWD/build/rust-native/opencv/lib/pkgconfig"
export LD_LIBRARY_PATH="$PWD/build/rust-native/opencv/lib"
# macOS instead uses DYLD_LIBRARY_PATH and LIBCLANG_PATH for the installed LLVM.
cargo build --workspace --locked
cargo run -p metin-desktop -- --data-dir build/dev-data
```

The native dependency script builds the configured OpenCV version into `build/`. On macOS, set `DYLD_LIBRARY_PATH` to that OpenCV library directory and `LIBCLANG_PATH` to `$(brew --prefix llvm)/lib`. Capture/input are unavailable on these platforms; see [platform support](platform-support.md).

## Checks

Run from the repository root with native dependencies configured:

```sh
cargo fmt --all -- --check
cargo test --workspace --all-targets --all-features --locked
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
```

Build and packaging verification helpers use Python 3. Install `requirements-build.txt` into a virtual environment, then run:

```sh
python -m unittest discover -s tests -v
```

On Windows, use `.venv/Scripts/python.exe` if the environment is not activated. On Linux/macOS, use the corresponding virtual-environment executable. These helpers are development tools and are not required to run the app.

Tests cover profile validation/import, targeting, scheduling, recognition fixtures, editor state, platform guards and packaging contracts. A self-test checks storage, vision and headless theme rendering without sending game input:

```powershell
./target/debug/metinbot.exe --self-test --self-test-report build/selftest.json
```

Use `target/debug/metinbot` on Linux/macOS. Self-test does not establish live-game behavior or clean-machine installation.

## Documentation screenshots

On Windows, run `./tools/capture-docs.ps1` from a normal interactive desktop. It builds the `ui-smoke` feature, generates fresh isolated demonstration data with the `docs_fixture` example, and captures thirteen native views (including Run in both themes and the small event-history dialog) under `docs/assets/`. The scene is a geometric fixture, not a game screenshot. Logs and temporary profiles stay under ignored `build/docs-capture/`; existing user data is never used.

Review every generated image before committing. Keep labels neutral, avoid personal/server information and retain the demonstration caption in the README. The `ui-smoke` feature is for verification and is not enabled in release packages.

## Configured action data

Periodic actions and visual-rule execution share the tagged `ActionInput` schema: `key` contains `shortcut` and `hold_seconds`; `click` contains an optional calibrated `region`. Periodic actions store `input`, `interval_seconds`, `wait_seconds` and `first_run` (`after_interval` or `immediate`). The old periodic `timing` object is read once as immediate first use; serialization writes only the new schema and mixed formats are rejected. Puller timing remains separate.

Visual rules appear in Actions, while their stored `safety.rules` location remains the saved-profile compatibility contract. Each rule's `execution` stores input, verification (`reference_gone` or `delay`), wait, timeout, total attempts and `after_success` (`pause` or `continue`). Default execution verifies disappearance and pauses. Deterministic tests establish controller and guard behavior; fixture captures do not establish live-game or native macOS acceptance.

`Command::Connect` attaches a profile/window session independently of draft validation, so mount can be confirmed without a Snapshot or completed profile. `Command::Configure` applies a valid profile and never declares mount state. A new `SessionKey` starts unknown; `DeclareMount` explicitly confirms that session, and same-session reconfiguration preserves it. UI settings read and discard the obsolete `last_mounted` hint, and never write it again.

Paused capture is demand-driven: `SetPreview` subscribes the current profile/window, and `Snapshot` requests one fresh frame. Run and Calibration live preview publish demand; other paused pages need no continuous capture. Input cancellation generations also retire queued capture requests, so the UI reasserts preview demand after a stop. Active automation always samples at the profile's configured cadence.

`ProfileStore::save` returns `SaveOutcome`: an error means the profile did not commit; a successful outcome contains the committed profile and an optional obsolete-template cleanup warning. UI state/runtime configuration still update after a committed save with a warning.

## Repository boundaries

Commit sources, locked dependencies, reusable tests, icon assets and current documentation. Keep profiles, settings, logs, downloaded native tools and build/package output out of Git. `.gitignore` covers these locations. Do not delete ignored user data as part of source cleanup.

Use [releasing](releasing.md) for artifact identity, package checks, publication and signing requirements.


Additional isolated UI QA uses `--ui-actions buffs|loot|mount|periodic|visual-rules|puller|out-of-target` and `--ui-run-state looting|break|stopped|history|attack` with `--ui-screenshot`, `--ui-fixture` and an explicit data directory. `--ui-small` captures the minimum 800×560 window. These fixtures are excluded from standard release builds, use the real controller to construct state, and never enable game input.

New page fixtures: `--ui-settings detection|combat|appearance|diagnostics`, `--ui-safety schedule|recovery|notifications`, and `--ui-safety-capture`. Visual-rule fixtures use `--ui-tab actions --ui-actions visual-rules`; the internal enum is `ActionsPage::VisualRules`. Periodic scheduling and first-use migration, visual persistence/latching, guarded key/click execution, bounded verification retries, bounded recovery and cancelled-input cleanup have deterministic tests. Notification delivery has an injected failure test; `cargo run -p metin-desktop --example notification_probe -- --system` explicitly submits one local OS test alert.
