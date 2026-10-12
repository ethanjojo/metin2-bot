[CmdletBinding()]
param(
    [string]$Version = '0.1.0-dev',
    [ValidateRange(0, 65535)][int]$BuildNumber = 0,
    [string]$CommitSha = '0000000',
    [string]$PythonPath = '',
    [string]$CompilerPath = ''
)

$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$repoRoot = Split-Path -Parent $PSScriptRoot
if (-not $PythonPath) { $PythonPath = Join-Path $repoRoot '.venv\Scripts\python.exe' }
if (-not (Test-Path -LiteralPath $PythonPath -PathType Leaf)) {
    throw "Python not found: $PythonPath. Create .venv and install requirements.txt and requirements-build.txt."
}
$PythonPath = (Resolve-Path -LiteralPath $PythonPath).Path
$buildDir = Join-Path $repoRoot 'build\windows'
$metadataDir = Join-Path $buildDir 'metadata'
$bundleDist = Join-Path $repoRoot 'dist\windows'
$releaseDir = Join-Path $repoRoot 'dist\release'

$metadataJson = & $PythonPath (Join-Path $repoRoot 'tools\build_metadata.py') --version $Version --build-number $BuildNumber --commit-sha $CommitSha --output-dir $metadataDir
if ($LASTEXITCODE -ne 0) { throw 'Invalid release metadata.' }
$metadata = $metadataJson | ConvertFrom-Json

if (-not $CompilerPath) {
    $compilerCommand = Get-Command 'ISCC.exe' -ErrorAction SilentlyContinue
    if ($compilerCommand) { $CompilerPath = $compilerCommand.Source }
    else {
        foreach ($programDir in @(${env:ProgramFiles(x86)}, $env:ProgramFiles)) {
            if (-not $programDir) { continue }
            foreach ($innoVersion in @(6, 7)) {
                $candidate = Join-Path $programDir "Inno Setup $innoVersion\ISCC.exe"
                if (Test-Path -LiteralPath $candidate -PathType Leaf) { $CompilerPath = $candidate; break }
            }
            if ($CompilerPath) { break }
        }
    }
}
if (-not $CompilerPath -or -not (Test-Path -LiteralPath $CompilerPath -PathType Leaf)) {
    throw 'Inno Setup compiler ISCC.exe not found. Install Inno Setup 6.7.3 or provide -CompilerPath.'
}
$CompilerPath = (Resolve-Path -LiteralPath $CompilerPath).Path
& $CompilerPath /Q (Join-Path $repoRoot 'packaging\check_compiler.iss')
if ($LASTEXITCODE -ne 0) { throw 'Inno Setup 6.7.3 is required.' }

& $PythonPath (Join-Path $repoRoot 'packaging\check_environment.py')
if ($LASTEXITCODE -ne 0) { throw 'Build environment validation failed. Install requirements.txt and requirements-build.txt with this Python.' }

# Clear only the known generated bundle: stale dependency files must never enter an installer.
$bundleDir = Join-Path $bundleDist 'MetinBot'
if (Test-Path -LiteralPath $bundleDir) {
    $resolvedBundle = (Resolve-Path -LiteralPath $bundleDir).Path
    if ($resolvedBundle -ne [IO.Path]::GetFullPath($bundleDir)) { throw 'Unexpected bundle path.' }
    Remove-Item -LiteralPath $resolvedBundle -Recurse -Force
}
New-Item -ItemType Directory -Path $releaseDir -Force | Out-Null
$previousMetadataEnv = $env:METINBOT_BUILD_METADATA_DIR
try {
    $env:METINBOT_BUILD_METADATA_DIR = $metadataDir
    & $PythonPath -m PyInstaller --noconfirm --clean --distpath $bundleDist --workpath $buildDir (Join-Path $repoRoot 'packaging\MetinBot.spec')
    if ($LASTEXITCODE -ne 0) { throw 'PyInstaller build failed.' }
    & $CompilerPath "/DAppVersion=$($metadata.version)" "/DWindowsVersion=$($metadata.windows_version)" "/DBundleDir=$bundleDir" "/DReleaseDir=$releaseDir" (Join-Path $repoRoot 'packaging\installer.iss')
    if ($LASTEXITCODE -ne 0) { throw 'Inno Setup installer compilation failed.' }
    $installer = Join-Path $releaseDir $metadata.artifact_name
    if (-not (Test-Path -LiteralPath $installer -PathType Leaf)) { throw 'Installer artifact missing.' }
    $hash = (Get-FileHash -LiteralPath $installer -Algorithm SHA256).Hash.ToLowerInvariant()
    [IO.File]::WriteAllText((Join-Path $releaseDir 'SHA256SUMS.txt'), "$hash  $($metadata.artifact_name)`n", [Text.UTF8Encoding]::new($false))
    Write-Host "Installer: $installer"
    Write-Host "SHA256: $hash"
} finally {
    $env:METINBOT_BUILD_METADATA_DIR = $previousMetadataEnv
}
