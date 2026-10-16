# Development-only native dependencies. Runtime packages must carry their own DLLs.
$nativeRoot = Join-Path $PSScriptRoot '../build/rust-native/windows/opencv/build'
if (-not (Test-Path (Join-Path $nativeRoot 'include/opencv2/core.hpp'))) { throw 'Native OpenCV is missing; see docs/development.md.' }
$env:OPENCV_INCLUDE_PATHS = (Resolve-Path (Join-Path $nativeRoot 'include')).Path
$env:OPENCV_LINK_PATHS = (Resolve-Path (Join-Path $nativeRoot 'x64/vc16/lib')).Path
$env:OPENCV_LINK_LIBS = 'opencv_world4130'
$env:LIBCLANG_PATH = (Resolve-Path (Join-Path $PSScriptRoot '../build/rust-native/windows/llvm20/bin')).Path
$env:PATH = (Resolve-Path (Join-Path $nativeRoot 'x64/vc16/bin')).Path + ';' + $env:PATH
$clangBin = Join-Path $PSScriptRoot '../build/rust-native/windows/llvm20/bin'
if (Test-Path (Join-Path $clangBin 'clang.exe')) { $env:PATH = (Resolve-Path $clangBin).Path + ';' + $env:PATH }
