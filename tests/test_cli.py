import json
from pathlib import Path
import subprocess
import tempfile
import unittest
from unittest.mock import patch
import sys
from metin_bot.cli import main
from metin_bot.config import Profile
from metin_bot.profiles import ProfileStore


ROOT=Path(__file__).resolve().parent.parent


class CLITests(unittest.TestCase):
    def test_explicit_directory_is_passed_to_single_app_path_and_default_log(self):
        with tempfile.TemporaryDirectory(dir=ROOT/'tests') as folder:
            directory=Path(folder)/'data'
            with patch('metin_bot.app.run') as run,patch('metin_bot.cli.logging.basicConfig') as config:
                self.assertEqual(main(['--data-dir',str(directory)]),0)
            run.assert_called_once_with(data_directory=directory.resolve())
            configured=next(call.kwargs for call in config.call_args_list if 'filename' in call.kwargs)
            self.assertEqual(configured['filename'],directory.resolve()/'logs'/'run.log')

    def test_self_test_is_a_real_isolated_source_process(self):
        with tempfile.TemporaryDirectory(dir=ROOT/'tests') as folder:
            report=Path(folder)/'result.json'
            process=subprocess.run([sys.executable,'-m','metin_bot','--self-test','--self-test-report',str(report)],
                                   cwd=ROOT,capture_output=True,text=True,timeout=30)
            self.assertEqual(process.returncode,0,process.stdout+process.stderr)
            data=json.loads(report.read_text())
            self.assertTrue(data['ok']);self.assertIn('profile-preference-roundtrip',data['checks'])
            self.assertEqual(list(Path(folder).iterdir()),[report])

    def test_self_test_cannot_target_real_data(self):
        with patch('metin_bot.self_test.run_self_test') as check:
            with self.assertRaises(SystemExit) as failure:
                main(['--self-test','--self-test-report','example.json','--data-dir','profiles'])
            self.assertEqual(failure.exception.code,2);check.assert_not_called()

    def test_first_installed_launch_can_import_source_profiles(self):
        with tempfile.TemporaryDirectory(dir=ROOT/'tests') as folder:
            old=Path(folder)/'old';destination=Path(folder)/'new'
            profile=ProfileStore(old/'profiles').save(Profile.create('Existing server'))
            with patch('metin_bot.cli.sys.frozen',True,create=True),patch('metin_bot.cli.build_info',return_value={'version':'0.1.0','commit_sha':'test'}),\
                 patch('metin_bot.data_paths.default_data_directory',return_value=destination),\
                 patch('metin_bot.data_paths.legacy_data_directory',return_value=Path(folder)/'installation'),\
                 patch('metin_bot.data_import_ui.choose_legacy_directory',return_value=old) as choose,\
                 patch('metin_bot.app.run'),patch('metin_bot.cli.logging.basicConfig'):
                self.assertEqual(main([]),0)
            choose.assert_called_once()
            self.assertEqual(ProfileStore(destination/'profiles').load(profile.id)[0],profile)
            self.assertTrue((old/'profiles'/profile.id/'profile.json').exists())

    def test_explicit_import_refuses_existing_data(self):
        with tempfile.TemporaryDirectory(dir=ROOT/'tests') as folder:
            old=Path(folder)/'old';destination=Path(folder)/'new'
            ProfileStore(old/'profiles').save(Profile.create('Old'))
            original=ProfileStore(destination/'profiles').save(Profile.create('Current'))
            with patch('metin_bot.app.run') as run,patch('tkinter.messagebox.showerror'):
                self.assertEqual(main(['--data-dir',str(destination),'--import-data',str(old)]),1)
            run.assert_not_called()
            self.assertEqual(ProfileStore(destination/'profiles').load(original.id)[0],original)
