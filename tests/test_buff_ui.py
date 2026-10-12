import tkinter as tk
import unittest
import numpy as np
from metin_bot.config import BuffSkill, BuffIcon
from metin_bot.buff_ui import BuffList
from metin_bot.buff_detection import Presence


class BuffListTests(unittest.TestCase):
    def test_names_follow_rows_and_profile_roundtrip(self):
        from dataclasses import replace
        from metin_bot.config import Profile,BuffConfig
        self.editor.load((BuffSkill('3',name='Strach'),BuffSkill('4',name='Brnění')))
        self.editor.move(0,1)
        self.assertEqual([s.name for s in self.editor.skills()],['Brnění','Strach'])
        p=replace(Profile.create('Names'),buffs=BuffConfig(skills=self.editor.skills()))
        self.assertEqual(Profile.from_dict(p.to_dict()),p)
        self.editor.rows[0].name.set('Ochrana')
        self.assertEqual(self.editor.skills()[0].name,'Ochrana')
        with self.assertRaises(ValueError):BuffSkill('3',name='x'*61)

    def test_skill_status_countdown_and_reset_are_separate_and_follow_row_order(self):
        requests=[];self.editor.on_reset=requests.append
        self.editor.load((BuffSkill('3'),BuffSkill('2',mode='inactive',icon=BuffIcon.capture(np.random.default_rng(2).integers(0,256,(12,12,3),dtype=np.uint8)))))
        self.editor.update_timers((3661,None),enabled=True,dirty=False,running=True,states=(Presence.UNKNOWN,Presence.ACTIVE))
        row=self.editor.rows[0]
        self.assertEqual(row.timer_label.cget('text'),'01:01:01')
        self.assertEqual(row.state_label.cget('text'),'Countdown running')
        self.assertEqual(self.editor.rows[1].state_label.cget('text'),'Active')
        row.reset_button.invoke();self.assertEqual(requests,[0])
        self.editor.move(0,1)
        self.editor.update_timers((None,0),enabled=True,dirty=False,running=False)
        row.reset_button.invoke();self.assertEqual(requests,[0,1])
        self.assertEqual(row.timer_label.cget('text'),'00:00:00')
        self.assertEqual(row.state_label.cget('text'),'Ready to use')
        self.editor.update_timers((None,0),enabled=True,dirty=True,running=False)
        self.assertTrue(row.reset_button.instate(['disabled']))

    def test_interval_units_convert_without_changing_skill_duration(self):
        self.editor.load((BuffSkill('3',interval_seconds=14400),BuffSkill('4',interval_seconds=45)))
        row=self.editor.rows[0]
        self.assertEqual(row.interval.get(),'240')
        self.assertEqual(row.interval_unit,'min')
        self.editor.set_interval_unit(row,'s')
        self.assertEqual(row.interval.get(),'14400')
        self.assertEqual(self.editor.skills()[0].interval_seconds,14400)
        self.editor.set_interval_unit(row,'min');row.interval.set('239.5')
        self.assertEqual(self.editor.skills()[0].interval_seconds,14370)
        other=self.editor.rows[1]
        self.editor.set_interval_unit(other,'min')
        self.assertEqual(other.interval.get(),'0.75')
        self.assertEqual(self.editor.skills()[1].interval_seconds,45)
        self.editor.move(0,1)
        self.assertEqual(self.editor.skills()[1].interval_seconds,14370)
        row.interval.set('');self.editor.set_interval_unit(row,'s')
        self.assertEqual(row.interval.get(),'')
        row.interval.set('86401')
        with self.assertRaises(ValueError):self.editor.skills()

    def setUp(self):
        self.root=tk.Tk();self.root.withdraw();self.addCleanup(self.root.destroy)
        self.editor=BuffList(self.root,on_change=lambda:None,on_begin=lambda:None,
                             on_error=lambda message:None,on_pick=lambda index:None)

    def test_dynamic_rows_keep_whole_skill_together_when_sorted_and_removed(self):
        icon=BuffIcon.capture(np.random.default_rng(2).integers(0,256,(12,12,3),dtype=np.uint8))
        first=BuffSkill('F1',2,'inactive',icon);second=BuffSkill('F2',3)
        self.editor.load((first,second))
        self.assertEqual(len(self.editor.rows),2)
        self.editor.move(1,-1)
        self.assertEqual(self.editor.skills(),(second,first))
        self.editor.remove(0);self.assertEqual(self.editor.skills(),(first,))
        self.editor.add();self.assertEqual(len(self.editor.rows),2)
        self.assertEqual(self.editor.skills(),(first,))
        self.editor.load(());self.assertEqual(len(self.editor.rows),0)

    def test_limit_and_picker_follow_current_row_order(self):
        picked=[];self.editor.on_pick=lambda index,active:picked.append((index,active))
        self.editor.load((BuffSkill('F1'),BuffSkill('F2')))
        row=self.editor.rows[0];self.editor.move(0,1);self.editor.pick(row)
        self.assertEqual(picked,[(1,True)])
        for _ in range(10):self.editor.add()
        self.assertEqual(len(self.editor.rows),8)

    def test_both_thumbnails_follow_sort_and_can_be_replaced_or_deleted_independently(self):
        icon=BuffIcon.capture(np.random.default_rng(2).integers(0,256,(12,12,3),dtype=np.uint8))
        other=BuffIcon.capture(np.random.default_rng(3).integers(0,256,(12,12,3),dtype=np.uint8))
        self.editor.load((BuffSkill('F1',mode='inactive',icon=icon,inactive_icon=other),BuffSkill('F2')))
        self.editor.move(0,1);row=self.editor.rows[1]
        self.assertTrue(row.photo);self.assertTrue(row.inactive_photo)
        self.assertEqual(self.editor.skills()[1].inactive_icon,other)
        replacement=BuffIcon.capture(np.random.default_rng(5).integers(0,256,(10,10,3),dtype=np.uint8))
        self.editor.set_icon(1,replacement)
        self.assertEqual(self.editor.skills()[1].inactive_icon,other)
        self.editor.set_icon(1,icon,active=False)
        self.assertEqual(self.editor.skills()[1].inactive_icon,icon)
        self.editor.set_icon(1,None)
        self.assertIsNone(self.editor.skills()[1].icon)
        self.assertEqual(self.editor.skills()[1].inactive_icon,icon)
        self.editor.clear_icons()
        self.assertIsNone(row.photo);self.assertIsNone(row.inactive_photo)

    def test_identical_inactive_capture_is_rejected_without_changing_row(self):
        icon=BuffIcon.capture(np.random.default_rng(2).integers(0,256,(12,12,3),dtype=np.uint8))
        self.editor.load((BuffSkill('F1',mode='inactive',icon=icon),))
        with self.assertRaisesRegex(ValueError,'Both templates'):
            self.editor.set_icon(0,icon,active=False)
        self.assertIsNone(self.editor.skills()[0].inactive_icon)

    def test_either_icon_can_be_first_and_reused_when_region_is_saved(self):
        from metin_bot.config import Region
        icon=BuffIcon.capture(np.random.default_rng(2).integers(0,256,(12,12,3),dtype=np.uint8),Region(.1,.1,.2,.2))
        self.editor.load((BuffSkill('2',mode='inactive',inactive_icon=icon),))
        row=self.editor.rows[0]
        self.assertEqual(str(row.reuse_active.cget('state')),'normal')
        self.assertEqual(str(row.reuse_inactive.cget('state')),'disabled')
        picked=[];self.editor.on_pick=lambda *args:picked.append(args)
        row.reuse_active.invoke()
        self.assertEqual(picked,[(0,True,True)])
        self.editor.set_icon(0,None,False)
        self.assertEqual(str(row.reuse_active.cget('state')),'disabled')

    def test_intervals_and_timers_belong_to_rows_and_switch_has_no_interval(self):
        self.editor.load((BuffSkill('3',interval_seconds=14400),BuffSkill('4',interval_seconds=600)))
        self.editor.move(0,1)
        self.assertEqual(self.editor.skills()[1].interval_seconds,14400)
        self.editor.update_timers((42,900),enabled=True,dirty=False)
        self.assertEqual(self.editor.rows[0].timer_label.cget('text'),'00:00:42')
        row=self.editor.rows[1];row.mode.set('Activate when inactive');self.editor._mode_changed(row)
        self.assertIsNone(self.editor.skills()[1].interval_seconds)
        self.assertFalse(row.interval_box.winfo_manager())
