# Companion Windows release candidate

Companion for Windows x64: a compact Rust/egui desktop application for local visual recognition, calibration and configurable actions.

## Highlights

- In-app updates keep profiles locked while verifying and copying their data without reading the transient lock file, fixing Windows error 33 during backup preparation.
- Specific Run pause causes, recovery links and configuration readiness; F7 and the main button share validated Save & Start, including while minimized. F8 cancels pending starts even during saving, and input requires the acknowledged current configuration.
- Review exact unsaved changes, return to the last Actions detail through breadcrumbs, and adjust UI scale as 75–150%. Pinned client-relative panel positions survive reconnects and remain accessible after monitor/size changes.
- Duplicate profiles or export/import one bounded `.companion-profile` package with settings and images, fresh identity and explicit preview. Named excluded areas retain geometry through rename, edit and undo, including legacy profile loading.
- Inspect HP/buff recognition on a fresh read-only snapshot with real matcher scores, reference/current previews and evidence-age explanations.

- Window capture retains the shared Windows runtime across repeated previews; individual capture sessions still release their resources.

- One resizable Metin-style panel with Run, Actions, Recognition, Safety and Settings. Searchable profiles keep Save/Revert/Undo/Redo available; Start/Pause and mount declaration stay in the footer.
- Choose a game window from an automatically refreshed screenshot grid. Pin the panel to the selected client or collapse it to its title bar.
- Select regions, colors and buff/action references directly over a fresh frozen game capture. Move/resize existing selections; Escape/F8 cancels without changing calibration. Review client-size changes explicitly.
- Recognize targets by color and one fixed HP-panel detail. Click-through annotations show current/next targets, exclusions and optional sizes without a persistent cross at the last click.
- Configure ordered timed/icon-based buffs, loot, mount behavior, periodic actions, visual rules, auto-puller and bounded out-of-target attacks. Animated buff icons use a short configurable Active-state hold.
- Apply stop limits, breaks, recovery limits and local notifications. Follow activity/countdowns in Run and open searchable event history inside the same panel.
- Check Stable or Preview updates at startup, or manually from Settings. Startup checking defaults on with an explicit opt-out. Install update follows verified download and real installation progress in Companion's own window; the canonical installer runs behind it, UAC remains visible when required, and restart stays paused.

## Install or upgrade

Requires Windows x64, Windows 10 build 19041 or newer, and a D3D11-capable graphics device. Download the setup plus `SHA256SUMS.txt` and verify the installer checksum. Native runtime libraries and the NativeAOT input bridge are included; no installed .NET runtime is needed.

Setup requires administrator access and explicit consent for the shared HIDMaestro 2.1.0 driver. It discloses machine signing-certificate trust/private-key changes and automatic startup of `GameInputSvc`. App/setup are unsigned; `jojo` is product metadata, not a verified publisher. See [driver installation and removal](user-guide.md#installation-and-hid-driver).

Companion installs under `Program Files/Companion` and keeps profiles, reference PNGs, settings and logs under `%LOCALAPPDATA%/Companion`. Upgrades migrate previous default paths with verified backups; custom data paths stay explicit. Keep complete backups until the migrated application is verified. Ordinary uninstall retains profiles and the shared driver. Removing the driver requires a separate explicit choice and refuses while another consumer runs.

## Known limits

- Windows x64 setup is the only supported distribution.
- Keep the selected game in the foreground. Capture/input guards remain active; this is not unattended background control of an inactive window.
- Windows capture privacy policy may retain a capture indicator when borderless permission is unavailable or denied.
- The complete integrated targeting, attack, buff, mount and emergency-stop cycle still requires live-game acceptance before stable release.
- Screenshot/update UI fixtures use isolated demonstration data; they do not establish live-game or interactive updater-install acceptance. Real UAC, installed update and restart acceptance for the integrated helper remain pending until an operator is available.

Project-authored source is [MIT licensed](../LICENSE). Original artwork and third-party components retain their existing terms; see [notices](../THIRD_PARTY_NOTICES.md).

[User guide](user-guide.md) · [Changelog](../CHANGELOG.md) · [Release procedure](releasing.md)
