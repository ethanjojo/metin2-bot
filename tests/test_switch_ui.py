import tkinter as tk
from tkinter import ttk
from types import SimpleNamespace
import unittest
from metin_bot.theme import apply_theme


class SwitchTests(unittest.TestCase):
    def test_native_switch_preserves_variable_command_and_unknown_state_across_themes(self):
        root=tk.Tk();self.addCleanup(root.destroy)
        variable=tk.IntVar(root,value=-1);calls=[]
        switch=ttk.Checkbutton(root,text='Mount',style='Switch.TCheckbutton',variable=variable,
                               onvalue=1,offvalue=0,command=lambda:calls.append(variable.get()))
        switch.pack()
        app=SimpleNamespace(root=root,canvas=tk.Canvas(root),icons={},icon_widgets=[])
        apply_theme(app,'light');root.update()
        switch.state(['alternate'])
        self.assertIn('Switch.light',ttk.Style(root).element_names())
        self.assertTrue(switch.instate(['alternate']))
        switch.invoke();self.assertEqual(calls,[1])
        apply_theme(app,'dark');root.update()
        self.assertEqual(variable.get(),1)
        switch.invoke();self.assertEqual(calls,[1,0])
        root.focus_force();switch.focus_set();root.update()
        switch.event_generate('<KeyPress-space>');root.update()
        self.assertEqual(calls,[1,0,1])
        switch.state(['disabled']);switch.invoke();self.assertEqual(calls,[1,0,1])
        apply_theme(app,'light');self.assertEqual(variable.get(),1)
