# Rust migration ledger — plan: docs/superpowers/plans/2026-10-16-rust-egui-migration.md

User authorized complete implementation on 2026-10-16. Branch: codex/rust-egui-migration.

## Verified baseline

- Python: 256 tests passed, 50.421 seconds; full log build/rust-migration/python-baseline.log.
- compileall passed; pip check: No broken requirements found.
- Rust/cargo 1.99.0, egui/eframe 0.37.0, OpenCV 4.13.0 with bindings 0.101.1, LLVM 19.1.7 verified on Windows x64. Exact dependencies are locked.
- Default sandbox blocks crates.io DNS; approved elevated dependency download works.

## Rulings and interface preflight

- Ruling: work in the current authorized checkout on a dedicated branch; no new worktree, preserving the existing Python environment and uncommitted approved planning documents.
- T2 -> T3/T5: domain geometry, profile, shortcuts, observations and monotonic timestamps live in core; platform reuses them. No circular core -> platform dependency.
- T3/T4 -> T5: frame identity and immutable BGR pixels cross the platform boundary; no GUI or OpenCV types in core.
- T5 -> T7: worker owns runtime profile/timers/session; draft UI state is applied with a revision.
- T9 -> T10/T11/T12: Python removal requires full Windows parity and acceptance. Unverified OS gameplay cannot justify deleting the working product.
- Ruling: lack of Linux/macOS interactive hosts is a verification limitation, not a reason to fake PASS or block portable implementation work.

## Tasks

- T0: all 256 reference test IDs inventoried; sanitized profiles, numeric rules, image/tracker/buff and controller fixtures exported. Vision performance measured in three runs; full capture/runtime/idle baseline remains open.
- T1: Windows native build and both-theme GPU screenshots verified; native three-OS CI prepared but not remotely run. Ubuntu 24.04 WSL2: isolated Rust 1.99.0/Clang 19.1.1 and pinned minimal OpenCV were installed/built only under build/rust-linux. Full workspace tests (61), fmt and strict Clippy passed. The real WSLg GUI rendered successfully using Vulkan llvmpipe software rendering; executable self-test passed. Linux release build is running. No local macOS host exists.
- T2: typed profiles/validation, shared numeric rules, shortcuts, atomic storage, Unicode sort, writer lock, canonical paths, staged import and legacy migration implemented. 327 reference schema cases pass on Windows and Linux. Migration/startup and first-save failures have regression tests, including read-only preflight, corrupt settings/backups, canonical UUID selection, nested destinations and Unix dangling/nested symlinks.
- T3: OpenCV detection, panel matching, candidate stability and buff detection implemented; fixtures match Python, including 84 buff observations. Native three-OS comparison remains open.
- T4: Windows HWND/PID/creation identity, DPI, capture, privilege, obstruction/focus/manual-cursor guards, hotkeys and cancellable input implemented. A real child-process fixture verified pixels and Ctrl+G/mouse down/up order; report `build/rust-migration/native-probe.json`, game_input=false. Real-client acceptance remains separate.
- T5: single runtime owner, bounded queue plus atomic Stop generation, acknowledged actions, scheduling/mount/session behavior implemented. 648 reference controller events pass. Worker regressions cover full queue, Stop/new Configure, refresh/mount identity and failed toggle.
- T6–T7: egui profile/calibration/buff/settings/run UI implemented with history, frozen/live preview, transforms, binds, numeric fields, crops, themes and runtime mount persistence. Actual egui drag/cancel and keyboard undo/redo tests pass; light/dark and small-window buff/settings GPU screenshots inspected. Eight-skill forms pass simulated 100–200% scaling tests. Exhaustive native interaction/OS HiDPI acceptance remains open.
- T8: unsupported native backends explicitly report unavailable capabilities. Linux/macOS gameplay is not advertised.
- T9: Rust release executable, preview ZIP and shared Inno installer build locally, with DLL inventory, notices and checksums. Extracted ZIP verified all 449 inventory hashes; release self-test passed with user-confirmed UAC and no development PATH. Clean-machine installation/upgrade/uninstall remain gates. Live gameplay is unverified because the user reported server downtime. Python cutover/removal and production release pipeline conversion are deliberately not performed before those gates.
- T10–T12: native X11/macOS/Wayland capture/input implementations and acceptance remain open; see platform-support.md.

## Independent review and corrections

The independent reviewer found first-save orphan directories, interrupted legacy migration overwriting an existing deterministic profile, and missing persistence of automatic mount changes. All three were corrected with regression tests. New profiles commit from a staging directory; migration reuses existing profiles and matches Python's Windows path-derived UUID. UI remembers mount only for its current acknowledged session/revision. Import destinations inside source trees are rejected before creating staging folders.

The final review also checked startup/import validation before writes, dangling and nested links, corrupt settings/backups, and canonical profile/selection UUIDs. Those corrections share one validation path and passed review without further material findings.

## Verification artifacts

- `build/rust-migration/python-baseline.log`: 256 reference tests passed.
- `build/rust-migration/workspace-tests.log`, `clippy.log`: complete Rust verification (rerun after final changes; acceptance.md records current result).
- `build/rust-migration/profile-parity.log`: 327 boundary cases match.
- `build/rust-migration/native-probe.json`: actual isolated Windows capture/input verification.
- `build/rust-migration/self-test-debug.json`: storage/vision/headless egui self-test passed, explicitly no GPU/game claim.
- `build/rust-migration/ui-light.png`, `ui-dark.png`: actual GPU-rendered fixture screens, inspected.
- `build/rust-migration/benchmark-comparison.json`: identical fixture-only workload in Python and Rust; performance.md states limits.

No user profile was used as a fixture or overwritten. No game input, production push, release publication or installation over the production application has been performed.

Latest whole-workspace result: 61 tests passed on Windows and 61 on Linux; fmt and strict all-target/all-feature Clippy passed on both. The shared tests include 327 schema boundaries and 648 controller events; their internal case count is separate from the Rust test count. Native input tests cover in-flight focus/geometry/emergency cancellation, held keys/permissions, obstruction/manual movement, modifier release order and cleanup retry. Linux evidence is stored in build/rust-linux/{workspace-tests,fmt,clippy}.log, self-test-debug.json and gui-native.png.
