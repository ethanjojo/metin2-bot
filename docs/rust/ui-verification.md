# Desktop UI redesign verification

Scope: approved Run / Calibration / Buffs / Settings workbench, retaining existing profiles, editor, buff execution and guarded runtime. No Python cutover, native Linux/macOS gameplay backend, production installation, push or publication is included.

## Delivered behavior

- Persistent profile/save/start/pause/theme header and four task-specific destinations. Run explains the current state and the next action, with honest session statistics and applied buff timers.
- Calibration has four explicit recognition steps, exact-window selection, fresh Snapshot, masks, loaded-template readiness, frozen/live preview, detections, named overlays, zoom/pan, crop handles, Escape cancellation, history, and a full-canvas toggle.
- Buffs uses a scrollable ordered eight-skill list and selected detail. Existing modes, shortcuts, interval units, wait duration, timer reset, both independent icon crops, clearing/reuse and detection area remain available. Image selection uses the same canvas and returns to the selected skill. Manual mount declaration is separate from desired fight-mounted behavior.
- Settings groups every original numeric field with units, shared ranges and hover explanations. The filename hint now filters the displayed window list by exact filename, ignoring ASCII case, while retaining the already selected target.
- One derived Start blocker drives the button tooltip and Run action link. Profile normalization and saved-row index mapping preserve the selected skill, unit preference and crop mode. New/switch use the same profile-replacement path.
- Shared painter-drawn vector icons replace font-dependent arrows and symbols. Buttons have accessible names and hover/disabled-hover help. Both themes use the same typography/spacing and distinct semantic colors.

## Automated evidence

Windows and Linux whole-workspace verification: **78 tests each**, all targets and features. `cargo fmt --all -- --check` and strict `cargo clippy --workspace --all-targets --all-features --locked --offline -- -D warnings` are the required final checks. Native debug EXE self-tests cover storage, vision and both headless themes; they explicitly do not claim gameplay or GPU-window acceptance.

Logs: `build/rust-migration/ui-{workspace-tests,fmt,clippy,build}.log`, `ui-self-test-debug.json`; `build/rust-linux/ui-{tests,fmt,clippy,build}.log`, `ui-self-test.json`.

The 21 desktop UI tests include actual egui pointer navigation, dirty-profile preservation, typing/text undo, drag commit plus Ctrl+Z/Y, Escape, real hover tooltip rendering, selected-skill reorder/remove, Save compaction/crop-return identity, profile replacement, Start routing, process filtering, scrolling into icon capture and returning, small-window bounds, wide Run connection visibility, long names, both themes, and simulated 100/125/150/200% scaling. The worker adds a regression initializing hotkey capabilities before window selection without sending any input.

## Native visual evidence

All screenshots are application-rendered, using isolated synthetic profiles and no game input. Windows uses the native wgpu renderer on the local NVIDIA adapter. Linux WSLg uses llvmpipe software Vulkan; this is not hardware GPU performance evidence.

Windows matrix: `build/ui-redesign/{dark,light}-{run,calibration,buffs,settings}-{large,small}.png` — 16 screens at 1280x820 / 800x560. Eight synthetic skills exercise list/detail density. `empty-{run,calibration,buffs,settings}.png` — four new-profile empty states at 800x560. Linux: `build/ui-redesign/linux/{run,calibration,buffs,settings}.png` at 800x560. Saved screenshots are inspected directly; they are not generated mockups.

Visual iteration corrected excessive space above skill details, ambiguous reference-crop readiness, truncated small-window controls/instructions and inaccurate pre-attachment capability status. Screenshot tooling now keeps repainting until the asynchronous GPU callback completes, with a regression for multi-pass rendering.

## Independent review

A fresh reviewer inspected the whole patch, editor/runtime contracts and native screenshots. Three Important findings were fixed with regressions: Save changing the selected skill after blank-row pruning; Save removing the icon editor's return route; stale icon modes surviving profile replacement. The follow-up source review found no remaining material issue. This was source/screenshot review, not a claim of manual native interaction.

Suggested normal-user review:
1. Start with no window: identify why Start is disabled and follow its action link.
2. Create/name/save a profile; change pages while dirty; switch profiles and check Save/Discard/Cancel.
3. In Calibration, distinguish the four steps, select/move/resize/delete a mask, undo/redo, cancel a drag, zoom/pan and hide/show steps.
4. Add eight skills; navigate their details, rename/reorder/remove; use long names and switch seconds/minutes without changing duration.
5. Open active/inactive icon capture, select an area, Save, then return to the same skill. Check coordinate reuse captures current pixels.
6. Distinguish desired mount behavior from the manually declared current state. Confirm an unknown state explicitly.
7. Reach advanced settings and inspect every label/unit/tooltip. Try invalid/duplicate shortcuts and check the error directs to the right page.
8. Repeat at 800x560 in light/dark; scroll each pane independently and inspect disabled controls, icon hints and keyboard focus.

## Verification boundary

No live target/attack/buff cycle was tested: the user reported the game server down. macOS, true OS multi-monitor/Retina/HiDPI interaction, assistive-technology acceptance and clean-machine installation/upgrade remain the migration gates in acceptance.md. Simulated scaling and hover input tests do not establish complete accessibility compliance. Release Windows elevation is not retried after the earlier user-canceled UAC launch; local packaging is not installed over the existing app.
