"""Verify a Rust Windows release inventory, PE identity and dependencies."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import xml.etree.ElementTree as ET


def verify_inventory(directory: Path) -> None:
    directory = directory.resolve()
    if any(path.is_symlink() for path in directory.rglob("*")):
        raise ValueError("Symbolic links are not allowed in release bundles")
    entries = json.loads((directory / "files.json").read_text(encoding="utf-8"))
    expected = set()
    for entry in entries:
        relative = Path(entry["path"])
        path = (directory / relative).resolve()
        if relative.is_absolute() or not path.is_relative_to(directory) or path == directory:
            raise ValueError("Inventory path escapes bundle")
        if path in expected or path.name == "files.json":
            raise ValueError("Invalid or duplicate inventory entry")
        expected.add(path)
        if not path.is_file() or hashlib.sha256(path.read_bytes()).hexdigest() != entry["sha256"]:
            raise ValueError(f"Inventory checksum mismatch: {relative}")
    actual = {p.resolve() for p in directory.rglob("*") if p.is_file() and p != directory / "files.json"}
    if actual != expected:
        raise ValueError("Bundle contains missing or unlisted files")


def verify_bundle(directory: Path, version: str, commit: str) -> dict:
    import pefile
    verify_inventory(directory)
    info = json.loads((directory / "build-info.json").read_text(encoding="utf-8"))
    if (info["version"], info["commit_sha"], info["runtime"], info["working_tree_dirty"]) != (version, commit.lower(), "rust", False):
        raise ValueError("Bundled release identity mismatch or dirty build")
    for name in ("opencv_world4130.dll", "msvcp140.dll", "vcruntime140.dll", "vcruntime140_1.dll", "concrt140.dll"):
        if not (directory / name).is_file():
            raise ValueError(f"Missing native runtime: {name}")
    for name in ("profiles", "ui-settings.json", "config.json", "config.queue-backup.json", "_internal"):
        if (directory / name).exists():
            raise ValueError("User data or obsolete runtime files included in bundle")
    bundled = {path.name.lower(): path for path in directory.iterdir() if path.is_file()}
    system = Path(os.environ["SystemRoot"]) / "System32"
    for path in directory.iterdir():
        if path.suffix.lower() not in (".exe", ".dll"):
            continue
        with pefile.PE(str(path), fast_load=True) as dependency:
            if dependency.FILE_HEADER.Machine != 0x8664:
                raise ValueError(f"Non-x64 native file: {path.name}")
            dependency.parse_data_directories(directories=[pefile.DIRECTORY_ENTRY["IMAGE_DIRECTORY_ENTRY_IMPORT"]])
            for entry in getattr(dependency, "DIRECTORY_ENTRY_IMPORT", []):
                name = entry.dll.decode("ascii").lower()
                if name in bundled or name.startswith(("api-ms-", "ext-ms-")):
                    continue
                if name.startswith(("msvcp", "vcruntime", "concrt", "opencv")) or not (system / name).is_file():
                    raise ValueError(f"Unresolved native import: {path.name} -> {name}")
    with pefile.PE(str(directory / "MetinBot.exe")) as image:
        if image.FILE_HEADER.Machine != 0x8664:
            raise ValueError("Executable must be Windows x64")
        strings = {}
        for group in image.FileInfo:
            for item in group:
                for table in getattr(item, "StringTable", []):
                    strings.update({k.decode(): v.decode().rstrip(" \0") for k, v in table.entries.items()})
        if strings.get("ProductVersion") != version or strings.get("Comments") != f"Commit {commit.lower()}" or strings.get("PrivateBuild") != "clean":
            raise ValueError("PE identity differs from release manifest")
        if strings.get("FileVersion") != info["windows_version"]:
            raise ValueError("PE numeric version differs from release manifest")
        levels = []
        for kind in image.DIRECTORY_ENTRY_RESOURCE.entries:
            if kind.id != 24:
                continue
            for resource in kind.directory.entries:
                for language in resource.directory.entries:
                    value = language.data.struct
                    manifest = ET.fromstring(image.get_data(value.OffsetToData, value.Size))
                    levels.extend(n.attrib.get("level") for n in manifest.iter() if n.tag.endswith("requestedExecutionLevel"))
        if levels != ["requireAdministrator"]:
            raise ValueError("Missing native administrator manifest")
    return info


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--directory", type=Path, required=True)
    parser.add_argument("--version", required=True)
    parser.add_argument("--commit", required=True)
    args = parser.parse_args()
    print(json.dumps(verify_bundle(args.directory, args.version, args.commit), indent=2))
