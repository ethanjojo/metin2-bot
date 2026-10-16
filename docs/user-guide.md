# User guide

## Profiles and game window

Use the top bar to create or switch profiles. Save persists the complete calibration, settings and skills. Each profile remembers its game-window selection. In Calibration, open the window picker, search by title/process and refresh when a client opens/closes. Search text is temporary. Ambiguous/closed windows need an explicit selection; the app does not attach to an arbitrary replacement.

## Calibration

1. Select the correct game window and take a Snapshot. Keep the game dimensions/UI layout consistent with this calibration.
2. Set a search area containing the playable scene. Exclude the inventory, minimap and other false matches with excluded areas.
3. Set Metin color using the color controls/picker. Detection settings control tolerance, dimensions and stability.
4. Select the target panel and a distinctive reference crop inside it. Avoid changing HP/name regions. A reference crop must be at least 5 by 5 source pixels and contain enough visual detail; a uniform crop can be rejected even when large enough.
5. Enable Detections to inspect results; enable Sizes (px) independently to display region dimensions.
6. Save and inspect the readiness information in Run before starting.

Editing mode is shown in the canvas toolbar. Click an area/capture control, then drag on the frozen image; Escape cancels the current operation. Selection instructions and compact control explanations are available on hover. Trash icons remove the corresponding area/color/icon. Drag inside a selected rectangle to move it; drag its handles to resize. Undo/redo restores edits within the current profile. The wheel or +/- zooms, middle-drag pans, and Fit restores an overview.

Live preview observes the selected game window and sends no input. Take a Snapshot to edit. Windows capture targets the selected window rather than whatever occupies the desktop. A capture/permission error is displayed instead of falling back to browser/desktop pixels.

## Buffs

Select a skill to edit its shortcut, mode and timing. The two compact arrows beside its list button reorder it; the adjacent trash icon removes that skill. Recognition is relevant to Activate when inactive skills. Supply the required area and active/inactive icon references according to the configured mode.

Capture controls open Calibration with the skill name and selected capture type. Take/verify the crop, inspect its thumbnail, then use Done & back to buffs. A failed capture shows the validation reason and does not replace a valid saved icon. Configure the actual current On/Off state before running when the skill requires it.

## Run and stop

The bot always starts stopped. Review the readiness message, save, and use Start or F7. Run displays current activity and a read-only game preview. Detection colors distinguish the current target, next eligible target and other candidates. The game must be foreground and match the calibration. The application pauses/rejects unsafe input when window identity, focus, size, permissions or manual-input guards fail.

Pause or F8 stops the worker; do not rely on closing a game window as the primary stop control. Test the full game cycle before unattended use. Linux/macOS currently have no accepted capture/input backend and therefore cannot Start automation.

## Settings and appearance

Settings contains recognition, timing and safety options. Invalid values prevent the affected action and display a reason. The far-right sun/moon control switches theme and remembers the choice. Runtime Windows/X11 icons follow that choice; Wayland launchers use the installed contrast icon. macOS has a bundle/Dock implementation awaiting native acceptance.

## Data and troubleshooting

Windows data: `%LOCALAPPDATA%/MetinBot`; Linux: `$XDG_DATA_HOME/MetinBot` or `~/.local/share/MetinBot`; macOS: `~/Library/Application Support/MetinBot`. Logs are in `logs/run.log` beneath that folder. Keep profile JSON and PNG assets together when backing up/importing.

If no window appears, refresh and confirm the client is running. If Start is unavailable, read its readiness explanation rather than repeatedly clicking it. If capture/input fails, check the selected window, foreground focus and privilege level. Recalibrate after resizing or changing the game's UI layout.

A diagnostic self-test uses temporary data and no game input:

```powershell
.\MetinBot.exe --self-test --self-test-report C:\Temp\metinbot-selftest.json
```

Use a writable report folder. The release executable still requests its normal UAC confirmation. Self-test verifies storage, vision and headless themes; it does not prove game reaction, native GPU rendering or clean-machine installation.
