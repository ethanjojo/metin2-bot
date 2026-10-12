"""Check the pinned Windows x64 build environment before freezing the app."""

from importlib.metadata import version
from pathlib import Path
import platform
import struct
import sys


def main():
    if sys.platform != "win32" or struct.calcsize("P") != 8 or platform.machine().upper() not in {"AMD64", "X86_64"}:
        raise RuntimeError("Windows x64 Python required")
    if sys.version_info[:2] != (3, 14):
        raise RuntimeError("Python 3.14 required")
    root = Path(__file__).resolve().parents[1]
    for requirements_file in (root / "requirements.txt", root / "requirements-build.txt"):
        for line in requirements_file.read_text(encoding="utf-8").splitlines():
            if not line.strip() or line.startswith("#"):
                continue
            name, required_version = line.split("==")
            installed_version = version(name)
            if installed_version != required_version:
                raise RuntimeError(f"{name} must be {required_version}; found {installed_version}")


if __name__ == "__main__":
    main()
