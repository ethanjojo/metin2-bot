"""Verify a Rust Windows release inventory, PE identity and dependencies."""
import argparse
import hashlib
import json
import os
from pathlib import Path
import xml.etree.ElementTree as ET


def verify_icon_resources(image, ico_path: Path) -> list[int]:
    """Require the primary Windows icon to contain the canonical ICO images."""
    ico = ico_path.read_bytes()
    if ico[:4] != b"\x00\x00\x01\x00":
        raise ValueError("Invalid canonical icon")
    expected = {}
    for index in range(int.from_bytes(ico[4:6], "little")):
        entry = ico[6 + 16 * index:22 + 16 * index]
        size, offset = int.from_bytes(entry[8:12], "little"), int.from_bytes(entry[12:16], "little")
        expected[(entry[0] or 256, entry[1] or 256)] = ico[offset:offset + size]
    if not expected:
        raise ValueError("Canonical icon has no images")
    icons, groups = {}, []
    for kind in image.DIRECTORY_ENTRY_RESOURCE.entries:
        if kind.id not in (3, 14):
            continue
        for resource in kind.directory.entries:
            blobs = [image.get_data(language.data.struct.OffsetToData, language.data.struct.Size)
                     for language in resource.directory.entries]
            if kind.id == 3:
                icons[resource.id] = blobs
            elif kind.id == 14:
                groups.append(blobs)
    if not groups or not groups[0]:
        raise ValueError("Primary Windows icon is missing")
    for group in groups[0]:
        if group[:4] != b"\x00\x00\x01\x00" or int.from_bytes(group[4:6], "little") != len(expected):
            raise ValueError("Invalid primary icon group")
        actual = {}
        for index in range(int.from_bytes(group[4:6], "little")):
            entry = group[6 + 14 * index:20 + 14 * index]
            if len(entry) != 14:
                raise ValueError("Invalid primary icon directory")
            key = (entry[0] or 256, entry[1] or 256)
            identifier = int.from_bytes(entry[12:14], "little")
            actual[key] = icons.get(identifier, [])
        if actual.keys() != expected.keys() or any(
                not actual[key] or any(blob != expected[key] for blob in actual[key]) for key in expected):
            raise ValueError("Primary Windows icon differs from canonical Companion artwork")
    return sorted(width for width, _ in expected)


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


def verify_input_bridge(image) -> None:
    """Require the complete native ABI and reject a managed-runtime shim."""
    exports = {entry.name for entry in getattr(image, "DIRECTORY_ENTRY_EXPORT", ()).symbols} if hasattr(image, "DIRECTORY_ENTRY_EXPORT") else set()
    required = {b"mi_abi_version", b"mi_manage", b"mi_open", b"mi_keyboard", b"mi_mouse", b"mi_close"}
    if not required.issubset(exports):
        raise ValueError("Input bridge is missing required C ABI exports")
    if image.OPTIONAL_HEADER.DATA_DIRECTORY[14].VirtualAddress:
        raise ValueError("Input bridge requires an installed .NET runtime")
    for entry in (*getattr(image, "DIRECTORY_ENTRY_IMPORT", []), *getattr(image, "DIRECTORY_ENTRY_DELAY_IMPORT", [])):
        if entry.dll.decode("ascii").lower() in {"mscoree.dll", "coreclr.dll", "hostfxr.dll", "hostpolicy.dll"}:
            raise ValueError("Input bridge depends on an installed .NET runtime")


def verify_bundle(directory: Path, version: str, commit: str) -> dict:
    import pefile
    verify_inventory(directory)
    info = json.loads((directory / "build-info.json").read_text(encoding="utf-8"))
    if (info["version"], info["commit_sha"], info["runtime"], info["working_tree_dirty"]) != (version, commit.lower(), "rust", False):
        raise ValueError("Bundled release identity mismatch or dirty build")
    for name in ("MetinInput.dll", "opencv_world4130.dll", "msvcp140.dll", "vcruntime140.dll", "vcruntime140_1.dll", "concrt140.dll"):
        if not (directory / name).is_file():
            raise ValueError(f"Missing native runtime: {name}")
    if not (directory / "licenses/HIDMaestro.txt").is_file():
        raise ValueError("Missing HIDMaestro license")
    for name in ("profiles", "ui-settings.json", "config.json", "config.queue-backup.json", "_internal", "MetinBot.exe"):
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
            dependency.parse_data_directories(directories=[pefile.DIRECTORY_ENTRY[name] for name in ("IMAGE_DIRECTORY_ENTRY_IMPORT", "IMAGE_DIRECTORY_ENTRY_DELAY_IMPORT", "IMAGE_DIRECTORY_ENTRY_EXPORT")])
            if path.name.lower() == "metininput.dll":
                verify_input_bridge(dependency)
            for entry in (*getattr(dependency, "DIRECTORY_ENTRY_IMPORT", []), *getattr(dependency, "DIRECTORY_ENTRY_DELAY_IMPORT", [])):
                name = entry.dll.decode("ascii").lower()
                if name in bundled or name.startswith(("api-ms-", "ext-ms-")):
                    continue
                if name.startswith(("msvcp", "vcruntime", "concrt", "opencv")) or not (system / name).is_file():
                    raise ValueError(f"Unresolved native import: {path.name} -> {name}")
    with pefile.PE(str(directory / "Companion.exe")) as image:
        verify_icon_resources(image, Path(__file__).resolve().parents[1] / "packaging/icons/companion.ico")
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
