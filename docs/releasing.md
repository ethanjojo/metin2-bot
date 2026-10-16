# Release procedure

Build native packages from the tested Rust workspace. See [development](development.md) for prerequisites and [platform support](platform-support.md) for current scope.

## Local Windows prerelease

Use a clean committed checkout, PowerShell 7, Rust 1.99.0, VS C++ tools/SDK, 7-Zip and Python build tooling:

```powershell
python -m venv .venv
.venv/Scripts/python.exe -m pip install -r requirements-build.txt
./tools/prepare-rust-native.ps1
$compiler = ./tools/prepare_compiler.ps1
$crt = ./tools/find-msvc-crt.ps1
./tools/package-rust-windows.ps1 -Version 0.1.0-rc.1 -BuildNumber 1 -CompilerPath $compiler -CrtDirectory $crt -PythonPath .venv/Scripts/python.exe
```

The script rejects dirty trees, invalid versions, a version core differing from VERSION, incomplete native runtimes and existing output directories. It builds from the current commit (no stale SkipBuild path), writes release identity into PE resources and validates the manifest/hash inventory against the real EXE before creating the ZIP/installer. Use `-OutputDirectory <fresh path>` to choose a destination. Both packages and SHA256SUMS.txt are emitted; all users must retain the complete ZIP directory.

The default numeric version is VERSION plus BuildNumber, while ProductVersion and runtime diagnostics retain the complete prerelease version. Development Cargo builds use VERSION and build number zero. Release variables are scoped by the packager; they do not change VERSION/Cargo.toml.

## Verification

```powershell
python -m unittest discover -s tests -v
. ./tools/rust-env.ps1
cargo fmt --all -- --check
cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
cargo test --workspace --all-targets --all-features --locked
```

Tests replay reference behavior and exercise UI/storage/runtime boundaries.

Inspect the final extracted ZIP with `python -m tools.verify_bundle --directory <MetinBot folder> --version <version> --commit <full SHA>`. Run the packaged EXE self-test with the normal UAC confirmation and without development DLL paths. Install/upgrade/uninstall with the production AppId is restricted to a disposable hosted runner; do not spoof its guard to test on a user's real installation.

## GitHub automation

`release.yml` builds/tests/packages Rust on Windows, verifies PE identity/icons/inventory, installs the package, checks removal of obsolete installation files during upgrade, runs the installed EXE without development PATH, uninstalls and verifies user-data retention. Only verified installer/ZIP/checksums reach the publication job. That job alone has contents:write; PR builds cannot publish.

Pushes to main create dev prereleases. Tags must match the VERSION core: a tag such as v0.1.0-rc.1 creates a prerelease, while v0.1.0 requests stable. Tagged commits must be on main. Manual publishing runs on main only. Publishing creates/resumes a draft for the same tested commit, refuses moved/conflicting tags or changes to published assets, and verifies downloaded installer/ZIP hashes after publication.

Stable releases are blocked by `packaging/release-policy.json` until its documented game and hosted-install acceptance conditions have been fulfilled. Do not change stable_ready merely to get a build past the gate. A prepared local package is not evidence of remote CI, installation or publication.

`rust-ci.yml` separately verifies native Windows/Linux/macOS builds and icon/GUI/package probes. Linux/macOS artifacts remain editor previews and must not be advertised as working automation clients. Intel and Apple Silicon must be built/tested separately before publishing both architectures.

## Distribution identity

jojo is the product metadata publisher. No signing certificate is configured. Authenticode/Developer ID/notarization remain separate release decisions. Windows packages include Cargo dependency/font notices and vendor OpenCV notices; Unix packaging uses the same Cargo notice helper plus installed OpenCV notices. A project source license still needs the owner's choice.
