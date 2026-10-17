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
    $views=@(
        @{name='calibration-dark';tab='calibration';theme='dark'},
        @{name='actions-dark';tab='actions';theme='dark'},
        @{name='run-dark';tab='run';theme='dark';run='looting'},
        @{name='run-light';tab='run';theme='light';run='looting'},
        @{name='event-history-dark';tab='run';theme='dark';run='history';small=$true},
        @{name='settings-dark';tab='settings';theme='dark'},
        @{name='safety-dark';tab='safety';theme='dark'},
        @{name='calibration-light';tab='calibration';theme='light'},
        @{name='actions-light';tab='actions';theme='light'},
        @{name='puller-dark';tab='actions';theme='dark';actions='puller'},
        @{name='out-of-target-dark';tab='actions';theme='dark';actions='out-of-target'},
        @{name='periodic-dark';tab='actions';theme='dark';actions='periodic'},
        @{name='visual-rules-dark';tab='actions';theme='dark';actions='visual-rules'}
    )
    foreach ($case in $views) {
        $view=$case.name
        $tab=$case.tab
        $theme=$case.theme
        $settings = Get-Content (Join-Path $data 'ui-settings.json') -Raw | ConvertFrom-Json
        $settings.theme = $theme
        [IO.File]::WriteAllText((Join-Path $data 'ui-settings.json'), ($settings | ConvertTo-Json -Depth 8))
        $screenshot = Join-Path $session ($view + '.png')
        $arguments = @('--data-dir', ('"' + $data + '"'), '--ui-fixture', ('"' + (Join-Path $data 'scene.png') + '"'), '--ui-screenshot', ('"' + $screenshot + '"'), '--ui-tab', $tab)
        if ($case.actions) { $arguments += @('--ui-actions',$case.actions) }
        if ($case.run) { $arguments += @('--ui-run-state',$case.run) }
        if ($case.small) { $arguments += '--ui-small' }
        if ($tab -eq 'calibration') { $arguments += @('--ui-window-picker','selected','--ui-sizes') }
        $app = Start-Process -FilePath (Join-Path $root 'target/debug/metinbot.exe') -ArgumentList $arguments -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $session ($view + '.stdout.log')) -RedirectStandardError (Join-Path $session ($view + '.stderr.log'))
        if (!$app.WaitForExit(45000)) { $app.Kill(); throw "Screenshot timed out: $view" }
        if ($app.ExitCode -ne 0 -or !(Test-Path $screenshot)) { throw "Screenshot failed: $view; inspect $session" }
        Copy-Item -LiteralPath $screenshot -Destination (Join-Path $output ($view + '.png'))
    }
    Write-Host "Captured $($views.Count) native application views. Logs and isolated data: $session"
} finally { Pop-Location }
