param([string]$ScriptPath,[string]$Case)
$ErrorActionPreference='Stop'
$script:created=$false
$script:tagCreated=$false
$script:uploaded=$false
$script:published=$false
$script:downloads=0
$script:deleted=@()
$script:sha='1234567890abcdef1234567890abcdef12345678'
$script:name='CompanionSetup-0.1.0-dev.1-windows-x64.exe'
$script:root=Split-Path (Split-Path $ScriptPath -Parent) -Parent
$script:source=Join-Path $script:root 'dist/release'
$script:remote=Join-Path $script:root 'build/stub-remote'
New-Item -ItemType Directory -Force $script:remote | Out-Null
Copy-Item (Join-Path $script:source '*') $script:remote -Force
$script:exists=$Case -ne 'new'
$script:draft=$Case -notlike 'published*'
$env:GH_REPO='test/repo'

function Set-RemoteFault {
    $manifest=Join-Path $script:remote 'SHA256SUMS.txt'
    switch -Regex ($Case) {
        'malformed-checksum$' { Set-Content -LiteralPath $manifest 'not a checksum' -Encoding ASCII }
        'missing-checksum$' { Remove-Item -LiteralPath $manifest }
        'wrong-checksum$' { Set-Content -LiteralPath $manifest ((('a'*64)+'  '+$script:name)) -Encoding ASCII }
        'extra-checksum$' { Add-Content -LiteralPath $manifest ((('a'*64)+'  obsolete.zip')) -Encoding ASCII }
        'extra-asset$|^draft-leftover$|^draft-keeps-extra$' {
            if ($Case -ne 'draft-leftover' -or !$script:uploaded) { Set-Content (Join-Path $script:remote 'obsolete.zip') 'old' }
        }
        'rebuilt$|^draft-wrong-build$' {
            Set-Content -LiteralPath (Join-Path $script:remote $script:name) 'previous installer build' -Encoding ASCII
            $hash=(Get-FileHash (Join-Path $script:remote $script:name) -Algorithm SHA256).Hash.ToLowerInvariant()
            Set-Content -LiteralPath $manifest "$hash  $script:name" -Encoding ASCII
        }
    }
}
Set-RemoteFault

function gh {
    $global:LASTEXITCODE=0
    if ($args[0] -eq 'api') {
        if ($args -contains 'POST') { $script:tagCreated=$true;return }
        if ($args[1] -match '/releases/1/assets') {
            $items=@(Get-ChildItem -LiteralPath $script:remote -File | ForEach-Object {
                $digest=(Get-FileHash -LiteralPath $_.FullName -Algorithm SHA256).Hash.ToLowerInvariant()
                $size=$_.Length
                if ($Case -match 'wrong-digest$' -and $_.Name -eq $script:name) { $digest='a'*64 }
                if ($Case -match 'wrong-manifest-digest$' -and $_.Name -eq 'SHA256SUMS.txt') { $digest='b'*64 }
                if ($Case -match 'wrong-size$' -and $_.Name -eq $script:name) { $size++ }
                $state=if ($Case -match 'incomplete$') { 'starter' } else { 'uploaded' }
                @{id=42;name=$_.Name;size=$size;digest="sha256:$digest";state=$state}
            })
            return ConvertTo-Json -InputObject @(@($items)) -Depth 5 -Compress
        }
        if ($Case -eq 'mismatch') { return ('f'*40) }
        if (!$script:tagCreated -and $Case -eq 'new') {
            $global:LASTEXITCODE=1
            if ($args[1] -match '/commits/') { return 'No commit found for SHA (HTTP 422)' }
            return 'Not found (HTTP 404)'
        }
        return $script:sha
    }
    if ($args[1] -eq 'view') {
        if (!$script:exists) { $global:LASTEXITCODE=1;return 'release not found' }
        if ($Case -eq 'view-failed') { $global:LASTEXITCODE=1;return 'HTTP 503 Service Unavailable' }
        $target=if ($Case -match 'wrong-target$') { 'f'*40 } else { $script:sha }
        $tag=if ($Case -match 'wrong-tag$') { 'v0.1.0-other' } else { 'v0.1.0-dev.1' }
        return @{databaseId=1;isDraft=$script:draft;isPrerelease=($Case -notmatch 'wrong-channel$');targetCommitish=$target;tagName=$tag} | ConvertTo-Json -Compress
    }
    if ($args[1] -eq 'create') { $script:created=$true;$script:exists=$true;Set-RemoteFault;return }
    if ($args[1] -eq 'upload') {
        if (!$script:draft) { throw 'Attempted to overwrite published assets' }
        $script:uploaded=$true
        Copy-Item (Join-Path $script:source '*') $script:remote -Force
        Set-RemoteFault
        return
    }
    if ($args[1] -eq 'delete-asset') {
        if (!$script:draft) { throw 'Attempted to delete published assets' }
        $script:deleted+=@($args[3])
        Remove-Item -LiteralPath (Join-Path $script:remote $args[3])
        return
    }
    if ($args[1] -eq 'download') {
        $script:downloads++
        $destination=$args[($args.IndexOf('--dir')+1)]
        Copy-Item (Join-Path $script:remote '*') $destination
        if ($Case -eq 'download-omits-manifest') { Remove-Item -LiteralPath (Join-Path $destination 'SHA256SUMS.txt') }
        if ($Case -eq 'download-corrupt-installer') { Add-Content -LiteralPath (Join-Path $destination $script:name) 'corruption' }
        return
    }
    if ($args[1] -eq 'edit') {
        if ($script:downloads -lt 1) { throw 'Draft was published before verifying its downloaded assets' }
        $script:published=$true;$script:draft=$false
        return
    }
    throw ('Unexpected gh invocation: '+($args -join ' '))
}

