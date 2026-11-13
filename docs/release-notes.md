# Companion 0.1.0

The first stable Companion release: a Metin2 bot for Windows with a compact Metin-style panel, configurable actions and recognition overlays.

## Main features

- **Connect and configure:** choose a game window from screenshot tiles, keep separate profiles and select colors or areas directly over the game.
- **Recognize targets:** use named color presets, exclude chat/minimap areas and confirm targets through a fixed HP-panel detail.
- **Choose actions:** buffs, loot, mount behavior, periodic keys/clicks, visual rules, auto-puller and out-of-target attacks.
- **Follow the run:** activity, countdowns, complete-cycle statistics, target outlines and event history.
- **Set limits:** automatic stops, breaks, recovery attempts and local alerts.
- **Keep your setup:** duplicate or export/import profiles, review edits and update inside the app.

## Latest improvements

**Unsaved changes** groups related edits and shows Saved/Edited values with color or image previews. A color selection or moved area counts as one change. Incomplete buffs are clearly identified before saving.

**Check recognition** opens a separate dialog with current/reference images, scores, explanations and retry. It helps distinguish a poor match from a capture problem without sending game input.

The panel remembers size and position for each game window. Profile controls stay aligned, and narrow layouts and 150% UI scale have been checked.

## Installing or updating

Download **CompanionSetup** from [Releases](https://github.com/ethanjojo/metin2-bot/releases), or use **Settings → Updates**. Profiles are retained, and Companion restarts paused.

Requires 64-bit Windows 10 version 2004 or newer, Direct3D 11 and administrator approval. Setup installs the required shared input driver and discloses Windows certificate-trust and GameInput changes. App and installer are unsigned. Read [installation details](installation.md) before installing.

Keep the selected game in the foreground. Detection depends on your own calibration and captures; check the setup for your server and game layout.

[User guide](user-guide.md) · [Actions guide](actions.md) · [Changelog](../CHANGELOG.md) · [Technical release procedure](releasing.md)
