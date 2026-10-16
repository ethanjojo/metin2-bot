# Current acceptance status

Rust is the only current application/build/release path following the approved prerelease cleanup. Sanitized parity fixtures and the legacy user-data importer remain; the historical Python product is recoverable from Git at 5af7ea98536f9884969f38b7eb9b1077c5910a49.

The current checklist and verification results are maintained in [release readiness](../release-readiness.md). Do not use historical artifact hashes or test counts in earlier reports as acceptance for a new release.

## Verified implementation evidence

- Windows/Linux typed profiles, validation, storage/import, UI interactions and worker/controller regression tests.
- Immutable differential fixtures: 648 controller events, 327 schema boundary cases and 84 buff observations. These case counts are not separate Cargo test counts.
- Windows child-process capture/input probes, foreground/identity/cancellation checks and both-theme native GPU UI screenshots.
- Native Linux X11 icon pixel readback, actual app WM_CLASS, Wayland app_id protocol and rendered editor screenshots.
- AppKit icon module compilation for Intel/Apple Silicon; full macOS execution remains unverified.

## Required before stable

- Real-client target selection/attack, panel loss, buff/mount cycle, focus/manual-input/geometry cancellation and F7/F8 acceptance.
- Hosted exact-commit installation, upgrade, dependency isolation, executable self-test, uninstall and retained-data checks.
- Final clean-commit package identity, PE resources, every inventory hash and downloaded-release checksums.

The stable-release policy is blocked in packaging/release-policy.json. The new hosted workflow implements the non-game checks; source/configuration is not a claim that a remote run passed. A local prerelease may be prepared while game acceptance is pending.

## Separate platform work

Linux/macOS automation backends and native macOS whole-application acceptance are not complete. They are not required to label the Windows build a prerelease, but must be completed before advertising working automation on those systems. See [platform support](platform-support.md).
