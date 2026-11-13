<img src="packaging/icons/companion-256.png" alt="Companion logo" width="72">

# Companion

A compact Windows desktop app for visual target recognition and configurable actions, built with **Rust**, **egui** and **OpenCV**. Profiles and image processing stay on your computer.

[Download](https://github.com/ethanjojo/metin2-bot/releases) · [User guide](docs/user-guide.md) · [Development](docs/development.md) · [Changelog](CHANGELOG.md)

<a href="docs/assets/run.png"><img src="docs/assets/run.png" alt="Companion Run overview with activity, timers, statistics and mount controls" width="460"></a>

*Screenshots are actual application views with an isolated demonstration profile and a synthetic scene; they do not show a live game session.*

## Features

- **Run:** bare activity artwork, countdowns to the next timed step, session statistics, mount confirmation and recent events fitted to the available space.
- **Recognition:** reusable Metin detection presets with color and shape settings; fixed HP-detail matching; direct frozen-capture selection and live game annotations.
- **Actions:** ordered buffs with icon recognition, loot, mount behavior, periodic keys/clicks, visual rules, auto-puller and bounded attacks outside a selected target.
- **Safety:** automatic stops, scheduled breaks, limited recovery, local notifications and emergency pause.
- **Profiles:** independent calibration and actions, inline rename, Save/Revert/Undo/Redo, plus remembered per-client panel position and size.
- **Updates:** startup checks, Stable/Preview channels and verified downloads. Confirm an update to follow download, installation and restart in Companion's own progress window.

## Install and start

Requires **Windows x64**, Windows 10 build 19041 or newer, and a D3D11-capable graphics device. Windows x64 setup is the only supported distribution.

1. Download the setup and `SHA256SUMS.txt` from [Releases](https://github.com/ethanjojo/metin2-bot/releases) and compare the installer's SHA-256 checksum.
2. Run setup and review its administrator, mandatory HIDMaestro driver, signing-certificate and GameInput service changes. Native dependencies are included; no installed .NET runtime is needed.
3. Open Companion. If an update is offered, choose **Install update** or **Skip for now**, then choose a game window from the thumbnail grid and select or create a profile.
4. In **Recognition**, select the search area, target color and a fixed HP-panel detail. Configure **Actions** and optional **Safety** limits, then save.
5. Review readiness in **Run**. **F7** starts; **F8** pauses. Keep the selected game in the foreground.

Companion starts paused. First installation uses the regular setup wizard; later updates use Companion's progress window with the same installer running behind it. Windows administrator approval may still be required. Startup checking is enabled by default; an existing explicit opt-out is preserved, and **Settings → Updates** keeps manual checks and channel preferences. Capture selection pauses automation and waits for held inputs to release. See the [setup guide](docs/user-guide.md) for calibration, shortcuts and troubleshooting.

**Release candidate:** the complete integrated targeting, attack, buff and mount cycle still requires live-game acceptance. App and setup are unsigned; `jojo` is product metadata, not a verified Windows publisher. Setup explains the shared driver's machine-wide changes and [removal policy](docs/user-guide.md#installation-and-hid-driver).

## Screenshots

<a href="docs/assets/calibration.png"><img src="docs/assets/calibration.png" alt="Recognition controls for search, color, HP detail and game annotations" width="360"></a>
<a href="docs/assets/actions.png"><img src="docs/assets/actions.png" alt="Ordered buffs with shortcut, timing and icon recognition settings" width="360"></a>

<a href="docs/assets/window-chooser.png"><img src="docs/assets/window-chooser.png" alt="Searchable window selection with screenshot thumbnails" width="740" loading="lazy"></a>

<a href="docs/assets/startup-update-available.png"><img src="docs/assets/startup-update-available.png" alt="Startup update offer with Install update and Skip for now" width="360" loading="lazy"></a>
<a href="docs/assets/update-installing.png"><img src="docs/assets/update-installing.png" alt="Companion update installation progress" width="360" loading="lazy"></a>

*Update screenshots use offline demonstration metadata; the shown versions are illustrative.*

More views and explanations are in the [user guide](docs/user-guide.md).

## Your data

Installation: `Program Files/Companion`. Profiles, PNG references, settings and logs: `%LOCALAPPDATA%/Companion`.

Back up the complete data folder. Upgrades preserve profiles and migrate previous default paths with verified backups; explicit custom data paths stay explicit. Uninstall retains user data and the shared driver by default. See [data and diagnostics](docs/user-guide.md#data-and-troubleshooting).

## Build and contribute

Use the pinned Rust toolchain and follow [development setup](docs/development.md) for native prerequisites, tests and reproducible screenshots. Release packages are built from clean, tested commits; see the [release procedure](docs/releasing.md).

| Directory | Purpose |
| --- | --- |
| `crates/metin-core` | Profiles, validation, targeting and scheduling |
| `crates/metin-vision` | Detection and image matching |
| `crates/metin-platform` | Guarded Windows capture and input |
| `crates/metin-desktop` | Interface, storage and runtime worker |
| `native` | Pinned HID SDK adapter and NativeAOT bridge |
| `tests`, `tools`, `packaging` | Fixtures, build checks and installation |

Project-authored source code is [MIT licensed](LICENSE). Artwork and other third-party materials retain their original rights; see [licenses and notices](THIRD_PARTY_NOTICES.md).
