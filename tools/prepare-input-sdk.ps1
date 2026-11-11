[CmdletBinding()]
param([Parameter(Mandatory)][string]$DotnetPath)
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$root = Split-Path -Parent $PSScriptRoot
$cache = Join-Path $root 'build/input-sdk/2.1.0'
$adapter = Join-Path $root 'native/hidmaestro-sdk'
$env:DOTNET_CLI_HOME = Join-Path $root 'build/dotnet-home'
$env:DOTNET_CLI_TELEMETRY_OPTOUT = '1'
$env:DOTNET_GENERATE_ASPNET_CERTIFICATE = 'false'
$env:DOTNET_ADD_GLOBAL_TOOLS_TO_PATH = '0'
$archive = Join-Path $cache 'source.zip'
$pin = Get-Content (Join-Path $root 'packaging/input-bridge.json') -Raw | ConvertFrom-Json
$sourceHash = $pin.source_sha256
$sourceUrl = $pin.source_url
$localArchive = Join-Path $root 'build/hid-research/hidmaestro-v2.1.0/source.zip'
$release = Join-Path $cache 'release/HIDMaestro.Core.dll'
if (!(Test-Path -LiteralPath $release)) { throw 'Prepare the verified canonical release cache before the source adapter.' }
New-Item -ItemType Directory -Force -Path $cache | Out-Null
if (!(Test-Path -LiteralPath $archive)) {
    if (Test-Path -LiteralPath $localArchive) { Copy-Item -LiteralPath $localArchive -Destination $archive }
    else { Invoke-WebRequest -Uri $sourceUrl -OutFile $archive }
}
if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ne $sourceHash) { throw 'Pinned HIDMaestro source archive hash mismatch.' }
$sourceDirectory = Join-Path $cache 'source'
& (Join-Path $env:ProgramFiles '7-Zip/7z.exe') x $archive ('-o' + $sourceDirectory) -y -bso0
if ($LASTEXITCODE -ne 0) { throw 'Pinned HIDMaestro source extraction failed.' }
$upstream = Join-Path $sourceDirectory 'hifihedgehog-HIDMaestro-c28c1f6/sdk/HIDMaestro.Core/Internal/DriverDeploy.cs'
if ((Get-FileHash -LiteralPath $upstream -Algorithm SHA256).Hash -ne 'f6e77013cc2a9939aa44c2d0c4fac7eed56533f15f8fa9ecb364190a0080551e') { throw 'Pinned HIDMaestro deploy source hash mismatch.' }
Import-Module (Join-Path $PSScriptRoot 'input-sdk-source.psm1') -Force
$patched = ConvertTo-ExplicitDriverSource -Source ([IO.File]::ReadAllText($upstream)) -AdapterDirectory $adapter
$generated = Join-Path $cache 'generated'
New-Item -ItemType Directory -Force -Path $generated | Out-Null
[IO.File]::WriteAllText((Join-Path $generated 'DriverDeploy.cs'), $patched, [Text.UTF8Encoding]::new($false))
& $DotnetPath run --project (Join-Path $root 'native/sdk-resource-extractor/SdkResourceExtractor.csproj') -c Release -- $release (Join-Path $cache 'resources') (Join-Path $cache 'resources.json')
if ($LASTEXITCODE -ne 0) { throw 'Pinned SDK resource extraction failed.' }
$resources = Get-Content -LiteralPath (Join-Path $cache 'resources.json') -Raw | ConvertFrom-Json
$kernelInputs = @('catalog')
foreach ($entry in $resources.PSObject.Properties | Sort-Object Name) {
    if ($entry.Name.StartsWith('HIDMaestro.Profiles.') -or $entry.Name.StartsWith('HIDMaestro.Models.')) {
        $kernelInputs += (Join-Path (Join-Path $cache 'resources') $entry.Name) + '|' + $entry.Value
    }
}
$kernelInputPath = Join-Path $generated 'catalog-inputs.txt'
[IO.File]::WriteAllLines($kernelInputPath, $kernelInputs, [Text.UTF8Encoding]::new($false))
$kernelGenerator = Join-Path $sourceDirectory 'hifihedgehog-HIDMaestro-c28c1f6/sdk/HIDMaestro.KernelGen/HIDMaestro.KernelGen.csproj'
& $DotnetPath run --project $kernelGenerator -c Release -- catalog (Join-Path $generated 'CatalogKernels.g.cs') $kernelInputPath
if ($LASTEXITCODE -ne 0) { throw 'Pinned SDK catalog kernel generation failed.' }
& $DotnetPath build (Join-Path $adapter 'HIDMaestro.Core.csproj') -c Release
if ($LASTEXITCODE -ne 0) { throw 'Explicit-install HIDMaestro source adapter build failed.' }
& $DotnetPath run --project (Join-Path $root 'native/hidmaestro-sdk-tests/HIDMaestroTest.csproj') -c Release -- (Join-Path $cache 'resources.json')
if ($LASTEXITCODE -ne 0) { throw 'Offline explicit-install policy/resource verification failed.' }
Write-Output (Join-Path $adapter 'HIDMaestro.Core.csproj')
