# Development

Companion is a Rust workspace with an egui desktop interface and OpenCV image processing. The toolchain is pinned in `rust-toolchain.toml`; dependency versions are recorded in `Cargo.lock`.

## Windows build

Install Rust 1.99.0, Visual Studio C++ build tools and the Windows SDK, PowerShell 7, 7-Zip and .NET SDK 10.0.403. The NativeAOT build pins runtime 10.0.14. The build script can download a hash-verified portable SDK when needed. These are build dependencies; users do not need an installed .NET runtime.

```powershell
./tools/prepare-rust-native.ps1
./tools/build-input-bridge.ps1
. ./tools/rust-env.ps1
cargo build --workspace --locked
cargo run -p metin-desktop -- --data-dir build/dev-data
```

The bridge build stages `MetinInput.dll` next to development and release executables and writes its distribution payload under `build/input-bridge/`. Rust loads the absolute sibling DLL only and requires ABI 1. Do not add a PATH lookup or alternative input transport. Use a separate development data directory. `run.cmd` launches an existing release build with the normal administrator prompt.

The only supported desktop platform is Windows x64. Portable core tests do not imply a supported desktop build elsewhere.

## Driver management

Driver installation/removal is an explicit administrator operation, independent of ordinary GUI startup and automation. From a built or installed executable, use a fresh absolute JSON report path whose parent directory already exists:

```powershell
./target/debug/companion.exe --input-driver preview --input-driver-report C:/Temp/companion-driver-preview.json
./target/debug/companion.exe --input-driver status --input-driver-report C:/Temp/companion-driver-status.json
# Explicit machine changes, normally performed by setup:
./target/debug/companion.exe --input-driver install --input-driver-report C:/Temp/companion-driver-install.json
./target/debug/companion.exe --input-driver remove --input-driver-report C:/Temp/companion-driver-remove.json
```

Reports contain `operation`, `success` and `message`; success exits zero and failure exits nonzero. The report file is reserved before any driver operation; existing files or invalid destinations refuse the operation without machine changes. Preview messages contain a versioned JSON inventory with entries, running-program count and explicit presence certainty; a failed readiness check never establishes absence. Only a fresh successful preview can establish zero managed artifacts. The retained GameInput service setting is reported separately from removable driver artifacts. These commands do not launch the GUI, open profile storage or send input. They cannot be combined with normal startup/self-test options. `preview` describes changes; `status` checks readiness. Removal refuses while another HIDMaestro consumer is running. Both `hidmaestro` and `hidmaestro_xusb` packages are shared resources.

Installation creates `CN=HIDMaestroTestCert`, an RSA 2048/SHA-256 code-signing certificate with an exportable private key in machine `My`, and copies the certificate into machine `Root` and `TrustedPublisher`. It installs the driver payload and configures/starts `GameInputSvc` for automatic startup. Setup displays these changes and requires consent; silent setup requires `/ACCEPTINPUTDRIVER=1`. Uninstall retains the shared driver unless `/REMOVEINPUTDRIVER=1` is explicitly supplied. When no driver package remains, explicit removal deletes the certificate and private key. It does **not** restore the original `GameInputSvc` startup setting: upstream `Uninstaller.ScanGameInputService` does not record that setting, so automatic startup remains configured.

The MIT SDK source is pinned to `c28c1f640d89038154b0962ae01627e0cda37cb0`; the integration uses generic HIDMaestro v2.1.0 and the original released embedded driver resources. `tools/build-input-bridge.ps1` invokes `tools/prepare-input-sdk.ps1`; `native/hidmaestro-sdk/HIDMaestro.Core.csproj` builds the source adapter before NativeAOT compilation. Distributions include `licenses/HIDMaestro.txt` and upstream third-party notices.

One scoped SDK policy patch changes `EnsureForCreate`: unavailable driver/readiness now fails instead of deploying during device creation. Driver installation and certificate trust changes stay inside explicit driver management. The SDK may still configure/start `GameInputSvc` when it initializes a runtime context. The bridge checks the pinned payload and driver readiness before opening devices; ordinary startup and input cannot repair/deploy the driver implicitly.

