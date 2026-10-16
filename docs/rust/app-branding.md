> Historical verification report. Use the repository README and docs/release-readiness.md for current installation/release status. Earlier commands, counts and package paths below describe their original snapshots.

# Application icon and publisher metadata

Current Linux/macOS implementation and verification are documented in [native desktop integration](unix-desktop-integration.md). The Windows artifact records below describe previous builds, which predate that follow-up. The generated ICNS is now consumed by the macOS bundle packager, and Linux has an installed desktop entry/icon path.

The supplied black SVG is preserved unchanged at `packaging/icons/logo.svg` (SHA-256 `d90de30bd25a6dcf437f3951eafefe51ac1b157be84e4b6707dc554c20f7bdb4`). The running window uses pure black for light mode and pure white for dark mode, with identical transparency and no contour. Separate static file icons retain a thin white contour for contrast on both desktop surfaces. They include PNG sizes 16–1024, a Windows ICO containing nine sizes 16–256, and an ICNS asset for future macOS bundle packaging.

The Rust window uses the matching embedded 256-pixel PNG through eframe's native IconData. Saved theme loading and the theme toggle share one apply_theme path that sends both native theme and Icon commands. The toggle saves the theme without modifying profiles. The generated white vector is packaging/icons/logo-dark.svg; the supplied black logo.svg remains unchanged. The static EXE and installer icon is independent of the app session theme and keeps the universal contour variant. Windows executable resources use the multi-size ICO; the installer uses the same ICO, and its shortcuts/uninstall entry reference the branded executable. The temporary legacy Python packaging path uses the same ICO. Linux/macOS now have native packaging and runtime integration; their acceptance boundaries are recorded in the linked follow-up.

`packaging/app-identity.json` is the canonical publisher/product metadata. The user selected **jojo**. Rust's build script validates the typed schema and writes CompanyName, FileDescription, ProductName, InternalName, OriginalFilename, version and copyright resources. Both Windows packaging scripts feed the same publisher/copyright into Inno Setup. The legacy version-resource generator reads the same identity. Runtime build diagnostics expose publisher and product as well.

## UAC and signing

The user confirmed that no signing certificate is available. The EXE and installer remain unsigned. CompanyName=jojo appears in file metadata and installer identity; it does **not** make jojo a verified publisher in Windows UAC. Windows verifies the publisher through a trusted Authenticode signature, not arbitrary version metadata. No certificate was generated, no trust-store settings were changed and administrator elevation remains enabled.

