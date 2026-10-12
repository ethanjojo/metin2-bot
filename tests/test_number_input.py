import tkinter as tk
import unittest
from unittest.mock import Mock
from metin_bot.config import NUMBER_RULES, BuffSkill
from metin_bot.number_input import number_input
from metin_bot.buff_ui import BuffList


class NumberInputTests(unittest.TestCase):
    def setUp(self):
        self.root=tk.Tk();self.addCleanup(self.root.destroy)

    def test_native_arrows_step_clamp_and_notify_without_rounding_offsets(self):
        for key,start,expected in (('min_area','120','130'),('key_hold_seconds','0.15','0.2'),('max_area_ratio','0.0001','0.0011')):
            with self.subTest(key=key):
                variable=tk.StringVar(value=start);changed=Mock();rule=NUMBER_RULES[key]
                control=number_input(self.root,variable=variable,rule=rule,on_change=changed)
                control.pack();self.root.update()
                control.event_generate('<<Increment>>')
                self.assertEqual(variable.get(),expected);changed.assert_called_once()
                for value,event in ((rule.high,'<<Increment>>'),(rule.low,'<<Decrement>>')):
                    variable.set(str(value));control.event_generate(event)
                    self.assertEqual(float(variable.get()),value)
                variable.set(start);self.assertEqual(variable.get(),start)
                control.destroy()

    def test_interval_step_tracks_minutes_and_seconds_and_preserves_limits(self):
        changed=Mock()
        editor=BuffList(self.root,on_change=changed,on_begin=Mock(),on_error=Mock(),on_pick=Mock())
        editor.load((BuffSkill('3',interval_seconds=14400),));self.root.update()
        row=editor.rows[0]
        self.assertEqual(row.interval_unit,'min')
        row.interval_entry.event_generate('<<Increment>>')
        self.assertEqual(editor.skills()[0].interval_seconds,14460)
        editor.set_interval_unit(row,'s')
        row.interval_entry.event_generate('<<Decrement>>')
        self.assertEqual(editor.skills()[0].interval_seconds,14459)
        self.assertEqual(float(row.interval_entry.cget('from')),10)
        self.assertEqual(float(row.interval_entry.cget('to')),86400)
