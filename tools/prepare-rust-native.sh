#!/usr/bin/env bash
set -euo pipefail
# Native Linux/macOS build dependency; no global installation or Python runtime.
native_root="${1:-build/rust-native}"
commit=fe38fc608f6acb8b68953438a62305d8318f4fcd
mkdir -p "$native_root"
native_root="$(cd "$native_root" && pwd)"
if [[ ! -d "$native_root/opencv-source/.git" ]]; then
  git init "$native_root/opencv-source"
  git -C "$native_root/opencv-source" remote add origin https://github.com/opencv/opencv.git
  git -C "$native_root/opencv-source" fetch --depth 1 origin "$commit"
  git -C "$native_root/opencv-source" checkout --detach FETCH_HEAD
fi
[[ "$(git -C "$native_root/opencv-source" rev-parse HEAD)" == "$commit" ]]
cmake -S "$native_root/opencv-source" -B "$native_root/opencv-build" \
  -DCMAKE_BUILD_TYPE=Release -DCMAKE_INSTALL_PREFIX="$native_root/opencv" \
  -DBUILD_LIST=core,imgproc,imgcodecs -DBUILD_SHARED_LIBS=ON \
  -DBUILD_TESTS=OFF -DBUILD_PERF_TESTS=OFF -DBUILD_EXAMPLES=OFF \
  -DBUILD_opencv_python2=OFF -DBUILD_opencv_python3=OFF -DBUILD_JAVA=OFF \
  -DWITH_FFMPEG=OFF -DWITH_GSTREAMER=OFF -DWITH_QT=OFF -DWITH_GTK=OFF \
  -DWITH_OPENEXR=OFF -DWITH_JASPER=OFF -DWITH_TIFF=OFF -DWITH_WEBP=OFF \
  -DOPENCV_GENERATE_PKGCONFIG=ON
cmake --build "$native_root/opencv-build" --parallel 4
cmake --install "$native_root/opencv-build"
printf 'Native OpenCV prefix: %s\n' "$native_root/opencv"
