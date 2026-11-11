[CmdletBinding()]
param([string]$DotnetPath = '', [switch]$SkipDevelopmentStaging)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
if (!$IsWindows) { throw 'The input bridge is built for Windows x64 only.' }
$root = Split-Path -Parent $PSScriptRoot
$sevenZip = Join-Path $env:ProgramFiles '7-Zip/7z.exe'
if (!(Test-Path $sevenZip)) { throw '7-Zip is required to unpack the pinned native SDK.' }
$pin = Get-Content (Join-Path $root 'packaging/input-bridge.json') -Raw | ConvertFrom-Json
$cache = Join-Path $root ('build/input-sdk/' + $pin.sdk_version)
New-Item -ItemType Directory -Force $cache | Out-Null
function Get-VerifiedArchive([string]$Url, [string]$Destination, [string]$Algorithm, [string]$Hash) {
    if (!(Test-Path -LiteralPath $Destination)) { Invoke-WebRequest $Url -OutFile $Destination }
    if ((Get-FileHash -LiteralPath $Destination -Algorithm $Algorithm).Hash -ne $Hash) { throw "Hash mismatch: $Destination" }
}
$archive = Join-Path $cache 'release.zip'
Get-VerifiedArchive $pin.sdk_url $archive SHA256 $pin.sdk_sha256
$sdk = Join-Path $cache 'release'
& $sevenZip x $archive ('-o' + $sdk) -y -bso0
if ($LASTEXITCODE -ne 0) { throw 'HIDMaestro SDK extraction failed.' }
if (!$DotnetPath) {
    $system = Get-Command dotnet -ErrorAction SilentlyContinue
    $systemVersion = if ($system) { (& $system.Source --version 2>$null | Out-String).Trim() } else { '' }
    if ($systemVersion -eq $pin.dotnet_version) { $DotnetPath = $system.Source }
    else {
        $portable = Join-Path $root 'build/dotnet-input'
        $DotnetPath = Join-Path $portable 'dotnet.exe'
        if (!(Test-Path -LiteralPath (Join-Path $portable ('sdk/' + $pin.dotnet_version + '/dotnet.dll')))) {
            $zip = Join-Path $cache ('dotnet-sdk-' + $pin.dotnet_version + '.zip')
            Get-VerifiedArchive $pin.dotnet_url $zip SHA512 $pin.dotnet_sha512
            & $sevenZip x $zip ('-o' + $portable) -y -bso0
            if ($LASTEXITCODE -ne 0) { throw '.NET build SDK extraction failed.' }
        }
    }
}
$DotnetPath = (Resolve-Path -LiteralPath $DotnetPath).Path
$env:DOTNET_CLI_TELEMETRY_OPTOUT = '1'
$env:DOTNET_GENERATE_ASPNET_CERTIFICATE = 'false'
$env:DOTNET_CLI_HOME = Join-Path $root 'build/dotnet-home'
$env:PATH = "${env:ProgramFiles(x86)}\Microsoft Visual Studio\Installer;" + $env:PATH
$out = Join-Path $root 'build/input-bridge'
Push-Location (Join-Path $root 'native/input-bridge')
try {
    $selectedVersion = (& $DotnetPath --version | Out-String).Trim()
    if ($LASTEXITCODE -ne 0 -or $selectedVersion -ne $pin.dotnet_version) { throw 'Use the pinned .NET SDK from packaging/input-bridge.json.' }
    & (Join-Path $PSScriptRoot 'prepare-input-sdk.ps1') -DotnetPath $DotnetPath
    & $DotnetPath publish MetinInput.csproj -c Release -r win-x64 --self-contained true -o $out
    if ($LASTEXITCODE -ne 0) { throw 'NativeAOT input bridge build failed.' }
} finally { Pop-Location }
$licenses = Join-Path $out 'licenses'
New-Item -ItemType Directory -Force $licenses | Out-Null
Copy-Item (Join-Path $sdk 'LICENSE') (Join-Path $licenses 'HIDMaestro.txt')
Copy-Item (Join-Path $sdk 'THIRD-PARTY-NOTICES.txt') (Join-Path $licenses 'HIDMaestro-third-party.txt')
@"
HIDMaestro $($pin.sdk_version), MIT, source $($pin.source_commit).
The Companion source adapter changes EnsureForCreate to reject a missing or
older driver instead of installing it. Explicit installer management retains
the upstream installation/removal path. Native driver payloads and embedded
profiles are byte-identical to the pinned official release.
"@ | Set-Content (Join-Path $licenses 'HIDMaestro-adapter.txt')
$runtimeNotices = Join-Path (Split-Path -Parent $DotnetPath) 'ThirdPartyNotices.txt'
if (!(Test-Path $runtimeNotices)) { throw 'Missing .NET native runtime notices.' }
Copy-Item $runtimeNotices (Join-Path $licenses 'Dotnet-third-party.txt')
Copy-Item (Join-Path (Split-Path -Parent $DotnetPath) 'LICENSE.txt') (Join-Path $licenses 'Dotnet.txt')
if (!$SkipDevelopmentStaging) {
    foreach ($folder in @('target/debug', 'target/release', 'target/debug/examples', 'target/release/examples')) {
        $destination = Join-Path $root $folder
        New-Item -ItemType Directory -Force $destination | Out-Null
        Copy-Item (Join-Path $out 'MetinInput.dll') $destination -Force
    }
}
Write-Output (Join-Path $out 'MetinInput.dll')
