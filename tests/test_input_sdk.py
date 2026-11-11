import os
from pathlib import Path
import subprocess
import unittest

ROOT = Path(__file__).resolve().parents[1]

class InputSdkTests(unittest.TestCase):
    def test_virtual_device_branding_retains_generic_identity_and_virtual_labels(self):
        source = (ROOT / 'native/input-bridge/Exports.cs').read_text()
        self.assertIn('.Id("metinbot-" + type)', source)
        self.assertIn('.Name("Companion virtual " + type)', source)
        self.assertIn('.ProductString("Companion virtual " + type)', source)
        self.assertIn('.ManufacturerString("Companion")', source)
        self.assertIn('.Vid(0xBEEF).Pid(keyboard ? (ushort)0x6761 : (ushort)0x6762)', source)
        self.assertIn('HIDMaestro 2.1.0 input driver', source)

    @unittest.skipUnless(os.name == 'nt', 'Windows PowerShell source adapter harness')
    def test_exact_source_adapter_rejects_drift_and_duplicate_gate(self):
        result = subprocess.run(['powershell.exe', '-NoProfile', '-ExecutionPolicy', 'Bypass', '-File', str(ROOT / 'tests/verify_input_sdk.ps1'), '-Root', str(ROOT)], capture_output=True, text=True, timeout=30)
        self.assertEqual(result.returncode, 0, result.stdout + result.stderr)
        self.assertIn('SDK source patch PASS', result.stdout)
