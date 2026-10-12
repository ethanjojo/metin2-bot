from pathlib import Path
import subprocess
import sys
import tempfile
import unittest

PROJECT=Path(__file__).resolve().parent.parent


@unittest.skipUnless(sys.platform=='win32','Windows launcher')
class LauncherTests(unittest.TestCase):
    def powershell(self,path,*arguments):
        return subprocess.run(['powershell.exe','-NoProfile','-ExecutionPolicy','Bypass','-File',str(path),*arguments],
                              cwd=PROJECT.parent,capture_output=True,text=True,timeout=30)

    def test_launch_and_check_use_local_environment_without_real_uac(self):
        result=self.powershell(PROJECT/'tests/verify_launcher.ps1')
        self.assertEqual(result.returncode,0,result.stdout+result.stderr)
        self.assertIn('Launcher PASS',result.stdout)

    def test_cancelled_uac_does_not_retry_or_launch(self):
        result=self.powershell(PROJECT/'tests/verify_launcher.ps1','-Cancel')
        self.assertEqual(result.returncode,2,result.stdout+result.stderr)
        self.assertIn('cancelled',result.stdout)

    def test_missing_environment_reports_setup_before_any_elevation(self):
        with tempfile.TemporaryDirectory(prefix='launcher path ',dir=PROJECT/'tests') as folder:
            tools=Path(folder)/'tools';tools.mkdir()
            launcher=tools/'launch.ps1'
            launcher.write_bytes((PROJECT/'tools/launch.ps1').read_bytes())
            result=self.powershell(launcher,'-CheckOnly')
            self.assertEqual(result.returncode,1,result.stdout+result.stderr)
            self.assertIn('environment missing',result.stdout)
