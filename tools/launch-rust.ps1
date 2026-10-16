[CmdletBinding()]
param([string]$ExecutablePath = (Join-Path $PSScriptRoot '../target/release/metinbot.exe'))
$ErrorActionPreference = 'Stop'
if (!(Test-Path -LiteralPath $ExecutablePath -PathType Leaf)) {
    throw 'Rust release executable not found. Build with cargo build --release --locked, or install the Windows prerelease.'
}
Start-Process -FilePath (Resolve-Path -LiteralPath $ExecutablePath).Path -Verb RunAs -WindowStyle Hidden
