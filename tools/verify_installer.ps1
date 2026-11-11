[CmdletBinding()]
param([Parameter(Mandatory)][string]$InstallerPath,[Parameter(Mandatory)][string]$Version,[Parameter(Mandatory)][string]$CommitSha,[Parameter(Mandatory)][string]$BundleDirectory,[string]$PythonPath = 'python')
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
. (Join-Path $PSScriptRoot 'installer_acceptance.ps1')
# The production AppId and shared driver may only be changed in a fresh hosted VM.
if ($env:GITHUB_ACTIONS -ne 'true' -or $env:RUNNER_ENVIRONMENT -ne 'github-hosted') { throw 'Installer acceptance runs only in the disposable GitHub Actions VM.' }
$root = Split-Path -Parent $PSScriptRoot
$build = Join-Path $root 'build'
$fixtureRoot = Join-Path $env:ProgramFiles ('Companion-CI-' + $env:GITHUB_RUN_ID)
$installation = Join-Path $fixtureRoot 'MetinBot'
if ((Test-Path -LiteralPath $fixtureRoot) -or (Test-Path -LiteralPath 'HKLM:\Software\Microsoft\Windows\CurrentVersion\Uninstall\{78F36AC5-1367-4498-B486-50E3326B9426}_is1')) { throw 'Hosted acceptance requires no existing production AppId or fixture installation.' }
New-Item -ItemType Directory -Force -Path $build | Out-Null
& $PythonPath (Join-Path $root 'tools/verify_bundle.py') --directory $BundleDirectory --version $Version --commit $CommitSha
if ($LASTEXITCODE -ne 0) { throw 'Bundle inspection failed.' }
$InstallerPath = (Resolve-Path -LiteralPath $InstallerPath).Path
$bundleApp = Join-Path (Resolve-Path -LiteralPath $BundleDirectory).Path 'Companion.exe'
$legacyUserRoot = Join-Path $env:LOCALAPPDATA 'MetinBot'
$userRoot = Join-Path $env:LOCALAPPDATA 'Companion'
if ((Test-Path -LiteralPath $userRoot) -or (Test-Path -LiteralPath $legacyUserRoot)) { throw 'Hosted acceptance requires empty current and previous application data roots.' }
function Invoke-Driver([string]$Executable, [string]$Operation, [string]$Label, [bool]$ExpectedSuccess = $true) {
    $report = Join-Path $build ('input-driver-' + $Label + '.json')
    $arguments = '--input-driver ' + $Operation + ' --input-driver-report "' + $report + '"'
    $process = Start-Process -FilePath $Executable -ArgumentList $arguments -WindowStyle Hidden -Wait -PassThru
    if (!(Test-Path -LiteralPath $report)) { throw "Missing driver report: $Label" }
    $result = Get-Content -LiteralPath $report -Raw | ConvertFrom-Json
    if ($result.operation -ne $Operation -or $result.success -ne $ExpectedSuccess -or (($process.ExitCode -eq 0) -ne $ExpectedSuccess)) { throw "Driver acceptance failed: $Label" }
    return $result
}
# Refuse to remove a pre-existing shared driver even on a misconfigured hosted VM.
$before = Invoke-Driver $bundleApp 'preview' 'before-install-preview'
Assert-NoManagedDriverArtifacts ($before.message | ConvertFrom-Json)
$initiallyEmptyDriverInventory = $true
Invoke-Driver $bundleApp 'status' 'before-install' $false | Out-Null
if (Test-Path -LiteralPath $userRoot) { throw 'Driver management created user data.' }
$baseArguments = '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /SP- /DIR="' + $installation + '"'
$refused = Start-Process -FilePath $InstallerPath -ArgumentList ($baseArguments + ' /LOG="' + (Join-Path $build 'installer-refused.log') + '"') -WindowStyle Hidden -Wait -PassThru
if ($refused.ExitCode -eq 0 -or (Test-Path -LiteralPath (Join-Path $installation 'Companion.exe'))) { throw 'Silent driver consent was not enforced.' }
$arguments = $baseArguments + ' /ACCEPTINPUTDRIVER=1 /LOG="' + (Join-Path $build 'installer.log') + '"'
$installer = Start-Process -FilePath $InstallerPath -ArgumentList $arguments -WindowStyle Hidden -Wait -PassThru
if ($installer.ExitCode -ne 0) { throw 'Installer acceptance failed.' }
$installedApp = Join-Path $installation 'Companion.exe'
$sentinel = Join-Path $legacyUserRoot 'ci-data-retained.txt'
try {
    Invoke-Driver $installedApp 'status' 'installed'
    if (Test-Path -LiteralPath $userRoot) { throw 'Driver install/status created user data.' }
    $example = Join-Path $root 'profile.example.json'
    $profileId = (Get-Content -LiteralPath $example -Raw | ConvertFrom-Json).id
    $relativeProfile = 'profiles/' + $profileId + '/profile.json'
    $relativeSelection = 'profiles/selection.json'
    New-Item -ItemType Directory -Path (Join-Path $legacyUserRoot ('profiles/' + $profileId)) | Out-Null
    Set-Content -LiteralPath $sentinel 'preserve during upgrade and uninstall'
    $profile = Join-Path $legacyUserRoot $relativeProfile
    Copy-Item -LiteralPath $example -Destination $profile
    $selection = Join-Path $legacyUserRoot $relativeSelection
    @{profile_id=$profileId} | ConvertTo-Json | Set-Content -LiteralPath $selection -Encoding utf8NoBOM
    $profileHash = (Get-FileHash -LiteralPath $profile).Hash
    $selectionHash = (Get-FileHash -LiteralPath $selection).Hash
    # Same-path overlay must retire old executable, DLL and notice using the pre-write inventory.
    $legacyFixture = Add-LegacyPackageFixture $installation
    $inplace = Start-Process -FilePath $InstallerPath -ArgumentList ($arguments.Replace('installer.log', 'installer-inplace.log')) -WindowStyle Hidden -Wait -PassThru
    if ($inplace.ExitCode -ne 0) { throw 'Same-path MetinBot upgrade failed.' }
    Assert-LegacyPackageRetired $installation $legacyFixture

    # Exercise the real default-path choice rather than passing /DIR for relocation.
    $relocateArguments = '/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /SP- /ACCEPTINPUTDRIVER=1 /LOG="' + (Join-Path $build 'installer-relocation.log') + '"'
    $relocated = Start-Process -FilePath $InstallerPath -ArgumentList $relocateArguments -WindowStyle Hidden -Wait -PassThru
    if ($relocated.ExitCode -ne 0 -or (Test-Path -LiteralPath $installation)) { throw 'Default installation relocation failed.' }
    $installation = Join-Path $fixtureRoot 'Companion'
    $installedApp = Join-Path $installation 'Companion.exe'
    $arguments = $arguments.Replace((Join-Path $fixtureRoot 'MetinBot'), $installation)
    $migrationReport = Join-Path $build 'data-migration-installed.json'
    $migrated = Start-Process -FilePath $installedApp -ArgumentList ('--migrate-data --migration-report "' + $migrationReport + '"') -WindowStyle Hidden -Wait -PassThru
    if ($migrated.ExitCode -ne 0 -or (Test-Path -LiteralPath $legacyUserRoot)) { throw 'Default user-data migration failed.' }
    $migration = Get-Content -LiteralPath $migrationReport -Raw | ConvertFrom-Json
    $sentinel = Join-Path $userRoot 'ci-data-retained.txt'
    $profile = Join-Path $userRoot $relativeProfile
    $selection = Join-Path $userRoot $relativeSelection
    if (!$migration.success) { throw 'Data migration validation failed.' }
    foreach ($data in @($userRoot, $migration.details.backup)) {
        if ((Get-FileHash -LiteralPath (Join-Path $data $relativeProfile)).Hash -ne $profileHash -or
            (Get-FileHash -LiteralPath (Join-Path $data $relativeSelection)).Hash -ne $selectionHash -or
            (Get-Content -LiteralPath (Join-Path $data $relativeSelection) -Raw | ConvertFrom-Json).profile_id -ne $profileId) {
            throw 'Migration changed the chosen profile, its validated UUID directory or selection.'
        }
    }

    Invoke-Driver $installedApp 'status' 'upgraded'
    $report = Join-Path $build 'selftest-installed.json'
    $savedPath = $env:PATH
    try {
        $env:PATH = "$env:SystemRoot\System32;$env:SystemRoot"
        $app = Start-Process -FilePath $installedApp -ArgumentList ('--self-test --self-test-report "' + $report + '"') -WindowStyle Hidden -PassThru
    } finally { $env:PATH = $savedPath }
    if (!$app.WaitForExit(60000)) { $app.Kill(); throw 'Installed self-test timed out.' }
    if ($app.ExitCode -ne 0 -or !(Test-Path -LiteralPath $report)) { throw 'Installed self-test failed.' }
    $result = Get-Content -LiteralPath $report -Raw | ConvertFrom-Json
    if (!$result.success -or $result.build.version -ne $Version -or $result.build.commit_sha -ne $CommitSha -or $result.build.runtime -ne 'rust' -or $result.build.working_tree_dirty) { throw 'Installed self-test report mismatch.' }
    # Default uninstall preserves shared input for other applications.
    $uninstaller = Join-Path $installation 'unins000.exe'
    $removed = Start-Process -FilePath $uninstaller -ArgumentList ('/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /LOG="' + (Join-Path $build 'installer-uninstall-default.log') + '"') -WindowStyle Hidden -Wait -PassThru
    if ($removed.ExitCode -ne 0 -or (Test-Path -LiteralPath $installedApp)) { throw 'Default uninstall failed.' }
    Invoke-Driver $bundleApp 'status' 'retained-after-uninstall'
    # A pre-existing custom installation stays in place and uses the same finalizer.
    $custom = Join-Path $fixtureRoot 'Custom installation'
    $reinstallArguments = $arguments.Replace($installation, $custom).Replace('installer.log', 'installer-reinstall.log')
    $installation = $custom
    $installedApp = Join-Path $installation 'Companion.exe'
    $reinstalled = Start-Process -FilePath $InstallerPath -ArgumentList $reinstallArguments -WindowStyle Hidden -Wait -PassThru
    if ($reinstalled.ExitCode -ne 0) { throw 'Reinstall failed.' }
    $legacyFixture = Add-LegacyPackageFixture $installation
    $customUpgrade = Start-Process -FilePath $InstallerPath -ArgumentList ($reinstallArguments.Replace('installer-reinstall.log', 'installer-custom-upgrade.log')) -WindowStyle Hidden -Wait -PassThru
    if ($customUpgrade.ExitCode -ne 0) { throw 'Custom-path upgrade failed.' }
    Assert-LegacyPackageRetired $installation $legacyFixture
} finally {
    # Explicit removal is safe here because the initial fixture proved no driver existed.
    $uninstaller = Join-Path $installation 'unins000.exe'
    if ($initiallyEmptyDriverInventory -and (Test-Path -LiteralPath $uninstaller)) {
        $removed = Start-Process -FilePath $uninstaller -ArgumentList ('/VERYSILENT /SUPPRESSMSGBOXES /NORESTART /REMOVEINPUTDRIVER=1 /LOG="' + (Join-Path $build 'installer-uninstall-driver.log') + '"') -WindowStyle Hidden -Wait -PassThru
        if ($removed.ExitCode -ne 0) { throw 'Explicit driver uninstall acceptance failed.' }
    }
}
$after = Invoke-Driver $bundleApp 'preview' 'after-removal-preview'
Assert-NoManagedDriverArtifacts ($after.message | ConvertFrom-Json)
Invoke-Driver $bundleApp 'status' 'removed' $false | Out-Null
if (Test-Path -LiteralPath $installedApp) { throw 'Application remains after uninstall.' }
if (!(Test-Path -LiteralPath $sentinel) -or (Get-FileHash -LiteralPath $profile).Hash -ne $profileHash -or (Get-FileHash -LiteralPath $selection).Hash -ne $selectionHash) { throw 'Upgrade/uninstall changed user profiles.' }
# Only remove the exact fixtures created above; never recursively clear user storage.
Remove-Item -LiteralPath $sentinel, $profile, $selection
Remove-Item -LiteralPath (Join-Path $userRoot ('profiles/' + $profileId))
Remove-Item -LiteralPath (Join-Path $userRoot 'profiles/.store.lock')
Remove-Item -LiteralPath (Join-Path $userRoot 'profiles')
Remove-Item -LiteralPath $userRoot
Write-Host 'Installer PASS: consent refusal, install/status, in-place and custom-path retirement, default-path relocation, data migration/backup, self-test, default shared-driver retention, explicit removal and profile retention.'
