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
                 'logs/run.log.1','diagnostics/run.log','diagnostics/screen.png','diagnostics/compact-ui-fixture-data/profile.json',
                 'build/output','dist/output','.env','target/debug/companion.exe',
                 'docs/superpowers/specs/new-feature.md','docs/superpowers/plans/new-feature.md',
                 'superpowers/new-plan.md','.superpowers/local-state.json',
                 '.DS_Store','packaging/._companion.ico','debug.dSYM/info'}
        sources={'README.md','run.cmd','tools/launch-rust.ps1','Cargo.lock',
                 'crates/metin-desktop/src/main.rs','packaging/icons/companion.ico','profile.example.json'}
        with tempfile.TemporaryDirectory(prefix='ignore-test-',dir=PROJECT/'tests') as folder:
            root=Path(folder).resolve()
            self.assertTrue(root.is_relative_to(PROJECT/'tests'))
            shutil.copyfile(PROJECT/'.gitignore',root/'.gitignore')
            subprocess.run(['git','init','--quiet',str(root)],check=True,capture_output=True)
            result=subprocess.run(['git','-C',str(root),'check-ignore','--no-index','--stdin','-z'],
                                  input=('\0'.join(sorted(ignored|sources))+'\0').encode(),capture_output=True,check=True)
            self.assertEqual(set(result.stdout.decode().rstrip('\0').split('\0')),ignored)
