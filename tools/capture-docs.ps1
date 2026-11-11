[CmdletBinding()]
param([string[]]$OnlyViews = @())
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
        @{name='window-chooser';tab='run';window_case='all'},
        @{name='calibration';tab='calibration'},
        @{name='size-review';tab='calibration';size_review=$true},
        @{name='overlay-controls';tab='calibration';overlay_controls=$true},
        @{name='overlay-controls-off';tab='calibration';overlay_controls=$false},
        @{name='actions';tab='actions';actions='buffs'},
        @{name='action-catalog';tab='actions'},
        @{name='buff-detail';tab='actions';actions='buffs';buff_detail=$true},
        @{name='buff-icon-picker';tab='actions';actions='buffs';buff_detail=$true;buff_icon_picker=$true},
        @{name='run';tab='run';run='looting'},
        @{name='profile-menu';tab='run';run='looting';profile_menu=$true},
        @{name='run-setup';tab='run';run='uncalibrated'},
        @{name='run-break';tab='run';run='break'},
        @{name='event-history';tab='run';run='history';small=$true},
        @{name='settings';tab='settings'},
        @{name='updates';tab='settings';updates='idle'},
        @{name='update-available';tab='settings';updates='available'},
        @{name='update-download';tab='settings';updates='downloading'},
        @{name='update-ready';tab='settings';updates='ready'},
        @{name='update-error';tab='settings';updates='failed'},
        @{name='safety';tab='safety'},
        @{name='puller';tab='actions';actions='puller'},
        @{name='out-of-target';tab='actions';actions='out-of-target'},
        @{name='periodic';tab='actions';actions='periodic'},
        @{name='visual-rules';tab='actions';actions='visual-rules'}
    )
    if ($OnlyViews.Count) {
        foreach ($name in $OnlyViews) {
            if ($name -notin $views.name) { throw "Unknown documentation view: $name" }
        }
        $views = @($views | Where-Object { $_.name -in $OnlyViews })
    }
    foreach ($case in $views) {
        $view=$case.name
        $tab=$case.tab
        $screenshot = Join-Path $session ($view + '.png')
        $arguments = @('--data-dir', ('"' + $data + '"'), '--ui-fixture', ('"' + (Join-Path $data 'scene.png') + '"'), '--ui-screenshot', ('"' + $screenshot + '"'), '--ui-tab', $tab)
        if ($case.actions) { $arguments += @('--ui-actions',$case.actions) }
        if ($case.buff_detail) { $arguments += '--ui-buff-detail' }
        if ($case.buff_icon_picker) { $arguments += '--ui-buff-icon-picker' }
        if ($case.profile_menu) { $arguments += '--ui-profile-menu' }
        if ($case.size_review) { $arguments += '--ui-size-review' }
        if ($case.run) { $arguments += @('--ui-run-state',$case.run) }
        if ($case.updates) { $arguments += @('--ui-updates',$case.updates) }
        if ($case.small) { $arguments += '--ui-small' }
        if ($case.window_case) { $arguments += @('--ui-window-picker',$case.window_case) }
        if ($case.ContainsKey('overlay_controls')) { $arguments += @('--ui-overlay-controls',([string]$case.overlay_controls).ToLowerInvariant()) }
        if ($tab -eq 'calibration') { $arguments += @('--ui-window-picker','selected','--ui-sizes') }
        $app = Start-Process -FilePath (Join-Path $root 'target/debug/companion.exe') -ArgumentList $arguments -WindowStyle Hidden -PassThru -RedirectStandardOutput (Join-Path $session ($view + '.stdout.log')) -RedirectStandardError (Join-Path $session ($view + '.stderr.log'))
        if (!$app.WaitForExit(45000)) { $app.Kill(); throw "Screenshot timed out: $view" }
        if ($app.ExitCode -ne 0 -or !(Test-Path $screenshot)) { throw "Screenshot failed: $view; inspect $session" }
        Copy-Item -LiteralPath $screenshot -Destination (Join-Path $output ($view + '.png'))
    }
    Write-Host "Captured $($views.Count) native application views. Logs and isolated data: $session"
} finally { Pop-Location }
