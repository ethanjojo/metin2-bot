"""Active and inactive hotbar samples supplied by the user."""
import unittest
from pathlib import Path
from dataclasses import replace
import cv2
import numpy as np
from metin_bot.config import BuffIcon,BuffSkill,BuffConfig,Region,Profile
from metin_bot.buff_detection import BuffDetector,Presence
from metin_bot.controller import Controller


class HotbarBuffTests(unittest.TestCase):
    def setUp(self):
        folder=Path(__file__).parent/'data'
        self.active=cv2.imread(str(folder/'hotbar-active.png'))
        self.inactive=cv2.imread(str(folder/'hotbar-inactive.png'))
        self.skill=BuffSkill('2',mode='inactive',icon=BuffIcon.capture(self.active),
                             inactive_icon=BuffIcon.capture(self.inactive))
        self.config=BuffConfig(enabled=True,skills=(self.skill,),icon_region=Region(0,0,1,1))

    def frame(self,icon):
        frame=np.random.default_rng(17).integers(0,256,(70,120,3),dtype=np.uint8)
        frame[10:45,50:85]=icon
        return frame

    def test_real_hotbar_inactive_confirmed_after_three_samples(self):
        d=BuffDetector(self.config,(120,70));frame=self.frame(self.inactive)
        self.assertEqual([d.observe(frame,n*.25)[0] for n in range(3)],
                         [Presence.UNKNOWN,Presence.UNKNOWN,Presence.ABSENT])
        self.assertEqual(d.observe(self.frame(self.active),.75)[0],Presence.ACTIVE)
        self.assertEqual(d.observe(frame,1)[0],Presence.UNKNOWN)

    def test_missing_both_states_never_means_inactive(self):
        d=BuffDetector(self.config,(120,70));frame=np.random.default_rng(9).integers(0,256,(70,120,3),dtype=np.uint8)
        for n in range(5):self.assertEqual(d.observe(frame,n*.25)[0],Presence.UNKNOWN)

    def test_ambiguous_both_templates_never_toggle(self):
        skill=replace(self.skill,inactive_icon=self.skill.icon)
        d=BuffDetector(replace(self.config,skills=(skill,)),(120,70))
        for n in range(5):self.assertEqual(d.observe(self.frame(self.active),n*.25)[0],Presence.UNKNOWN)

    def test_independently_selected_icon_sizes_roundtrip_and_detect_both_states(self):
        skill=replace(self.skill,inactive_icon=BuffIcon.capture(self.inactive[:30]))
        config=replace(self.config,skills=(skill,))
        p=replace(Profile.create('Hotbar'),buffs=config)
        self.assertEqual(Profile.from_dict(p.to_dict()),p)
        d=BuffDetector(config,(120,70))
        self.assertEqual(d.observe(self.frame(self.active),0)[0],Presence.ACTIVE)
        self.assertEqual([d.observe(self.frame(self.inactive),n*.25)[0] for n in (1,2,3)],
                         [Presence.UNKNOWN,Presence.UNKNOWN,Presence.ABSENT])

    def test_startup_sequence_only_sends_toggle_for_known_inactive_hotbar(self):
        config=replace(self.config,skills=(self.skill,BuffSkill('3')))
        profile=replace(Profile.create('Sequence'),buffs=config)
        from metin_bot.controller import Observation
        for image,expected in ((self.active,['Ctrl+G','3','Ctrl+G']),
                               (self.inactive,['Ctrl+G','2','3','Ctrl+G'])):
            c=Controller(profile);c.buffs.mounted=True;c.start(0);d=BuffDetector(config,(120,70));keys=[]
            for n in range(60):
                now=n*.25
                action=c.step(now,Observation(True,(),False,buffs=d.observe(self.frame(image),now)))
                if action:keys.append(action.shortcut);c.action_completed(now)
                if keys and not c.buffs.active:break
            self.assertEqual(keys,expected)
