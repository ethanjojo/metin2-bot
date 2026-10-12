import tkinter as tk
from tkinter import ttk
import unittest
from metin_bot.editor_ui import scroll_content


class SidebarScrollTests(unittest.TestCase):
    def setUp(self):
        self.root=tk.Tk();self.root.geometry('238x600')
        self.addCleanup(self.root.destroy)
        self.host,self.content,self.canvas=scroll_content(self.root)
        self.host.pack(fill='both',expand=True)
        self.label=ttk.Label(self.content,text='Krátký obsah')
        self.label.pack()
        self.root.update()

    def test_fitting_content_stays_at_top_after_wheel_or_scrollbar(self):
        self.assertEqual(self.canvas.yview(),(0.0,1.0))
        for movement in (lambda:self.canvas.yview_scroll(3,'units'),
                         lambda:self.canvas.yview_scroll(-3,'units'),
                         lambda:self.canvas.yview_moveto(1)):
            movement();self.root.update()
            self.assertEqual(self.canvas.coords(1),[0.0,0.0])
            self.assertEqual(self.canvas.canvasy(0),0)

    def test_overflow_scrolls_and_shrinking_content_resets_offset(self):
        self.label.configure(text='\n'.join(str(n) for n in range(80)))
        self.root.update()
        self.assertLess(self.canvas.yview()[1],1)
        self.canvas.yview_moveto(.5);self.root.update()
        self.assertGreater(self.canvas.canvasy(0),0)
        self.label.configure(text='Krátký obsah');self.root.update()
        self.assertEqual(self.canvas.yview(),(0.0,1.0))
        self.assertEqual(self.canvas.canvasy(0),0)

    def test_wheel_and_thumb_follow_real_overflow_and_resize(self):
        self.label.configure(text='\n'.join(str(n) for n in range(45)))
        self.root.update()
        bar=next(child for child in self.host.winfo_children() if isinstance(child,ttk.Scrollbar))
        self.assertEqual(bar.get(),self.canvas.yview())
        self.label.event_generate('<MouseWheel>',delta=-120);self.root.update()
        self.assertGreater(self.canvas.yview()[0],0)
        self.assertEqual(bar.get(),self.canvas.yview())
        self.root.geometry('238x900');self.root.update()
        self.assertEqual(self.canvas.yview(),(0.0,1.0))
        self.assertEqual(self.canvas.canvasy(0),0)
        self.assertTrue(bar.instate(['disabled']))
