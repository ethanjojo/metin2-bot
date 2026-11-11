"""Run the real publisher against file-backed GitHub transport fixtures."""
import hashlib
import os
from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest


@unittest.skipUnless(os.name == "nt", "Windows PowerShell publication harness")
class PublisherTests(unittest.TestCase):
    def test_remote_artifacts_are_verified_before_publication_and_on_rerun(self):
        root = Path(__file__).resolve().parents[1]
        cases = (
            "new", "draft", "draft-leftover", "published", "published-rebuilt",
            "published-extra-asset", "published-malformed-checksum",
            "published-missing-checksum", "published-wrong-checksum",
            "published-extra-checksum", "published-wrong-digest",
            "published-wrong-manifest-digest", "published-wrong-size",
            "published-wrong-channel", "published-wrong-target", "published-wrong-tag",
            "published-incomplete", "draft-wrong-channel", "draft-keeps-extra",
            "draft-wrong-build", "draft-wrong-manifest-digest",
            "download-omits-manifest", "download-corrupt-installer", "view-failed",
        )
        for case in cases:
            with self.subTest(case=case), tempfile.TemporaryDirectory(dir=root / "tests") as temporary:
                directory = Path(temporary)
                (directory / "tools").mkdir()
                (directory / "packaging").mkdir()
                shutil.copyfile(root / "packaging/app-identity.json", directory / "packaging/app-identity.json")
                script = directory / "tools/publish_release.ps1"
                shutil.copyfile(root / "tools/publish_release.ps1", script)
                release = directory / "dist/release"
                release.mkdir(parents=True)
                name = "CompanionSetup-0.1.0-dev.1-windows-x64.exe"
                (release / name).write_bytes(b"test installer")
                digest = hashlib.sha256(b"test installer").hexdigest()
                (release / "SHA256SUMS.txt").write_text(f"{digest}  {name}\n")
                environment = dict(os.environ)
                environment["PSMODULEPATH"] = str(Path(environment["SYSTEMROOT"]) / "System32/WindowsPowerShell/v1.0/Modules")
                result = subprocess.run(
                    ["powershell.exe", "-NoProfile", "-ExecutionPolicy", "Bypass", "-File",
                     str(root / "tests/verify_publish.ps1"), "-ScriptPath", str(script), "-Case", case],
                    env=environment, capture_output=True, text=True, timeout=20,
                )
                self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
                self.assertIn(f"Publish PASS: {case}", result.stdout)
