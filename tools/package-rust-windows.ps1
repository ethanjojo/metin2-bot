[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$CrtDirectory,
    [Parameter(Mandatory)][string]$CompilerPath,
    [Parameter(Mandatory)][string]$Version,
    [ValidateRange(1,65535)][int]$BuildNumber = 1,
    [string]$OutputDirectory = '',
    [string]$PythonPath = 'python'
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$repoRoot = Split-Path -Parent $PSScriptRoot
$identity = Get-Content -LiteralPath (Join-Path $repoRoot 'packaging/app-identity.json') -Raw | ConvertFrom-Json
$previousBuildVersion = [Environment]::GetEnvironmentVariable('COMPANION_BUILD_VERSION')
$previousBuildNumber = [Environment]::GetEnvironmentVariable('COMPANION_BUILD_NUMBER')
Push-Location $repoRoot
try {
    $commit = (git rev-parse HEAD).Trim()
    if ($LASTEXITCODE -ne 0 -or $commit -notmatch '^[0-9a-f]{40}$') { throw 'Cannot identify the source commit.' }
    if (git status --porcelain) { throw 'Release packaging requires a clean committed tree.' }
    $base = ([IO.File]::ReadAllText((Join-Path $repoRoot 'VERSION'))).Trim()
    if ($Version -notmatch ('^' + [regex]::Escape($base) + '($|[-+])')) { throw 'Release version must match VERSION core.' }
    $stage = if ($OutputDirectory) { [IO.Path]::GetFullPath($OutputDirectory) } else { Join-Path $repoRoot ('build/rust-packages/' + [Guid]::NewGuid().ToString()) }
    if (Test-Path -LiteralPath $stage) { throw 'Release output already exists; use a fresh directory.' }
    New-Item -ItemType Directory -Path $stage -Force | Out-Null
    & $PythonPath -m tools.build_metadata --version $Version --build-number $BuildNumber --commit-sha $commit --output-dir $stage | Out-Host
    if ($LASTEXITCODE -ne 0) { throw 'Invalid release metadata.' }
    $manifest = Get-Content (Join-Path $stage 'build-info.json') -Raw | ConvertFrom-Json
    if (!$manifest.prerelease) {
        $policy = Get-Content (Join-Path $repoRoot 'packaging/release-policy.json') -Raw | ConvertFrom-Json
        if (!$policy.stable_ready) { throw 'Stable release acceptance is incomplete; prepare a prerelease.' }
    }
    $env:COMPANION_BUILD_VERSION = $Version
    $env:COMPANION_BUILD_NUMBER = [string]$BuildNumber
    . ./tools/rust-env.ps1
    & ./tools/build-input-bridge.ps1
    if ($LASTEXITCODE -ne 0) { throw 'HID input bridge build failed.' }
    cargo build --workspace --release --locked -j 2 | Out-Host
    if ($LASTEXITCODE -ne 0) { throw 'Rust release build failed.' }
    $version = $Version
    $bundle = Join-Path $stage $identity.product_name
    New-Item -ItemType Directory -Path $bundle -Force | Out-Null
    Copy-Item -LiteralPath (Join-Path $stage 'build-info.json') -Destination $bundle
    Remove-Item -LiteralPath (Join-Path $stage 'build-info.json')
    Copy-Item -LiteralPath (Join-Path $repoRoot 'docs/release-notes.md') -Destination (Join-Path $bundle 'RELEASE-NOTES.md')
    Copy-Item -LiteralPath 'build/input-bridge/MetinInput.dll' -Destination $bundle
    Copy-Item -LiteralPath 'target/release/companion.exe' -Destination (Join-Path $bundle 'Companion.exe')
    Copy-Item -LiteralPath 'build/rust-native/windows/opencv/build/x64/vc16/bin/opencv_world4130.dll' -Destination $bundle
    foreach ($name in @('msvcp140.dll', 'vcruntime140.dll', 'vcruntime140_1.dll', 'concrt140.dll')) {
        $source = Join-Path $CrtDirectory $name
        if (-not (Test-Path -LiteralPath $source -PathType Leaf)) { throw "Missing redistributable: $source" }
        Copy-Item -LiteralPath $source -Destination $bundle
    }
    $licenses = Join-Path $bundle 'licenses'
    New-Item -ItemType Directory -Path $licenses | Out-Null
    Copy-Item -LiteralPath 'LICENSE' -Destination (Join-Path $licenses 'Companion.txt')
    Copy-Item -LiteralPath 'THIRD_PARTY_NOTICES.md' -Destination (Join-Path $licenses 'Third-party-notices.md')
    Copy-Item -LiteralPath 'crates/metin-desktop/assets/metin-ui/README.md' -Destination (Join-Path $licenses 'Metin-UI-assets.txt')
    Copy-Item -Path 'build/input-bridge/licenses/*' -Destination $licenses -Recurse
    Copy-Item -LiteralPath 'build/rust-native/windows/opencv/LICENSE.txt' -Destination (Join-Path $licenses 'OpenCV.txt')
    # Include OpenCV's bundled third-party notices as shipped by the vendor.
    $thirdParty = 'build/rust-native/windows/opencv/build/etc/licenses'
    if (-not (Test-Path -LiteralPath $thirdParty)) { throw 'OpenCV third-party notices are missing.' }
    Copy-Item -LiteralPath $thirdParty -Destination (Join-Path $licenses 'OpenCV-third-party') -Recurse
    $metadataPath = Join-Path $stage 'cargo-metadata.json'
    $metadataText = cargo metadata --format-version 1 --locked --offline --filter-platform x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw 'Cannot resolve dependency license inventory.' }
    [IO.File]::WriteAllText($metadataPath, ($metadataText -join "`n"), [Text.UTF8Encoding]::new($false))
    & $PythonPath -m tools.package_notices --metadata $metadataPath --destination $licenses
    if ($LASTEXITCODE -ne 0) { throw 'Dependency notice staging failed.' }
    Remove-Item -LiteralPath $metadataPath
    $inventory = Get-ChildItem -LiteralPath $bundle -Recurse -File | ForEach-Object {
        [ordered]@{ path = [IO.Path]::GetRelativePath($bundle, $_.FullName); sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant() }
    }
    [IO.File]::WriteAllText((Join-Path $bundle 'files.json'), (ConvertTo-Json -InputObject @($inventory) -Depth 4), [Text.UTF8Encoding]::new($false))
    & $PythonPath -m tools.verify_bundle --directory $bundle --version $version --commit $commit | Out-Host
    if ($LASTEXITCODE -ne 0) { throw 'Native bundle verification failed.' }
    & $PythonPath tools/verify-app-branding.py --require-admin (Join-Path $bundle 'Companion.exe') | Out-Host
    if ($LASTEXITCODE -ne 0) { throw 'Application branding verification failed.' }
    if ($CompilerPath) {
        & $CompilerPath /Q (Join-Path $repoRoot 'packaging/check_compiler.iss')
        if ($LASTEXITCODE -ne 0) { throw 'Installer compiler verification failed.' }
        & $CompilerPath "/DAppVersion=$version" "/DWindowsVersion=$($manifest.windows_version)" "/DAppProductName=$($identity.product_name)" "/DAppPublisher=$($identity.publisher)" "/DAppModelId=$($identity.app_id)" "/DAppCopyright=$($identity.copyright)" "/DBundleDir=$bundle" "/DReleaseDir=$stage" (Join-Path $repoRoot 'packaging/installer.iss')
        if ($LASTEXITCODE -ne 0) { throw 'Installer compilation failed.' }
    }
    & $PythonPath tools/verify-app-branding.py (Join-Path $stage $manifest.artifact_name) | Out-Host
    if ($LASTEXITCODE -ne 0) { throw 'Installer branding verification failed.' }
    $sums = Get-ChildItem -LiteralPath $stage -File | ForEach-Object { (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant() + '  ' + $_.Name }
    [IO.File]::WriteAllText((Join-Path $stage 'SHA256SUMS.txt'), ($sums -join "`n") + "`n", [Text.UTF8Encoding]::new($false))
    Write-Output $stage
} finally {
    [Environment]::SetEnvironmentVariable('COMPANION_BUILD_VERSION', $previousBuildVersion)
    [Environment]::SetEnvironmentVariable('COMPANION_BUILD_NUMBER', $previousBuildNumber)
    Pop-Location
}
