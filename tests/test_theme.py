import tempfile
import tkinter as tk
import unittest
from pathlib import Path
from tkinter import ttk
from metin_bot.theme import UISettings, apply_theme, PALETTES


class ThemeTests(unittest.TestCase):
    def test_theme_toggle_saves_both_directions_and_preserves_state_on_failure(self):
        from metin_bot.app import App
        from unittest.mock import Mock,patch
        app=App.__new__(App);app.data_directory=Path('isolated-test-data');app.ui_settings=UISettings('light',last_mounted=True);app.reason=Mock();app.render=Mock()
        with patch.object(UISettings,'save') as save,patch('metin_bot.app.apply_theme') as apply:
            app.toggle_theme()
            self.assertEqual(app.ui_settings.theme,'dark');apply.assert_called_with(app,'dark')
            self.assertTrue(app.ui_settings.last_mounted)
            app.toggle_theme()
            self.assertEqual(app.ui_settings.theme,'light');self.assertEqual(save.call_count,2)
            apply.reset_mock();save.side_effect=OSError('Cannot write')
            app.toggle_theme()
            self.assertEqual(app.ui_settings.theme,'light');apply.assert_not_called()

    def test_disclosure_preserves_values_and_restores_content(self):
        from metin_bot.ui_controls import Disclosure
        root=tk.Tk();self.addCleanup(root.destroy)
        box=Disclosure(root,'Mount');box.pack()
        field=ttk.Entry(box.content);field.insert(0,'Ctrl+G');field.pack()
        self.assertFalse(box.content.winfo_manager())
        box.toggle();self.assertEqual(box.content.winfo_manager(),'pack')
        box.toggle();self.assertFalse(box.content.winfo_manager())
        self.assertEqual(field.get(),'Ctrl+G')

    def test_preference_roundtrip_is_separate_and_validated(self):
        with tempfile.TemporaryDirectory(dir=Path(__file__).parent) as folder:
            path=Path(folder)/'ui-settings.json'
            self.assertEqual(UISettings.load(path).theme,'light')
            UISettings('dark').save(path)
            self.assertEqual(UISettings.load(path),UISettings('dark'))
            with self.assertRaises(ValueError):UISettings('invalid')
            UISettings('dark',last_mounted=True).save(path)
            self.assertTrue(UISettings.load(path).last_mounted)
            path.write_text('{"theme":"dark"}',encoding='utf-8')
            self.assertFalse(UISettings.load(path).last_mounted)
            with self.assertRaises(ValueError):UISettings(last_mounted=1)

    def test_switch_restyles_native_widgets_and_keeps_values(self):
        root=tk.Tk();self.addCleanup(root.destroy)
        entry=ttk.Entry(root);entry.insert(0,'240')
        canvas=tk.Canvas(root);canvas.pack()
        masks=tk.Listbox(root);masks.insert(0,'Maska 1');masks.pack()
        from types import SimpleNamespace
        app=SimpleNamespace(root=root,canvas=canvas,icons={},icon_widgets=[])
        apply_theme(app,'dark')
        self.assertEqual(canvas.cget('background'),PALETTES['dark']['preview'])
        self.assertEqual(masks.cget('background'),PALETTES['dark']['field'])
        self.assertEqual(entry.get(),'240')
        apply_theme(app,'light')
        self.assertEqual(canvas.cget('background'),PALETTES['light']['preview'])
