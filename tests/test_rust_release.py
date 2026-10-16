import hashlib
import json
from pathlib import Path
import tempfile
import unittest
import os
import shutil
import subprocess
import sys

from tools.build_metadata import BuildMetadata
from tools.verify_bundle import verify_inventory
from tools.package_notices import copy_cargo_notices

ROOT = Path(__file__).resolve().parents[1]


class RustReleaseTests(unittest.TestCase):
    def setUp(self):
        (ROOT / "build/release-tests").mkdir(parents=True, exist_ok=True)
        self.temp = tempfile.TemporaryDirectory(dir=ROOT / "build/release-tests")
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)

    def test_release_manifest_has_rust_identity_and_no_python_resource(self):
        result = BuildMetadata.parse("0.1.0-rc.1", 1, "a" * 40).write(self.root)
        self.assertEqual(result["runtime"], "rust")
        self.assertEqual(result["windows_version"], "0.1.0.1")
        self.assertEqual(json.loads((self.root / "build-info.json").read_text()), result)
        self.assertFalse((self.root / "version-info.txt").exists())

    def test_inventory_detects_tampering_extra_files_and_path_escape(self):
        data = self.root / "MetinBot.exe"
        data.write_bytes(b"test executable")
        entries = [{"path": data.name, "sha256": hashlib.sha256(data.read_bytes()).hexdigest()}]
        inventory = self.root / "files.json"
        inventory.write_text(json.dumps(entries))
        verify_inventory(self.root)
        data.write_bytes(b"tampered")
        with self.assertRaises(ValueError):
            verify_inventory(self.root)
        data.write_bytes(b"test executable")
        (self.root / "unexpected.dll").write_bytes(b"extra")
        with self.assertRaises(ValueError):
            verify_inventory(self.root)
        entries[0]["path"] = "../outside"
        inventory.write_text(json.dumps(entries))
        with self.assertRaises(ValueError):
            verify_inventory(self.root)

    def test_release_workflow_builds_rust_and_blocks_stable_until_acceptance(self):
        workflow = (ROOT / ".github/workflows/release.yml").read_text()
        self.assertIn("package-rust-windows.ps1", workflow)
        self.assertNotIn("-m metin_bot", workflow)
        self.assertIn("release-policy.json", workflow)

    def test_dependency_notices_include_nested_font_licenses_and_inventory(self):
        source = self.root / "crate"
        (source / "fonts").mkdir(parents=True)
        (source / "Cargo.toml").write_text("")
        (source / "LICENSE-MIT").write_text("crate license")
        (source / "fonts/OFL.txt").write_text("font license")
        package = {"name": "epaint_default_fonts", "version": "1", "source": "registry", "license": "MIT", "manifest_path": str(source / "Cargo.toml")}
        destination = self.root / "licenses"
        copy_cargo_notices({"packages": [package]}, destination)
        self.assertEqual((destination / "rust/epaint_default_fonts-1/fonts/OFL.txt").read_text(), "font license")
        self.assertEqual(json.loads((destination / "rust-dependencies.json").read_text())[0]["license"], "MIT")

    def test_notice_file_cannot_escape_dependency_root(self):
        source = self.root / "crate"
        source.mkdir()
        with self.assertRaises(ValueError):
            copy_cargo_notices({"packages": [{"name": "bad", "version": "1", "source": "registry", "manifest_path": str(source / "Cargo.toml"), "license_file": "../secret"}]}, self.root / "licenses")

    @unittest.skipUnless(os.name == "nt", "PowerShell packaging guard harness")
    def test_actual_packager_rejects_dirty_tree_and_stable_build_metadata_bypass(self):
        (self.root / "tools").mkdir()
        (self.root / "packaging").mkdir()
        for name in ("tools/package-rust-windows.ps1", "tools/build_metadata.py", "packaging/app-identity.json", "packaging/release-policy.json", "VERSION"):
            shutil.copyfile(ROOT / name, self.root / name)
        for case in ("dirty", "stable"):
            result = subprocess.run(["powershell.exe", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File", str(ROOT / "tests/verify_release_guards.ps1"), "-ScriptPath", str(self.root / "tools/package-rust-windows.ps1"), "-OutputPath", str(self.root / case), "-PythonPath", sys.executable, "-Case", case], capture_output=True, text=True, timeout=30)
            self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
            self.assertIn("Release guard PASS", result.stdout)


class ReleaseVersionTests(unittest.TestCase):
    def test_stable_and_prerelease_keep_same_windows_core(self):
        for version in ("1.2.3", "1.2.3-dev.42+ABC1234"):
            with self.subTest(version=version):
                metadata = BuildMetadata.parse(version, 42, "ABC1234")
                self.assertEqual(metadata.numeric_version, "1.2.3.42")
                self.assertEqual(metadata.commit_sha, "abc1234")
                self.assertEqual(metadata.artifact_name, f"MetinBotSetup-{version}-windows-x64.exe")
        self.assertFalse(BuildMetadata.parse("1.2.3+build-test", 1, "a" * 40).prerelease)
        self.assertTrue(BuildMetadata.parse("1.2.3-rc.1+build-test", 1, "a" * 40).prerelease)

    def test_rejects_invalid_or_injectable_release_identifiers(self):
        for version in ("", "v1.2.3", "01.2.3", "1.2.3-dev.01", "65536.1.1", "1.2.3\n", "1.2.3/evil", "1.2.3\"", "1.2.3;evil", "1.2.3-", "1.2.3+", "1.2.3-" + "a" * 100):
            with self.subTest(version=version), self.assertRaises(ValueError):
                BuildMetadata.parse(version, 0, "123abcd")
        for build_number in (-1, 65536, True, "1"):
            with self.subTest(build_number=build_number), self.assertRaises(ValueError):
                BuildMetadata.parse("1.2.3", build_number, "123abcd")
        for sha in ("", "123456", "123456g", "a" * 41, "123abcd\n", "123abcd'", "123abcd/evil"):
            with self.subTest(sha=sha), self.assertRaises(ValueError):
                BuildMetadata.parse("1.2.3", 0, sha)

    def test_boundary_values_are_supported(self):
        self.assertEqual(BuildMetadata.parse("65535.65535.65535-0", 65535, "a" * 40).numeric_version, "65535.65535.65535.65535")



if __name__ == "__main__":
    unittest.main()
