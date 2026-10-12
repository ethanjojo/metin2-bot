# Metin Bot

A local Windows application that finds and destroys Metin stones one at a time in a selected game window. Detection uses color, size and stability across frames. The bot clicks one target, confirms its target panel, and selects the next target only after that panel has consistently disappeared. It includes server profiles, interactive calibration and automatic buffing.

The UI and this guide are in English. Custom profile and skill names are preserved. Processing stays local: the bot does not read game memory or upload screenshots or profiles.

## Install the standalone application

Download `MetinBotSetup-<version>-windows-x64.exe` from the repository Releases page, run it and confirm UAC. The installer places the application and bundled Python/libraries in Program Files and creates a Start menu shortcut; a desktop shortcut is optional. Python and pip are not required on the user machine. Starting MetinBot requests UAC through its native manifest. Uninstalling keeps user profiles and settings.

The first installed launch offers to import profiles and settings from an old source or data folder. Select the folder containing `profiles/`, `ui-settings.json` or legacy `config.json`; originals are preserved. Choose No for a fresh profile or Cancel to stop without creating new data.

## Requirements for running from source

- Windows; Windows 11 is recommended. Native title-bar colors use DWM attributes supported on Windows 11 build 22000+. Unsupported attributes are recorded in the log.
- **64-bit Python 3.14**, including Tkinter/Tcl/Tk. This is the verified environment; other Python versions have not been verified.
- Windows PowerShell 5.1 for the launcher, normally included with Windows.
- A writable project directory and a game running in a window. During operation, the game must be visible and in the foreground, with dimensions and UI layout matching its calibration.
- Dependencies from [requirements.txt](requirements.txt):

| Library | Version | Purpose |
| --- | --- | --- |
| NumPy | 2.3.5 | Frame processing |
| OpenCV (`opencv-python`) | 4.13.0.92 | Detection and template matching |
| Pillow | 12.2.0 | Screen capture and UI images |

Tkinter, ctypes, JSON and unittest are included with Python. No GPU, Node.js or compiler is required.

## Installation

Open PowerShell in the project directory:

```powershell
py -3.14 -m venv .venv
.\.venv\Scripts\python.exe -m pip install -r requirements.txt
.\.venv\Scripts\python.exe -m pip check
```

You do not need to activate the virtual environment. If `py` is unavailable, replace `py -3.14` with the path to your Python 3.14 interpreter. Do not copy the virtual environment to another computer; recreate it there.

Check the environment without opening the GUI or requesting elevation:

```powershell
powershell.exe -NoProfile -ExecutionPolicy Bypass -File .\tools\launch.ps1 -CheckOnly
```

## Starting the application and UAC

Double-click **[run.cmd](run.cmd)**. The launcher checks the environment and starts the local pythonw.exe with an administrator request. **Windows displays a UAC prompt that you confirm manually.** You do not need to right-click and choose Run as administrator. If the launcher is already elevated, Windows may not ask again. Cancelling UAC prevents the bot from starting; on failure, the console stays open with an explanation.

