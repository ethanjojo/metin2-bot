[CmdletBinding()]
param([Parameter(Mandatory)][string]$InstallerPath,[Parameter(Mandatory)][string]$Version,[Parameter(Mandatory)][string]$CommitSha,[Parameter(Mandatory)][string]$BundleDirectory,[string]$PythonPath = "python")
$ErrorActionPreference = 'Stop'
# Installing/uninstalling with the production AppId is safe only in a fresh hosted VM.
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') { throw 'Installer acceptance runs only in the disposable GitHub Actions VM.' }
$root=Split-Path -Parent $PSScriptRoot
$build=Join-Path $root 'build'
$installation=Join-Path $env:ProgramFiles ('MetinBot-CI-' + $env:GITHUB_RUN_ID)
New-Item -ItemType Directory -Force -Path $build | Out-Null
& $PythonPath (Join-Path $root 'tools\verify_bundle.py') --directory $BundleDirectory --version $Version --commit $CommitSha
if ($LASTEXITCODE -ne 0) { throw 'Bundle inspection failed.' }
$InstallerPath=(Resolve-Path -LiteralPath $InstallerPath).Path
$log=Join-Path $build 'installer.log'
$arguments='/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /SP- /DIR="'+$installation+'" /LOG="'+$log+'"'
$installer=Start-Process -FilePath $InstallerPath -ArgumentList $arguments -WindowStyle Hidden -Wait -PassThru
if ($installer.ExitCode -ne 0) { throw 'Installer acceptance failed.' }
$userRoot = Join-Path $env:LOCALAPPDATA 'MetinBot'
if (Test-Path $userRoot) { throw 'Hosted acceptance requires an empty application data root.' }
New-Item -ItemType Directory $userRoot | Out-Null
$sentinel = Join-Path $userRoot 'ci-data-retained.txt'
Set-Content $sentinel 'preserve during upgrade and uninstall'
try {
    # Exercise upgrade cleanup of the former Python dependency directory.
    $oldRuntime = Join-Path $installation '_internal'
    New-Item -ItemType Directory -Force $oldRuntime | Out-Null
    Set-Content (Join-Path $oldRuntime 'obsolete-runtime.txt') 'legacy fixture'
    $upgraded = Start-Process -FilePath $InstallerPath -ArgumentList $arguments -WindowStyle Hidden -Wait -PassThru
    if ($upgraded.ExitCode -ne 0 -or (Test-Path $oldRuntime)) { throw 'Upgrade cleanup failed.' }
    $report=Join-Path $build 'selftest-installed.json'
    $arguments='--self-test --self-test-report "'+$report+'"'
    $savedPath = $env:PATH
    $env:PATH = "$env:SystemRoot\System32;$env:SystemRoot"
    $app=Start-Process -FilePath (Join-Path $installation 'MetinBot.exe') -ArgumentList $arguments -WindowStyle Hidden -PassThru
    $env:PATH = $savedPath
    if (!$app.WaitForExit(60000)) { $app.Kill();throw 'Installed self-test timed out.' }
    if ($app.ExitCode -ne 0 -or !(Test-Path -LiteralPath $report)) { throw 'Installed self-test failed.' }
    $result=Get-Content -LiteralPath $report -Raw | ConvertFrom-Json
    if (!$result.success -or $result.build.version -ne $Version -or $result.build.commit_sha -ne $CommitSha -or $result.build.runtime -ne 'rust' -or $result.build.working_tree_dirty) { throw 'Installed self-test report mismatch.' }
} finally {
    $uninstaller=Join-Path $installation 'unins000.exe'
    if (Test-Path -LiteralPath $uninstaller) {
        $removed=Start-Process -FilePath $uninstaller -ArgumentList '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART' -WindowStyle Hidden -Wait -PassThru
        if ($removed.ExitCode -ne 0) { throw 'Uninstall acceptance failed.' }
    }
}
if (Test-Path -LiteralPath (Join-Path $installation 'MetinBot.exe')) { throw 'Application remains after uninstall.' }
Write-Host 'Installer PASS: install, bundled self-test and uninstall in hosted Windows VM.'

if (!(Test-Path $sentinel)) { throw 'Upgrade/uninstall removed user data.' }
Remove-Item -LiteralPath $sentinel
Remove-Item -LiteralPath $userRoot
