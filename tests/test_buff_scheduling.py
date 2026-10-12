import unittest
from dataclasses import replace
from metin_bot.config import BuffConfig,BuffSkill,BuffIcon,Profile,Region
from metin_bot.buffs import BuffCycle
from metin_bot.buff_detection import Presence
from metin_bot.controller import Controller,Observation,ActionKind
from metin_bot.vision import Candidate
import numpy as np


class BuffSchedulingTests(unittest.TestCase):
    def test_reset_during_active_target_waits_for_completion(self):
        config=BuffConfig(enabled=True,fight_mounted=False,
                          skills=(BuffSkill('3',.2,interval_seconds=120),))
        c=Controller(replace(Profile.create('Reset during attack'),buffs=config));c.buffs.mounted=False;c.start(0)
        target=Candidate(40,50,10,10,(45,55));now=0
        for _ in range(40):
            action=c.step(now,Observation(True,(target,),False))
            if action:
                if action.kind==ActionKind.CLICK:break
                c.action_completed(now)
            now+=.25
        else:self.fail('No initial click')
        for _ in range(4):
            now+=.25;self.assertIsNone(c.step(now,Observation(True,(target,),True)))
        c.buffs.reset_timer(0,now,paused=False)
        for _ in range(5):
            now+=.25;self.assertIsNone(c.step(now,Observation(True,(target,),True)))
        for _ in range(20):
            now+=.25;action=c.step(now,Observation(True,(target,),False))
            if action:
                self.assertEqual(action.shortcut,'3')
                self.assertEqual(c.session.completed,1)
                break
        else:self.fail('Reset skill was not used after target completion')

    def test_short_refresh_intervals_do_not_starve_next_target(self):
        config=BuffConfig(enabled=True,fight_mounted=False,
                          skills=(BuffSkill('3',30,interval_seconds=10),BuffSkill('4',30,interval_seconds=10)))
        c=Controller(replace(Profile.create('Short intervals'),buffs=config));c.buffs.mounted=False;c.start(0)
        target=Candidate(40,50,10,10,(45,55));keys=[]
        for n in range(1200):
            action=c.step(n*.25,Observation(True,(target,),False))
            if action:
                if action.kind==ActionKind.CLICK:break
                keys.append(action.shortcut);c.action_completed(n*.25)
        else:self.fail('Refresh skills prevented target selection')
        self.assertEqual(keys,['3','4'])

    def config(self,regular=True,mounted=False):
        icon=BuffIcon.capture(np.random.default_rng(4).integers(0,256,(10,10,3),dtype=np.uint8))
        skills=(BuffSkill('2',.2,'inactive',icon),)+( (BuffSkill('3',.2,interval_seconds=14400),) if regular else ())
        return BuffConfig(enabled=True,
                          fight_mounted=mounted,mount_wait_seconds=.2,skills=skills,icon_region=Region(0,0,.2,.2))

    def test_toggle_between_intervals_preserves_regular_deadline(self):
        b=BuffCycle(self.config());b.mounted=False;b.resume(0)
        b.begin(0,(Presence.ACTIVE,Presence.UNKNOWN))
        self.assertEqual(b.step(0,(Presence.ACTIVE,)).shortcut,'3');b.acknowledge(0);b.step(.2)
        due=b.due.copy()
        self.assertTrue(b.needed(10,(Presence.ABSENT,)))
        b.begin(10,(Presence.ABSENT,))
        self.assertEqual(b.step(10,(Presence.ABSENT,)).shortcut,'2');b.acknowledge(10);b.step(10.2)
        self.assertEqual(b.due,due)

    def test_toggle_only_has_no_interval_or_mount_cycle_when_active_or_unknown(self):
        b=BuffCycle(self.config(regular=False,mounted=True));b.mounted=True;b.resume(0)
        self.assertIsNone(b.remaining(0))
        for state in (Presence.ACTIVE,Presence.UNKNOWN):self.assertFalse(b.needed(100,(state,)))
        self.assertTrue(b.needed(100,(Presence.ABSENT,)))
        b.begin(100,(Presence.ABSENT,));keys=[]
        for t in (100,100.25,100.5):
            step=b.step(t,(Presence.ABSENT,));keys.append(step.shortcut);b.acknowledge(t)
        self.assertEqual(keys,['Ctrl+G','2','Ctrl+G'])

    def test_toggle_expiry_during_target_waits_and_is_checked_before_next_click(self):
        p=replace(Profile.create('Expiry'),buffs=self.config(regular=False))
        c=Controller(p);c.buffs.mounted=False;c.start(0);target=Candidate(40,50,10,10,(45,55))
        active=Observation(True,(target,),False,buffs=(Presence.ACTIVE,))
        c.step(0,active);c.step(.25,active)
        self.assertEqual(c.step(.5,active).kind,ActionKind.CLICK)
        panel=Observation(True,(target,),True,buffs=(Presence.ABSENT,))
        for t in (.75,1,1.25,1.5):self.assertIsNone(c.step(t,panel))
        absent=Observation(True,(target,),False,buffs=(Presence.ABSENT,))
        for t in (1.75,2,2.25,2.5,2.75,3):self.assertIsNone(c.step(t,absent))
        action=c.step(3.25,absent);self.assertEqual(action.shortcut,'2');c.action_completed(3.25)
        self.assertIsNone(c.step(3.5,absent))
        for t in (3.75,4,4.25):self.assertIsNone(c.step(t,absent))
        self.assertEqual(c.step(4.5,absent).kind,ActionKind.CLICK)

    def test_toggle_expiring_during_regular_animation_is_not_skipped_at_next_click(self):
        config=self.config()
        config=replace(config,skills=(BuffSkill('3',2),config.skills[0]))
        c=Controller(replace(Profile.create('Animation expiry'),buffs=config));c.buffs.mounted=False;c.start(0)
        target=Candidate(40,50,10,10,(45,55))
        active=Observation(True,(target,),False,buffs=(Presence.UNKNOWN,Presence.ACTIVE))
        c.step(0,active);c.step(.25,active)
        self.assertEqual(c.step(.5,active).shortcut,'3');c.action_completed(.5)
        absent=Observation(True,(target,),False,buffs=(Presence.UNKNOWN,Presence.ABSENT))
        for n in range(3,14):self.assertIsNone(c.step(n*.25,absent))
        self.assertEqual(c.step(3.5,absent).shortcut,'2')

    def test_second_toggle_is_checked_without_resending_first_before_same_target(self):
        config=self.config(regular=False)
        config=replace(config,skills=(config.skills[0],replace(config.skills[0],shortcut='4')))
        c=Controller(replace(Profile.create('Two toggles'),buffs=config));c.buffs.mounted=False;c.start(0)
        target=Candidate(40,50,10,10,(45,55));keys=[]
        for n in range(30):
            state=Presence.ABSENT if keys else Presence.ACTIVE
            action=c.step(n*.25,Observation(True,(target,),False,buffs=(Presence.ABSENT,state)))
            if action:
                if action.kind==ActionKind.CLICK:break
                keys.append(action.shortcut);c.action_completed(n*.25)
        else:self.fail('Next target was never selected')
        self.assertEqual(keys,['2','4'])
