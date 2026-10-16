# Platform support

Platform availability is explicit. An unsupported backend prevents Start and displays the reason; the application does not substitute desktop capture or unguarded input.

| Capability | Windows x64 | Linux | macOS |
| --- | --- | --- | --- |
| Profiles, calibration editor, buff configuration | Implemented | Implemented | Implemented; full native acceptance pending |
| Recognition on stored images | Tested | Tested | Native verification pending |
| Selected-window capture | Implemented | Unavailable | Unavailable |
| Guarded game input | Implemented | Unavailable | Unavailable |
| Automation Start | Subject to readiness checks | Disabled | Disabled |
| Distribution | Installer and portable ZIP prerelease | Native editor preview packaging | Application bundle implementation |

## Appearance and desktop integration

The interface uses black artwork on light surfaces and white artwork on dark surfaces. Windows runtime window/taskbar icons follow the selected theme; executable/installer icons use a fixed contrast design suitable for either taskbar background.

Linux X11 runtime icons follow the theme. Wayland identifies the application through `app.jojo.MetinBot`; launchers resolve its installed contrast icon from the desktop entry and icon theme. The package includes desktop entries and standard icon sizes.

macOS includes an ICNS bundle icon and a themed Dock icon implementation. The platform-specific module has been cross-checked for Intel and Apple Silicon, but that does not establish full application execution, Finder appearance or Dock behavior on real hardware. Build and verify each architecture natively before distribution.

## Acceptance still required

- Complete the Windows target selection, attack, buff and mount cycle in a live game with calibrated profiles.
- Run the final artifact through disposable hosted installation, upgrade and uninstall checks before publication.
- Complete native macOS build/runtime and Finder/Dock verification.
- Implement and accept native capture/input before enabling Linux or macOS automation.

Local checks do not imply that hosted CI has run or that an artifact has been published. See [release notes](release-notes.md) and [release procedure](releasing.md).
