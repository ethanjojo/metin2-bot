# Windows prerelease

MetinBot includes a Rust/egui interface with Run, Calibration, Actions, Safety and Settings, searchable per-profile window selection, window capture, interactive calibration, skill ordering, icon recognition and light/dark appearance.

## Run overview

Run groups the current activity, reason, countdowns and compact mount declaration in one panel. Mount confirmation is shared with Actions and needs only a connected game window, not a Snapshot or finished calibration; confirming pauses automation without sending a game key. A confirmed state offers Change, while an unknown state shows On foot / Mounted choices.

Action and skill names open their exact settings. The overview keeps the latest five events; full history opens in a dialog with its own scrolling, warning filter and clear controls. Closing the dialog preserves the Run page position. Preview can be switched off without changing active detection or safety processing.

## Installation

Download the Windows x64 installer or portable ZIP and verify it against `SHA256SUMS.txt`. Extract the complete ZIP if using the portable package. Runtime dependencies are included. Windows 10 version 2004 / build 19041 or newer and a D3D11-capable graphics device are required.

Packages request the normal administrator confirmation and are unsigned. `jojo` is product metadata, not a verified Windows publisher.

## Data

Back up the complete MetinBot data folder before upgrading. Profiles, assets and settings are kept outside the installation directory and survive uninstall. Import copies data without modifying its source. Windows stores data under `%LOCALAPPDATA%/MetinBot`; keep profile JSON and PNG assets together when backing up.

## Known limitations

- Live-game acceptance of targeting, attacks, buffs and mount handling is pending.
- Hosted installation, upgrade and uninstall checks must pass for the final artifact before publication.
- Linux supports an editor/recognition preview. Linux and macOS automation is disabled because capture/input are unavailable.
- Full native macOS execution and Finder/Dock appearance still require verification.

When reporting an issue, include the application version/commit, operating-system version, reproduction steps and relevant log excerpt. Review logs and screenshots for private data before sharing.

Action scheduling includes configurable pickup after confirmed target ends, active-time/local-time automatic stopping, deferred breaks and bounded session-event history. All new automation options default to off. Live-game acceptance of these additions is pending.

Actions additionally includes named periodic keys/clicks, user-calibrated visual rules and an auto-puller triggered at confirmed combat start or repeated during combat. Visual rules have no event presets: configure a reference and choose a notification, pause or guarded input. Inputs can verify reference disappearance with bounded retries, or explicitly finish after an unverified delay, then pause or continue from fresh recognition. Periodic actions choose immediate first use or waiting for the first interval; new actions default to waiting, while existing profiles retain immediate first use. Click regions are relative to the calibrated game window. Safety includes bounded recovery and optional local sound/system alerts. Settings separates detection, timing presets, appearance/performance and diagnostics. Run links events to their settings and shows upcoming action timers. New controls default off; image rules require explicit calibration. Out-of-target attacks can hold a configured key after combat or during long searching, with bounded attempts and guarded release. These additions still require live-game acceptance.
