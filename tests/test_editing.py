import unittest
from dataclasses import replace
import numpy as np
from metin_bot.config import Profile, Region, PanelConfig
from metin_bot.calibration import Calibration, Mode
from metin_bot.editing import hit_handle, transform_region


class EditingTests(unittest.TestCase):
    def test_handles_win_over_interior_and_outside_creates(self):
        r = Region(.2, .2, .6, .6)
        self.assertEqual(hit_handle(r, (200, 120), (1000, 600)), 'nw')
        self.assertEqual(hit_handle(r, (400, 120), (1000, 600)), 'n')
        self.assertEqual(hit_handle(r, (400, 240), (1000, 600)), 'move')
        self.assertIsNone(hit_handle(r, (100, 100), (1000, 600)))

    def test_move_preserves_size_and_clamps_to_image(self):
        r = Region(.2, .2, .6, .6)
        moved = transform_region(r, (400, 300), (1200, 1000), (1000, 600), 'move')
        self.assertAlmostEqual(moved.right-moved.left, .4)
        self.assertAlmostEqual(moved.bottom-moved.top, .4)
        self.assertEqual((moved.right, moved.bottom), (1, 1))

    def test_resize_and_crossing_never_inverts_rectangle(self):
        r = Region(.2, .2, .6, .6)
        resized = transform_region(r, (600, 360), (800, 480), (1000, 600), 'se')
        self.assertEqual(resized, Region(.2, .2, .8, .8))
        crossed = transform_region(r, (200, 120), (900, 500), (1000, 600), 'nw')
        self.assertGreater(crossed.right-crossed.left, .001)
        self.assertGreater(crossed.bottom-crossed.top, .001)

    def test_mask_edit_replaces_only_selected_mask_and_delete_one(self):
        a, b = Region(.1,.1,.2,.2), Region(.6,.6,.9,.9)
        c = Calibration(replace(Profile.create('Test'), masks=(a,b)))
        c.freeze(np.zeros((600,1000,3),np.uint8))
        changed = Region(.2,.2,.4,.4)
        c.set_region(Mode.MASK, changed, 0)
        self.assertEqual(c.profile.masks, (changed,b))
        c.remove_region(Mode.MASK,0)
        self.assertEqual(c.profile.masks,(b,))

    def test_template_region_survives_reload_and_can_be_adjusted(self):
        c = Calibration(replace(Profile.create('Test'),panel=PanelConfig(Region(0,0,.5,.5))))
        c.freeze(np.random.default_rng(4).integers(0,255,(600,1000,3),dtype=np.uint8))
        c.drag(Mode.TEMPLATE,(30,30),(100,70))
        loaded = Profile.from_dict(c.profile.to_dict())
        self.assertEqual(loaded.template_region,Region(.03,.05,.1,70/600))
        c.set_region(Mode.TEMPLATE,Region(.04,.05,.12,.15))
        self.assertEqual(c.template.shape[:2],(61,81))

    def test_panel_resize_preserves_contained_anchor_and_invalidates_excluded(self):
        c = Calibration(replace(Profile.create('Test'),panel=PanelConfig(Region(0,0,.5,.5))))
        c.freeze(np.random.default_rng(4).integers(0,255,(600,1000,3),dtype=np.uint8))
        c.drag(Mode.TEMPLATE,(30,30),(100,70))
        c.set_region(Mode.PANEL,Region(0,0,.6,.6))
        self.assertIsNotNone(c.template)
        c.set_region(Mode.PANEL,Region(.5,.5,.9,.9))
        self.assertIsNone(c.template)
        self.assertIsNone(c.profile.template_region)

    def test_invalid_template_edit_keeps_previous_selection(self):
        c = Calibration(replace(Profile.create('Test'),panel=PanelConfig(Region(0,0,.9,.9))))
        frame=np.zeros((600,1000,3),np.uint8)
        frame[30:80,30:110]=np.random.default_rng(2).integers(0,255,(50,80,3),dtype=np.uint8)
        c.freeze(frame);c.drag(Mode.TEMPLATE,(30,30),(100,70))
        previous=c.profile
        with self.assertRaises(ValueError):c.set_region(Mode.TEMPLATE,Region(.5,.5,.6,.6))
        self.assertEqual(c.profile,previous)
