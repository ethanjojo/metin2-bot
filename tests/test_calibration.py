import unittest
from dataclasses import replace
import numpy as np
from metin_bot.calibration import Calibration, Mode, map_point
from metin_bot.config import Profile, Region


class CalibrationTests(unittest.TestCase):
    def test_display_coordinates_and_padding(self):
        self.assertEqual(map_point(50,25,(100,50),(800,400)),(400,200))
        with self.assertRaises(ValueError):map_point(100,25,(100,50),(800,400))

    def test_template_is_frozen_and_must_be_inside_panel(self):
        frame=np.random.default_rng(2).integers(0,255,(200,300,3),dtype=np.uint8)
        p=replace(Profile.create('Test'),panel=__import__('metin_bot.config',fromlist=['PanelConfig']).PanelConfig(Region(0,0,.5,.4)))
        c=Calibration(p);c.freeze(frame)
        c.drag(Mode.TEMPLATE,(10,10),(50,40))
        template=c.template.copy();frame[:]=0
        np.testing.assert_array_equal(c.template,template)
        with self.assertRaises(ValueError):c.drag(Mode.TEMPLATE,(100,100),(150,150))

    def test_profile_switch_has_no_shared_template(self):
        a=Calibration(Profile.create('One'));b=Calibration(Profile.create('Two'))
        a.template=np.ones((10,10,3),np.uint8)
        self.assertIsNone(b.template)

    def test_new_frame_dimensions_invalidate_old_calibration(self):
        p=replace(Profile.create('Test'),frame_size=(200,200),region=Region(.1,.1,.8,.8))
        c=Calibration(p);c.freeze(np.zeros((300,400,3),np.uint8))
        self.assertIsNone(c.profile.region);self.assertIsNone(c.profile.template_file)
        self.assertEqual(c.profile.frame_size,(400,300))

    def test_resizing_invalidates_active_buff_templates_but_keeps_skill_settings(self):
        from metin_bot.config import BuffConfig,BuffIcon,BuffSkill
        icon=BuffIcon.capture(np.random.default_rng(2).integers(0,255,(20,20,3),dtype=np.uint8))
        skill=BuffSkill('F1',2,'inactive',icon)
        buffs=BuffConfig(enabled=True,skills=(skill,),icon_region=Region(0,0,.5,.3))
        c=Calibration(replace(Profile.create('Resize'),frame_size=(200,100),buffs=buffs))
        c.freeze(np.zeros((150,300,3),dtype=np.uint8))
        self.assertIsNone(c.profile.buffs.icon_region)
        self.assertEqual(c.profile.buffs.skills,(replace(skill,icon=None),))
        self.assertFalse(c.profile.buffs.ready)

    def test_color_and_masks_update_immutable_draft(self):
        c=Calibration(Profile.create('Test'));c.freeze(np.full((200,200,3),(40,100,180),np.uint8))
        c.color((100,100));self.assertIsNotNone(c.profile.color)
        c.drag(Mode.MASK,(20,20),(50,50))
        self.assertEqual(len(c.profile.masks),1)

    def test_small_panel_anchor_on_large_game_window(self):
        from metin_bot.config import PanelConfig
        c=Calibration(replace(Profile.create('Test'),panel=PanelConfig(Region(0,0,.3,.1))))
        c.freeze(np.random.default_rng(9).integers(0,255,(1200,2200,3),dtype=np.uint8))
        c.drag(Mode.TEMPLATE,(10,10),(60,25))
        self.assertEqual(c.template.shape[:2],(16,51))
