[CmdletBinding()]
param(
    [Parameter(Mandatory)][string]$CrtDirectory,
    [string]$CompilerPath = '',
    [switch]$SkipBuild
)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    . ./tools/rust-env.ps1
    if (-not $SkipBuild) {
        cargo build --workspace --release --locked
        if ($LASTEXITCODE -ne 0) { throw 'Rust release build failed.' }
    }
    $version = ([IO.File]::ReadAllText((Join-Path $repoRoot 'VERSION'))).Trim()
    if ($version -notmatch '^\d+\.\d+\.\d+$') { throw 'VERSION must contain a three-part numeric version.' }
    # A unique staging directory prevents stale libraries from entering a package.
    $stage = Join-Path $repoRoot ('build/rust-packages/' + [Guid]::NewGuid().ToString())
    $bundle = Join-Path $stage 'MetinBot'
    New-Item -ItemType Directory -Path $bundle -Force | Out-Null
    Copy-Item -LiteralPath 'target/release/metinbot.exe' -Destination (Join-Path $bundle 'MetinBot.exe')
    Copy-Item -LiteralPath 'build/rust-migration/opencv/build/x64/vc16/bin/opencv_world4130.dll' -Destination $bundle
    foreach ($name in @('msvcp140.dll', 'vcruntime140.dll', 'vcruntime140_1.dll', 'concrt140.dll')) {
        $source = Join-Path $CrtDirectory $name
        if (-not (Test-Path -LiteralPath $source -PathType Leaf)) { throw "Missing redistributable: $source" }
        Copy-Item -LiteralPath $source -Destination $bundle
    }
    $licenses = Join-Path $bundle 'licenses'
    New-Item -ItemType Directory -Path $licenses | Out-Null
    Copy-Item -LiteralPath 'build/rust-migration/opencv/LICENSE.txt' -Destination (Join-Path $licenses 'OpenCV.txt')
    # Include OpenCV's bundled third-party notices as shipped by the vendor.
    $thirdParty = 'build/rust-migration/opencv/build/etc/licenses'
    if (-not (Test-Path -LiteralPath $thirdParty)) { throw 'OpenCV third-party notices are missing.' }
    Copy-Item -LiteralPath $thirdParty -Destination (Join-Path $licenses 'OpenCV-third-party') -Recurse
    $metadataText = cargo metadata --format-version 1 --locked --offline --filter-platform x86_64-pc-windows-msvc
    if ($LASTEXITCODE -ne 0) { throw 'Cannot resolve dependency license inventory.' }
    $metadata = $metadataText | ConvertFrom-Json
    $dependencyInventory = @()
    foreach ($package in $metadata.packages) {
        if (-not $package.source) { continue }
        $dependencyInventory += [ordered]@{ name = $package.name; version = $package.version; license = $package.license; source = $package.source }
        $crateRoot = Split-Path -Parent $package.manifest_path
        $notices = @(Get-ChildItem -LiteralPath $crateRoot -File | Where-Object { $_.Name -match '^(LICENSE|LICENCE|COPYING|NOTICE)' })
        if ($package.license_file) {
            $declared = if ([IO.Path]::IsPathRooted($package.license_file)) { $package.license_file } else { Join-Path $crateRoot $package.license_file }
            $notices += Get-Item -LiteralPath $declared
        }
        if ($package.name -eq 'epaint_default_fonts') {
            $notices += Get-ChildItem -LiteralPath (Join-Path $crateRoot 'fonts') -File -Filter '*.txt'
        }
        if ($notices) {
            $noticeFolder = Join-Path $licenses ("rust/" + $package.name + '-' + $package.version)
            New-Item -ItemType Directory -Path $noticeFolder -Force | Out-Null
            foreach ($notice in ($notices | Sort-Object FullName -Unique)) {
                $relative = [IO.Path]::GetRelativePath($crateRoot, $notice.FullName)
                if ($relative.StartsWith('..') -or [IO.Path]::IsPathRooted($relative)) { throw 'Dependency notice is outside its crate.' }
                $destination = Join-Path $noticeFolder $relative
                New-Item -ItemType Directory -Path (Split-Path -Parent $destination) -Force | Out-Null
                Copy-Item -LiteralPath $notice.FullName -Destination $destination
            }
        }
    }
    [IO.File]::WriteAllText((Join-Path $licenses 'rust-dependencies.json'), (ConvertTo-Json -InputObject $dependencyInventory -Depth 4), [Text.UTF8Encoding]::new($false))
    $inventory = Get-ChildItem -LiteralPath $bundle -Recurse -File | ForEach-Object {
        [ordered]@{ path = [IO.Path]::GetRelativePath($bundle, $_.FullName); sha256 = (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant() }
    }
    [IO.File]::WriteAllText((Join-Path $bundle 'files.json'), (ConvertTo-Json -InputObject @($inventory) -Depth 4), [Text.UTF8Encoding]::new($false))
    $zip = Join-Path $stage "MetinBot-$version-rust-preview-windows-x64.zip"
    foreach ($file in Get-ChildItem -LiteralPath $bundle -Recurse -File) { $file.LastWriteTimeUtc = [datetime]'1980-01-01T00:00:00Z' }
    Compress-Archive -LiteralPath $bundle -DestinationPath $zip
    if ($CompilerPath) {
        & $CompilerPath /Q (Join-Path $repoRoot 'packaging/check_compiler.iss')
        if ($LASTEXITCODE -ne 0) { throw 'Installer compiler verification failed.' }
        & $CompilerPath "/DAppVersion=$version" "/DWindowsVersion=$version.0" "/DBundleDir=$bundle" "/DReleaseDir=$stage" (Join-Path $repoRoot 'packaging/installer.iss')
        if ($LASTEXITCODE -ne 0) { throw 'Installer compilation failed.' }
    }
    $sums = Get-ChildItem -LiteralPath $stage -File | ForEach-Object { (Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant() + '  ' + $_.Name }
    [IO.File]::WriteAllText((Join-Path $stage 'SHA256SUMS.txt'), ($sums -join "`n") + "`n", [Text.UTF8Encoding]::new($false))
    Write-Output $stage
} finally { Pop-Location }
