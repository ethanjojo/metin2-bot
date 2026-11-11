"""Validate release identity shared by Rust packaging and publication."""

import argparse
from dataclasses import dataclass
import json
from pathlib import Path
import re


SEMVER = re.compile(
    r"(0|[1-9]\d*)\.(0|[1-9]\d*)\.(0|[1-9]\d*)"
    r"(?:-([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?"
    r"(?:\+([0-9A-Za-z-]+(?:\.[0-9A-Za-z-]+)*))?", re.ASCII
)


@dataclass(frozen=True)
class BuildMetadata:
    version: str
    build_number: int
    commit_sha: str
    windows_version: tuple[int, int, int, int]

    @classmethod
    def parse(cls, version: str, build_number: int, commit_sha: str):
        match = SEMVER.fullmatch(version)
        if not match or len(version) > 100:
            raise ValueError("Version must be a SemVer identifier, at most 100 characters")
        prerelease = match[4]
        if prerelease and any(p.isdigit() and len(p) > 1 and p.startswith("0") for p in prerelease.split(".")):
            raise ValueError("Numeric prerelease identifiers cannot have leading zeroes")
        core = tuple(int(match[i]) for i in range(1, 4))
        if type(build_number) is not int or any(n < 0 or n > 65535 for n in (*core, build_number)):
            raise ValueError("Windows version components and BuildNumber must be in 0..65535")
        if not re.fullmatch(r"[0-9a-fA-F]{7,40}", commit_sha):
            raise ValueError("CommitSha must contain 7..40 hexadecimal characters")
        return cls(version, build_number, commit_sha.lower(), (*core, build_number))

    @property
    def numeric_version(self):
        return ".".join(map(str, self.windows_version))

    @property
    def artifact_name(self):
        identity = json.loads((Path(__file__).resolve().parents[1] / "packaging/app-identity.json").read_text(encoding="utf-8"))
        return f"{identity['product_name']}Setup-{self.version}-windows-x64.exe"

    @property
    def prerelease(self):
        return "-" in self.version.split("+", 1)[0]

    def write(self, output_dir: Path):
        output_dir.mkdir(parents=True, exist_ok=True)
        metadata = {
            "version": self.version,
            "windows_version": self.numeric_version,
            "build_number": self.build_number,
            "commit_sha": self.commit_sha,
            "artifact_name": self.artifact_name,
            "runtime": "rust",
            "prerelease": self.prerelease,
            "working_tree_dirty": False,
        }
        (output_dir / "build-info.json").write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
        return metadata


def main():
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument("--version", required=True)
    parser.add_argument("--build-number", type=int, required=True)
    parser.add_argument("--commit-sha", required=True)
    parser.add_argument("--output-dir", type=Path, required=True)
    args = parser.parse_args()
    try:
        metadata = BuildMetadata.parse(args.version, args.build_number, args.commit_sha)
    except ValueError as error:
        parser.error(str(error))
    print(json.dumps(metadata.write(args.output_dir)))


if __name__ == "__main__":
    main()
