[CmdletBinding()]
param([Parameter(Mandatory)][string]$Tag,[Parameter(Mandatory)][string]$Version,
      [Parameter(Mandatory)][string]$CommitSha,[Parameter(Mandatory)][string]$InstallerName,[bool]$Prerelease)
$ErrorActionPreference='Stop'
if ($Tag -notmatch '^v[0-9A-Za-z.+-]+$' -or $CommitSha -notmatch '^[0-9a-f]{40}$' -or
    $InstallerName -ne "MetinBotSetup-$Version-windows-x64.exe" -or $Version -notmatch '^[0-9A-Za-z.+-]+$') { throw 'Invalid release identifiers.' }
$root=Split-Path -Parent $PSScriptRoot
if ($Tag -ne "v$Version" -or $Prerelease -ne $Version.Split('+')[0].Contains('-')) { throw 'Release channel/tag mismatch.' }
if (!$Prerelease) {
    $policy = Get-Content (Join-Path $root 'packaging/release-policy.json') -Raw | ConvertFrom-Json
    if (!$policy.stable_ready) { throw 'Stable release acceptance is incomplete.' }
}
$release=Join-Path $root 'dist\release'
$installer=Join-Path $release $InstallerName
$checksums=Join-Path $release 'SHA256SUMS.txt'
$zip = Join-Path $release "MetinBot-$Version-windows-x64.zip"
$assets = @($installer, $zip, $checksums)
$expectedNames = @($InstallerName, "MetinBot-$Version-windows-x64.zip") | Sort-Object
$lines = @(Get-Content -LiteralPath $checksums | Where-Object { $_ })
if ($lines.Count -ne 2) { throw 'Unexpected release checksum inventory.' }
$remoteHashes = @{}
foreach ($line in $lines) {
    if ($line -notmatch '^([0-9a-f]{64})  ([A-Za-z0-9.+_-]+)$') { throw 'Invalid checksum entry.' }
    $name = $Matches[2]
    if ($name -notin $expectedNames -or $remoteHashes.ContainsKey($name)) { throw 'Unexpected or duplicate release artifact.' }
    $remoteHashes[$name] = $Matches[1]
    if ((Get-FileHash -LiteralPath (Join-Path $release $name) -Algorithm SHA256).Hash.ToLowerInvariant() -ne $Matches[1]) { throw 'Release checksum mismatch.' }
}
function Test-RemoteTag([bool]$AllowMissing) {
    # Check the ref first: a missing commit lookup returns 422, not a missing-ref 404.
    $ref=[Uri]::EscapeDataString($Tag)
    $reference=gh api "repos/$env:GH_REPO/git/ref/tags/$ref" --jq .object.sha 2>&1
    if ($LASTEXITCODE -ne 0) {
        if ($AllowMissing -and (($reference | Out-String) -match 'HTTP 404')) { return $false }
        throw 'Could not verify the remote release tag.'
    }
    # Resolve both lightweight and annotated tags to their final commit.
    $resolved=gh api "repos/$env:GH_REPO/commits/$ref" --jq .sha 2>&1
    if ($LASTEXITCODE -ne 0) { throw 'Could not resolve the remote release tag commit.' }
    if (($resolved | Out-String).Trim() -ne $CommitSha) { throw 'Remote release tag does not point to the tested commit.' }
    return $true
}
if (!(Test-RemoteTag $Prerelease)) {
    gh api --method POST "repos/$env:GH_REPO/git/refs" -f "ref=refs/tags/$Tag" -f "sha=$CommitSha"
    if ($LASTEXITCODE -ne 0) { throw 'Creating the release tag failed.' }
    Test-RemoteTag $false | Out-Null
}
$existing=gh release view $Tag --json isDraft,targetCommitish 2>$null
if ($LASTEXITCODE -eq 0) {
    $existing=$existing | ConvertFrom-Json
    if ($existing.targetCommitish -ne $CommitSha) { throw 'Release tag belongs to a different commit.' }
    if (!$existing.isDraft) { Test-RemoteTag $false | Out-Null;Write-Host 'Release is already published; leaving its immutable assets unchanged.';return }
    gh release upload $Tag @assets --clobber
    if ($LASTEXITCODE -ne 0) { throw 'Draft asset upload failed.' }
} else {
    $arguments=@('release','create',$Tag) + $assets + @('--target',$CommitSha,'--title',"MetinBot $Version",'--notes-file',(Join-Path $root 'docs/release-notes.md'),'--draft')
    if ($Prerelease) { $arguments+=@('--prerelease','--latest=false') } else { $arguments+='--latest' }
    & gh @arguments
    if ($LASTEXITCODE -ne 0) { throw 'Draft release creation failed.' }
}
# Publish only after both verified assets were uploaded successfully.
Test-RemoteTag $false | Out-Null
gh release edit $Tag --draft=false
if ($LASTEXITCODE -ne 0) { throw 'Publishing the verified draft failed.' }

$download = Join-Path $root ('build/release-download/' + [Guid]::NewGuid().ToString())
New-Item -ItemType Directory -Force $download | Out-Null
gh release download $Tag --dir $download
if ($LASTEXITCODE -ne 0) { throw 'Published release download failed.' }
foreach ($name in $expectedNames) {
    if ((Get-FileHash -LiteralPath (Join-Path $download $name) -Algorithm SHA256).Hash.ToLowerInvariant() -ne $remoteHashes[$name]) { throw 'Downloaded release checksum mismatch.' }
}
Write-Host 'Published installer and ZIP download checksums verified.'
