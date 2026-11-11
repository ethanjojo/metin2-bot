"""Read-only verification of Windows EXE branding and embedded icon sizes."""
import argparse
import json
from pathlib import Path
import xml.etree.ElementTree as ET

import pefile


def verify(path: Path, require_admin: bool) -> dict:
    root = Path(__file__).resolve().parents[1]
    identity = json.loads((root / "packaging/app-identity.json").read_text(encoding="utf-8"))
    with pefile.PE(str(path)) as image:
        strings = {}
        for group in image.FileInfo:
            for item in group:
                for table in getattr(item, "StringTable", []):
                    # Inno Setup pads its fixed-capacity version strings.
                    strings.update({k.decode("utf-8"): v.decode("utf-8").rstrip(" \0") for k, v in table.entries.items()})
        expected = {"CompanyName": identity["publisher"], "ProductName": identity["product_name"], "LegalCopyright": identity["copyright"]}
        if require_admin:
            expected.update({"InternalName": identity["internal_name"], "OriginalFilename": identity["original_filename"]})
        for key, value in expected.items():
            if strings.get(key) != value:
                raise ValueError(f"{path.name}: {key} does not match identity")
        sizes = set()
        levels = set()
        for kind in image.DIRECTORY_ENTRY_RESOURCE.entries:
            for resource in kind.directory.entries:
                for language in resource.directory.entries:
                    value = language.data.struct
                    blob = image.get_data(value.OffsetToData, value.Size)
                    if kind.id == 14:
                        count = int.from_bytes(blob[4:6], "little")
                        for i in range(count):
                            sizes.add(blob[6 + 14 * i] or 256)
                    elif kind.id == 24:
                        manifest = ET.fromstring(blob)
                        levels.update(node.attrib["level"] for node in manifest.iter() if node.tag.endswith("requestedExecutionLevel"))
        if not {16, 32, 48, 256}.issubset(sizes):
            raise ValueError(f"{path.name}: missing icon resolutions: {sizes}")
        if require_admin and "requireAdministrator" not in levels:
            raise ValueError(f"{path.name}: missing administrator manifest")
        return {"file": str(path), "metadata": strings, "icon_sizes": sorted(sizes), "execution_levels": sorted(levels)}


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("files", nargs="+", type=Path)
    parser.add_argument("--require-admin", action="store_true")
    args = parser.parse_args()
    print(json.dumps([verify(path, args.require_admin) for path in args.files], indent=2))
