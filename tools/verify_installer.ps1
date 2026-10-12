[CmdletBinding()]
param([Parameter(Mandatory)][string]$InstallerPath,[Parameter(Mandatory)][string]$Version,[Parameter(Mandatory)][string]$CommitSha)
$ErrorActionPreference = 'Stop'
# Installing/uninstalling with the production AppId is safe only in a fresh hosted VM.
if ($env:GITHUB_ACTIONS -ne 'true') { throw 'Installer acceptance runs only in the disposable GitHub Actions VM.' }
$root=Split-Path -Parent $PSScriptRoot
$build=Join-Path $root 'build'
$installation=Join-Path $env:ProgramFiles ('MetinBot-CI-' + $env:GITHUB_RUN_ID)
New-Item -ItemType Directory -Force -Path $build | Out-Null
& (Join-Path $root '.venv\Scripts\python.exe') (Join-Path $root 'tools\verify_bundle.py') --directory (Join-Path $root 'dist\windows\MetinBot') --version $Version --commit $CommitSha
if ($LASTEXITCODE -ne 0) { throw 'Bundle inspection failed.' }
$InstallerPath=(Resolve-Path -LiteralPath $InstallerPath).Path
$log=Join-Path $build 'installer.log'
$arguments='/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /SP- /DIR="'+$installation+'" /LOG="'+$log+'"'
$installer=Start-Process -FilePath $InstallerPath -ArgumentList $arguments -WindowStyle Hidden -Wait -PassThru
if ($installer.ExitCode -ne 0) { throw 'Installer acceptance failed.' }
try {
    $report=Join-Path $build 'selftest-installed.json'
    $arguments='--self-test --self-test-report "'+$report+'"'
    $app=Start-Process -FilePath (Join-Path $installation 'MetinBot.exe') -ArgumentList $arguments -WindowStyle Hidden -PassThru
    if (!$app.WaitForExit(60000)) { $app.Kill();throw 'Installed self-test timed out.' }
    if ($app.ExitCode -ne 0 -or !(Test-Path -LiteralPath $report)) { throw 'Installed self-test failed.' }
    $result=Get-Content -LiteralPath $report -Raw | ConvertFrom-Json
    if (!$result.ok -or $result.build.version -ne $Version -or $result.build.commit_sha -ne $CommitSha) { throw 'Installed self-test report mismatch.' }
} finally {
    $uninstaller=Join-Path $installation 'unins000.exe'
    if (Test-Path -LiteralPath $uninstaller) {
        $removed=Start-Process -FilePath $uninstaller -ArgumentList '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART' -WindowStyle Hidden -Wait -PassThru
        if ($removed.ExitCode -ne 0) { throw 'Uninstall acceptance failed.' }
    }
}
if (Test-Path -LiteralPath (Join-Path $installation 'MetinBot.exe')) { throw 'Application remains after uninstall.' }
Write-Host 'Installer PASS: install, bundled self-test and uninstall in hosted Windows VM.'
