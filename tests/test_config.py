import math
import unittest
from dataclasses import replace
from metin_bot.config import Profile, Region, TimingConfig

class ConfigTests(unittest.TestCase):
    def test_invalid_region(self):
        for values in [(0.5, .2, .3, .8), (-.1, 0, .5, 1), (0, 0, math.nan, 1)]:
            with self.subTest(values=values), self.assertRaises(ValueError):
                Region(*values)
    def test_invalid_intervals(self):
        for value in [0, -1, math.nan, math.inf, True]:
            with self.subTest(value=value), self.assertRaises(ValueError):
                TimingConfig(sample_interval=value)
    def test_round_trip(self):
        profile=replace(Profile.create('Test'), region=Region(.15,.15,.8,.75))
        self.assertEqual(profile,Profile.from_dict(profile.to_dict()))
    def test_unknown_keys_rejected(self):
        with self.assertRaises(ValueError):Profile.from_dict({'unexpected':1})
    def test_pixel_conversion(self):
        self.assertEqual(Region(.1,.2,.9,.8).pixels(1000,500),(100,100,900,400))
    def test_confirmation_window_allows_stable_frames(self):
        with self.assertRaises(ValueError):
            Profile.create('Test').__class__(**{**Profile.create('Test').__dict__, 'timing':TimingConfig(sample_interval=2,acquire_timeout=3)})
