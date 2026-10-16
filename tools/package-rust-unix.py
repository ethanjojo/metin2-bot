"""Stage native Linux/macOS distributions and dependency notices."""
import argparse
import json
import platform
import plistlib
import shutil
import subprocess
from pathlib import Path
import sys
sys.path.insert(0, str(Path(__file__).resolve().parent))
from package_notices import copy_cargo_notices

ROOT = Path(__file__).resolve().parents[1]
IDENTITY = json.loads((ROOT / "packaging/app-identity.json").read_text(encoding="utf-8"))
APP_ID = IDENTITY["app_id"]


def copy(source, target, executable=False):
    target.parent.mkdir(parents=True, exist_ok=True)
    shutil.copyfile(source, target)
    target.chmod(0o755 if executable else 0o644)


def write(target, text, executable=False):
    target.parent.mkdir(parents=True, exist_ok=True)
    target.write_text(text, encoding="utf-8", newline="\n")
    target.chmod(0o755 if executable else 0o644)


def stage_linux(binary, output):
    copy(binary, output / "lib/metinbot/metinbot", executable=True)
    write(output / "bin/metinbot", '''#!/bin/sh
set -eu
prefix=$(CDPATH= cd -- "$(dirname -- "$0")/.." && pwd)
export LD_LIBRARY_PATH="$prefix/lib/metinbot${LD_LIBRARY_PATH:+:$LD_LIBRARY_PATH}"
exec "$prefix/lib/metinbot/metinbot" "$@"
''', executable=True)
    write(output / f"share/applications/{APP_ID}.desktop", f'''[Desktop Entry]
Type=Application
Name={IDENTITY['product_name']}
Comment={IDENTITY['description']}
Exec=metinbot
TryExec=metinbot
Icon={APP_ID}
StartupWMClass={APP_ID}
Terminal=false
Categories=Utility;
''')
    for size in (16, 32, 48, 64, 128, 256, 512):
        copy(ROOT / f"packaging/icons/metinbot-{size}.png",
             output / f"share/icons/hicolor/{size}x{size}/apps/{APP_ID}.png")
    write(output / "install.sh", '''#!/bin/sh
set -eu
prefix=${1:-"$HOME/.local"}
case "$prefix" in /*) ;; *) echo "Use an absolute installation prefix" >&2; exit 1;; esac
source_dir=$(CDPATH= cd -- "$(dirname -- "$0")" && pwd)
mkdir -p "$prefix/bin" "$prefix/lib/metinbot" "$prefix/share"
cp -R "$source_dir/lib/metinbot/." "$prefix/lib/metinbot/"
cp "$source_dir/bin/metinbot" "$prefix/bin/metinbot"
cp -R "$source_dir/share/." "$prefix/share/"
if command -v update-desktop-database >/dev/null 2>&1; then
    update-desktop-database "$prefix/share/applications"
fi
if command -v gtk-update-icon-cache >/dev/null 2>&1; then
    gtk-update-icon-cache -f -t "$prefix/share/icons/hicolor"
fi
printf 'Installed in %s. Ensure %s/bin is in PATH.\\n' "$prefix" "$prefix"
''', executable=True)


def stage_macos(binary, output):
    copy(binary, output / "Contents/MacOS/metinbot", executable=True)
    copy(ROOT / "packaging/icons/metinbot.icns", output / "Contents/Resources/metinbot.icns")
    version = (ROOT / "VERSION").read_text().strip()
    info = {
        "CFBundleIdentifier": APP_ID,
        "CFBundleName": IDENTITY["product_name"],
        "CFBundleDisplayName": IDENTITY["product_name"],
        "CFBundleExecutable": "metinbot",
        "CFBundleIconFile": "metinbot.icns",
        "CFBundlePackageType": "APPL",
        "CFBundleShortVersionString": version.split("-")[0],
        "CFBundleVersion": version.split("-")[0],
        "NSHighResolutionCapable": True,
        "LSMinimumSystemVersion": "11.0",
        "NSHumanReadableCopyright": IDENTITY["copyright"],
    }
    (output / "Contents/Info.plist").write_bytes(plistlib.dumps(info))


