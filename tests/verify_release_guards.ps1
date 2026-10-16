param([string]$ScriptPath,[string]$OutputPath,[string]$PythonPath,[string]$Case)
$ErrorActionPreference = 'Stop'
function git {
    $global:LASTEXITCODE = 0
    if ($args[0] -eq 'rev-parse') { return ('a' * 40) }
    if ($args[0] -eq 'status' -and $Case -eq 'dirty') { return ' M source.rs' }
}
try {
    . $ScriptPath -CrtDirectory 'unused' -CompilerPath 'unused' -Version '0.1.0+build-test' -OutputDirectory $OutputPath -PythonPath $PythonPath
    throw 'Release guard accepted unsafe build'
} catch {
    $expected = if ($Case -eq 'dirty') { 'clean committed tree' } else { 'Stable release acceptance is incomplete' }
    if ($_.Exception.Message -notmatch $expected) { throw }
}
Write-Host ('Release guard PASS: ' + $Case)
