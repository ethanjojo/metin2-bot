[CmdletBinding()]
param([Parameter(Mandatory)][string]$Tag,[Parameter(Mandatory)][string]$Version,
      [Parameter(Mandatory)][string]$CommitSha,[Parameter(Mandatory)][string]$InstallerName,[bool]$Prerelease)
$ErrorActionPreference='Stop'
$root=Split-Path -Parent $PSScriptRoot
$identity = Get-Content -LiteralPath (Join-Path $root 'packaging/app-identity.json') -Raw | ConvertFrom-Json
if ($Tag -notmatch '^v[0-9A-Za-z.+-]+$' -or $CommitSha -notmatch '^[0-9a-f]{40}$' -or
    $InstallerName -ne "$($identity.product_name)Setup-$Version-windows-x64.exe" -or $Version -notmatch '^[0-9A-Za-z.+-]+$') { throw 'Invalid release identifiers.' }
if ($Tag -ne "v$Version" -or $Prerelease -ne $Version.Split('+')[0].Contains('-')) { throw 'Release channel/tag mismatch.' }
if (!$Prerelease) {
    $policy = Get-Content (Join-Path $root 'packaging/release-policy.json') -Raw | ConvertFrom-Json
    if (!$policy.stable_ready) { throw 'Stable release acceptance is incomplete.' }
}
$release=Join-Path $root 'dist\release'
$installer=Join-Path $release $InstallerName
$checksums=Join-Path $release 'SHA256SUMS.txt'
$assets = @($installer, $checksums)
$expectedNames = @($InstallerName, 'SHA256SUMS.txt')
function Test-ArtifactPair([string]$Directory) {
    $lines = @(Get-Content -LiteralPath (Join-Path $Directory 'SHA256SUMS.txt'))
    if ($lines.Count -ne 1) { throw 'Unexpected release checksum inventory.' }
    if ($lines[0] -cnotmatch '^([0-9a-f]{64})  ([A-Za-z0-9.+_-]+)$') { throw 'Invalid checksum entry.' }
    $hash=$Matches[1]
    if ($Matches[2] -cne $InstallerName) { throw 'Unexpected release checksum inventory.' }
    if ((Get-FileHash -LiteralPath (Join-Path $Directory $InstallerName) -Algorithm SHA256).Hash.ToLowerInvariant() -cne $hash) { throw 'Release checksum mismatch.' }
}
Test-ArtifactPair $release

function Get-RemoteRelease([bool]$AllowMissing) {
    $ErrorActionPreference='Continue'
    $result=gh release view $Tag --json databaseId,isDraft,isPrerelease,targetCommitish,tagName 2>&1
    $ErrorActionPreference='Stop'
    if ($LASTEXITCODE -ne 0) {
        if ($AllowMissing -and (($result | Out-String) -match 'release not found|HTTP 404')) { return $null }
        throw 'Could not inspect the remote release.'
    }
    $metadata=($result | Out-String) | ConvertFrom-Json
    if ($metadata.tagName -cne $Tag -or $metadata.targetCommitish -cne $CommitSha -or
        $metadata.isPrerelease -isnot [bool] -or $metadata.isPrerelease -ne $Prerelease -or
        $metadata.isDraft -isnot [bool] -or $metadata.databaseId -le 0) { throw 'Release identity/channel mismatch.' }
    return $metadata
}

function Get-RemoteAssets($Metadata) {
    # Enumerate every page so a stale asset cannot hide outside the first page.
    $result=gh api "repos/$env:GH_REPO/releases/$($Metadata.databaseId)/assets?per_page=100" --paginate --slurp
    if ($LASTEXITCODE -ne 0) { throw 'Could not inspect the remote release assets.' }
    $pages=($result | Out-String) | ConvertFrom-Json
    return @(foreach ($page in $pages) { foreach ($asset in $page) { $asset } })
}

