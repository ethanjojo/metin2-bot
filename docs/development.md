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

On Windows, run `./tools/capture-docs.ps1` from a normal interactive desktop. It builds the `ui-smoke` feature, generates fresh isolated demonstration data with the `docs_fixture` example, and captures six native views under `docs/assets/`. The scene is a geometric fixture, not a game screenshot. Logs and temporary profiles stay under ignored `build/docs-capture/`; existing user data is never used.

Review every generated image before committing. Keep labels neutral, avoid personal/server information and retain the demonstration caption in the README. The `ui-smoke` feature is for verification and is not enabled in release packages.

## Repository boundaries

Commit sources, locked dependencies, reusable tests, icon assets and current documentation. Keep profiles, settings, logs, downloaded native tools and build/package output out of Git. `.gitignore` covers these locations. Do not delete ignored user data as part of source cleanup.

Use [releasing](releasing.md) for artifact identity, package checks, publication and signing requirements.
