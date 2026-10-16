[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
Set-StrictMode -Version Latest
$repoRoot = Split-Path -Parent $PSScriptRoot
Push-Location $repoRoot
try {
    . ./tools/rust-env.ps1
    $out = Join-Path $repoRoot 'build/rust-migration'
    $results = @()
    foreach ($runtime in @('python', 'rust')) {
        $start = [Diagnostics.ProcessStartInfo]::new()
        $start.WorkingDirectory = $repoRoot
        $start.UseShellExecute = $false
        $start.CreateNoWindow = $true
        $start.RedirectStandardOutput = $true
        $start.RedirectStandardError = $true
        if ($runtime -eq 'python') {
            $start.FileName = Join-Path $repoRoot '.venv/Scripts/python.exe'
            $start.ArgumentList.Add('tools/benchmark-python-reference.py')
        } else { $start.FileName = Join-Path $repoRoot 'target/release/examples/benchmark.exe' }
        $process = [Diagnostics.Process]::Start($start)
        $stdout = $process.StandardOutput.ReadToEndAsync()
        $stderr = $process.StandardError.ReadToEndAsync()
        $process.WaitForExit()
        $text = $stdout.GetAwaiter().GetResult()
        $errors = $stderr.GetAwaiter().GetResult()
        if ($process.ExitCode -ne 0) { throw "${runtime} benchmark failed: $errors" }
        $report = $text | ConvertFrom-Json
        [IO.File]::WriteAllText((Join-Path $out "benchmark-$runtime.json"), $text, [Text.UTF8Encoding]::new($false))
        $results += [ordered]@{ runtime = $runtime; peak_working_set_bytes = $report.peak_working_set_bytes; cases = $report.cases }
        $process.Dispose()
    }
    [IO.File]::WriteAllText((Join-Path $out 'benchmark-comparison.json'), (ConvertTo-Json -InputObject $results -Depth 8), [Text.UTF8Encoding]::new($false))
    $results | ForEach-Object { Write-Output "$($_.runtime): peak working set $($_.peak_working_set_bytes) bytes" }
} finally { Pop-Location }
