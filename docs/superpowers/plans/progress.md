> SUPERSEDED: historical queue-mode record. Active design: 2026-10-11-profile-target-design.md; runtime now uses individual clicks and target panels.

# SDD ledger — plan: docs/superpowers/plans/2026-10-11-metin-queue.md

Approved design and direct implementation: 2026-10-11.

Ruling: This folder is not a Git repository. Work directly in the approved workspace; no worktree, Git-based skill scripts or commits. Keep this ledger as the durable record.

Pre-flight: task 1 Observation/Action is consumed by task 4; matched.
Pre-flight: task 1 Region/VisionConfig is consumed by tasks 2 and 3; matched.
Pre-flight: task 2 detector/tracker and task 3 adapter are consumed by task 4; matched.
Ruling: A single measured screenshot cannot prove attack or destruction. Track stable beam presence only; do not infer destruction from motion or a precise queue length. Timeout remains independent of animation.

Task 1: complete — unittest controller/config 13/13 passed; first run failed because the package was absent. Coordinate bounds fixed against the pixel conversion test. Test temporary directories use the approved workspace because sandbox TEMP denied writes.
Task 2: core tests complete — vision 7/7 passed; screenshot calibration still pending.

Task 2: complete — 8/8 vision tests passed including slanted/fragmented rays. User beam screenshot yields 2 markers, damage number excluded; blue-selection screenshot yields 0.
Task 3: complete — 9/9 driver tests passed. Live read-only process/window discovery passed outside sandbox; inside sandbox process queries return no windows.
Task 4: implementation and core tests complete — 38/38 full suite passed, compileall passed; live app opened stopped with actual game preview. Remaining: live input tests and final fresh review.
Ruling: Runtime is separated into runtime.py so Tkinter does not duplicate controller behavior. Cancellation Event is the only cross-thread mutable control state. This adds one focused file, not another runtime path.

Final fresh review: four Important issues reproduced; one lifecycle coverage gap. No Critical or Minor findings. Reviewer final_review read-only, no game input.
Final: fixed cursor race — actual cursor checks plus atomic move/down SendInput batch; physical movement test RED→GREEN.
Final: fixed endless unchanged markers — separate 120 s stale marker timeout; same-marker and jitter tests RED→GREEN. Stable position/count changes are a heuristic only.
Final: fixed recovery bypass of fill interval — interval guards all FILL actions; recovery interval test RED→GREEN.
Final: fixed slow sampling confirmation — cadence-derived continuity tolerance and validated confirmation window; scheduling-overhead and config tests RED→GREEN.
Final: added real Worker shutdown-during-input test, using the actual guarded driver against fake external Win32 calls; worker terminates and releases Ctrl/mouse. Full suite 46/46 passed.
Ruling: Three global OpenCV distributions share cv2. Create project-local .venv with one pinned opencv-python distribution; never uninstall or change global packages. Cost: local environment disk space.
Ruling: Use WindowsAPI instance methods as the canonical OS boundary; remove unused convenience functions that constructed a new adapter per call. Cost: external callers must use documented application entry point; no external callers exist.
Final review considered but unverified: live SendInput/attack continuity/F8 await root tests; screenshot false-positive evidence applies only to sampled examples; physical presses after injection ownership cannot be fully distinguished; tiny overlays may evade sparse capture checks; external pre-set DPI hosts are not supported by standalone launch. WASD/death/login/pathfinding remain explicitly out of scope.

Live follow-up: short helper-generated F7 events were not observed; replaced polling start detection with native RegisterHotKey + WM_HOTKEY and NOREPEAT. Driver pumps emergency messages during gestures. No duplicate polling start path retained. Worker event test RED→GREEN; hotkey release on shutdown checked.
Live follow-up: capture at foreground transition could hit temporary occlusion. Arm start requires 0.5 s stable foreground. Worker test RED→GREEN.
Live follow-up: readonly capture errors overwrote real pause reasons. Preserve stop reason; regression test RED→GREEN. Added opt-in local text action log, not automatic screenshots.
Live follow-up: queued SendInput motion can be observed before cursor update. Ordinary drag motion uses synchronous SetCursorPos; mouse-down retains atomic move+down batch. Wrapper test RED→GREEN; insertion delay after down allows queued event processing before first drag step.
Environment limit: tool-launched process receives WinError 5 from SetCursorPos. Final measured start logged FILL requested then pause before Ctrl/button down. Live queue filling, attack continuity, live hotkeys and rotation remain unverified; do not claim them passing. User desktop launch is needed for the next live check.

Final verification: isolated .venv full suite 48/48 PASS; compileall PASS; pip check clean. Sample image diagnostics: 2 beam candidates, blue selection 0. Live input remains blocked as documented above.

Follow-up authorized by user: self-activate selected window on Start. GUI activates once via SetForegroundWindow without a world click; actual foreground required before arm. Tests RED->GREEN.
Root-cause evidence: user reproduces after manual launch; readonly TokenElevation game29516=True, bot34860=False. Earlier tool-only attribution retracted. Standard Windows UIPI restricts SendInput to equal/lower integrity. Actual SetCursorPos failure alone does not identify every cause.
Canonical input change: remove SetCursorPos, await delivered SendInput position <=250ms. No backend fallback. Delayed-delivery and absent-delivery tests RED->GREEN. SendInput errors expose exact stage/count; optional log includes traceback.
Privilege preflight shared by GUI activation and guarded actions. Pure readonly permissions.py is reused by tools/check_permissions.py; no duplicated token logic. Elevated game + ordinary bot refuses before input with manual Run-as-administrator instruction; no automatic UAC/elevation. Tests RED->GREEN.
Verification: 54/54 PASS, compileall PASS, pip check clean. Live Computer Use activation failed twice; live full action remains unverified. Our diagnostic instance only (34860/40644) terminated to release hotkeys. User must manually confirm UAC for further live verification.

Next user report: administrator launch reaches Activation modal, not WinError5. Root code issue: immediate GetForegroundWindow after cross-process SetForegroundWindow can be premature. Replace synchronous activate assertion with request_activation; existing Worker stability/12s deadline is the canonical gate. No OS lock bypass and no input without observed foreground. Tests RED->GREEN; suite55/55 PASS, compileall PASS. Live transition still unverified.
New user-selected direction: individual metin clicks, target-panel lifecycle, per-server profiles and color picker. Written new architectural spec created for review; implementation awaits spec approval under brainstorming gate. Queue-mode implementation is not yet removed because migration is not yet approved/implemented.
