param([string]$ScriptPath,[string]$Case)
$ErrorActionPreference='Stop'
$script:created=$false
$script:tagCreated=$false
$script:uploaded=$false
$script:published=$false
$script:sha='1234567890abcdef1234567890abcdef12345678'
$env:GH_REPO='test/repo'
function gh {
    if ($args[0] -eq 'api') {
        if ($args -contains 'POST') { $script:tagCreated=$true;$global:LASTEXITCODE=0;return }
        if ($Case -eq 'mismatch') { $global:LASTEXITCODE=0;return ('f'*40) }
        if (!$script:tagCreated -and $Case -eq 'new') {
            $global:LASTEXITCODE=1
            if ($args[1] -match '/commits/') { return 'No commit found for SHA (HTTP 422)' }
            return 'Not found (HTTP 404)'
        }
        $global:LASTEXITCODE=0;return $script:sha
    }
    if ($args[1] -eq 'view') {
        if ($Case -eq 'new') { $global:LASTEXITCODE=1;return }
        $global:LASTEXITCODE=0
        return @{isDraft=($Case -ne 'published');targetCommitish=$script:sha} | ConvertTo-Json -Compress
    }
    $global:LASTEXITCODE=0
    if ($args[1] -eq 'create') { $script:created=$true;return }
    if ($args[1] -eq 'upload') { $script:uploaded=$true;return }
    if ($args[1] -eq 'download') {
        $destination = $args[($args.IndexOf('--dir') + 1)]
        $source = Join-Path (Split-Path (Split-Path $ScriptPath -Parent) -Parent) 'dist/release'
        Copy-Item (Join-Path $source '*') $destination
        return
    }
    if ($args[1] -eq 'edit') { $script:published=$true;return }
    throw 'Unexpected gh invocation'
}
try {
    . $ScriptPath -Tag 'v0.1.0-dev.1' -Version '0.1.0-dev.1' -CommitSha $script:sha -InstallerName 'MetinBotSetup-0.1.0-dev.1-windows-x64.exe' -Prerelease $true
    if ($Case -eq 'mismatch') { throw 'Mismatched remote tag was accepted' }
} catch {
    if ($Case -ne 'mismatch' -or $_.Exception.Message -notmatch 'Remote release tag') { throw }
}
if ($Case -eq 'new' -and (!$script:tagCreated -or !$script:created -or !$script:published)) { throw 'New release was not published' }
if ($Case -eq 'draft' -and (!$script:uploaded -or !$script:published)) { throw 'Draft was not resumed' }
if ($Case -in @('published','mismatch') -and ($script:uploaded -or $script:published -or $script:created)) { throw 'Immutable or conflicting release was changed' }
Write-Host ('Publish PASS: '+$Case)
