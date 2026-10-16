"""Copy dependency license files from Cargo metadata; shared by all packagers."""
import argparse
import json
from pathlib import Path
import shutil


def copy_cargo_notices(metadata: dict, destination: Path) -> None:
    destination.mkdir(parents=True, exist_ok=True)
    inventory = []
    for package in metadata["packages"]:
        if not package.get("source"):
            continue
        root = Path(package["manifest_path"]).resolve().parent
        files = {p for p in root.iterdir() if p.is_file() and p.name.upper().startswith(("LICENSE", "LICENCE", "COPYING", "NOTICE"))}
        if package.get("license_file"):
            declared = (root / package["license_file"]).resolve()
            if not declared.is_relative_to(root):
                raise ValueError("Dependency license file escapes crate")
            files.add(declared)
        if package["name"] == "epaint_default_fonts":
            files.update((root / "fonts").glob("*.txt"))
        for source in files:
            target = destination / "rust" / f"{package['name']}-{package['version']}" / source.relative_to(root)
            target.parent.mkdir(parents=True, exist_ok=True)
            shutil.copyfile(source, target)
        inventory.append({key: package.get(key) for key in ("name", "version", "license", "source")})
    (destination / "rust-dependencies.json").write_text(json.dumps(inventory, indent=2) + "\n", encoding="utf-8")


if __name__ == "__main__":
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--metadata", type=Path, required=True)
    parser.add_argument("--destination", type=Path, required=True)
    args = parser.parse_args()
    copy_cargo_notices(json.loads(args.metadata.read_text(encoding="utf-8-sig")), args.destination)
