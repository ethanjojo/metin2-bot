# MetinBot Rust Windows prerelease

Rust/egui replaces the Python application. This preview includes the redesigned interface, per-profile window selection, HWND-only Windows capture, calibration tools, buff icon capture, skill ordering and light/dark application branding.

## Download

Use the Windows x64 installer or extract the entire portable ZIP. Compare the download with SHA256SUMS.txt. No Python or Rust installation is required. Windows 10 2004/build 19041 or newer is required. Packages are unsigned and request the normal Windows administrator confirmation.

## Existing users

Back up the full MetinBot data folder before upgrading. The installer keeps the original application identity and removes the obsolete Python dependency directory. Profiles/settings live outside the installation folder and are retained by uninstall. First-run import copies old data without changing the source.

## Known limitations

- The real-game target/attack/buff/mount cycle has not been accepted; the test server was unavailable.
- Clean-machine install/upgrade/uninstall for the final commit must pass hosted CI before publication.
- Linux offers the editor/vision preview; Linux and macOS capture/input are unavailable and Start is disabled.
- macOS bundle/Dock code cross-compiles for Intel and Apple Silicon; full native execution and Finder/Dock appearance remain unverified.
- The application is unsigned: jojo metadata is not a verified UAC publisher. No signing certificate is configured.

Report the application version/commit, Windows version, reproduction steps and relevant log excerpt. Review logs/screenshots for private data before sharing.
