from pathlib import Path
import shutil
import subprocess
import tempfile
import unittest

PROJECT=Path(__file__).resolve().parent.parent


@unittest.skipUnless(shutil.which('git'),'Git is needed to verify ignore rules')
class GitIgnoreTests(unittest.TestCase):
    def test_private_runtime_data_is_ignored_and_sources_remain_visible(self):
        ignored={'.venv/Scripts/pythonw.exe','profiles/example/profile.json','ui-settings.json',
                 'profiles/example/template-uuid.png','profiles/example/profile.json.tmp',
                 'profiles/selection.json','profiles/selection.json.tmp','ui-settings.json.tmp',
                 'config.json','config.json.tmp','config.queue-backup.json','config.queue-backup.json.tmp',
                 'diagnostics/run.log','diagnostics/screen.png','diagnostics/compact-ui-fixture-data/profile.json',
                 'build/output','dist/output','.env','target/debug/metinbot.exe',
                 '.DS_Store','packaging/._metinbot.icns','debug.dSYM/info'}
        sources={'README.md','run.cmd','tools/launch-rust.ps1','Cargo.lock',
                 'crates/metin-desktop/src/main.rs','packaging/icons/metinbot.icns','profile.example.json'}
        with tempfile.TemporaryDirectory(prefix='ignore-test-',dir=PROJECT/'tests') as folder:
            root=Path(folder).resolve()
            self.assertTrue(root.is_relative_to(PROJECT/'tests'))
            shutil.copyfile(PROJECT/'.gitignore',root/'.gitignore')
            subprocess.run(['git','init','--quiet',str(root)],check=True,capture_output=True)
            result=subprocess.run(['git','-C',str(root),'check-ignore','--no-index','--stdin','-z'],
                                  input=('\0'.join(sorted(ignored|sources))+'\0').encode(),capture_output=True,check=True)
            self.assertEqual(set(result.stdout.decode().rstrip('\0').split('\0')),ignored)
