param(
    [string]$NativeRoot = (Join-Path $PSScriptRoot '../build/rust-native/windows'),
    [string]$SevenZipPath = (Join-Path $env:ProgramFiles '7-Zip/7z.exe')
)
$ErrorActionPreference = 'Stop'
if (-not (Test-Path -LiteralPath $SevenZipPath)) { throw '7-Zip is required to extract the official native archives. Set -SevenZipPath explicitly.' }
New-Item -ItemType Directory -Force $NativeRoot | Out-Null
$NativeRoot = (Resolve-Path -LiteralPath $NativeRoot).Path
function Get-VerifiedArchive([string]$Url, [string]$Name, [string]$Sha256) {
    $archive = Join-Path $NativeRoot $Name
    if (-not (Test-Path -LiteralPath $archive)) { Invoke-WebRequest -Uri $Url -OutFile $archive }
    if ((Get-FileHash -LiteralPath $archive -Algorithm SHA256).Hash -ne $Sha256) { throw "Checksum mismatch: $Name. Remove the invalid cached archive before retrying." }
    return $archive
}
$opencvArchive = Get-VerifiedArchive 'https://github.com/opencv/opencv/releases/download/4.13.0/opencv-4.13.0-windows.exe' 'opencv-4.13.0-windows.exe' 'F0E98C302464D6860777A7015065E11B9B271B5394E6BA92663F0CF1FC303F2C'
$llvmArchive = Get-VerifiedArchive 'https://github.com/llvm/llvm-project/releases/download/llvmorg-20.1.8/LLVM-20.1.8-win64.exe' 'LLVM-20.1.8-win64.exe' '3197846A2B19063687DD56E93E34CD941E3548D907F23A6131571321BDF9FE7B'
if (-not (Test-Path -LiteralPath (Join-Path $NativeRoot 'opencv/build/include/opencv2/core.hpp'))) {
    & $SevenZipPath x $opencvArchive "-o$NativeRoot" '-y' | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'OpenCV extraction failed.' }
}
if (-not (Test-Path -LiteralPath (Join-Path $NativeRoot 'llvm20/bin/libclang.dll'))) {
    & $SevenZipPath x $llvmArchive "-o$(Join-Path $NativeRoot 'llvm20')" 'bin/clang.exe' 'bin/libclang.dll' 'lib/clang/*' '-y' | Out-Null
    if ($LASTEXITCODE -ne 0) { throw 'LLVM extraction failed.' }
}
Write-Output $NativeRoot
