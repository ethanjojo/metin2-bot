# Rust Windows prerelease preparation

Scope approved 2026-10-16: repository cleanup, Rust release workflow, current documentation, all available non-game verification and a local Windows prerelease. Publication is a separate action; no stable release is accepted by this work.

## Work ledger

- [x] One Rust application entrypoint, build and release path; remove obsolete Python application/build/UI test code. Keep sanitized parity fixtures and Rust's existing legacy profile importer.
- [x] Release identity is shared by executable/installer/package tooling; dirty-tree and stable-channel guards have actual PowerShell regression coverage.
- [x] ZIP/PE/native-dependency verification tools and hosted install/upgrade/uninstall workflow implemented; hosted execution remains open below.
- [x] Current README, changelog, platform/support and migration documentation; ignore local outputs and private data.
- [x] Windows 122 / Linux 120 Rust tests, formatting and strict Clippy passed. Release tooling tests and workflow/PowerShell syntax checks passed; final artifact verification is performed at the package stage.
- [x] Local Windows prerelease 0.1.0-rc.1 built from clean commit 04e96d189ca33129721698b404ccc6a34adf7227; installer/ZIP/checksums, extracted inventory and final packaged EXE self-test verified.

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

## Local handoff

Packages are in `dist/prerelease/0.1.0-rc.1/`: installer, complete ZIP, SHA256SUMS.txt and VERIFICATION.md. The generated report records artifact hashes and file count. The exact EXE self-test is `build/prerelease-rc1-selftest.json`; structured full verification is `build/prerelease-rc1-verification.json`. Both artifacts identify source commit 04e96d189ca33129721698b404ccc6a34adf7227, numeric version 0.1.0.1 and a clean tree. A later documentation-only commit recording these results does not change their provenance.

No remote CI run, GitHub prerelease publication or clean-machine installation was claimed. Stable policy remains blocked until its acceptance conditions pass.
