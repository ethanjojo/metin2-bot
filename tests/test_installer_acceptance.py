"""Offline hosted acceptance guards; never installs or manages a driver."""
import json
from pathlib import Path
import subprocess
import hashlib
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]

class InstallerAcceptanceTests(unittest.TestCase):
    def guard(self, preview):
        command = ". '" + str(ROOT / 'tools/installer_acceptance.ps1') + "'; $value = '" + json.dumps(preview).replace("'", "''") + "' | ConvertFrom-Json; Assert-NoManagedDriverArtifacts $value"
        return subprocess.run(['pwsh', '-NoProfile', '-Command', command], capture_output=True, text=True)

    def test_zero_authoritative_inventory_is_required(self):
        valid = dict(schema_version=1, preview=True, status='Previewed', presence_known=True, managed_artifact_count=0, running_program_count=0, entries=[])
        self.assertEqual(self.guard(valid).returncode, 0)
        for changed in [dict(managed_artifact_count=1), dict(presence_known=False), dict(status='Failed'), dict(preview=False), dict(running_program_count=1), dict(managed_artifact_count=None), dict(preview="true"), dict(presence_known="true"), dict(managed_artifact_count="0"), dict(entries=[dict(kind='certificate', outcome='Kept', location='root', reason='unreadable')])]:
            with self.subTest(changed=changed):
                self.assertNotEqual(self.guard(valid | changed).returncode, 0)

    def test_retained_gameinput_setting_does_not_count_as_managed_artifact(self):
        preview = dict(schema_version=1, preview=True, status='Previewed', presence_known=True, managed_artifact_count=0, running_program_count=0, entries=[dict(kind='service setting', outcome='Kept', location='GameInputSvc', reason='retained')])
        self.assertEqual(self.guard(preview).returncode, 0)

    def test_legacy_fixture_produces_real_inventory_and_active_payload_is_rejected(self):
        with tempfile.TemporaryDirectory() as folder:
            root = Path(folder)
            install = root / 'Custom install'
            install.mkdir()
            (install / 'Companion.exe').write_bytes(b'application fixture')
            (install / 'MetinInput.dll').write_bytes(b'native fixture')
            (install / 'files.json').write_text('[]')
            helper = str(ROOT / 'tools/installer_acceptance.ps1').replace("'", "''")
            directory = str(install).replace("'", "''")
            command = f". '{helper}'; $fixture = Add-LegacyPackageFixture '{directory}'; Assert-LegacyPackageRetired '{directory}' $fixture"
            result = subprocess.run(['pwsh', '-NoProfile', '-Command', command], capture_output=True, text=True)
            self.assertNotEqual(result.returncode, 0)
            inventory = json.loads((install / 'files.json').read_text(encoding='utf8'))
            self.assertEqual([item['path'] for item in inventory], ['MetinBot.exe', 'retired-input.dll', 'licenses/retired-ci/NOTICE.txt'])
            for item in inventory:
                self.assertEqual(item['sha256'], hashlib.sha256((install / item['path']).read_bytes()).hexdigest())

if __name__ == '__main__': unittest.main()
