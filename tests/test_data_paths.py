import json
from pathlib import Path
import shutil
import tempfile
import unittest
from unittest.mock import patch

from metin_bot.config import Profile
from metin_bot.data_paths import (default_data_directory, legacy_data_directory,
                                  prepare_data_directory)
from metin_bot.profiles import ProfileStore
from metin_bot.theme import UISettings


class DataPathsTests(unittest.TestCase):
    def setUp(self):
        self.temporary = tempfile.TemporaryDirectory(prefix='data-paths-', dir=Path(__file__).parent)
        self.addCleanup(self.temporary.cleanup)
        self.root = Path(self.temporary.name)
        self.source = self.root / 'old'
        self.source.mkdir()
        self.destination = self.root / 'local' / 'MetinBot'

    def populate(self):
        store = ProfileStore(self.source / 'profiles')
        profile = store.save(Profile.create('Original'))
        store.select(profile.id)
        UISettings('dark', last_mounted=True).save(self.source / 'ui-settings.json')
        (self.source / 'config.queue-backup.json').write_text('{"process_name":"old.exe"}', encoding='utf-8')
        return profile

    def snapshot(self, directory):
        return {p.relative_to(directory).as_posix(): p.read_bytes()
                for p in directory.rglob('*') if p.is_file()}

    def test_default_is_per_user_and_missing_or_relative_environment_is_explicit(self):
        with patch.dict('os.environ', {'LOCALAPPDATA': str(self.root)}):
            self.assertEqual(default_data_directory(), self.root / 'MetinBot')
        with patch.dict('os.environ', {'LOCALAPPDATA': ''}):
            with self.assertRaisesRegex(RuntimeError, 'LOCALAPPDATA'):
                default_data_directory()
        with patch.dict('os.environ', {'LOCALAPPDATA': 'relative'}):
            with self.assertRaisesRegex(RuntimeError, 'absolute'):
                default_data_directory()

    def test_source_checkout_and_frozen_executable_selection(self):
        import metin_bot.data_paths as module
        with patch.object(module.sys, 'frozen', False, create=True):
            self.assertEqual(legacy_data_directory(), Path(module.__file__).resolve().parent.parent)
        executable = self.root / 'installed' / 'MetinBot.exe'
        with patch.object(module.sys, 'frozen', True, create=True), \
             patch.object(module.sys, 'executable', str(executable)), \
             patch.object(module.sys, '_MEIPASS', str(self.root / 'internal'), create=True):
            self.assertEqual(legacy_data_directory(), executable.parent)

    def test_injected_directory_never_discovers_real_source(self):
        with patch('metin_bot.data_paths.legacy_data_directory') as discover:
            self.assertEqual(prepare_data_directory(self.destination), self.destination)
            discover.assert_not_called()
        self.assertEqual(list(self.destination.iterdir()), [])

    def test_default_migrates_using_discovered_paths(self):
        self.populate()
        with patch('metin_bot.data_paths.default_data_directory', return_value=self.destination), \
             patch('metin_bot.data_paths.legacy_data_directory', return_value=self.source):
            self.assertEqual(prepare_data_directory(), self.destination)
        self.assertEqual(self.snapshot(self.source), self.snapshot(self.destination))

    def test_atomic_copy_preserves_original_and_repeat_never_overwrites_or_merges(self):
        self.populate()
        before = self.snapshot(self.source)
        prepare_data_directory(self.destination, legacy_directory=self.source)
        self.assertEqual(self.snapshot(self.source), before)
        self.assertEqual(self.snapshot(self.destination), before)
        store = ProfileStore(self.destination / 'profiles')
        updated = Profile.create('New user data')
        store.save(updated)
        store.select(updated.id)
        UISettings('light').save(self.destination / 'ui-settings.json')
        expected = self.snapshot(self.destination)
        ProfileStore(self.source / 'profiles').save(Profile.create('Changed legacy data'))
        prepare_data_directory(self.destination, legacy_directory=self.source)
        self.assertEqual(self.snapshot(self.destination), expected)
        self.assertEqual(store.selected(), updated.id)

    def test_partial_copy_failure_is_recoverable_and_does_not_publish_partial_data(self):
        self.populate()
        before = self.snapshot(self.source)
        real_copy = shutil.copy2
        def fail_after_copy(source, target, **kwargs):
            real_copy(source, target, **kwargs)
            raise OSError('Simulated disk failure')
        with patch('metin_bot.data_paths.shutil.copy2', side_effect=fail_after_copy):
            with self.assertRaisesRegex(OSError, 'disk failure'):
                prepare_data_directory(self.destination, legacy_directory=self.source)
        self.assertFalse(self.destination.exists())
        self.assertEqual(self.snapshot(self.source), before)
        self.assertEqual(list(self.destination.parent.glob('.MetinBot-migration-*')), [])
        prepare_data_directory(self.destination, legacy_directory=self.source)
        self.assertEqual(self.snapshot(self.destination), before)

    def test_empty_destination_and_stale_stage_allow_fresh_retry(self):
        self.populate()
        self.destination.mkdir(parents=True)
        stale = self.destination.parent / '.MetinBot-migration-interrupted'
        stale.mkdir()
        (stale / 'partial').write_text('Never import partial data', encoding='utf-8')
        prepare_data_directory(self.destination, legacy_directory=self.source)
        self.assertEqual(self.snapshot(self.source), self.snapshot(self.destination))
        self.assertTrue((stale / 'partial').exists())

    def test_corrupt_existing_destination_is_reported_without_changes(self):
        self.populate()
        self.destination.mkdir(parents=True)
        settings = self.destination / 'ui-settings.json'
        settings.write_text('{broken', encoding='utf-8')
        with self.assertRaises(ValueError):
            prepare_data_directory(self.destination, legacy_directory=self.source)
        self.assertEqual(settings.read_text(encoding='utf-8'), '{broken')
        self.assertFalse((self.destination / 'profiles').exists())

    def test_incomplete_profile_destination_and_file_conflict_are_reported(self):
        (self.destination / 'profiles' / 'unfinished').mkdir(parents=True)
        with self.assertRaisesRegex(ValueError, 'Incomplete profile'):
            prepare_data_directory(self.destination, legacy_directory=self.source)
        conflict = self.root / 'not-a-directory'
        conflict.write_text('preserve', encoding='utf-8')
        with self.assertRaisesRegex(ValueError, 'must be a directory'):
            prepare_data_directory(conflict, legacy_directory=self.source)
        self.assertEqual(conflict.read_text(encoding='utf-8'), 'preserve')

    def test_invalid_source_never_publishes_destination(self):
        (self.source / 'config.json').write_text('[]', encoding='utf-8')
        with self.assertRaisesRegex(ValueError, 'JSON object'):
            prepare_data_directory(self.destination, legacy_directory=self.source)
        self.assertFalse(self.destination.exists())

    def test_legacy_backup_conflict_is_reported_without_publishing_or_overwriting(self):
        self.populate()
        (self.source / 'config.json').write_text('{}', encoding='utf-8')
        before = self.snapshot(self.source)
        with self.assertRaisesRegex(ValueError, 'Both legacy configuration'):
            prepare_data_directory(self.destination, legacy_directory=self.source)
        self.assertFalse(self.destination.exists())
        self.assertEqual(self.snapshot(self.source), before)

    def test_copied_legacy_config_conversion_only_changes_destination(self):
        legacy = self.source / 'config.json'
        legacy.write_text(json.dumps({'process_name': 'legacy.exe'}), encoding='utf-8')
        original = legacy.read_bytes()
        prepare_data_directory(self.destination, legacy_directory=self.source)
        store = ProfileStore(self.destination / 'profiles')
        converted = store.migrate_legacy(self.destination / 'config.json')
        self.assertEqual(converted.process_name, 'legacy.exe')
        self.assertEqual(legacy.read_bytes(), original)
        self.assertFalse((self.destination / 'config.json').exists())
        self.assertTrue((self.destination / 'config.queue-backup.json').exists())
        self.assertIsNone(store.migrate_legacy(self.destination / 'config.json'))
        prepare_data_directory(self.destination, legacy_directory=self.source)
        self.assertEqual(len(store.list()), 1)


if __name__ == '__main__':
    unittest.main()
