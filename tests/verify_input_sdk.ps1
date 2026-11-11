param([Parameter(Mandatory)][string]$Root)
$ErrorActionPreference = 'Stop'
Import-Module (Join-Path $Root 'tools/input-sdk-source.psm1') -Force
$adapter = Join-Path $Root 'native/hidmaestro-sdk'
$original = [IO.File]::ReadAllText((Join-Path $adapter 'EnsureForCreate.original.txt')).Replace("`r`n", "`n")
$replacement = [IO.File]::ReadAllText((Join-Path $adapter 'EnsureForCreate.replacement.txt')).Replace("`r`n", "`n")
$source = "preamble`n" + $original + "`nepilogue"
$actual = ConvertTo-ExplicitDriverSource -Source $source -AdapterDirectory $adapter
if ($actual -cne ("preamble`n" + $replacement + "`nepilogue")) { throw 'Source patch changed unrelated code.' }
foreach ($invalid in @($source.Replace('Machine.Lock()', 'Machine.OtherLock()'), ($source + $original), $replacement)) {
    try {
        ConvertTo-ExplicitDriverSource -Source $invalid -AdapterDirectory $adapter | Out-Null
        throw 'Unsafe gate patch was accepted.'
    } catch {
        if ($_.Exception.Message -notmatch 'Pinned HIDMaestro EnsureForCreate source changed') { throw }
    }
}
Write-Host 'SDK source patch PASS: exact replacement, unrelated code retained, changed/duplicate/already-patched source rejected.'
