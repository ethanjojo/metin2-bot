import unittest
from dataclasses import replace
import numpy as np
from metin_bot.config import BuffIcon,BuffSkill,BuffConfig,Profile,Region
from metin_bot.buff_detection import BuffDetector,Presence
from metin_bot.buffs import BuffCycle
from metin_bot.controller import Controller, Observation


class BuffDetectionTests(unittest.TestCase):
    def setUp(self):
        self.image=np.random.default_rng(42).integers(0,256,(20,20,3),dtype=np.uint8)
        self.frame=np.random.default_rng(7).integers(0,256,(100,200,3),dtype=np.uint8)
        self.icon=BuffIcon.capture(self.image)
        self.config=BuffConfig(enabled=True,fight_mounted=False,
            icon_region=Region(0,0,.5,.6),skills=(BuffSkill('F1',.2,'inactive',self.icon),BuffSkill('F2',.2,interval_seconds=10)))

    def test_icon_and_modes_survive_profile_roundtrip(self):
        p=replace(Profile.create('Modes'),buffs=self.config)
        restored=Profile.from_dict(p.to_dict())
        self.assertEqual(restored,p)
        np.testing.assert_array_equal(restored.buffs.skills[0].icon.decode(),self.image)
        old=p.to_dict();old['buffs']['skills'][0].pop('mode');old['buffs']['skills'][0].pop('icon')
        self.assertEqual(Profile.from_dict(old).buffs.skills[0].mode,'refresh')

    def test_active_icon_can_move_within_buff_area(self):
        detector=BuffDetector(self.config,(200,100))
        for n,(x,y) in enumerate(((10,5),(60,30))):
            frame=self.frame.copy();frame[y:y+20,x:x+20]=self.image
            self.assertEqual(detector.observe(frame,n*.25)[0],Presence.ACTIVE)

    def test_hotbar_copy_is_not_active_and_absence_requires_three_frames(self):
        detector=BuffDetector(self.config,(200,100));frame=self.frame.copy()
        frame[75:95,170:190]=self.image
        self.assertEqual([detector.observe(frame,n*.25)[0] for n in range(3)],
                         [Presence.UNKNOWN,Presence.UNKNOWN,Presence.ABSENT])
        self.assertEqual(detector.observe(frame,4)[0],Presence.UNKNOWN)
        self.assertEqual(detector.observe(frame,4)[0],Presence.UNKNOWN)

    def test_black_frame_and_wrong_size_are_unknown_and_reset_evidence(self):
        detector=BuffDetector(self.config,(200,100))
        for n in range(3):detector.observe(self.frame,n*.25)
        self.assertEqual(detector.observe(np.zeros_like(self.frame),.75)[0],Presence.UNKNOWN)
        self.assertEqual(detector.observe(self.frame,1)[0],Presence.UNKNOWN)
        self.assertEqual(detector.observe(self.frame[:90],1.25)[0],Presence.UNKNOWN)

    def test_absence_with_valid_slow_sampling_and_stale_gap(self):
        detector=BuffDetector(self.config,(200,100),sample_interval=2)
        self.assertEqual([detector.observe(self.frame,n*2)[0] for n in range(3)],
                         [Presence.UNKNOWN,Presence.UNKNOWN,Presence.ABSENT])
        self.assertEqual(detector.observe(self.frame,8)[0],Presence.UNKNOWN)

    def test_cycle_skips_active_and_unknown_but_refreshes_regular_skill(self):
        for state in (Presence.ACTIVE,Presence.UNKNOWN):
            cycle=BuffCycle(self.config);cycle.mounted=False;cycle.resume(0);cycle.begin(10)
            step=cycle.step(10,(state,Presence.UNKNOWN))
            self.assertEqual(step.shortcut,'F2')
            self.assertTrue(cycle.skipped)
            cycle.acknowledge(10);self.assertIsNone(cycle.step(10.3))

    def test_cycle_checks_latest_state_before_each_skill(self):
        config=replace(self.config,skills=tuple(reversed(self.config.skills)))
        cycle=BuffCycle(config);cycle.mounted=False;cycle.resume(0);cycle.begin(10,(Presence.UNKNOWN,Presence.ABSENT))
        self.assertEqual(cycle.step(10,(Presence.UNKNOWN,Presence.ABSENT)).shortcut,'F2')
        cycle.acknowledge(10)
        self.assertIsNone(cycle.step(10.3,(Presence.UNKNOWN,Presence.ACTIVE)))
        self.assertFalse(cycle.active)

    def test_confirmed_absence_activates_toggle(self):
        cycle=BuffCycle(self.config);cycle.mounted=False;cycle.resume(0);cycle.begin(10,(Presence.ABSENT,))
        self.assertEqual(cycle.step(10,(Presence.ABSENT,)).shortcut,'F1')

    def test_first_cycle_checks_icon_after_three_samples_even_with_fast_panel(self):
        for state,key in ((Presence.ACTIVE,'F2'),(Presence.ABSENT,'F1'),(Presence.UNKNOWN,'F2')):
            profile=replace(Profile.create('Start'),buffs=self.config)
            profile=replace(profile,panel=replace(profile.panel,stable_frames=2))
            c=Controller(profile);c.buffs.mounted=False;c.start(0)
            first=Observation(True,(),False,buffs=(Presence.UNKNOWN,Presence.UNKNOWN))
            self.assertIsNone(c.step(0,first));self.assertIsNone(c.step(.25,first))
            action=c.step(.5,Observation(True,(),False,buffs=(state,Presence.UNKNOWN)))
            self.assertEqual(action.shortcut,key)

    def test_missing_icon_can_be_saved_as_draft_but_blocks_running(self):
        config=replace(self.config,skills=(BuffSkill('F1',mode='inactive'),))
        self.assertFalse(config.ready)
        controller=Controller(replace(Profile.create('Draft'),buffs=config));controller.start(0)
        self.assertFalse(controller.running)
        self.assertIn('icon',controller.status.reason)
        with self.assertRaises(ValueError):BuffSkill('F1',mode='invalid')
        with self.assertRaises(ValueError):BuffIcon.capture(np.zeros((20,20,3),np.uint8))
        with self.assertRaises(ValueError):BuffIcon('not png')
