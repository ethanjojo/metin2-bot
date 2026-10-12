import ast
import importlib.util
import json
import os
from pathlib import Path
import runpy
import sys
import tempfile
from types import SimpleNamespace
import unittest
from unittest.mock import patch


ROOT = Path(__file__).resolve().parents[1]
spec = importlib.util.spec_from_file_location("metinbot_build_metadata", ROOT / "tools" / "build_metadata.py")
metadata_module = importlib.util.module_from_spec(spec)
sys.modules[spec.name] = metadata_module
spec.loader.exec_module(metadata_module)
BuildMetadata = metadata_module.BuildMetadata


class BuildMetadataTests(unittest.TestCase):
    def test_stable_and_prerelease_keep_same_windows_core(self):
        for version in ("1.2.3", "1.2.3-dev.42+ABC1234"):
            with self.subTest(version=version):
                metadata = BuildMetadata.parse(version, 42, "ABC1234")
                self.assertEqual(metadata.numeric_version, "1.2.3.42")
                self.assertEqual(metadata.commit_sha, "abc1234")
                self.assertEqual(metadata.artifact_name, f"MetinBotSetup-{version}-windows-x64.exe")

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

    def test_resource_and_manifest_preserve_version_and_commit(self):
        with tempfile.TemporaryDirectory(dir=Path(__file__).parent) as directory:
            output = Path(directory)
            metadata = BuildMetadata.parse("0.1.0-dev.21+abcdef0", 21, "abcdef0")
            manifest = metadata.write(output)
            self.assertEqual(json.loads((output / "build-info.json").read_text()), manifest)
            resource = ast.parse((output / "version-info.txt").read_text())
            fixed_info = resource.body[0].value.keywords[0].value
            self.assertEqual(ast.literal_eval(fixed_info.keywords[0].value), (0, 1, 0, 21))
            text = (output / "version-info.txt").read_text()
            self.assertIn("'ProductVersion', '0.1.0-dev.21+abcdef0'", text)
            self.assertIn("'Commit abcdef0'", text)

    def test_spec_bundles_only_entrypoint_and_build_metadata(self):
        calls = {}
        def analysis(*args, **kwargs):
            calls["analysis"] = (args, kwargs)
            return SimpleNamespace(pure=[], scripts=[], binaries=[], datas=kwargs["datas"])
        def exe(*args, **kwargs):
            calls["exe"] = (args, kwargs)
            return "exe"
        def collect(*args, **kwargs):
            calls["collect"] = (args, kwargs)
        with patch.dict(os.environ, {"METINBOT_BUILD_METADATA_DIR": str(ROOT / "build" / "test-metadata")}):
            runpy.run_path(str(ROOT / "packaging" / "MetinBot.spec"), init_globals={
                "SPECPATH": str(ROOT / "packaging"), "Analysis": analysis,
                "PYZ": lambda pure: "pyz", "EXE": exe, "COLLECT": collect,
            })
        self.assertEqual(calls["analysis"][0][0], [str(ROOT / "packaging_entry.py")])
        self.assertEqual(len(calls["analysis"][1]["datas"]), 1)
        options = calls["exe"][1]
        self.assertFalse(options["console"])
        self.assertTrue(options["uac_admin"])
        self.assertTrue(options["exclude_binaries"])
        self.assertFalse(options["upx"])
        self.assertEqual(calls["collect"][1]["name"], "MetinBot")


if __name__ == "__main__":
    unittest.main()
