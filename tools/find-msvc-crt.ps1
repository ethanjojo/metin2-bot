$ErrorActionPreference = 'Stop'
$locator = Join-Path ${env:ProgramFiles(x86)} 'Microsoft Visual Studio/Installer/vswhere.exe'
if (!(Test-Path -LiteralPath $locator)) { throw 'Visual Studio locator is missing.' }
$installation = & $locator -latest -products '*' -requires Microsoft.VisualStudio.Component.VC.Tools.x86.x64 -property installationPath
if ($LASTEXITCODE -ne 0 -or !$installation) { throw 'Visual Studio C++ tools are missing.' }
$candidates = Get-ChildItem (Join-Path $installation 'VC/Redist/MSVC/*/x64/Microsoft.VC*.CRT') -Directory | Sort-Object FullName -Descending
foreach ($candidate in $candidates) {
    $missing = @('msvcp140.dll','vcruntime140.dll','vcruntime140_1.dll','concrt140.dll') | Where-Object { !(Test-Path (Join-Path $candidate.FullName $_)) }
    if (!$missing) { Write-Output $candidate.FullName; return }
}
throw 'No complete x64 VC runtime redistributable directory found.'
