import json
import tempfile
import unittest
from dataclasses import replace
from pathlib import Path
from unittest.mock import patch

import numpy as np

from metin_bot.config import Profile, ColorConfig, PanelConfig, Region, TimingConfig
from metin_bot.profiles import ProfileStore


class ProfileTests(unittest.TestCase):
    def setUp(self):
        self.temp = tempfile.TemporaryDirectory(dir=Path(__file__).parent)
        self.addCleanup(self.temp.cleanup)
        self.root = Path(self.temp.name)
        self.store = ProfileStore(self.root / 'profiles')

    def test_incomplete_profile_cannot_start(self):
        self.assertFalse(Profile.create('Verzum').ready)

    def test_roundtrip_and_template_isolation(self):
        first = replace(Profile.create('One'), color=ColorConfig((10, 170, 200)))
        second = replace(Profile.create('Two'), color=ColorConfig((110, 80, 70)))
        a = np.arange(300, dtype=np.uint8).reshape(10, 10, 3)
        b = np.flip(a, axis=0).copy()
        self.store.save(first, a)
        self.store.save(second, b)
        loaded, template = self.store.load(first.id)
        other, other_template = self.store.load(second.id)
        self.assertEqual(loaded.color, first.color)
        self.assertNotEqual(loaded.color, other.color)
        np.testing.assert_array_equal(template, a)
        np.testing.assert_array_equal(other_template, b)

    def test_unknown_fields_and_unsafe_paths_rejected(self):
        p = Profile.create('Test')
        self.store.save(p)
        with self.assertRaises(ValueError):
            self.store.load('../other')
        path = self.store.root / p.id / 'profile.json'
        data = json.loads(path.read_text())
        for field, value in [('unexpected', 1), ('template_file', '../outside.png')]:
            mutated = dict(data, **{field: value})
            path.write_text(json.dumps(mutated))
            with self.assertRaises(ValueError):
                self.store.load(p.id)

    def test_missing_template_rejected(self):
        p = Profile.create('Test')
        self.store.save(p, np.arange(300, dtype=np.uint8).reshape(10, 10, 3))
        path = self.store.root / p.id
        for asset in path.glob('*.png'):
            asset.unlink()
        with self.assertRaises(ValueError):
            self.store.load(p.id)

    def test_failed_commit_preserves_previous_template(self):
        p = Profile.create('Test')
        original = np.arange(300, dtype=np.uint8).reshape(10, 10, 3)
        self.store.save(p, original)
        with patch('metin_bot.profiles.os.replace', side_effect=OSError('disk')):
            with self.assertRaises(OSError):
                self.store.save(replace(p, name='Changed'), np.flip(original, 0).copy())
        actual, image = self.store.load(p.id)
        self.assertEqual(actual.name, 'Test')
        np.testing.assert_array_equal(image, original)
        self.assertEqual(len(list((self.store.root / p.id).glob('*.png'))), 1)

    def test_migration_once_preserves_only_applicable_fields(self):
        legacy = self.root / 'config.json'
        legacy.write_text(json.dumps({'process_name': 'verzumclient_x64.exe',
                                     'region': dict(left=.1, top=.1, right=.8, bottom=.8),
                                     'vision': {'hue_low': 15}, 'fill_interval': 8}))
        profile = self.store.migrate_legacy(legacy)
        self.assertFalse(profile.ready)
        self.assertEqual(profile.region, Region(.1, .1, .8, .8))
        self.assertFalse(legacy.exists())
        self.assertTrue(legacy.with_suffix('.queue-backup.json').exists())
        self.assertIsNone(self.store.migrate_legacy(legacy))
        self.assertEqual(len(self.store.list()), 1)

    def test_validation_rejects_nan_and_wrong_types(self):
        for value in [float('nan'), True, -1, 0]:
            with self.assertRaises(ValueError):
                TimingConfig(attack_timeout=value)
        with self.assertRaises(ValueError):
            ColorConfig((180, 0, 0))
        with self.assertRaises(ValueError):
            Profile.create('')

    def test_last_selection_is_separate_from_profile_data(self):
        p = Profile.create('Test')
        self.store.save(p)
        self.store.select(p.id)
        self.assertEqual(self.store.selected(), p.id)
