# Release procedure

Build the Windows x64 installer from the tested Rust workspace. See [development](development.md) for prerequisites and [platform support](platform-support.md) for current scope.

## Local Windows prerelease

Use a clean committed checkout, PowerShell 7, Rust 1.99.0, VS C++ tools/SDK, 7-Zip, .NET SDK 10.0.403 and Python build tooling:

```powershell
python -m venv .venv
.venv/Scripts/python.exe -m pip install -r requirements-build.txt
./tools/prepare-rust-native.ps1
$compiler = ./tools/prepare_compiler.ps1
$crt = ./tools/find-msvc-crt.ps1
./tools/package-rust-windows.ps1 -Version 0.1.0-rc.1 -BuildNumber 1 -CompilerPath $compiler -CrtDirectory $crt -PythonPath .venv/Scripts/python.exe
```

The script rejects dirty trees, invalid versions, a version core differing from VERSION, incomplete native runtimes and existing output directories. It builds from the current commit (no stale SkipBuild path), writes release identity into PE resources and validates the manifest/hash inventory against the real EXE before creating setup. Use `-OutputDirectory <fresh path>` to choose a destination. Only setup and `SHA256SUMS.txt` are release assets. The internal staged application directory is verification input, not a public portable distribution. The packager builds the pinned NativeAOT HID bridge and includes its DLL and notices; no .NET runtime installer is bundled.

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

Inspect the staged application directory with `python -m tools.verify_bundle --directory <Companion folder> --version <version> --commit <full SHA>`. Run the packaged EXE self-test with the normal UAC confirmation and without development DLL paths. The automated production-AppId install/upgrade/uninstall acceptance script is restricted to a disposable hosted runner; do not spoof its guard to test on a user's real installation.

The destructive `tools/verify_installer.ps1` remains restricted to that disposable environment. A separately authorized host upgrade must first back up the complete data directory, retain before/after file hashes, use the exact verified package and leave the shared driver installed. Record cancellation/failure and profile-retention evidence. Such an upgrade does not prove clean installation, uninstall or shared-driver removal.

## Driver consent and shared removal

Setup requires administrator access and explicit consent for the mandatory driver. It discloses `CN=HIDMaestroTestCert`: an RSA 2048/SHA-256 code-signing certificate with an exportable private key in machine `My`, trusted through machine `Root` and `TrustedPublisher`. It also configures/starts `GameInputSvc` for automatic startup. Silent installation must supply `/ACCEPTINPUTDRIVER=1`; absence of consent must fail without silently installing the driver.

Hosted tests must cover the driver management report, install/upgrade readiness and silent-consent refusal as well as app inventory. Ordinary uninstall preserves user data and the shared driver. Explicit `/REMOVEINPUTDRIVER=1` removal must refuse while another consumer runs. Both HIDMaestro driver packages are shared. When neither remains, explicit removal deletes the certificate/private key; it does not restore the original `GameInputSvc` startup setting, which upstream does not record. Automatic startup remains configured. Use a disposable hosted runner for these machine changes, not the developer's accepted driver installation. The complete integrated gameplay cycle remains a separate operator acceptance gate.

## GitHub automation

`release.yml` builds/tests/packages Rust on Windows, verifies PE identity/icons/inventory, installs the package, checks relocation and in-place retirement of obsolete owned files during upgrade, runs the installed EXE without development PATH, uninstalls and verifies user-data retention. Only verified installer/checksums reach the publication job. That job alone has contents:write; PR builds cannot publish.

Pushes to main create dev prereleases. Tags must match the VERSION core: a tag such as v0.1.0-rc.1 creates a prerelease, while v0.1.0 requests stable. Tagged commits must be on main. Manual publishing runs on main only. Publishing creates/resumes a draft for the same tested commit and refuses moved/conflicting tags. Before publication it verifies the exact remote asset inventory, release channel, API sizes/digests and both downloaded installer/checksum bytes. An already-published rerun verifies that immutable remote pair again; a rebuild of the same source never replaces published bytes. Stale draft assets are removed before verification.

Stable releases are blocked by `packaging/release-policy.json` until its documented game and hosted-install acceptance conditions have been fulfilled. Do not change stable_ready merely to get a build past the gate. A prepared local package is not evidence of remote CI, installation or publication.

`rust-ci.yml` verifies Windows native builds and icon/GUI/package probes. Windows x64 setup is the only supported release target.

## Update feed and installer handoff

