# Windows prerelease

MetinBot includes a Rust/egui interface with Run, Calibration, Buffs and Settings, searchable per-profile window selection, window capture, interactive calibration, skill ordering, icon recognition and light/dark appearance.

## Installation

Download the Windows x64 installer or portable ZIP and verify it against `SHA256SUMS.txt`. Extract the complete ZIP if using the portable package. Runtime dependencies are included. Windows 10 version 2004 / build 19041 or newer and a D3D11-capable graphics device are required.

Packages request the normal administrator confirmation and are unsigned. `jojo` is product metadata, not a verified Windows publisher.

## Data

Back up the complete MetinBot data folder before upgrading. Profiles, assets and settings are kept outside the installation directory and survive uninstall. Import copies data without modifying its source. Windows stores data under `%LOCALAPPDATA%/MetinBot`; keep profile JSON and PNG assets together when backing up.

## Known limitations

- Live-game acceptance of targeting, attacks, buffs and mount handling is pending.
- Hosted installation, upgrade and uninstall checks must pass for the final artifact before publication.
- Linux supports an editor/recognition preview. Linux and macOS automation is disabled because capture/input are unavailable.
- Full native macOS execution and Finder/Dock appearance still require verification.

When reporting an issue, include the application version/commit, operating-system version, reproduction steps and relevant log excerpt. Review logs and screenshots for private data before sharing.
