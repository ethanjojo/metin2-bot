# Rust Windows prerelease preparation

Scope approved 2026-10-16: repository cleanup, Rust release workflow, current documentation, all available non-game verification and a local Windows prerelease. Publication is a separate action; no stable release is accepted by this work.

## Work ledger

- [x] One Rust application entrypoint, build and release path; remove obsolete Python application/build/UI test code. Keep sanitized parity fixtures and Rust's existing legacy profile importer.
- [x] Release identity is shared by executable/installer/package tooling; dirty-tree and stable-channel guards have actual PowerShell regression coverage.
- [x] ZIP/PE/native-dependency verification tools and hosted install/upgrade/uninstall workflow implemented; hosted execution remains open below.
- [x] Current README, changelog, platform/support and migration documentation; ignore local outputs and private data.
- [x] Windows 122 / Linux 120 Rust tests, formatting and strict Clippy passed. Release tooling tests and workflow/PowerShell syntax checks passed; final artifact verification is performed at the package stage.
- [ ] Local Windows prerelease installer, ZIP, checksums and release notes from a committed tree.

Ruling: the Python application is recoverable from Git history at `5af7ea98536f9884969f38b7eb9b1077c5910a49`; retaining a second runnable product/build path would undermine the requested Rust cutover. The user-data import contract stays supported and tested. Game acceptance remains a prerelease limitation, not a reason to publish a stable release.

Ruling: do not install over the user's running application or production AppId locally. Install/upgrade/uninstall verification belongs to the disposable hosted Windows runner, preserving existing user profiles.

Ruling: no open-source license is invented on the owner's behalf. Include third-party notices; owner selection of a project distribution license remains explicit.

Independent final review found a stable-channel gate bypass involving hyphens inside SemVer build metadata. Classification now comes from the validated prerelease component, all three gates enforce it, regression tests cover the actual packager, and independent re-review accepted the correction.

Verification logs: `build/rust-migration/release-{tests,clippy}.log`, `build/rust-linux/release-{tests,clippy,tooling-tests}.log`, `build/release-tooling-tests.log`. Final package hashes and exact source identity belong in the generated local handoff report; historical hashes in earlier reports are not current-release identities.

## Remaining acceptance

- Real game target/attack/buff/mount cycle and F7/F8/input guards.
- Native macOS application build, Dock/Finder and runtime acceptance; Linux/macOS automation backends remain unavailable.
- Hosted CI execution and clean-machine installation/upgrade/uninstall evidence for the exact release commit.
- Trusted Windows publisher signing and macOS Developer ID/notarization if desired; no certificate is available.
- Owner selection of a project license before granting redistribution rights.