The fixed update source is the public [ethanjojo/metin2-bot releases](https://github.com/ethanjojo/metin2-bot/releases), read through the [anonymous releases API](https://api.github.com/repos/ethanjojo/metin2-bot/releases). The repository and source are public by the owner's decision; update checks need no account or token. Keep HTTPS, release identity and asset validation intact. Verify live feed access with the [read-only update probe](development.md#confirmed-updates), then independently verify the exact offered package. Public visibility does not establish that a new Companion package has been uploaded; older MetinBot assets are filtered out.

Publish only the tested Windows x64 setup and `SHA256SUMS.txt` for the exact version/commit. Ensure GitHub exposes the setup's correct byte size and SHA-256 digest, that the published tag/release identity matches the package and that Stable excludes prereleases. Preview is an explicit user choice. Verify the downloaded public asset independently; offline UI fixtures and a successful local build do not establish feed publication or remote installation.

Accept both the startup Install update path and the manual Settings → Updates path with the exact release. Exercise Continue/Skip and a late response after connecting, offline/rate-limit failures, cancellation, malformed/stale metadata, wrong hashes/sizes, unsaved-save/discard/cancel decisions, failed input release, failed helper preparation/handshake, UAC cancellation and installer failure. Check the visible HIDMaestro driver/certificate/GameInput disclosure before explicit installation consent. The existing Inno setup is the only install path; it runs silently for updates and remains the normal wizard for first installation. Confirm the staged helper survives application exit, displays session-matched actual file progress, retains backups on failure, and does not claim 100% before installed verification. Verify installed inventory/version, isolated self-test, driver readiness, unchanged profile/data hashes and the correct custom/default data directory on restart. The reopened app stays paused. No automatic check may execute an installer.

**Current acceptance:** on 2026-11-13 the owner confirmed the complete game cycle and successful official-source in-app update/restart with preserved profiles on `v0.1.0-review.22` (`4fbb0f12329e07f91d03280f104aba7b0a8cd0be`). That revision also passed hosted installer and protected-staging ACL acceptance. The owner explicitly has **not** tested refusing UAC; verify that refusal leaves the original installation and profiles intact before clearing the remaining stable blocker. Keep `stable_ready` false until that result is recorded. Every new package still runs exact-commit hosted installation/upgrade/uninstall and ACL tests. Unattended local validation must not launch setup, request UAC or change the accepted shared driver. Automated error paths and offline screenshots do not establish interactive cancellation acceptance.

## Distribution identity

Companion is the product display name; release assets are `CompanionSetup-<version>-windows-x64.exe` and `SHA256SUMS.txt`. `packaging/app-identity.json` supplies the product metadata to Rust and setup, including AppUserModelID `app.jojo.Companion`. Keep the opaque installer AppId `{78F36AC5-1367-4498-B486-50E3326B9426}` to retain one upgrade/uninstall registration. The default installation is `Program Files/Companion`; setup redirects the previous default-name directory to its Companion sibling and preserves other custom install locations. Cargo builds `target/release/companion.exe`; packaging ships `Companion.exe`. Setup registers previous executables with Restart Manager and replaces old shortcuts. After successful installation, the new EXE verifies its installed inventory and isolated self-test, then archives an intact previous installation into a sibling `Companion-install-backup-<UUID>` directory. Unknown/modified old application files or a busy directory prevent retirement and produce an explicit setup failure; previous files are retained. Obsolete textual dependency notices under the previous `licenses/` tree are retained intact in the archive; this exception never allows extra files in the new active bundle. Setup preflights destination occupancy and redirected ancestors before writing files, recovers a pending default-path migration on retry, and removes legacy shortcuts only after success. Migration reports reserve a fresh file outside the affected trees before any mutation. The archived uninstaller is a backup, not the registered uninstall entry: never run it against the new installation. No old uninstaller is invoked by migration.

The canonical user-data root is `%LOCALAPPDATA%/Companion`. Normal startup migrates the previous default only when the destination is absent or empty: validate data and reject links, acquire the old writer lock, copy and hash-check a complete `Companion-data-backup-*` backup, then atomically rename. The Windows lock handle closes immediately before rename; an active old writer blocks the directory move. Two occupied roots are never merged. Explicit custom roots stay unchanged, including during update restart; the canonical old default passed by an older updater is recognized and migrated. Setup never migrates another account's data. `--migrate-data --migration-report <absolute-path>` performs the same data migration without GUI or game input. Profiles remain outside the installation and survive uninstall. Our virtual input devices retain their truthful Companion labels and existing profile IDs/VID/PID; shared HIDMaestro driver/certificate/service identities are unchanged.

jojo is the product metadata publisher. Application/setup Authenticode signing is not configured. The driver uses its disclosed machine test certificate; it does not make the app/setup a verified Windows publisher. Packages include Cargo dependency/font notices, vendor OpenCV notices, the pinned MIT HIDMaestro SDK license and upstream third-party notices. Project-authored code is MIT licensed; artwork and third-party components retain their own terms. See [licenses and notices](../THIRD_PARTY_NOTICES.md).