## Native input acceptance

Build `cargo build -p metin-platform --example native-probe --locked`, then run `target/debug/examples/native-probe.exe --input-only build/native-input.json` from an interactive desktop after building/staging the bridge and installing the driver. Close Companion first to free F7/F8. The probe targets its own child window and may need one activation click. It checks Ctrl+G, a left click, held Space, balanced release and actual Windows key/button state. Input-only mode does not verify capture; omit `--input-only` for combined capture/input acceptance.

The bridge ABI can be checked without creating devices or sending input: `python tools/verify-input-bridge.py build/input-bridge/MetinInput.dll --installed` (omit `--installed` when testing the missing-driver case).

The native helper passed these isolated HID checks. The operator also confirmed isolated Escape and X-control click responses in the game on 10 November 2026. Complete automation through the Rust application still needs a calibrated live-game cycle; a fixture or individual game response does not establish targeting, attack, buff or mount behavior.

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

Use `.venv/Scripts/python.exe` if the environment is not activated. These helpers are development tools and are not required to run the app.

Tests cover profile validation/import, targeting, scheduling, recognition fixtures, editor state, platform guards and packaging contracts. A self-test checks storage, vision and headless interface rendering without sending game input:

```powershell
./target/debug/companion.exe --self-test --self-test-report build/selftest.json
```

Self-test does not install the driver or send HID input, and does not establish live-game behavior or clean-machine installation.

## Documentation screenshots

On Windows, run `./tools/capture-docs.ps1` from a normal interactive desktop. It builds the `ui-smoke` feature, generates fresh isolated demonstration data with the `docs_fixture` example, and captures twenty-five native views in the fixed Metin appearance under `docs/assets/`. They include the window grid, Recognition, calibration size review, overlay controls enabled/disabled, Actions and buff details, the four-class icon picker, Run, profile management, incomplete setup, a break, compact event history, Safety, Settings, individual action pages and five Updates states. Screenshot names are stable (`run.png`, `actions.png`, `buff-icon-picker.png`, `size-review.png`) without appearance suffixes. The scene is a geometric fixture, not a game screenshot. Logs and temporary profiles stay under ignored `build/docs-capture/`; existing user data is never used. Use `-OnlyViews <names>` for a focused recapture.

Review every generated image before committing. Keep labels neutral, avoid personal/server information and retain the demonstration caption in the README. The `ui-smoke` feature is for verification and is not enabled in release packages. `--ui-size-review` requires both `--ui-fixture` and `--ui-screenshot`; the documentation case opens Recognition at 460 × 620 with a preserved-area review draft and no game input. Run `./tools/capture-docs.ps1 -OnlyViews size-review` for that case alone. Screenshot tests validate all twenty-five PNGs, their expected dimensions, chunk integrity and distinct content. Standard views are 460 × 620, the window chooser is 780 × 600 and compact event history is 400 × 500.

Updates uses `--ui-updates idle|available|downloading|ready|failed` with the isolated screenshot/fixture arguments. The capture script maps these to `updates.png`, `update-available.png`, `update-download.png`, `update-ready.png` and `update-error.png`. All five states are offline fixtures. The ready fixture uses inert data and disables installation; it proves neither a network download nor a published update feed. Recapture them with `./tools/capture-docs.ps1 -OnlyViews updates,update-available,update-download,update-ready,update-error`.

New page fixtures: `--ui-settings detection|combat|appearance|diagnostics`, `--ui-safety schedule|recovery|notifications`, and `--ui-safety-capture`. Visual-rule fixtures use `--ui-tab actions --ui-actions visual-rules`; the internal enum is `ActionsPage::VisualRules`. Periodic scheduling and first-use migration, visual persistence/latching, guarded key/click execution, bounded verification retries, bounded recovery and cancelled-input cleanup have deterministic tests. Notification delivery has an injected failure test; `cargo run -p metin-desktop --example notification_probe -- --system` explicitly submits one local OS test alert.

