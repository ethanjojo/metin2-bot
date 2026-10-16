[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
Push-Location $root
try {
    . ./tools/rust-env.ps1
    cargo build -p metin-desktop --all-targets --features ui-smoke --locked -j 2
    if ($LASTEXITCODE -ne 0) { throw 'Documentation build failed.' }
    $session = Join-Path $root ('build/docs-capture/' + [Guid]::NewGuid().ToString())
    $data = Join-Path $session 'data'
    cargo run -p metin-desktop --example docs_fixture --locked -- $data
    if ($LASTEXITCODE -ne 0) { throw 'Demonstration profile creation failed.' }
    $output = Join-Path $root 'docs/assets'
    New-Item -ItemType Directory -Force $output | Out-Null
    foreach ($view in @('calibration-dark','buffs-dark','run-dark','settings-dark','calibration-light','buffs-light')) {
        $tab, $theme = $view.Split('-')
        $settings = Get-Content (Join-Path $data 'ui-settings.json') -Raw | ConvertFrom-Json
        $settings.theme = $theme
        [IO.File]::WriteAllText((Join-Path $data 'ui-settings.json'), ($settings | ConvertTo-Json -Depth 8))
        $screenshot = Join-Path $output ($view + '.png')
        $arguments = @('--data-dir', ('"' + $data + '"'), '--ui-fixture', ('"' + (Join-Path $data 'scene.png') + '"'), '--ui-screenshot', ('"' + $screenshot + '"'), '--ui-tab', $tab)
        if ($tab -eq 'calibration') { $arguments += @('--ui-window-picker','selected','--ui-sizes') }
        $app = Start-Process -FilePath (Join-Path $root 'target/debug/metinbot.exe') -ArgumentList $arguments -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $session ($view + '.stdout.log')) -RedirectStandardError (Join-Path $session ($view + '.stderr.log'))
        if (!$app.WaitForExit(45000)) { $app.Kill(); throw "Screenshot timed out: $view" }
        if ($app.ExitCode -ne 0 -or !(Test-Path $screenshot)) { throw "Screenshot failed: $view; inspect $session" }
    }
    Write-Host "Captured six native application views. Logs and isolated data: $session"
} finally { Pop-Location }
