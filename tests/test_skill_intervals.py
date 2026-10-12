import unittest
from dataclasses import replace
from metin_bot.config import BuffSkill,BuffConfig,Profile
from metin_bot.buffs import BuffCycle


class SkillIntervalTests(unittest.TestCase):
    def test_reset_only_selected_timer_to_due_now_and_preserves_pause(self):
        b=BuffCycle(self.config());b.mounted=False;b.resume(0);b.begin(0)
        b.step(0);b.acknowledge(0);b.step(.5);b.acknowledge(.5);b.step(1)
        b.reset_timer(0,2,paused=False)
        self.assertEqual(b.skill_remaining(2),(0,29))
        b.pause(3);b.reset_timer(1,50,paused=True);b.resume(100)
        self.assertEqual(b.skill_remaining(100),(0,0))
        with self.assertRaises(ValueError):b.reset_timer(9,100,paused=False)
        switch=BuffCycle(BuffConfig(skills=(BuffSkill('2',mode='inactive'),)))
        with self.assertRaises(ValueError):switch.reset_timer(0,100,paused=True)

    def config(self):
        return BuffConfig(enabled=True,fight_mounted=False,
                          skills=(BuffSkill('3',.5,interval_seconds=10),BuffSkill('4',.5,interval_seconds=30)))

    def test_independent_deadlines_start_after_each_skills_animation(self):
        b=BuffCycle(self.config());b.mounted=False;b.resume(0);b.begin(0)
        self.assertEqual(b.step(0).shortcut,'3');b.acknowledge(0)
        self.assertEqual(b.step(.5).shortcut,'4');b.acknowledge(.5);b.step(1)
        self.assertEqual(b.skill_remaining(1),(9.5,30))
        b.begin(10.5);self.assertEqual(b.step(10.5).shortcut,'3');b.acknowledge(10.5);b.step(11)
        self.assertEqual(b.skill_remaining(11),(10,20))
        b.pause(12);b.resume(100)
        self.assertEqual(b.skill_remaining(100),(9,19))

    def test_legacy_global_interval_migrates_only_regular_skills(self):
        data=Profile.create('Old').to_dict()
        data['buffs']={'enabled':True,'interval_seconds':14400,'skills':[{'shortcut':'3'},{'shortcut':'4','mode':'inactive'}]}
        p=Profile.from_dict(data)
        self.assertEqual(p.buffs.skills[0].interval_seconds,14400)
        self.assertIsNone(p.buffs.skills[1].interval_seconds)
        self.assertNotIn('interval_seconds',p.to_dict()['buffs'])
        self.assertEqual(Profile.from_dict(p.to_dict()),p)

    def test_invalid_intervals_and_switch_interval_rejected(self):
        for value in (0,86401,float('nan')):
            with self.assertRaises(ValueError):BuffSkill('3',interval_seconds=value)
        with self.assertRaises(ValueError):BuffSkill('2',mode='inactive',interval_seconds=100)