def bundle_macos_libraries(binary, output, native_lib):
    """Resolve non-system dylibs recursively, then relocate every load command."""
    frameworks = output / "Contents/Frameworks"
    frameworks.mkdir()
    executable = output / "Contents/MacOS/metinbot"
    queue = [(binary.resolve(), executable)]
    copied = {}
    while queue:
        original, staged = queue.pop(0)
        listing = subprocess.check_output(["otool", "-L", str(original)], text=True)
        for line in listing.splitlines()[1:]:
            dependency = line.strip().split(" (", 1)[0]
            if dependency.startswith(("/System/Library/", "/usr/lib/")):
                continue
            if dependency.startswith("@loader_path/"):
                resolved = original.parent / dependency.removeprefix("@loader_path/")
            elif dependency.startswith("@executable_path/"):
                resolved = binary.resolve().parent / dependency.removeprefix("@executable_path/")
            elif dependency.startswith("@rpath/"):
                # OpenCV's configured native prefix is explicit; fail for unresolved dependencies.
                resolved = native_lib / dependency.removeprefix("@rpath/")
            else:
                resolved = Path(dependency)
            resolved = resolved.resolve(strict=True)
            if resolved == original:
                continue  # dylib's own install name
            destination = frameworks / resolved.name
            if destination.name in copied and copied[destination.name] != resolved:
                raise ValueError(f"Conflicting dylib name: {destination.name}")
            if destination.name not in copied:
                copied[destination.name] = resolved
                copy(resolved, destination, executable=True)
                subprocess.run(["install_name_tool", "-id", f"@rpath/{destination.name}", str(destination)], check=True)
                queue.append((resolved, destination))
            relative = f"@loader_path/{destination.name}" if staged != executable else f"@executable_path/../Frameworks/{destination.name}"
            subprocess.run(["install_name_tool", "-change", dependency, relative, str(staged)], check=True)
    # Relocation invalidates existing signatures. Ad-hoc signatures enable local Apple Silicon execution;
    # this is not Developer ID signing or notarization.
    for path in sorted(frameworks.iterdir()):
        subprocess.run(["codesign", "--force", "--sign", "-", str(path)], check=True)
    subprocess.run(["codesign", "--force", "--sign", "-", str(output)], check=True)


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--binary", type=Path, required=True)
    parser.add_argument("--output", type=Path, required=True)
    parser.add_argument("--native-lib", type=Path, required=True)
    args = parser.parse_args()
    if platform.system() not in ("Linux", "Darwin"):
        parser.error("Run packaging on the native Linux/macOS build host")
    if args.output.exists():
        parser.error("Output already exists; use a new output directory")
    if not args.binary.is_file() or not args.native_lib.is_dir():
        parser.error("Binary and native library directory must exist")
    if platform.system() == "Darwin":
        if args.output.suffix != ".app":
            parser.error("macOS output must end with .app")
        stage_macos(args.binary, args.output)
    else:
        libraries = list(args.native_lib.glob("*.so*"))
        if not libraries:
            parser.error("No native shared libraries found")
        stage_linux(args.binary, args.output)
        for path in libraries:
            copy(path, args.output / "lib/metinbot" / path.name, executable=True)
    notices = args.output / ("Contents/Resources/licenses" if platform.system() == "Darwin" else "share/doc/metinbot/licenses")
    rust_version = subprocess.check_output(["rustc", "-vV"], text=True)
    target = next(line.removeprefix("host: ") for line in rust_version.splitlines() if line.startswith("host: "))
    metadata = json.loads(subprocess.check_output(["cargo", "metadata", "--format-version", "1", "--locked", "--offline", "--filter-platform", target], cwd=ROOT, text=True))
    copy_cargo_notices(metadata, notices)
    opencv_notices = args.native_lib.parent / "share/licenses"
    if not opencv_notices.is_dir():
        raise ValueError("Installed OpenCV license directory is missing")
    shutil.copytree(opencv_notices, notices / "OpenCV")
    if platform.system() == "Darwin":
        bundle_macos_libraries(args.binary, args.output, args.native_lib.resolve())


if __name__ == "__main__":
    main()
