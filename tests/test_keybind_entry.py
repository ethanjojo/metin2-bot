import tkinter as tk
from types import SimpleNamespace
from unittest.mock import Mock
import unittest
from metin_bot.keybind_entry import KeybindEntry, captured_shortcut


class KeybindTests(unittest.TestCase):
    def test_shortcuts_capture_modifiers_numbers_and_function_keys(self):
        self.assertEqual(captured_shortcut('g',71,4),'Ctrl+G')
        self.assertEqual(captured_shortcut('F4',115,0),'F4')
        self.assertEqual(captured_shortcut('exclam',49,1),'Shift+1')
        self.assertEqual(captured_shortcut('g',71,0x20000|4),'Ctrl+Alt+G')
        self.assertIsNone(captured_shortcut('Control_L',17,0))
        with self.assertRaises(ValueError):captured_shortcut('F7',118,0)

    def widget(self,allow_empty=True):
        root=tk.Tk();root.geometry('240x120');self.addCleanup(root.destroy)
        value=tk.StringVar(root,value='F1');changed=Mock();pause=Mock();error=Mock()
        entry=KeybindEntry(root,variable=value,on_change=changed,on_begin=pause,on_error=error,allow_empty=allow_empty)
        entry.pack();root.focus_force();root.update()
        return root,entry,value,changed,pause,error

    def test_capture_commits_chord_once_and_does_not_store_hint(self):
        root,entry,value,changed,pause,error=self.widget()
        entry.begin();root.update()
        self.assertEqual(value.get(),'F1');pause.assert_called_once()
        entry.event_generate('<KeyPress>',keysym='g',state=4);root.update()
        self.assertEqual(value.get(),'Ctrl+G');changed.assert_called_once()
        entry.event_generate('<KeyPress>',keysym='g',state=4);root.update()
        changed.assert_called_once()

    def test_escape_focus_loss_clear_and_reserved_keys(self):
        root,entry,value,changed,pause,error=self.widget()
        entry.begin();root.update();entry.key_press(SimpleNamespace(keysym='F8',keycode=119,state=0))
        self.assertEqual(value.get(),'F1');error.assert_called_once()
        entry.key_press(SimpleNamespace(keysym='Escape',keycode=27,state=0))
        self.assertFalse(entry.capturing);self.assertEqual(value.get(),'F1')
        entry.begin();root.update();entry.event_generate('<FocusOut>');root.update()
        self.assertFalse(entry.capturing);self.assertEqual(value.get(),'F1')
        entry.begin();root.update();entry.key_press(SimpleNamespace(keysym='BackSpace',keycode=8,state=0))
        self.assertEqual(value.get(),'');changed.assert_called_once()

    def test_mount_cannot_be_cleared_and_profile_reload_updates_display(self):
        root,entry,value,changed,pause,error=self.widget(False)
        entry.begin();root.update();entry.key_press(SimpleNamespace(keysym='Delete',keycode=46,state=0))
        self.assertEqual(value.get(),'F1');changed.assert_not_called()
        entry.cancel();value.set('Ctrl+G')
        self.assertEqual(entry.display.get(),'Ctrl+G')

    def test_recording_consumes_editor_shortcuts_and_tab_preserves_value(self):
        root,entry,value,changed,pause,error=self.widget()
        undo=Mock();root.bind('<Control-z>',undo)
        entry.begin();root.update();entry.event_generate('<KeyPress>',keysym='z',state=4);root.update()
        self.assertEqual(value.get(),'Ctrl+Z');undo.assert_not_called()
        entry.begin();root.update()
        self.assertIsNone(entry.key_press(SimpleNamespace(keysym='Tab',keycode=9,state=0)))
        self.assertFalse(entry.capturing);self.assertEqual(value.get(),'Ctrl+Z')
