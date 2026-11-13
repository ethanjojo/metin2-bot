# Installation, input driver and backups

## Requirements

- 64-bit Windows 10 version 2004 / build 19041 or newer.
- Direct3D 11-capable graphics.
- Administrator approval for setup and Companion.

Download **CompanionSetup** and **SHA256SUMS.txt** from [Releases](https://github.com/ethanjojo/metin2-bot/releases). To check the download in PowerShell:

    Get-FileHash .\CompanionSetup-<version>-windows-x64.exe -Algorithm SHA256

Compare the result with the checksum file. Run setup, read the driver disclosure and finish the wizard. Required app libraries are included; no separate Rust or .NET installation is needed. The default app folder is **C:\Program Files\Companion**.

## What setup changes in Windows

The required **HIDMaestro 2.1.0** driver provides keyboard and mouse input. Setup asks for explicit consent because it makes machine-wide changes:

| Change | Details |
| --- | --- |
| Input driver | Installs shared virtual input-driver packages. Other apps may also use them. |
| Signing certificate | Creates CN=HIDMaestroTestCert, with an exportable private key in the machine My certificate store. |
| Certificate trust | Copies that certificate into machine Root and TrustedPublisher, trusting code signed by this local certificate. |
| GameInput | Configures and starts GameInputSvc with automatic startup. |

The driver certificate uses RSA 2048/SHA-256. It does **not** sign the Companion app or installer. Those remain unsigned; **jojo** is product metadata, not a verified Windows publisher.

Starting the bot does not install or repair the driver. If it is missing, Run explains why Start is unavailable. Run setup again to repair the required installation.

## Profiles and backups

Your data is separate from the app: **%LOCALAPPDATA%/Companion** contains profiles, images, settings and **logs/run.log**.

- Close Companion before copying the whole folder as a backup.
- **Export… / Import…** in the profile selector moves one profile with its images.
- Keep update backups until the new installation is checked. On failure, retain the error and setup log.

Updates may retain backups in their temporary update folder. Do not delete a folder while its updater or installer is running. Copy a backup you want to keep long-term to your own backup location.

When upgrading an older default installation, you may also see a folder named **Companion-data-backup-…** beside the data directory or **Companion-install-backup-…** beside the installation. These are recovery copies, not separate registered installations. Do not run uninstallers inside backup folders.

## Uninstalling

Use **Windows Settings → Apps → Installed apps → Companion → Uninstall**. Ordinary uninstall retains profiles and the shared driver.

Only choose explicit driver removal if no other app needs HIDMaestro. Stop other consumers first; removal refuses while another is running. When neither shared driver package remains, removal also deletes its local certificate and private key.

**GameInput remains configured for automatic startup after driver removal.** The driver does not record the original startup setting.

For administrator commands and reports, see [Driver management](development.md#driver-management).

[User guide](user-guide.md) · [Updates](user-guide.md#updating-companion) · [Troubleshooting](user-guide.md#data-and-troubleshooting)
