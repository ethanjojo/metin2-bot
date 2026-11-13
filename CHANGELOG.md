# Changelog

## Unreleased

- Reusable named Metin detection presets with complete color/shape settings, private cancellable editor and self-contained per-profile copies. Recognition now owns detection and full-client HP-reference controls.
- Stable header Save confirmation, aligned inline profile/exclusion rename controls and bounded select menus at narrow/scaled sizes.
- Atlas-shaped native windows in expanded/collapsed/helper views, plus remembered per-client expanded size and position with a shared reset.
- Session throughput toggles between Metins/min and average seconds per target using the complete active cycle, including search and loot.
- Fix Windows error 33 during in-app updates: retain the exclusive profile-store lock while excluding only its transient control file from backup hashes and copying; all profile data and reference images remain verified.
- Precise Run pause causes with details/recovery and distinct setup readiness; one validated Save & Start path for F7 and the main button, including hidden/minimized-window logic and short-lived start requests.
- Cached unsaved-field summaries, retained Actions details with breadcrumbs, DPI-aware per-client pinned panel positions and percentage UI scale with reset.
- Read-only fresh HP/buff recognition checks with actual shared matcher scores, reference/current crops and Unknown/evidence-age explanations.
- Independent profile duplication and bounded portable profile import/export with all images, explicit preview and no overwrite or runtime-state transfer.
- Named excluded areas with inline rename/show/edit/delete/undo and lossless loading of existing rectangle-only profiles.

- Startup update check with Continue/Skip, preserved explicit opt-out and an in-app update progress window backed by the existing installer; manual update controls remain in Settings.
- Canonical phase countdowns in Run, including the next search transition; bare activity artwork and complete recent-event rows fitted to the remaining space.
- Consistent Settings/Safety headings and separators, Add buff terminology, and tooltips bounded to the viewport when resized or scaled.
- Keep Windows capture runtime resources alive across repeated thumbnail/capture sessions, preventing premature native runtime teardown.
- Real UAC/install/restart acceptance of the new update helper remains pending; offline fixtures and package checks do not establish that acceptance.

## 0.1.0-rc.1

First public Companion release candidate for Windows x64.

- Compact Metin-style interface with Run, Actions, Recognition, Safety and Settings, searchable profiles, native icons and fixed Start/Pause and mount controls.
- Automatic window discovery with screenshot thumbnails, session-based connection, pinning and collapse controls.
- Direct frozen-capture selection with dimming, move/resize, color preview, cancellable edits and explicit calibration review after a client-size change.
- Full-client HP-detail matching, color detection, exclusions and click-through game annotations. Current targets keep their outline without a persistent click-position cross.
- Ordered timed/icon-based buffs, animated-icon stabilization and a 44-skill English icon picker across four classes.
- Loot, mount behavior, periodic key/click actions, visual rules with bounded verification, combat-triggered/repeated puller use and bounded out-of-target key holds.
- Active-time/local-time stop limits, scheduled breaks, recovery limits, local alerts and event history.
- Background update checks/downloads with Stable/Preview channels, size/SHA256 validation and explicit Install and restart. Restart stays paused.
- One pinned HIDMaestro NativeAOT input path with focus, geometry, privilege, cancellation and balanced-release checks; setup discloses required machine changes.
- Companion executable, install/data folders and application identity; verified migration and retained backups for previous default paths.
- Windows setup/checksums, native dependencies and third-party notices, exact-commit CI and installer acceptance.
- MIT license for project-authored source code, with separate third-party/artwork terms.

The complete integrated live-game cycle remains an acceptance requirement for a stable release. See [release notes](docs/release-notes.md).
