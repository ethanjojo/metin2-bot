# Windows Distribution Implementation Plan

> Implement in this chat using independently owned data, packaging and CI tasks. User approved the distribution design and inline execution.

**Goal:** A Windows x64 installer with bundled Python and automatic GitHub prereleases on main pushes, stable releases on version tags.

**Architecture:** One application entry point for source and bundle. Writable data lives in LocalAppData/MetinBot with idempotent migration. PyInstaller onedir with native UAC manifest is installed under protected Program Files; one shared build script produces installer and SHA-256 checksums. CI only publishes successful tested artifacts.

**Tech stack:** Python 3.14, Tk, pinned PyInstaller, Inno Setup, Windows GitHub Actions.

## Tasks

- [x] Add canonical data paths and atomic, recoverable migration without overwriting user data. Verify in temporary directories only.
- [x] Refactor CLI into a callable entry with guarded source/packaging bootstrap; add explicit environment/self-test entry for bundle verification without game inputs.
- [x] Add pinned spec/version resource and installer/build script with validated version metadata, no user data bundled, no hidden fallback compiler.
- [x] Add Windows CI for PR validation and main/tag builds; release job alone has contents write. Pin actions to immutable commits, unique main prerelease tags, stable semver tags, artifact checksums and explicit commit targeting.
- [x] Run source regression, dependencies/compile checks and real local bundle+installer build. Verify bundle contents and embedded UAC manifest; never automate UAC confirmation or game input.
- [x] Update English README, verification and final plan status. Distinguish local verification from unavailable remote CI/clean-machine/UAC runtime acceptance.

## Review focus

- Existing profiles, PNG templates, selection and theme/mount preferences survive migration/restart.
- Failed migrations are retriable and cannot merge unrelated user data silently.
- Frozen paths never put persistent data into PyInstaller extraction/internal directories.
- Elevated executable libraries are installed in a protected directory; uninstall does not remove user data.
- Failed builds or PR code never receive publication credentials; release assets match the tested commit.

## Constraints

- No repository exists locally yet: prepare workflow files; do not claim a remote run, push or release.
- Existing local diagnostics fixtures may depend on screenshots outside the checkout; keep portable tests independent.
- UAC confirmation remains manual. The local bundled self-test passed through the standard RunAs launch. Real-client acceptance is separate; CI validation must not pretend it proved gameplay.

## Final implementation and verification

- All distribution layers are implemented: canonical user data and import, shared CLI, frozen metadata/self-test, pinned build tooling, installer and least-privilege release workflow. English README documents setup, migration, development, stable tags and limits.
- Full source regression: 256 tests PASS (43.207 s). Dependency check, compileall, PowerShell syntax and actionlint PASS. Reviewer confirmed fixes for first-launch import and draft/tag publication lifecycle; mocked release tests independently track tag creation and draft creation.
- Final x64 directory bundle passed static identity/dependency/data-exclusion/administrator-manifest verification and its actual isolated frozen self-test (Tk/Pillow themes, OpenCV/NumPy detection, profile/preferences roundtrip). No game input or production profile changes.
- Remote CI/publication cannot be executed here: no local Git repository or remote exists. The installer installation/uninstallation test is configured for disposable GitHub Windows runners and was not run on this user's machine. A Windows machine without Python and real-client gameplay remain unverified.
