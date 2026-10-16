# Calibration usability follow-up — 2026-10-16

Scope: the five observations from hands-on review of the HWND-capture preview. Preserve recognition thresholds, profiles, undo/history and automation behavior; leave the currently running app and user data untouched.

## Changes

1. Reference validation previously used one error for either a crop smaller than 5 × 5 source pixels or grayscale standard deviation below 5. A distinctive exact 5 × 5 crop is accepted. Size errors now report the actual dimensions; insufficient contrast gets its own explanation recommending a fixed edge/symbol. The existing contrast guard is retained because a flat template cannot reliably recognize a panel. Tests cover the validator and the complete Editor crop path.
2. Checklist instructions live behind visible vector info icons and remain available by hovering their labels. Shared help is keyboard focusable, has an accessible description and a Help cursor; numeric settings use the same affordance. Instructions no longer expand when a different step is selected.
3. A fixed-height, unwrapped editing strip always reserves the same space for Done, help and the active tool. Tool labels truncate within their row rather than pushing the canvas. Done is disabled in the idle state. Escape still cancels an active gesture first, then retires an idle tool.
4. Every excluded-area row has its own trash button and numbered hover explanation. Recognition steps and the buff capture return panel also expose their own clear action; the global Delete selected button is removed. Pointer and keyboard deletion share one path, stop runtime input, cancel gestures and retain history. Removing an earlier mask reindexes an independently selected later mask; removing the selected mask disarms editing.
5. With Detections enabled, calibration areas and detected candidates show their source-pixel width × height above the rectangle. Crops use the same inclusive Region::pixels conversion as actual image extraction, independent of zoom. The current drag is measured too. Compact dark labels are bounded to the canvas and avoid one another when panel/crop edges coincide.

## Verification

The initial tests reproduced the ambiguous crop error, missing measurements and inline instructions. Actual egui regressions cover stable canvas geometry across tools/armed state, conditional source-pixel labels, row-specific deletion and selection/Undo, and real delayed hover help. Existing interaction tests are adapted to the row trash controls and compact editing strip.

Windows final workspace verification passed **104 tests**, including **43 desktop UI tests**. `cargo fmt --all -- --check`, strict all-target/all-feature Clippy, native ui-smoke build and executable storage/vision/both-theme self-test passed. Logs: `build/rust-migration/refinement-{tests,fmt,clippy,build}.log`, `refinement-self-test.json`. Both initial failing-test logs are retained as `refinement-red-{vision,ui}.log`.

Native Windows screenshots were recaptured for Calibration, Settings and Buffs in both themes at 1280×820 and 800×560 (12 screens), plus two large Calibration screens with two synthetic excluded areas. Calibration was recaptured again after fixing rectangle borders painting through measurement tags. The screenshots show the final tag layering, info/clear controls, stable idle strip and per-mask trash. Actual pointer/hover/tool transitions are egui RawInput tests, not claimed manual OS interaction. Fixtures and screenshot profile copies are isolated from user data.

![Dark calibration with two excluded areas](../../build/ui-redesign/dark-calibration-large-masks.png)
![Light small calibration](../../build/ui-redesign/light-calibration-small.png)

Linux final results and package identity are recorded below after verification. No live-client gameplay, macOS native build, Linux/macOS automation backend or installation acceptance is implied by these checks.
