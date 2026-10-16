> Historical verification report. Use the repository README and docs/release-readiness.md for current installation/release status. Earlier commands, counts and package paths below describe their original snapshots.

# Calibration toolbar and buff capture follow-up

Scope: the six observations from the user's hands-on review of window selection. Keep calibration, detection, saved profiles and undo behavior intact.

Follow-up: skill-order controls now form one vertical up/down column beside each skill, with that row's trash button beside the arrows. Removed the duplicate Remove action and obsolete post-delete guard from the detail pane. Each icon has a skill-specific accessible name and tooltip; first/last reorder limits remain disabled. Structural actions reuse the existing reorder/remove helpers and preserve selection, display units, paused runtime behavior and Undo.

The user's drawing clarified the compact layout: each arrow is exactly 26×15 logical pixels, with a 2-pixel gap, so the pair fits the 32-pixel skill button. The trash button is also 32 pixels high; shortcut/mode text sits beneath the row. The compact control explicitly allocates its rectangle, avoiding the standard empty button's font-based minimum height. Icon painting, accessibility and tooltip handling are shared with existing small icon buttons. The pointer regression checks the exact shared top/bottom edges as well as reorder/delete behavior.

The compact-layout correction passed 115 workspace tests (52 UI), strict Clippy, formatting and native debug build. All four final eight-skill native screenshots were inspected under `build/compact-sort-ui/{dark,light}-{large,small}.png`; the arrow pair, skill button and trash now share top/bottom edges in both sizes/themes. Logs: `build/rust-migration/compact-sort-verified-{tests,build,clippy}.log`. The first geometry test rejected the ordinary widget's 16-pixel minimum height before the exact-allocation fix.

Build-tool follow-up: stale check-only metadata has recurred in the local test → Clippy → test sequence (`required to be available in rlib format`). Cleaning only metin-desktop's compiled artifacts restores the build. Reproduce and resolve Cargo's check/build artifact selection, or isolate lint artifacts, before considering this toolchain issue permanently resolved; no application dependencies or runtime fallbacks were changed for it.

The final debug executable also passed the storage, vision and both-theme self-test without game input: `build/skill-row-ui/self-test.json`.

The skill-list follow-up passed **115 workspace tests**, including **52 UI tests**, formatting, strict Clippy, native debug and release builds. Pointer tests verify stacked arrow geometry, downward movement, removal of the selected skill, removal of an unselected skill, last-skill empty state and Undo. Four native screenshots with eight synthetic skills and a long name were inspected in dark/light at 1280×820 and 800×560: `build/skill-row-ui/{dark,light}-{large,small}.png`. Logs: `build/rust-migration/skill-row-{workspace,native,clippy,release}.log`. The older counts and screenshot matrices below describe the preceding toolbar/capture verification.

- `Sizes (px)` independently controls source-pixel measurements on areas, current drags and visible detected candidates. It starts off, like a session display preference; toggling it does not modify the profile. Detections controls candidate visibility.
- Removed Hide/Show steps and its state entirely. The checklist remains available.
- Editing status and Done are on the left; overlays, Fit, zoom and history are flush right on one row in a wide canvas. At the minimum 800×560 window, overlays use a second fixed row. Idle, armed and live states reserve the same toolbar space.
- Checklist and excluded-area trash controls align to the right edge. Active/inactive icon deletion is an accessible vector trash button with hover and disabled help. All use the shared deletion path and preserve Undo.
- Buff capture identifies the skill, provides area/active/inactive choices with readiness marks, shows the selected captured icon, and offers an explicit Done & back to buffs button. Capturing stays in Calibration until the user returns. The return button precedes the capture choices and remains visible at minimum window size. Profile-name editing remains in regular Calibration; the global profile selector identifies the profile during capture. Skill readiness requires the detection area and active icon, with inactive optional. Returning keeps unsaved edits; Save applies them.
- Stored icons without original crop coordinates are recognized as captured and can be deleted. Coordinates are only needed to show or reuse a rectangle.

## Verification

Real egui pointer tests exercise the independent Sizes toggle without moving the canvas, toolbar positions, aligned row trash actions, per-mask deletion/Undo, capture state changes, thumbnail creation and return to the same skill. The capture test also checks minimum-size return visibility and icon readiness without stored coordinates. Existing tool, cursor, keyboard, profile and window-selection tests remain in the suite.

Native screenshot fixtures use isolated synthetic profiles, a synthetic frame and a synthetic icon. `--ui-buff-capture` and `--ui-sizes` are available only in the existing ui-smoke build, require screenshot mode and use the production rendering/edit-tool path. No gameplay inputs or production profile writes are involved.

Windows verification passed **114 workspace tests**, including **51 UI tests**, plus **49 standard-build UI tests** without ui-smoke. Formatting, strict all-target/all-feature Clippy and native debug build passed; the executable self-test passed storage, vision and both theme checks. Logs: `build/rust-migration/toolbar-clean-{tests,native}.log`, `toolbar-delivered-{clippy,standard}.log`; self-test: `build/toolbar-ui/self-test.json`. A stale local compiler metadata cache encountered during final verification was resolved by cleaning only metin-desktop's compiled artifacts and rerunning the full suite successfully.

Inspected **20 native Windows screenshots**: Calibration, Sizes on, active capture, inactive capture and Buffs, each in dark/light at 1280×820 and 800×560. The eight buff capture screenshots were recaptured and reinspected after moving the return action upward and removing redundant profile-name editing from capture mode. The right-aligned controls remain bounded; sizes are absent when disabled; the selected capture state and optional inactive state are clear. Small sidebars scroll for lower content; the buff return action stays visible without scrolling. Screenshots and paired review sheets are in `build/toolbar-ui/`.

Linux/macOS native rendering and real-client gameplay were not rerun in this Windows UI follow-up. No production profiles were changed and the user's existing running app was left open.

## Local Windows preview

Optimized standard release build passed. The portable ZIP is `build/rust-packages/da0f3a64-f200-4de7-91a4-5038518a3748/MetinBot-0.1.0-rust-preview-windows-x64.zip` (33,439,512 bytes; SHA-256 `365ead1feae76d8201fc488685398478536176c6e97c395bcaf907cda2575939`). All 449 inventory files in an isolated extracted copy matched their SHA-256 hashes. This build includes the current uncommitted UI changes on top of `5af7ea9`; its build metadata explicitly reports a dirty working tree. Logs: `compact-sort-release.log`, `compact-sort-package.log`, `compact-sort-package-verification.json` in `build/rust-migration/`.

This ZIP supersedes the earlier toolbar and window-selection previews for local UI review. It was not launched against user data, installed or published. No installer was rebuilt in this follow-up. The release executable's UAC flow and game inputs were not exercised; the executable self-test above used the debug screenshot build from the same final sources.

![Calibration toolbar](../../build/toolbar-ui/dark-large-calibration.png)
![Buff capture and return](../../build/toolbar-ui/dark-large-active.png)
![Minimum-size buff capture](../../build/toolbar-ui/light-small-active.png)