## Compact capture and annotation architecture

The existing Editor, controller and guarded HID executor remain authoritative. Recognition exposes search area, color point and a fixed HP-panel detail; HP detail matching always searches the whole selected client. The retired `panel.region` is discarded when loading existing profiles and is never serialized again. Matching publishes the match position as well as the score and retains consecutive stable-frame confirmation. A buff icon capture creates or expands `buffs.icon_region` around its source coordinates with a six-source-pixel margin when the existing area does not contain it. Manual detection-area editing remains available.

The startup/reconnect screenshot grid owns session connection independently of profile. The target catalog refreshes each second in the chooser and every two seconds while connected to detect a closed/replaced window. A bounded worker captures only visible tile thumbnails with the same read-only WGC implementation, without constructing HID or hotkey backends. Search/thumbnail generations discard stale results; selection never persists a window preference. Selecting a capture tool requests a new `Command::Snapshot` internally; the normal UI has no Refresh/Snapshot button. One `PendingCapture` records request identity, session, revision, cancellation generation, requested time, mode and launched frozen frame. Native selection starts only after the current generation's release acknowledgement, a fresh matching frame and valid target geometry arrive. Stale frames, profile/target changes, cancellation and mismatched results cannot commit a selection. The native adapter returns typed region/point results through Editor validation. Selection owns its own focus and never invokes the game input executor.

The frozen native tool dims outside the selection. One physical-pixel gesture model owns create/move/edge/corner resize and clamping. It preselects existing regions only after fresh-source geometry validation. The floating toolbar appears after selection, avoids the selected rectangle and supports Tab hiding. Enter confirms; Escape, F8 or right-click cancels. Color selection locks a sample and previews canonical `metin_vision` candidates with preserved profile tolerances before confirmation. No separate in-panel screenshot canvas or zoom/gesture implementation remains.

Annotations are a separate nonactivating, click-through native window following the current captured geometry. They hide on stale frames, focus/visibility loss, minimize, invalid target identity, target/profile changes and shutdown. Input guards remain strict: annotation visibility does not whitelist the control panel or any unrelated foreground/occluding window. During start arming, focus transitions can retry until the configured deadline; actual input still needs fresh stable observations and active-target checks.

Pinning persists as an inspectable UI preference. The native panel link follows the selected client, yields its elevated level when an unrelated application becomes active, and hides an inactive panel with a minimized/hidden target. An active panel and a destroyed target leave controls available for reselection. Unlinking restores an independent panel. Run history is a detail view inside the panel, and Actions uses catalog/detail navigation. Main navigation and Start/Pause/Save remain fixed while forms scroll.

### Manual native selection fixture

Run this explicitly on an interactive Windows desktop after configuring native dependencies. It opens only synthetic QA windows: there is no game attachment, runtime or HID input executor. Windows may require a manual activation click on the QA target before selection starts. The fixture is ignored by the ordinary suite and supports region and point tools:

```powershell
. ./tools/rust-env.ps1
New-Item -ItemType Directory -Force build/overlay-qa | Out-Null
$env:METIN_OVERLAY_FIXTURE_KIND = 'region'
$env:METIN_OVERLAY_FIXTURE_SECONDS = '60'
$env:METIN_OVERLAY_FIXTURE_REPORT = Join-Path (Get-Location) 'build/overlay-qa/region.json'
$env:METIN_OVERLAY_FIXTURE_SOURCE = Join-Path (Get-Location) 'build/overlay-qa/source.bmp'
cargo test -p metin-desktop --lib --all-features --locked frozen_selection_visual_fixture -- --ignored --nocapture --test-threads=1

$env:METIN_OVERLAY_FIXTURE_KIND = 'point'
$env:METIN_OVERLAY_FIXTURE_REPORT = Join-Path (Get-Location) 'build/overlay-qa/point.json'
cargo test -p metin-desktop --lib --all-features --locked frozen_selection_visual_fixture -- --ignored --nocapture --test-threads=1
Remove-Item Env:METIN_OVERLAY_FIXTURE_KIND, Env:METIN_OVERLAY_FIXTURE_SECONDS, Env:METIN_OVERLAY_FIXTURE_REPORT, Env:METIN_OVERLAY_FIXTURE_SOURCE
```