Sources: [Microsoft UAC behavior](https://learn.microsoft.com/en-us/windows/security/application-security/application-control/user-account-control/how-it-works), [Authenticode identity verification](https://learn.microsoft.com/en-us/windows-hardware/drivers/install/authenticode).

If a code-signing identity is obtained later, sign the final EXE before generating the file inventory/ZIP, then sign the compiled installer, timestamp signatures and verify trust on the actual artifacts. The verified publisher name follows the certificate identity. Signing is not configured as a pretend success or a self-signed workaround.

## Asset regeneration

Run `node tools/generate-app-icons.cjs` with Node.js and sharp 0.35.5 available in module resolution. Generated assets are checked in; building the app does not need Node.js, sharp or access to the original Downloads file. `logo.svg` remains the single original vector source. The output includes separate PNGs plus ICO/ICNS containers; these are platform assets, not alternative recognition logic.

## Verification

Windows taskbar follow-up: egui-winit 0.37.0 dispatches ViewportCommand::Icon through winit's set_window_icon, which updates Windows ICON_SMALL only. The running taskbar uses the separate ICON_BIG. The desktop now retains the native root window from CreationContext and uses winit's Windows set_taskbar_icon on startup and theme changes. The same decoded themed pixels feed both APIs. Production icon handles remain owned by winit; no custom unsafe native code was introduced in production and no alternate branding state was added.

The theme control is now fixed to the top-right edge independently of wrapped profile/run controls. Light mode offers a vector moon with "Switch to dark mode"; dark mode offers a vector sun with "Switch to light mode".

This follow-up passed **121 Windows workspace tests** (55 UI), including a real hidden winit window test that reads the actual WM_GETICON/ICON_BIG bitmap and verifies black → white → black. The test releases GetIconInfo's copied GDI bitmaps and never sends gameplay input. A separate real egui pointer/hover regression checks the right edge at 800/1280 widths, both target-mode tooltips and switching. Strict Clippy, formatting and native debug build passed. Four final native screenshots were inspected under `build/taskbar-ui/{dark,light}-{large,small}.png`. Logs: `build/rust-migration/taskbar-verified-{tests,native,clippy}.log`, `taskbar-native-test.log`. This verifies the native icon API, not an Explorer pinned-shortcut screenshot or macOS/Linux acceptance.

Theme follow-up: **119 workspace tests** passed (53 UI and three branding tests). The new asset test compares alpha byte-for-byte and verifies pure black/white on every visible pixel. The UI regression verifies startup from the default saved preference, both toggle directions, native Icon commands, persistence and an unchanged profile. A test-harness error initially assumed dark rather than the real light default; it was corrected and texture deltas are now cleared before assertions. Strict Clippy and the focused theme regression passed after the final lint adjustment. Logs: `build/rust-migration/themed-logo-final-tests.log`, `themed-logo-final-focus.log`, `themed-logo-verified-clippy.log`, `themed-logo-final-native.log`. A visual comparison of the actual PNG assets is `build/themed-logo/preview.png`; it is an asset preview, not a screenshot of native window chrome.

Integration tests decode the native PNG and every ICO payload, check each expected resolution, transparency, black fill and the runtime publisher/product identity. Legacy build-metadata tests verify jojo and copyright. Native PE inspection uses `tools/verify-app-branding.py` (pefile is already a build dependency) to check the actual resources of both application and installer, including icon sizes and the application administrator manifest. Inno retains its normal asInvoker outer-loader manifest and PrivilegesRequired=admin configuration; the verifier trims Inno version-string padding.


Windows passed **117 Rust workspace tests** (52 UI and two branding tests), **five Python packaging tests**, strict all-target/all-feature Clippy, formatting, actionlint, native debug build, optimized release build and Inno installer compilation. The debug self-test passed storage, vision and both themes with publisher=jojo and no game input. Original/source SVG hashes match. All seven ICNS PNG chunks decode at their expected dimensions; transparent icons were visually inspected on light/dark surfaces. A live UAC dialog, macOS/Linux native rendering and installation were not exercised. The current user app and profiles were left untouched.

Both final EXEs contain CompanyName=jojo, MetinBot metadata and nine icon resolutions 16–256. The app retains requireAdministrator. Both Authenticode statuses are NotSigned. Logs: `build/rust-migration/branding-final-{tests,native,clippy}.log`, `branding-python-tests.log`, `branding-release.log`, `branding-package.log`, `branding-{app,installer}-resources.json`; self-test: `build/branding/self-test.json`.

## Local Windows artifacts

Directory: `build/rust-packages/12a8959a-5c78-4a11-b895-778a169f4a36/`. These include all current UI fixes and supersede previous local previews. Build metadata records the current uncommitted changes on top of 5af7ea9. All 449 inventory files in an isolated extracted ZIP match their SHA-256 hashes (`build/rust-migration/taskbar-package-verification.json`). Nothing was installed, pushed or published.

- `MetinBot-0.1.0-rust-preview-windows-x64.zip`: 33,509,352 bytes; SHA-256 `ee83e7f06bbc40e8eca600405cd74f895c85b4ab0568cff8a907f46545bdec07`.
- `MetinBotSetup-0.1.0-windows-x64.exe`: 22,620,690 bytes; SHA-256 `8034178a44735c3aa0d1af9fd4c200ce0cda74dbf4fba2180a0d0e10e4a20173`.

The themed-logo follow-up also passed executable self-test, optimized release build, installer compilation and final PE resource/inventory verification. Logs: `build/rust-migration/themed-logo-{app-resources,installer-resources,package-verification}.json` and `themed-logo-{release,package}.log`; self-test: `build/themed-logo/self-test.json`. The current packages above include the themed runtime icons.

The taskbar follow-up also passed optimized release build, installer compilation, debug executable self-test and PE resource checks for both artifacts. All 449 extracted ZIP inventory hashes match. Reports: `build/rust-migration/taskbar-{package-verification,app-resources,installer-resources}.json`; logs: `taskbar-{release,package}.log`; self-test: `build/taskbar-ui/self-test.json`. The packages above are the current taskbar/theme-control preview. No user profiles, running user app, installation, trust settings or remote release were changed.
