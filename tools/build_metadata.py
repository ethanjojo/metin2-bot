"""Validate release identifiers and generate the executable version resource."""

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
        return f"MetinBotSetup-{self.version}-windows-x64.exe"

    def write(self, output_dir: Path):
        output_dir.mkdir(parents=True, exist_ok=True)
        metadata = {
            "version": self.version,
            "windows_version": self.numeric_version,
            "build_number": self.build_number,
            "commit_sha": self.commit_sha,
            "artifact_name": self.artifact_name,
        }
        (output_dir / "build-info.json").write_text(json.dumps(metadata, indent=2) + "\n", encoding="utf-8")
        resource = f'''VSVersionInfo(
  ffi=FixedFileInfo(filevers={self.windows_version!r}, prodvers={self.windows_version!r},
    mask=0x3f, flags=0x0, OS=0x40004, fileType=0x1, subtype=0x0, date=(0, 0)),
  kids=[StringFileInfo([StringTable('040904B0', [
    StringStruct('CompanyName', 'MetinBot'),
    StringStruct('FileDescription', 'MetinBot'),
    StringStruct('FileVersion', '{self.numeric_version}'),
    StringStruct('ProductName', 'MetinBot'),
    StringStruct('ProductVersion', '{self.version}'),
    StringStruct('OriginalFilename', 'MetinBot.exe'),
    StringStruct('Comments', 'Commit {self.commit_sha}')
  ])]), VarFileInfo([VarStruct('Translation', [1033, 1200])])]
)
'''
        (output_dir / "version-info.txt").write_text(resource, encoding="utf-8")
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
