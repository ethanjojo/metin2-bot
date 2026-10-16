# Searchable window selection per profile — 2026-10-16

Approved scope: remove the process filter from Settings, list all visible application windows, provide temporary search and remember a window separately for each profile. Preserve capture/input guards and never start automation automatically.

## Behavior and data

- Calibration's Game window picker searches titles, process filenames and exact PIDs. Matching search ignores case and surrounding whitespace. Clear resets only the transient search; switching profiles clears it too. Search and the full catalog never enter saved configuration.
- Each row identifies the title, process and PID; long rows truncate and expose their full description on hover. The popup remains open when entering/clearing search and closes after selecting a window, pressing Escape or clicking outside.
- `UISettings.profile_windows` is the only source for remembered choices, keyed by profile UUID with typed `WindowPreference { process_name, title }` values. A selection is written atomically immediately; it does not dirty calibration, and Save/Discard of profile edits does not change this UI preference. A failed write reports the error and does not claim the preference was saved or switch the active target.
- Only one exact remembered title/process match is automatically preselected. Process filenames ignore ASCII case; titles match exactly. Several matches require manual selection, with PID to distinguish instances. Missing windows remain unselected with a clear message. There is no process-only or fuzzy automatic fallback. Existing attached windows keep their verified runtime token if their title changes.
- Every successful enumeration publishes a revision, including an empty list. Startup, profile changes and Refresh reconcile against that list. A selected token removed by Refresh is stopped and detached before any new unique match is configured. Remembered HWND/PID/creation times are never persisted. Configure preserves the existing platform identity checks; it sends no Start command.
- The existing platform enumerator already excludes the bot's own process. Unavailable backends remain visibly unavailable. This change does not implement Linux/macOS gameplay support.

The old `Profile.process_name` JSON field remains solely for the existing Python/import/export profile contract and its boundary validation. It no longer filters or preselects windows anywhere in the Rust UI. Removing that field would break existing profile interchange; no legacy runtime filter or alternate selection path is retained. Remove the interchange field with an explicit profile-schema migration when Python compatibility is retired at the T9 cutover. Earlier UI reports describing a process filter are historical.

## Verification

The initial tests reproduced the missing preference schema and obsolete filter section. Actual egui pointer/typing regressions caught the popup's default close-on-any-click behavior; it now explicitly closes on outside clicks or selected rows. Tests cover search/Clear and selecting another process, independent profiles and application restart with a new runtime token, ambiguous/missing/renamed/closed windows, failed preference writes, Unicode search, validated settings upgrade and data import. A worker regression checks enumeration revisions without configuring or starting input.

Native screenshot fixtures use synthetic catalogs and unavailable automation capabilities. They are compiled only for tests or the existing ui-smoke feature; standard release builds exclude the fixtures and CLI flag. Both fixtures and production use the same picker renderer and match/selection helpers. Synthetic screenshots do not establish live-client gameplay or native input acceptance.

Windows passed 112 whole-workspace tests, including 49 UI tests, and 47 UI tests in the standard build without ui-smoke. Final UI tests were rerun after improving screenshot settling. Formatting, strict all-target/all-feature Clippy, native debug build and executable storage/vision/both-theme self-test passed. Logs: `build/rust-migration/window-selection-{tests,ui,default,fmt,clippy,build}.log`, `window-selection-self-test.json`. Screenshot capture waits for three real layout frames before starting its 700 ms settling delay, preventing startup GPU work from capturing a newly opened popup mid-animation.

The Windows visual matrix is `build/window-picker-ui/{dark,light}-{all,filtered,empty,ambiguous,missing,selected}-{large,small}.png` (24 native screenshots at 1280×820 and 800×560). Four updated Settings screenshots are in `build/ui-redesign/{dark,light}-settings-{large,small}.png`. Popup width is bounded, long rows truncate, no-result/Clear states remain readable and ambiguous equal titles expose distinct PIDs. The currently open user app and production profiles are untouched.

Final Linux results, final visual inspection and package identity are recorded below after completion.
