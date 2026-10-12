param([switch]$Cancel)
$ErrorActionPreference = 'Stop'
$script:launchCalls = @()
function Start-Process {
    param($FilePath, $ArgumentList, $WorkingDirectory, $Verb, $ErrorAction)
    if ($Cancel) { throw [System.ComponentModel.Win32Exception]::new(1223) }
    $script:launchCalls += [PSCustomObject]@{
        FilePath=$FilePath; ArgumentList=$ArgumentList; WorkingDirectory=$WorkingDirectory; Verb=$Verb
    }
}
$projectRoot = Split-Path -Parent $PSScriptRoot
$launcher = Join-Path $projectRoot 'tools\launch.ps1'
if (!$Cancel) {
    . $launcher -CheckOnly
    if ($script:launchCalls.Count -ne 0) { throw 'CheckOnly attempted to start a process.' }
}
. $launcher
if ($Cancel) { exit $LASTEXITCODE }
if ($script:launchCalls.Count -ne 1) { throw 'Expected exactly one application launch.' }
$request = $script:launchCalls[0]
if ($request.Verb -ne 'RunAs') { throw 'Missing administrator request.' }
if ($request.FilePath -ne (Join-Path $projectRoot '.venv\Scripts\pythonw.exe')) { throw 'Wrong interpreter.' }
if ($request.WorkingDirectory -ne $projectRoot) { throw 'Wrong working directory.' }
if ($request.ArgumentList -ne '-m metin_bot') { throw 'Wrong application arguments.' }
Write-Host 'Launcher PASS: checks, one native RunAs request, absolute interpreter and working directory.'
