param([switch]$CheckOnly)

$ErrorActionPreference = 'Stop'
$projectRoot = Split-Path -Parent $PSScriptRoot
$pythonConsole = Join-Path $projectRoot '.venv\Scripts\python.exe'
$pythonGui = Join-Path $projectRoot '.venv\Scripts\pythonw.exe'

try {
    if (!(Test-Path -LiteralPath $pythonConsole -PathType Leaf) -or
        !(Test-Path -LiteralPath $pythonGui -PathType Leaf)) {
        throw 'Python environment missing. Follow the installation steps in README.md first.'
    }
    & $pythonConsole -c "import sys, tkinter, numpy, cv2, PIL; print('Python ' + sys.version.split()[0] + ' - dependencies OK')"
    if ($LASTEXITCODE -ne 0) {
        throw 'Dependencies could not be loaded. Install requirements.txt using the project virtual environment.'
    }
    if ($CheckOnly) { return }
    # This is the visible application, not a background helper. Pythonw avoids a console.
    Start-Process -FilePath $pythonGui -ArgumentList '-m metin_bot' `
        -WorkingDirectory $projectRoot -Verb RunAs -ErrorAction Stop
}
catch {
    $failure = $_.Exception
    while ($failure.InnerException) { $failure = $failure.InnerException }
    if ($failure -is [System.ComponentModel.Win32Exception] -and $failure.NativeErrorCode -eq 1223) {
        Write-Host 'Administrator permission was cancelled. The bot was not started.'
        exit 2
    }
    Write-Host ('Could not start Metin Bot: ' + $failure.Message)
    exit 1
}
