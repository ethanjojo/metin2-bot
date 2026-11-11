# Offline-testable hosted acceptance checks. No driver operations here.
function Assert-NoManagedDriverArtifacts($Preview) {
    foreach ($field in @('schema_version', 'managed_artifact_count', 'running_program_count')) {
        if ($Preview.$field -isnot [int] -and $Preview.$field -isnot [long]) { throw "Driver inventory field must be an integer: $field" }
    }
    if ($Preview.preview -isnot [bool] -or $Preview.presence_known -isnot [bool] -or $Preview.entries -isnot [array]) { throw 'Driver inventory has invalid field types.' }
    if ($Preview.schema_version -ne 1 -or $Preview.preview -ne $true -or
        $Preview.status -cne 'Previewed' -or $Preview.presence_known -ne $true -or
        $null -eq $Preview.managed_artifact_count -or $Preview.managed_artifact_count -ne 0 -or
        $Preview.running_program_count -ne 0) {
        throw 'Fresh authoritative driver preview must prove zero managed packages and certificates.'
    }
    foreach ($entry in $Preview.entries) {
        if ($entry.kind -cne 'service setting' -or $entry.outcome -cne 'Kept') {
            throw 'Driver preview contains managed or inaccessible artifacts.'
        }
    }
}

function Add-LegacyPackageFixture([string]$Directory) {
    $existingBackups = @(Get-ChildItem -LiteralPath (Split-Path -Parent $Directory) -Directory -Filter 'Companion-install-backup-*' | ForEach-Object FullName)
    $owned = @()
    $inventory = @(Get-Content -LiteralPath (Join-Path $Directory 'files.json') -Raw | ConvertFrom-Json)
    foreach ($fixture in @(
        @{path='MetinBot.exe';source='Companion.exe'},
        @{path='retired-input.dll';source='MetinInput.dll'},
        @{path='licenses/retired-ci/NOTICE.txt';source=$null}
    )) {
        $path = Join-Path $Directory $fixture.path
        New-Item -ItemType Directory -Force -Path (Split-Path -Parent $path) | Out-Null
        if ($fixture.source) { Copy-Item -LiteralPath (Join-Path $Directory $fixture.source) -Destination $path }
        else { Set-Content -LiteralPath $path 'verified retired CI package notice' -Encoding utf8NoBOM }
        $entry = @{path=$fixture.path;sha256=(Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant()}
        $owned += $entry
        $inventory += $entry
    }
    $inventory | ConvertTo-Json -Depth 4 | Set-Content -LiteralPath (Join-Path $Directory 'files.json') -Encoding utf8NoBOM
    return [pscustomobject]@{files=$owned;existing_backups=$existingBackups}
}

function Assert-LegacyPackageRetired([string]$Directory, $Fixture) {
    foreach ($name in @('MetinBot.exe','retired-input.dll','licenses/retired-ci/NOTICE.txt','.companion-upgrade-pending.json')) {
        if (Test-Path -LiteralPath (Join-Path $Directory $name)) { throw "Retired package file remains active: $name" }
    }
    $backups = @(Get-ChildItem -LiteralPath (Split-Path -Parent $Directory) -Directory -Filter 'Companion-install-backup-*' | Where-Object { $_.FullName -notin $Fixture.existing_backups })
    if ($backups.Count -ne 1) { throw 'Exactly one new recovery archive must retain this upgrade.' }
    foreach ($file in $Fixture.files) {
        $path = Join-Path $backups[0].FullName $file.path
        if (!(Test-Path -LiteralPath $path) -or (Get-FileHash -LiteralPath $path -Algorithm SHA256).Hash.ToLowerInvariant() -ne $file.sha256) {
            throw "Recovery archive did not retain the original retired package file: $($file.path)"
        }
    }
    if (!(Test-Path -LiteralPath (Join-Path $backups[0].FullName 'files.json'))) { throw 'Original package inventory missing from recovery archive.' }
}
