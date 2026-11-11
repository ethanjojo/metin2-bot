# Platform support

Companion supports Windows x64 only. Releases contain a setup installer and `SHA256SUMS.txt`.

| Capability | Windows x64 |
| --- | --- |
| Profiles, calibration, actions and recognition | Implemented |
| Selected-window capture | Implemented |
| Guarded keyboard and mouse input | Generic HIDMaestro v2.1.0 through bundled NativeAOT bridge |
| Automation Start | Requires valid profile, selected target and ready driver/bridge |
| Distribution | Administrator-installed setup prerelease |

Windows 10 version 2004 / build 19041 or newer and a D3D11-capable graphics device are required. Setup includes native runtime dependencies; an installed .NET runtime is unnecessary. Portable core logic remains testable independently of the Windows desktop backend.

Driver installation is explicit setup/management work, never a side effect of Start. Setup discloses the machine code-signing certificate stores and `GameInputSvc` changes. App uninstall retains the shared driver by default; explicit removal refuses while another consumer is running. Removal does not restore the original `GameInputSvc` startup setting. See [installation](user-guide.md#installation-and-hid-driver).

The interface uses one fixed Metin appearance with original game artwork and readable text. Window/taskbar, executable and installer icons use the fixed Companion identity. Previous saved theme preferences are discarded safely while retaining other settings.

## Verification boundaries

Native fixture checks confirmed Ctrl+G, a held Space key, mouse click and balanced release. On 10 November 2026, the operator confirmed isolated Escape and X-control click responses in the game. These checks establish the chosen input transport, not acceptance of the complete integrated automation cycle.

The final Windows target-selection, attack, buff and mount cycle still requires a calibrated live-game test. The final artifact must also pass disposable hosted installation, upgrade and uninstall checks before publication. Local tests do not establish hosted CI or publication. See [release procedure](releasing.md).