The timeout is clamped to 1–120 seconds (45 by default). Confirm a rectangle and a color point, repeat with Escape/F8 cancellation, and inspect the JSON result against the source BMP. The asymmetric gradient, grid, RGB swatches and 803-pixel row width expose orientation, color, stride and coordinate errors. Tab should expose all pixels under the controls. Verify the actual native rendering separately from headless geometry tests.

A separate ignored fixture displays two owned windows and verifies that annotations do not replace the underlying target in `WindowFromPoint`, without adding any input exception:

```powershell
cargo test -p metin-desktop --lib --all-features --locked annotation_is_ignored_by_window_from_point_without_any_input_exception -- --ignored --nocapture --test-threads=1
```

These checks establish only the exercised synthetic native behavior. They do not establish a calibrated game cycle, driver installation, live game reaction or release-artifact acceptance. Keep those gates separate and record the commands/results actually observed.

## Configured action data

Periodic actions and visual-rule execution share the tagged `ActionInput` schema: `key` contains `shortcut` and `hold_seconds`; `click` contains an optional calibrated `region`. Periodic actions store `input`, `interval_seconds`, `wait_seconds` and `first_run` (`after_interval` or `immediate`). The old periodic `timing` object is read once as immediate first use; serialization writes only the new schema and mixed formats are rejected. Puller timing remains separate.

Visual rules appear in Actions, while their stored `safety.rules` location remains the saved-profile compatibility contract. Each rule's `execution` stores input, verification (`reference_gone` or `delay`), wait, timeout, total attempts and `after_success` (`pause` or `continue`). Default execution verifies disappearance and pauses. Deterministic tests establish controller and guard behavior; fixture captures do not establish live-game acceptance.

`Command::Connect` attaches a profile/window session independently of draft validation, so mount can be confirmed without a capture or completed profile. `Command::Configure` applies a valid profile and never declares mount state. A new `SessionKey` starts unknown; `DeclareMount` explicitly confirms that session, and same-session reconfiguration preserves it. UI settings read and discard the obsolete `last_mounted` hint, and never write it again.

Paused capture is demand-driven: `SetPreview` subscribes the current profile/window, and `Snapshot` requests one fresh frame. The global Game overlay toggle publishes presentation demand across tabs. While paused, unsaved recognition edits use the canonical vision functions for presentation only; they never feed the controller. Overlay scene freshness updates continue at the capture cadence while Lite Mode/refresh interval throttle drawing only, so slow drawing never keeps stale annotations visible. Input cancellation generations also retire queued capture requests, so the UI reasserts preview demand after a stop. Active automation always samples at the profile's configured cadence.

`PlatformBackend::stop_capture` releases WGC resources without detaching the session identity, hotkeys or input backend. Idle/no-preview, one-shot completion, detach, errors and shutdown use this lifecycle hook. Capture initialization checks border-property availability and requests `GraphicsCaptureAccessKind::Borderless` once per process before `SetIsBorderRequired(false)`. Unsupported versions, denied access and system policy keep functioning capture with the Windows-required indicator, with diagnostic logging; no registry or monitor-capture workaround is used.

`ProfileStore::save` returns `SaveOutcome`: an error means the profile did not commit; a successful outcome contains the committed profile and an optional obsolete-template cleanup warning. UI state/runtime configuration still update after a committed save with a warning.

## Repository boundaries

Commit sources, locked dependencies, reusable tests, icon assets and current documentation. Keep profiles, settings, logs, downloaded native tools and build/package output out of Git. `.gitignore` covers these locations. Do not delete ignored user data as part of source cleanup.

Use [releasing](releasing.md) for artifact identity, package checks, publication and signing requirements.

## Confirmed updates

