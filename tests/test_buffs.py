import unittest
from dataclasses import replace
from metin_bot.config import Profile, BuffConfig, BuffSkill
from metin_bot.buffs import BuffCycle, hotkey_keys
from metin_bot.controller import Controller, Observation, ActionKind
from metin_bot.vision import Candidate


class BuffTests(unittest.TestCase):
    def test_startup_buffs_before_target_and_resume_preserves_interval(self):
        c=Controller(replace(Profile.create('Start'),buffs=self.config(fight_mounted=False)));c.buffs.mounted=False
        c.start(100);empty=Observation(True,(),False)
        self.assertIsNone(c.step(100,empty));self.assertIsNone(c.step(100.25,empty))
        self.assertEqual(c.step(100.5,empty).shortcut,'F1');c.action_completed(100.5)
        self.assertEqual(c.step(101,empty).shortcut,'2');c.action_completed(101)
        c.step(101.5,empty)
        self.assertEqual(c.buffs.skill_remaining(101.5),(9.5,10))
        c.pause('Pause',103.5);c.start(200)
        self.assertEqual(c.buffs.skill_remaining(200),(7.5,8))

    def config(self,**kwargs):
        return BuffConfig(enabled=True,mount_wait_seconds=.5,
                          skills=(BuffSkill('F1',.5,interval_seconds=10),BuffSkill('2',.5,interval_seconds=10)),**kwargs)

    def test_keys_and_reserved_hotkeys(self):
        self.assertEqual(hotkey_keys('Ctrl+G'),(0x11,0x47))
        self.assertEqual(hotkey_keys('F4'),(0x73,))
        for key in ('F7','Ctrl+F8','Ctrl+Ctrl+G','Alt+Tab',''):
            with self.subTest(key=key),self.assertRaises(ValueError):hotkey_keys(key)

    def test_validation_round_trip_and_disabled_old_profile(self):
        p=replace(Profile.create('Test'),buffs=self.config(fight_mounted=False))
        self.assertEqual(Profile.from_dict(p.to_dict()),p)
        old=p.to_dict();old.pop('buffs')
        self.assertFalse(Profile.from_dict(old).buffs.enabled)
        with self.assertRaises(ValueError):BuffConfig(enabled=True)

    def test_initial_mount_is_not_profile_data_and_new_cycle_is_unknown(self):
        data=Profile.create('Legacy mount').to_dict()
        data['buffs']['initial_mounted']=True
        profile=Profile.from_dict(data)
        self.assertNotIn('initial_mounted',profile.to_dict()['buffs'])
        self.assertIsNone(BuffCycle(profile.buffs).mounted)

    def test_four_hour_interval_survives_profile_round_trip_and_pause(self):
        config=BuffConfig(enabled=True,skills=(BuffSkill('F1',interval_seconds=14400),))
        profile=replace(Profile.create('Long buffs'),buffs=config)
        restored=Profile.from_dict(profile.to_dict())
        cycle=BuffCycle(restored.buffs);cycle.mounted=True;cycle.resume(94)
        self.assertTrue(cycle.needed(94))
        cycle.begin(94)
        for t in (94,96,98):
            cycle.step(t)
            cycle.acknowledge(t)
        cycle.step(100)
        self.assertFalse(cycle.needed(14497.49))
        cycle.pause(3700);cycle.resume(20000)
        self.assertEqual(cycle.remaining(20000),10797.5)
        self.assertFalse(cycle.needed(30797.49))
        self.assertTrue(cycle.needed(30797.5))

    def test_mounted_cycle_acknowledges_every_step_and_waits(self):
        b=BuffCycle(self.config());b.mounted=True;b.resume(0)
        self.assertTrue(b.needed(0))
        b.begin(10)
        keys=[]
        for now in (10,10.5,11,11.5):
            step=b.step(now);keys.append(step.shortcut)
            self.assertIsNone(b.step(now))
            b.acknowledge(now)
            self.assertIsNone(b.step(now+.25))
        self.assertEqual(keys,['Ctrl+G','F1','2','Ctrl+G'])
        self.assertTrue(b.mounted)
        self.assertIsNone(b.step(12));self.assertFalse(b.active)
        self.assertEqual(b.skill_remaining(12),(9,9.5))

    def test_foot_combat_never_mounts_and_pause_preserves_interval(self):
        b=BuffCycle(self.config(fight_mounted=False));b.mounted=False;b.resume(0)
        b.begin(0)
        for now in (0,.5):b.step(now);b.acknowledge(now)
        b.step(1)
        b.pause(4);b.resume(100)
        self.assertEqual(b.skill_remaining(100),(6.5,7))
        b.begin(107)
        for now,key in ((107,'F1'),(107.5,'2')):
            self.assertEqual(b.step(now).shortcut,key);b.acknowledge(now)
        b.step(108)
        self.assertFalse(b.mounted);self.assertFalse(b.active)

    def test_due_buff_waits_for_confirmed_target_end_before_first_key(self):
        p=replace(Profile.create('Test'),buffs=self.config())
        c=Controller(p);c.buffs.mounted=True;c.start(0)
        target=Candidate(40,50,10,10,(45,55))
        panel=Observation(True,(target,),True);empty=Observation(True,(target,),False)
        for t in (0,.25,.5):self.assertIsNone(c.step(t,panel))
        for n in range(3,45):self.assertIsNone(c.step(n*.25,panel))
        for t in (11.25,11.5,11.75):self.assertIsNone(c.step(t,empty))
        for t in (12,12.25,12.5):self.assertIsNone(c.step(t,empty))
        action=c.step(12.75,empty)
        self.assertEqual(action.kind,ActionKind.MOUNT)
        self.assertEqual(action.shortcut,'Ctrl+G')
        self.assertEqual(c.session.clicks,0)

    def test_mount_preference_is_aligned_before_first_target(self):
        for fight,initial in ((True,False),(False,True)):
            c=Controller(replace(Profile.create('Test'),buffs=self.config(fight_mounted=fight)));c.buffs.mounted=initial
            c.start(0);empty=Observation(True,(),False)
            c.step(0,empty);c.step(.25,empty)
            action=c.step(.5,empty)
            keys=[]
            now=.5
            while c.buffs.active:
                if action:
                    keys.append(action.shortcut);c.action_completed(now)
                now+=.5;action=c.step(now,empty)
            self.assertEqual(keys,['F1','2','Ctrl+G'] if fight else ['Ctrl+G','F1','2'])
            self.assertEqual(c.buffs.mounted,fight)

    def test_unknown_mount_blocks_resume_and_panel_interrupts_buffing(self):
        c=Controller(replace(Profile.create('Test'),buffs=self.config(fight_mounted=False)));c.buffs.mounted=True
        c.start(0);empty=Observation(True,(),False)
        c.step(0,empty);c.step(.25,empty);c.step(.5,empty)
        self.assertIsNone(c.step(.75,Observation(True,(),True)))
        self.assertFalse(c.running)
        c.buffs.mounted=None;c.start(5)
        self.assertFalse(c.running)
        self.assertIn('unknown',c.status.reason)
