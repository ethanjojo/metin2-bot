import tkinter as tk
import unittest
from types import SimpleNamespace
from metin_bot.theme import UISettings
from metin_bot.preview_ui import render_empty_preview


class EmptyPreviewTests(unittest.TestCase):
    def test_centered_icon_and_context_text_follow_size_and_theme(self):
        root=tk.Tk();root.geometry('600x400');self.addCleanup(root.destroy)
        canvas=tk.Canvas(root);canvas.pack(fill='both',expand=True);root.update()
        app=SimpleNamespace(canvas=canvas,ui_settings=UISettings('dark'),icons={},preview_busy=False,selected_window=lambda:None)
        render_empty_preview(app)
        self.assertEqual(canvas.type(canvas.find_withtag('empty-icon')[0]),'image')
        self.assertEqual(canvas.coords('empty-icon')[0],canvas.winfo_width()/2)
        self.assertIn('Select',canvas.itemcget('empty-title','text'))
        canvas.delete('all');app.selected_window=lambda:object();app.preview_busy=True
        render_empty_preview(app)
        self.assertIn('Capturing',canvas.itemcget('empty-title','text'))
        canvas.delete('all');app.ui_settings=UISettings('light');app.preview_busy=False
        root.geometry('400x300');root.update();render_empty_preview(app)
        self.assertEqual(canvas.coords('empty-icon')[0],canvas.winfo_width()/2)
        self.assertIn('Snapshot',canvas.itemcget('empty-description','text'))