$expectedFailures=@{
    mismatch='Remote release tag';extra='Unexpected release checksum inventory';'view-failed'='Could not inspect';
    'published-extra-asset'='asset inventory';'published-malformed-checksum'='Invalid checksum';
    'published-missing-checksum'='asset inventory';'published-wrong-checksum'='checksum mismatch';
    'published-extra-checksum'='checksum inventory';'published-wrong-digest'='digest mismatch';
    'published-wrong-manifest-digest'='digest mismatch';'published-wrong-size'='size mismatch';
    'published-wrong-channel'='identity/channel';'published-wrong-target'='identity/channel';
    'published-wrong-tag'='identity/channel';'published-incomplete'='asset metadata';
    'draft-wrong-channel'='identity/channel';'draft-keeps-extra'='asset inventory';
    'draft-wrong-build'='uploaded local artifact';'download-omits-manifest'='Downloaded release asset inventory';
    'download-corrupt-installer'='size mismatch|digest mismatch';'draft-wrong-manifest-digest'='digest mismatch'
}
$failure=$null
try {
    . $ScriptPath -Tag 'v0.1.0-dev.1' -Version '0.1.0-dev.1' -CommitSha $script:sha -InstallerName $script:name -Prerelease $true
} catch { $failure=$_.Exception.Message }
if ($expectedFailures.ContainsKey($Case)) {
    if (!$failure -or $failure -notmatch $expectedFailures[$Case]) { throw "Expected rejection for $Case; received: $failure" }
    if ($script:published -or $script:created) { throw 'Invalid artifacts or metadata were published' }
} elseif ($failure) { throw $failure }
if ($Case -eq 'new' -and (!$script:tagCreated -or !$script:created -or !$script:published)) { throw 'New release was not published' }
if ($Case -in @('draft','draft-leftover') -and (!$script:uploaded -or !$script:published)) { throw 'Draft was not resumed' }
if ($Case -eq 'draft-leftover' -and ('obsolete.zip' -notin $script:deleted -or (Test-Path (Join-Path $script:remote 'obsolete.zip')))) { throw 'Stale draft asset survived publication' }
if ($Case -like 'published*' -and ($script:uploaded -or $script:published -or $script:created -or $script:deleted.Count)) { throw 'Immutable release was changed' }
if ($Case -in @('published','published-rebuilt') -and $script:downloads -lt 1) { throw 'Published artifacts were not verified' }
Write-Host ('Publish PASS: '+$Case)
