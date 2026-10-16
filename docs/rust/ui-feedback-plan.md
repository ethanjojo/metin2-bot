# UI feedback implementation — 2026-10-16

User-requested scope: preserve automation behavior while correcting eight observed UI/capture issues.

1. Move vector reorder controls into skill rows; preserve selection and all eight slots.
2. Show icon recognition only for Activate when inactive. Periodic scheduling does not consume icon presence; preserve existing stored crops when switching modes.
3. Replace desktop BitBlt with one HWND-based Windows Graphics Capture path, cropped to verified client geometry. Keep cancellation, identity, geometry and input guards. No screen-copy fallback. Verify with our own occluded window, without game input.
4. Scale zoom from raw wheel delta once per event rather than each smoothed animation frame.
5. Show/edit canonical HSV sampled color and vector green check/red cross readiness.
6. Explicit armed edit tool, drawing instructions, Done/cancel and frozen-only manipulation.
7. Shared hit testing for resize/move/create/color cursor and gestures.
8. Replace Run connection card with read-only game preview and controller-authoritative selected/next target overlays. Window selection remains in Calibration; preserve actionable readiness links.

Verification: meaningful RawInput regressions, controller/worker tests, native occluded-window capture probe, Windows/Linux tests/fmt/strict Clippy, actual theme/size screenshots. Never call UI or synthetic-input checks live gameplay acceptance. Existing open app/profile stays untouched; package only after checks.

## Implemented and verified

All eight changes are implemented. Skill arrows are vector buttons beside each row with boundary disabling and hover explanations. Icon controls appear only for inactive-mode skills, retaining saved crops across mode changes. Calibration exposes the canonical sampled color through an sRGB picker and HSV values, vector readiness checks, explicit armed tools/Done/Escape, frozen-only manipulation and shared gesture/cursor hit testing. Raw wheel events affect zoom once, with unit-aware bounded changes. Run uses the controller's actual acquisition/failure-exclusion predicates for amber current, green next and blue remaining markers, with honest live/stale/snapshot labels.

Windows now captures the selected HWND with Windows Graphics Capture, cropping to its verified client rectangle. Queued frames retain their compositor timestamps and must be newer than the request; cancellation, deadlines, resize, identity and post-readback freshness checks remain enforced. There is no desktop-copy fallback. Windows 10 2004 / build 19041 is the explicit minimum for cursor exclusion; installer and platform documentation match.

Windows: 98 whole-workspace tests, including 39 desktop UI tests; fmt, strict Clippy, native debug build and storage/vision/both-theme self-test passed. Logs: `build/rust-migration/feedback-{tests,fmt,clippy,build}.log`, `feedback-self-test.json`. RawInput regressions cover the reported zoom failure, row sorting, conditional icon controls, tool arming/cancel, cursors, color conversion and read-only Run overlays. A controller regression validates current/next/failed candidates against actual acquisition.

The rebuilt native own-window probe passed capture while independently verified as occluded by a second window, source freshness after a 450 ms queued-frame delay, client resize and cancellation: `build/rust-migration/window-capture-occluded-v3.json`. It sends no game input. Original input fixture results are historical; this capture probe does not establish a live Metin cycle.

Windows visual evidence: all 20 existing theme/size/empty-state screens were recaptured, plus four inactive-mode Buffs screens. Independent review inspected 16 relevant Run/Calibration/Buffs screenshots and rechecked all eight Buffs screenshots after the final disabled-arrow contrast fix. No open finding remains; see [the review](ui-feedback-review.md). Armed tools/cursors and live colored markers are interaction/source tests, not claims of manual native game interaction.

Linux: 97 whole-workspace tests, including 39 desktop UI tests; fmt, strict Clippy, native debug build and executable self-test passed. Logs: `build/rust-linux/feedback-{tests,fmt,clippy,build}.log`, `feedback-self-test.json`. The extra Windows test covers compositor timestamp age conversion. macOS native build, Linux/macOS gameplay backends, real-client combat/buffs, hardware HiDPI and clean-machine installation remain unverified. The existing user window and profiles have not been replaced or modified.
