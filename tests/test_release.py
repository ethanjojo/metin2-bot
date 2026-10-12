import unittest
from pathlib import Path
import hashlib
import os
import shutil
import subprocess
import tempfile
from tools.release_metadata import release_metadata


class ReleaseTests(unittest.TestCase):
    sha='1234567890abcdef1234567890abcdef12345678'

    def test_main_identity_is_unique_and_prerelease(self):
        data=release_metadata('1.2.3','refs/heads/main',42,self.sha)
        self.assertEqual(data['version'],'1.2.3-dev.42.1234567')
        self.assertEqual(data['tag'],'v'+data['version'])
        self.assertEqual(data['prerelease'],'true')
        self.assertNotEqual(data['version'],release_metadata('1.2.3','refs/heads/main',43,self.sha)['version'])

    def test_stable_tag_matches_single_version_source(self):
        data=release_metadata('1.2.3','refs/tags/v1.2.3',42,self.sha)
        self.assertEqual(data['version'],'1.2.3');self.assertEqual(data['prerelease'],'false')
        with self.assertRaises(ValueError):release_metadata('1.2.3','refs/tags/v1.2.4',42,self.sha)
        with self.assertRaises(ValueError):release_metadata('1.2.3','refs/tags/v1.2.3-dev.4',42,self.sha)

    def test_pr_build_has_separate_identity_and_invalid_values_rejected(self):
        self.assertIn('-test.',release_metadata('1.2.3','refs/pull/1/merge',42,self.sha)['version'])
        for base,run,sha in (('bad',1,self.sha),('1.2.3',65536,self.sha),('1.2.3',1,'bad\nvalue')):
            with self.assertRaises(ValueError):release_metadata(base,'refs/heads/main',run,sha)

    def test_publication_checks_remote_tags_and_resumes_only_safe_drafts(self):
        root=Path(__file__).resolve().parent.parent
        with tempfile.TemporaryDirectory(dir=root/'tests') as folder:
            directory=Path(folder);(directory/'tools').mkdir()
            script=directory/'tools'/'publish_release.ps1'
            shutil.copyfile(root/'tools/publish_release.ps1',script)
            release=directory/'dist/release';release.mkdir(parents=True)
            name='MetinBotSetup-0.1.0-dev.1-windows-x64.exe'
            (release/name).write_bytes(b'test installer')
            digest=hashlib.sha256(b'test installer').hexdigest()
            (release/'SHA256SUMS.txt').write_text(f'{digest}  {name}\n')
            for case in ('new','draft','published','mismatch'):
                with self.subTest(case=case):
                    environment=dict(os.environ)
                    environment['PSMODULEPATH']=str(Path(environment['SYSTEMROOT'])/'System32/WindowsPowerShell/v1.0/Modules')
                    result=subprocess.run(['powershell.exe','-NoProfile','-ExecutionPolicy','Bypass','-File',str(root/'tests/verify_publish.ps1'),'-ScriptPath',str(script),'-Case',case],env=environment,capture_output=True,text=True,timeout=20)
                    self.assertEqual(result.returncode,0,result.stdout+result.stderr)
                    self.assertIn('Publish PASS',result.stdout)
