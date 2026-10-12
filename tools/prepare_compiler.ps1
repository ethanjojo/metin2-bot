[CmdletBinding()]
param()
$ErrorActionPreference = 'Stop'
$root = Split-Path -Parent $PSScriptRoot
$toolRoot = Join-Path $root 'build\tools'
$compilerDirectory = Join-Path $toolRoot 'inno'
$compiler = Join-Path $compilerDirectory 'ISCC.exe'
function Test-Compiler {
    & $compiler /Q (Join-Path $root 'packaging\check_compiler.iss')
    if ($LASTEXITCODE -ne 0) { throw 'Inno Setup 6.7.3 compiler check failed.' }
}
if (Test-Path -LiteralPath $compiler -PathType Leaf) {
    Test-Compiler
    Write-Output $compiler
    return
}
New-Item -ItemType Directory -Force -Path $toolRoot | Out-Null
$download = Join-Path $toolRoot 'innosetup-6.7.3.exe'
$digest = '9c73c3bae7ed48d44112a0f48e66742c00090bdb5bef71d9d3c056c66e97b732'
if (!(Test-Path -LiteralPath $download -PathType Leaf)) {
    Invoke-WebRequest -Uri 'https://github.com/jrsoftware/issrc/releases/download/is-6_7_3/innosetup-6.7.3.exe' -OutFile $download
}
if ((Get-FileHash -LiteralPath $download -Algorithm SHA256).Hash.ToLowerInvariant() -ne $digest) { throw 'Compiler download checksum mismatch.' }
$signature = Get-AuthenticodeSignature -LiteralPath $download
if ($signature.Status -ne 'Valid' -or $signature.SignerCertificate.Subject -notmatch 'O=Pyrsys B.V.') { throw 'Compiler publisher signature verification failed.' }
# Official portable installation remains inside the ignored build directory.
$arguments = '/PORTABLE=1 /VERYSILENT /SUPPRESSMSGBOXES /NORESTART /CURRENTUSER /DIR="' + $compilerDirectory + '"'
$process = Start-Process -FilePath $download -ArgumentList $arguments -WindowStyle Hidden -Wait -PassThru
if ($process.ExitCode -ne 0 -or !(Test-Path -LiteralPath $compiler -PathType Leaf)) { throw 'Portable compiler setup failed.' }
Test-Compiler
Write-Output $compiler