This uses [standard Start-Process -Verb RunAs](https://learn.microsoft.com/en-us/powershell/module/microsoft.powershell.management/start-process?view=powershell-5.1). The launcher does not change UAC configuration or the persistent execution policy. `-ExecutionPolicy Bypass` applies only to that PowerShell invocation. Direct source execution does not request elevation; the packaged executable requests it through its manifest.

The bot always starts stopped. The default log is `%LOCALAPPDATA%\MetinBot\logs\run.log`. The launcher sets the project directory as the working directory, so it can be opened from another directory as well.

For development, start it manually without requesting elevation:

```powershell
.\.venv\Scripts\python.exe -m metin_bot --log diagnostics/run.log
```

If the game is elevated and this command is not, the bot rejects input. Use `run.cmd`, or run both the game and bot without elevation. The UAC request opens this application; it does not install anything.

## Building, testing and development

Build on Windows with 64-bit Python 3.14.0. Runtime dependencies and build dependencies are pinned separately. The same build script is used locally and by GitHub Actions.

```powershell
.\.venv\Scripts\python.exe -m pip install -r requirements.txt -r requirements-build.txt
.\.venv\Scripts\python.exe -m pip check
.\.venv\Scripts\python.exe -m compileall -q metin_bot tools tests packaging_entry.py
.\.venv\Scripts\python.exe -m unittest discover -s tests -v
$compiler = powershell.exe -NoProfile -ExecutionPolicy Bypass -File tools/prepare_compiler.ps1
powershell.exe -NoProfile -ExecutionPolicy Bypass -File tools/build.ps1 -Version 0.1.0-dev.1 -BuildNumber 1 -CommitSha 0000000 -CompilerPath $compiler
```

`prepare_compiler.ps1` downloads the pinned official Inno Setup 6.7.3 installer into ignored `build/tools`, checks its SHA-256 and publisher signature, and installs the compiler in portable mode there. It does not install a global development tool. `build.ps1` verifies dependency pins/compiler version and creates:

- `dist/windows/MetinBot/MetinBot.exe` and its `_internal` directory: the complete directory bundle. Do not distribute the executable alone.
- `dist/release/MetinBotSetup-<version>-windows-x64.exe`: the single distributable installer.
- `dist/release/SHA256SUMS.txt`: the installer checksum.

The local `0000000` SHA means an unversioned development build; CI embeds the actual commit SHA. `BuildNumber` and each numeric version component must fit 0–65535. Build tools do not silently install missing Python packages.

Tests include real Tk widgets; run them in a Windows desktop session. Game input is replaced with test APIs. Restart an open bot after source edits. See [docs/verification.md](docs/verification.md) for evidence and limits. Compiling the installer or passing a self-test does not prove compatibility with every game server.

### Isolated self-test

```powershell
.\.venv\Scripts\python.exe -m metin_bot --self-test --self-test-report build/selftest-source.json
```

The packaged executable accepts the same options and requires its normal UAC confirmation. It tests Tk/Pillow, both themes, synthetic OpenCV/NumPy detection and profile/preference roundtrips in a disposable directory. It never selects a game window or touches existing user data. The JSON report records build identity and failures. `--self-test` cannot be combined with normal data/import/log overrides.

### Automated GitHub releases

[.github/workflows/release.yml](.github/workflows/release.yml) runs on Windows Server 2025:

1. Pull requests to `main`: validate, test, build and verify an installer; no publication permissions.
2. Pushes to `main`: the same checks, followed by a prerelease such as `v0.1.0-dev.42.abcdef0`.
3. Tags such as `v0.1.0`: publish a stable release. The tag must exactly match `v` plus [VERSION](VERSION) and point to a commit on `main`.
4. Manual workflow runs: publish a prerelease only when run on `main`.

The pipeline installs the built installer in its disposable hosted Windows VM, runs the installed executable's self-test, uninstalls it, then uploads the verified installer/checksum. A separate publication job alone gets `contents: write`. It resolves the remote tag to the tested commit, stages a draft, uploads both assets and publishes after success. Interrupted drafts can be resumed for the same commit; published releases are left unchanged. A moved/conflicting tag is rejected. No personal token is required; the workflow uses `GITHUB_TOKEN`.

To publish a stable version, update VERSION to `1.0.0`, commit it to main, then:

```powershell
git tag v1.0.0
git push origin v1.0.0
```

GitHub Actions must be enabled and repository/organization policies must allow the release job's token to write contents and the pinned actions to run. The workflow run number must fit 1–65535. These workflow files need to be committed to a GitHub repository before remote execution can be verified.

Release binaries are currently unsigned; no signing certificate is configured. SHA-256 checksums provide integrity checks, not publisher authentication. The native UAC manifest remains present.

## Initial calibration

1. Select or create a profile, then select the game window under **Window**. The refresh icon reloads the window list.
2. Select a Metin manually in the game so its target panel is visible. Click **Snapshot** in the application; calibration uses a frozen frame.
3. **Search area:** select the playable area where Metins should be detected.
4. **Excluded areas:** add masks for inventory, minimap, hotbar or other similarly colored objects. The target panel area is excluded automatically.
5. **Metin color:** click the stone's surface. Green boxes show candidates. Under **Settings**, adjust tolerances, minimum area/width/height and maximum area ratio. Use minimum dimensions to exclude small false detections.
6. **Target panel:** select the panel area. **Fixed crop:** select a distinctive, unchanging part inside it, such as an icon and close button. Avoid HP, names and other changing details. The crop must be at least 5×5 pixels.
7. **Save** stores the profile. **Start** also saves and requests game activation. If activation is refused, activate the game manually and press F7. Starting requires stable foreground focus, with a minimum delay of 3 seconds and a timeout of 12 seconds.

Changing game dimensions requires recalibration. After changing its UI layout, check masks, the target panel and buff icons. Calibration does not automatically adapt to arbitrary window sizes.

## Editor controls

| Control | Action |
| --- | --- |
| Drag inside an area / drag a handle | Move / resize |
| **New area**, drag outside the selection | Create an area or mask |
| **Delete** | Remove the selected area |
| Escape | Cancel the current drag |
| Ctrl+Z / Ctrl+Y or toolbar icons | Undo / redo; 32 states for the current profile |
| Mouse wheel or −/+ | Zoom up to 800% |
| Middle mouse button | Pan the frame |
| Fit icon | Fit the full frame into the preview |
| **Detections** | Show detection boxes; preview alone sends no game input |
| Numeric field arrows, ↑/↓ | Adjust by an appropriate step within valid limits; typing remains available |
| Moon/sun button | Switch dark/light theme, including the native title bar |

The selected calibration step has a blue highlight and handles. Hovering another step highlights its area without changing the selection. Clicking a tab does not automatically select the first input. The first icon above the preview hides/shows the sidebar. An empty preview explains the next required step.

## Running and statistics

**F7** in the selected game toggles start/pause. **F8** stops operation. **Pause** also cancels waiting for activation. Lost foreground focus, an obstructed click point, manual mouse movement during input, changed dimensions or capture/input errors pause the bot. Closing releases held inputs and system hotkeys.

By default, candidate/panel confirmation requires three frames at a 0.25-second capture interval. Target acquisition times out after 20 seconds, an active target after 120 seconds, and settling after panel disappearance takes 1 second. With no candidates for 5 seconds, the bot briefly rotates the camera using E, up to four times. Adjust timing and thresholds under Settings.

- **Active time:** running time excluding pauses.
- **Targets ended:** confirmed panel disappearances, not independent proof that a Metin was destroyed.
- **Clicks:** target clicks sent.
- **Unconfirmed:** an unconfirmed target panel or target timeout.

Resetting statistics does not change skill timers or safety limits. Pausing or saving the same profile preserves statistics; a new profile/window starts a new session. Statistics are not persisted after closing. The information icon or tooltip shows the full current explanation. **Frozen frame / Live preview / Stale frame** distinguish calibration from current capture.

The bot addresses Windows input to the selected window. Windows accepting a message does not prove the game reacted; target selection is confirmed by the game panel. Compatibility depends on the client and calibration. Automatic WASD exploration, healing, login, Ctrl-drag queuing and multiple simultaneous clients are not implemented.

## Automatic buffing

Under **Buffs**, enable **Automatic buffs** and use **+ Add skill**. Click a keybind input and press the desired key or combination. Escape cancels capture; Delete/Backspace clears a skill shortcut, but the mount shortcut cannot be cleared. F7/F8 are reserved for the bot. Up to eight skills are supported; empty shortcuts are skipped. Names are optional; ↑/↓ moves the complete skill configuration and × removes the skill.

| Mode | Behavior |
| --- | --- |
| **Refresh periodically** | A separate interval of 10–86400 seconds (up to 24 hours). Choose **s/min**, for example 240 min. Switching units preserves duration; profiles store seconds. |
| **Activate when inactive** | A toggle buff without an interval, such as a blade buff. Icons are checked between Metins; the skill is activated only when inactivity is confirmed. |

### Buff icons

Use **Edit active buff area** to select a separate buff-icon or hotbar area. It does not need to be inside the Metin search area or target panel.

- **Active icon:** select the active appearance including its activity indicator, without a tooltip or changing countdown.
- **Inactive icon:** for hotbar detection, also select a distinct inactive appearance after manually disabling the buff and taking a new Snapshot. Both crops must be inside the buff area; identical images are rejected.
- **Reuse active/inactive area:** captures the current appearance at the other template's saved coordinates. It does not copy its image. Each × button removes only its own template.
- For a buff icon that disappears when disabled, the inactive template may be omitted. A persistent hotbar icon needs both appearances. Without an active template, recalibration is required.

With both templates, matching requires at least 90% similarity, an 8-percentage-point lead for the winning template and three fresh frames confirming inactivity. Uncertain states are skipped; black frames/errors do not count as inactivity. With only an active template, confirmed icon disappearance is required, making this unsuitable for persistent hotbar icons.

### Mount state and timers

**Fight mounted** controls whether the character remounts after buffing and fights while mounted. **Current mount state – On/Off:** On means mounted, Off means on foot. You can set this before choosing a window. It is saved automatically outside the profile and restored for new connections; the default is Off. With a window attached, changing it pauses operation and immediately sends the declaration to the worker, without a confirmation button or Save.

The default mount shortcut is **Ctrl+G**. **Mount settings** contains the shortcut, wait and skill-key hold duration. The actual mount state is not detected from the image: after changing it manually, correct On/Off. A failed mount input pauses operation and requires checking the state; the UI keeps the last known value.

Regular buffs are initially prepared when starting and used after any active target ends and panel absence is confirmed. When buffs are due, the bot finishes the current Metin, dismounts if needed, uses due skills in order with their waits, and remounts if configured. Active/uncertain toggle buffs are not switched. A check without skill use does not trigger dismounting.

A regular skill's **Next use** HH:MM:SS countdown starts after its key input and its own wait; pausing stops the countdown. Reset ↻ clears only that timer and prepares use after the current Metin or after resuming. A skill is used at most once before the next target attempt. Changing buff configuration prepares a new initial cycle; renaming alone does not reset timers. Save configuration with **Save/Start**. The bot does not detect skill cooldowns; it relies on the configured waits.

## Data and project layout

| Path | Contents |
| --- | --- |
| `metin_bot/` | Application, detection, runtime, Windows input and UI |
| `run.cmd`, `tools/launch.ps1` | UAC launcher and environment check |
| `requirements.txt` | Pinned runtime dependencies |
| `tests/`, `tests/data/` | Tests and versioned images |
| `docs/` | Designs and verification evidence |
| `%LOCALAPPDATA%/MetinBot/profiles/<UUID>/profile.json` | Profile and references to local PNG templates |
| `%LOCALAPPDATA%/MetinBot/profiles/selection.json` | Last selected profile |
| `%LOCALAPPDATA%/MetinBot/ui-settings.json` | Theme and last mount On/Off value |
| `%LOCALAPPDATA%/MetinBot/logs/run.log` | Local log without automatic screenshots |
| `profile.example.json` | Safe, incomplete example profile |

JSON and templates are saved atomically. All writable application data belongs to the Windows account running the application and lives under `%LOCALAPPDATA%/MetinBot`. Before manual editing or moving it, close the bot and back up the entire data directory. Legacy source-folder data is copied on the first source launch when canonical storage is empty; originals are retained. Legacy config.json is converted in the copied storage and backed up as config.queue-backup.json. Existing canonical data wins as a whole; migration never silently merges or overwrites it.

For an explicit first import, run `MetinBot.exe --import-data "C:/path/to/old-project"` before creating new data. An import requires an empty destination. `--data-dir <directory>` is an explicit advanced/testing override and disables automatic legacy discovery. Changing Windows accounts, including elevating with another administrator account, uses that account's LocalAppData.

.gitignore excludes .venv, caches, local profiles/settings, logs, screenshots and build output, including profile PNG templates, selection files and temporary JSON writes. Diagnostic Python source scripts and tests/data remain versionable.

## Troubleshooting

| Problem | What to check |
| --- | --- |
| Missing environment/libraries | Installation steps above, `-CheckOnly`, `pip check`. |
| UAC cancelled | Open run.cmd again and confirm the Windows prompt manually. |
| Game missing from the list | Start the client and refresh the window list. |
| Game cannot be activated | Activate it manually and press F7; elevation does not guarantee foreground focus. |
| Cursor moves but the game does not react | Privileges, obstructions, click location, Cursor settling/Mouse hold; compare a manual click and inspect the log. |
| Small false Metin detections | Minimum dimensions/area, narrower color tolerances and masks. |
| Target panel not confirmed | New Snapshot, correct panel area, distinctive Fixed crop and matching threshold. |
| Toggle buff skipped | Different active/inactive templates, buff area and the reported reason. |
| Mount state incorrect | Correct On/Off and verify the manual mount shortcut. |
| Old UI/behavior after edits | Restart the application after source changes. |

Read-only privilege checks for the current process and a game PID:

```powershell
.\.venv\Scripts\python.exe tools/check_permissions.py
.\.venv\Scripts\python.exe tools/check_permissions.py 1234
```

Replace 1234 with the actual PID. Additional explicit local diagnostics are available through `tools/check_capture.py --help`. Interactive UI fixtures under diagnostics/ use separate data, but some depend on local reference screenshots and are not portable clean-checkout tests.

## Packaging design and verification limits

The application uses a PyInstaller directory bundle (`onedir`) with a windowed executable and native administrator manifest. Inno Setup installs its libraries under protected Program Files. Profiles, preferences and logs stay in LocalAppData; upgrades replace obsolete bundled dependencies and uninstall retains user data. One shared entry point handles source and packaged launches.

References: [PyInstaller operating modes](https://pyinstaller.org/en/stable/operating-mode.html), [runtime paths](https://pyinstaller.org/en/stable/runtime-information.html) and [GitHub Windows runner privileges](https://docs.github.com/en/actions/reference/runners/github-hosted-runners). Hosted Windows runners are elevated with UAC disabled, allowing automatic installed self-tests; this does not test a user's interactive UAC confirmation. Verify a release on a separate Windows computer without Python and calibrate it for the actual client before relying on gameplay automation.