function Test-RemoteArtifacts($Metadata,[bool]$RequireLocalBytes) {
    $remoteAssets=@(Get-RemoteAssets $Metadata)
    if ($remoteAssets.Count -ne $expectedNames.Count -or
        @($remoteAssets.name | Sort-Object -Unique).Count -ne $expectedNames.Count) { throw 'Unexpected remote release asset inventory.' }
    foreach ($asset in $remoteAssets) {
        if ($expectedNames -cnotcontains $asset.name) { throw 'Unexpected remote release asset inventory.' }
        if ($asset.state -cne 'uploaded' -or $asset.size -le 0 -or $asset.digest -cnotmatch '^sha256:[0-9a-f]{64}$') { throw 'Invalid remote release asset metadata.' }
    }
    $download=Join-Path $root ('build/release-download/' + [Guid]::NewGuid().ToString())
    New-Item -ItemType Directory -Force $download | Out-Null
    gh release download $Tag --dir $download
    if ($LASTEXITCODE -ne 0) { throw 'Release asset download failed.' }
    $files=@(Get-ChildItem -LiteralPath $download)
    if ($files.Count -ne $expectedNames.Count -or @($files | Where-Object { $_.PSIsContainer -or $expectedNames -cnotcontains $_.Name }).Count) { throw 'Downloaded release asset inventory mismatch.' }
    foreach ($asset in $remoteAssets) {
        $path=Join-Path $download $asset.name
        if ((Get-Item -LiteralPath $path).Length -ne $asset.size) { throw 'Downloaded release asset size mismatch.' }
        $hash=(Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()
        if ('sha256:'+$hash -cne $asset.digest) { throw 'Downloaded release asset digest mismatch.' }
        if ($RequireLocalBytes -and $hash -cne (Get-FileHash -LiteralPath (Join-Path $release $asset.name) -Algorithm SHA256).Hash.ToLowerInvariant()) { throw 'Remote asset differs from the uploaded local artifact.' }
    }
    Test-ArtifactPair $download
}
function Test-RemoteTag([bool]$AllowMissing) {
    # Check the ref first: a missing commit lookup returns 422, not a missing-ref 404.
    $ref=[Uri]::EscapeDataString($Tag)
    $ErrorActionPreference='Continue'
    $reference=gh api "repos/$env:GH_REPO/git/ref/tags/$ref" --jq .object.sha 2>&1
    $ErrorActionPreference='Stop'
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
$existing=Get-RemoteRelease $true
if ($null -ne $existing) {
    if (!$existing.isDraft) {
        # A same-source rebuild can have different installer bytes. The published
        # pair is canonical: verify its identity and integrity without replacing it.
        Test-RemoteArtifacts $existing $false
        Test-RemoteTag $false | Out-Null
        Write-Host 'Published release identity, inventory, digests and downloaded checksum pair verified; immutable assets unchanged.'
        return
    }
    foreach ($asset in @(Get-RemoteAssets $existing)) {
        if ($expectedNames -cnotcontains $asset.name) {
            gh release delete-asset $Tag $asset.name --yes
            if ($LASTEXITCODE -ne 0) { throw 'Removing a stale draft asset failed.' }
        }
    }
    gh release upload $Tag @assets --clobber
    if ($LASTEXITCODE -ne 0) { throw 'Draft asset upload failed.' }
} else {
    $arguments=@('release','create',$Tag) + $assets + @('--target',$CommitSha,'--title',"$($identity.product_name) $Version",'--notes-file',(Join-Path $root 'docs/release-notes.md'),'--draft')
    if ($Prerelease) { $arguments+=@('--prerelease','--latest=false') } else { $arguments+='--latest' }
    & gh @arguments
    if ($LASTEXITCODE -ne 0) { throw 'Draft release creation failed.' }
}
# Check the draft's complete inventory and downloaded bytes before exposure.
$draft=Get-RemoteRelease $false
if (!$draft.isDraft) { throw 'Release stopped being a draft before verification.' }
Test-RemoteArtifacts $draft $true
Test-RemoteTag $false | Out-Null
gh release edit $Tag --draft=false
if ($LASTEXITCODE -ne 0) { throw 'Publishing the verified draft failed.' }
$published=Get-RemoteRelease $false
if ($published.isDraft) { throw 'Release is still a draft after publication.' }
Test-RemoteArtifacts $published $true
Test-RemoteTag $false | Out-Null
Write-Host 'Published release identity, inventory, digests and downloaded checksum pair verified.'
