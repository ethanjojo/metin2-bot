# Changelog

## Unreleased

- Integrate compact mount confirmation into Run activity, link scheduled items to their exact editors, and open full event history in a scrollable dialog without moving the Run page.

- Redesign Run around activity, actionable setup, game preview, session statistics and upcoming actions. Add preview control and a five-event summary linked to the full-history dialog.
- Share mount confirmation between Run and Actions; connect and declare without a Snapshot or valid draft, while keeping Start blocked until configuration is ready.

- Fix safety capture budgets, refresh buffs during uninterrupted search, periodic-action search accounting, committed-save cleanup reporting and mount declarations across sessions.
- Capture paused previews only on demand, reuse readback textures, copy only requested image regions and avoid cloning profiles for unsaved-state checks.

- Separate visual-rule notification, immediate-pause and action choices into checkboxes; keep pause after completion and migrate saved reactions without changing their alert behavior.

- Move visual rules to Actions, remove event presets, and add guarded key/click reactions with reference-disappearance verification, bounded retries and pause/continue choices. Explicit wait-only mode records an unverified outcome.
- Add calibrated clicks and a first-use choice to periodic actions. New actions wait for the first interval; existing saved actions keep their immediate first use.

- Trigger puller sequences at confirmed combat start or periodically during combat; add configurable, bounded attack-key holds after combat and during long searching, with ongoing vision and guarded release.

- Added grouped Actions/Settings navigation, periodic actions, configurable combat-triggered auto-puller, calibrated visual rules, bounded recovery, local alerts, preview performance controls and diagnostic export.

- Split navigation into Run, Calibration, Actions, Safety and Settings.
- Add profile-specific loot, active-time/local-time stop limits, safe-boundary breaks and session event history.
- Preserve existing buff, mount, calibration and guarded-input workflows.

## 0.1.0-rc.1

- Rust/egui desktop interface with Run, Calibration, Buffs and Settings.
- Local target recognition, target tracking, buff scheduling and mount state.
- Per-profile searchable window selection and guarded Windows capture/input.
- Interactive regions, excluded areas, color picker, crop validation, dimension overlays and zoom.
- Ordered skills with compact move/delete controls and active/inactive icon capture.
- Light/dark themes, application branding and platform icon resources.
- Profile import, dependency notices, verified package inventories and release checksums.

This is a prerelease. See [release notes](docs/release-notes.md) for acceptance requirements and platform limits.