`updates.rs` owns a typed Stable/Preview channel, candidate and state machine plus one bounded background worker. The fixed public [GitHub release source](https://github.com/ethanjojo/metin2-bot/releases) and [anonymous releases API](https://api.github.com/repos/ethanjojo/metin2-bot/releases) require no account or token. Verified HTTPS and constrained release/asset identities remain mandatory. Candidates must be published, compatible and newer; installer size and SHA-256 are checked before a retained verified-file guard can authorize handoff. Cancellation and worker generations discard stale results. Profiles, captures and game data are not sent. Startup checking defaults off, the channel defaults to Stable, and downloads remain manual. A public repository alone does not establish publication of a compatible new Companion installer.

Probe the actual feed without installing or sending game input:

```powershell
cargo run -p metin-desktop --example update_probe --locked -- build/update-feed-probe.json
```

Append `--download` to verify and discard an offered installer as well. The JSON receipt records feed access, candidate availability, download verification and `installer_executed: false`. A successful report with `compatible_new_installer: false` can be correct when the feed contains only legacy MetinBot assets, which are filtered out. Offline screenshot fixtures do not establish live feed or package acceptance.

Settings → Updates owns the explicit Download and Install and restart decisions. Installation requires the verified artifact, resolved unsaved edits, no active capture/review and successful runtime shutdown/input release. The normal Inno setup remains the only installation path and retains administrator/driver consent. Its explicit update restart parameter restarts Companion only on success; automation stays paused. Cancellation removes temporary downloads. After installer launch Windows can retain the cached setup file while it is in use. Tests cover malformed metadata, unsafe URLs/assets, hashes/sizes, cancellation, stale results, offline failures and guarded handoff; native screenshots use inert offline fixtures.

## Form clarity and feedback

The shared atlas-backed segmented choice preserves pointer, keyboard, focus and disabled behavior, including logical left-to-right choices inside right-aligned form values. Buff modes, periodic first use, puller modes and visual verification/completion use this one control. Essential key holds, periodic waits and click-target previews stay inline. Detailed groups show canonical configured values while collapsed; the profile/icon catalogs and time units retain their searchable/select controls.

Visual rules follow When, Do and Then, with independent Notify and exclusive immediate-pause/action choices. Typed Actions summaries derive On, Off and Setup needed from the edited profile and do not claim runtime application before Save. Safety schedule has one editing summary. A fixed-height transient notice is tied to the exact committed editor change and profile; a later edit retires its Undo. Save feedback follows a committed save, not an attempted save. Numeric alignment and background dimming retain the existing Metin artwork and fixed footer.

Additional isolated UI QA uses `--ui-actions buffs|loot|mount|periodic|visual-rules|puller|out-of-target` and `--ui-run-state looting|break|stopped|history|attack` with `--ui-screenshot`, `--ui-fixture` and an explicit data directory. `--ui-buff-detail --ui-buff-icon-picker` opens the real presentation icon picker. The normal viewport is 460 × 620 logical pixels with a 400 × 480 minimum; `--ui-small` captures 400 × 500. These fixtures are excluded from standard release builds, use the real controller to construct state, and never enable game input.

## Runtime progress and session reset

`ActivityProgress` publishes the controller phase label, elapsed time and optional remaining/total seconds as one typed snapshot. `ActivityKind` and the active buff index describe the actual controller phase and owned action, independently of status wording or due timers. Run selects its artwork from that snapshot, including the active skill's optional presentation icon. Periodic actions use the UI Clock; waits and breaks use the atlas hourglass. Unknown end times have no percent; combat/acquisition values are labeled as limits. Arming distinguishes countdown from waiting for stable focus. Buff presence is published only for fresh recognized frames; UI verifies session/revision and source freshness.

`ResetSession` replaces statistics-only reset. The single runtime owner releases held inputs first, then pauses/reset counters/history and marks refresh buffs due for next Start. Mount state, periodic timebase, active safety budget, clock deadline and interrupted-break deadline remain intact. A release failure does not clear session data or claim reset success. One reset audit event remains. HP matching always receives the complete client frame; explicit profile masks and the confirmed matched detail control target exclusions.

## Window chrome and icons

The full-width title strip is independent of the inset profile controls and follows the source title artwork’s logical height. Its fixed-size Window and Pin icons are bare controls; Collapse and Close use original native art. Undo, Redo, Revert and Save always occupy the profile toolbar outside its popup, including disabled states and narrow layouts. The popup contains only profile search/list, New profile and Rename. The fixed action footer keeps a 10-pixel inset from the outer frame. Native icons use one 24-unit grid in `ui/icons.rs`, with a standard 18-pixel size and proportional fitting for the 15-pixel sort controls. Help uses the same icon family. No icon font or external renderer is required.

The `ui-smoke` build can capture the complete icon family with `--ui-icon-sheet --ui-screenshot <absolute PNG path> --data-dir <isolated data directory>`. The review sheet shows native, disabled and enlarged forms and is excluded from standard releases.

## Buff recognition stability

`BuffSkill.active_hold_seconds` defaults to 1.5 for saved profiles and uses the shared numeric rule `buff_active_hold_seconds` (0–5 seconds). `BuffDetector` owns the last genuine active-match timestamp for each skill. The same typed Presence output feeds runtime input decisions and UI. Held results do not refresh evidence. Positive inactive-reference confirmation overrides the hold; absence inferred solely from a missing active template waits until expiry. The worker resets detector evidence when it retires stale/invalid capture presentation. Synthetic image-sequence tests cover expiry, renewal, positive inactive, missing-template absence, per-skill overrides, disabled hold, resets, invalid/flat frames and time gaps. Candidate and HP matching retain their existing behavior.

## Capture transaction

`Editor::begin_capture(mode, frame)` returns a private `CaptureEdit` containing the immutable frozen source and a base calibration snapshot. The native selector receives its existing region and source pixels. Opening, cancellation and native errors never modify the live editor. A current request ID/session/revision/generation plus unchanged base calibration are required before `CaptureEdit::commit` applies one validated live edit. Template/color/buff/action changes share the same path. A known frame-size mismatch preserves the original calibration and offers explicit size review rather than committing the selection. `Editor::freeze` retains its non-destructive dimension guard for ordinary fixture loading.

`Editor::begin_size_review` creates a private review editor and retains the complete previous editor, including reference pixels and history. Existing inclusive physical pixel endpoints are expressed against the new dimensions without scaling them; a coordinate beyond the new frame stays invalid instead of being clamped into it. The canonical review list includes search area, masks, HP detail, buff detection area, Active/Inactive buff icon positions, visual-rule references, visual-rule click targets and periodic click targets. Existing reference PNG pixels and colors are retained unless explicitly edited. `SelectionRequest.context_regions` sends all draft areas to the frozen selector; one shared annotation helper renders valid gold outlines and invalid red outlines with an English outside-image warning. Only display bounds are clipped. Nonfinite/inverted shapes are ignored by painting and remain invalid data.

The keyed `Command::CalibrationReview` inhibits both GUI and F7 starts while the review is active, even though the prior runtime profile remains configured. Confirmation requires the original session/revision, fresh visible capture, matching current geometry, no pending selector and no invalid areas; normal profile validation also applies. `finish_size_review` records the entire reviewed calibration as one undoable edit; Save applies/persists it afterward. Escape, including native selection cancellation during review, cancels the entire review and restores the previous editor/history. A profile/window switch or second geometry change also invalidates review and selection work. Automation remains paused. No silent scaling, data clamping or calibration deletion is a migration path.

## Embedded Metin UI skin

`crates/metin-desktop/src/ui/skin.rs` owns the embedded PNG/typed manifest, per-context texture cache, clipped atlas tiles and three/nine-part painting. `ui/catalog.rs` maps typed artwork and skill IDs to that same atlas, including four class groups with 10/10/12/12 original base skill images. No separate icon texture loader is used. `widgets.rs` owns shared skinned buttons, tabs and gauges; `select.rs` owns selects and their native scrollbar; egui owns focus, menus, accessible labels and editing. UI zoom scales logical points once. Textures load with straight RGBA and nearest filtering. Selects use a recessed field, the original arrow, a matching full-width popup and clear selected/hover rows. Generic selects retain an actual menu and ListBox accessibility role; profile search remains open while typing, and keyboard focus scrolls the corresponding row into view.

`crates/metin-desktop/assets/metin-ui/catalog.json` is the single maintained English display catalog. It records Warrior/Ninja/Sura/Shaman class labels, all 44 official normal/base skill names, verification date and individual English Gameforge wiki source URLs. `tools/import-ui-skin.py` generates `catalog.rs` for typed const runtime entries and English labels in `manifest.json`; built-in names are not independently maintained in Rust. Source labels and their origin remain only in manifest `source_label`/`source_label_origin` provenance. User-entered profile/skill names are never translated.

Regenerate the verified subset with `python tools/import-ui-skin.py <path-to-metin2-ui-egui-with-icons.zip>` (install Pillow 12.2.0 for this developer tool). Add `--check` to verify all three generated outputs (`widgets.png`, `manifest.json`, `catalog.rs`) without writing. The importer validates the exact class skill IDs/order, verifies source atlas SHA256 values and per-region RGBA hashes, then deterministically packs the same 116 regions into one 512 × 512 atlas. Provenance, original rectangles and ownership remain in the asset manifest and the bundled `licenses/Metin-UI-assets.txt`. Original pixels are unchanged. Catalog tests need only Python’s standard library; Pillow is used only for source-art regeneration/checking. Disabled buttons and wide, center-tiled button variants are documented application skin choices.

`BuffSkill.display_icon` is an optional validated `SkillIconId`; only the 44 base skills of the four classes are accepted. Missing values load as `None` and are omitted when saved. The choice is presentation metadata: `BuffConfig::execution_eq` ignores it alongside the display name, while `icon` and `inactive_icon` remain canonical recognition captures. A profile with an unknown supplied display icon is rejected rather than treated as an arbitrary asset path.

The interface has one fixed appearance; settings read and discard the retired theme preference while preserving other preferences. The fixed app icon uses the same identity for every window. `tools/capture-docs.ps1` captures `profile-menu.png`; the `ui-smoke`-only `--ui-profile-menu` fixture renders the real egui popup without game input. Production binaries do not expose screenshot fixtures.

The installer restart callback is shared in `packaging/update-restart.iss`. Run `python tools/verify-update-restart.py --compiler <ISCC.exe> --output build/update-restart-acceptance` to compile and execute its isolated argument/consent fixture. It returns before any wizard or installation and checks Windows argument parsing for spaces, Unicode, extended paths, drive roots, missing consent, silent mode and missing directories. Windows release CI runs this before packaging.

## Installation and data migration

The product metadata is shared through `packaging/app-identity.json`. Build overrides use `COMPANION_BUILD_VERSION` and `COMPANION_BUILD_NUMBER`; the application manifest is `packaging/companion.manifest`. Release packaging scopes these values and restores them afterward.

One finalizer owns default-path relocation and in-place upgrades. Setup snapshots the previous owned-file inventory before replacement, records its source/target in a protected pending journal and verifies the new installation before retiring old owned files. A persisted archive identity makes interrupted moves resumable. Unknown or changed files are retained; constrained historical textual license notices are captured with their hashes before cleanup. See [release verification](releasing.md#verification).

Normal startup resolves the canonical `%LOCALAPPDATA%/Companion` store. Previous default data migrates only into an absent/empty destination after validation, writer-lock acquisition and a complete hash-verified backup. Explicit custom stores remain explicit; two occupied defaults are never merged. Migration and driver reports reserve a fresh file before side effects.

The optional `input-diagnostics` example tests one armed key/click through the production HID backend against an exact operator-selected PID. Unlike the synthetic `native-probe`, it is a supervised live-client diagnostic: do not automate or run it without explicit operator approval. Both reuse the same backend and are excluded from release packages.
