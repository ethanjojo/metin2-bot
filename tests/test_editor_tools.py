import unittest
from dataclasses import replace
import numpy as np
from metin_bot.calibration import Calibration, Mode
from metin_bot.config import Profile
from metin_bot.editor_tools import History, Viewport


class EditorToolsTests(unittest.TestCase):
    def calibration(self):
        c=Calibration(Profile.create('Test'))
        c.freeze(np.random.default_rng(3).integers(0,255,(600,800,3),dtype=np.uint8))
        return c

    def test_undo_redo_and_edit_after_undo_discards_future(self):
        c=self.calibration();history=History(c)
        c.drag(Mode.WORLD,(10,20),(300,400));history.record(c)
        c.drag(Mode.MASK,(50,60),(100,120));history.record(c)
        history.undo(c);self.assertEqual(c.profile.masks,())
        history.redo(c);self.assertEqual(len(c.profile.masks),1)
        history.undo(c);c.color((200,200));history.record(c)
        self.assertFalse(history.can_redo)
        history.undo(c);self.assertIsNone(c.profile.color)

    def test_template_snapshots_are_independent_and_restored(self):
        c=self.calibration();c.drag(Mode.PANEL,(0,0),(400,200));history=History(c)
        c.drag(Mode.TEMPLATE,(10,10),(50,30));history.record(c)
        expected=c.template.copy();c.template[:]=0
        history.undo(c);self.assertIsNone(c.template)
        history.redo(c);np.testing.assert_array_equal(c.template,expected)

    def test_history_is_bounded_and_does_not_record_duplicate_states(self):
        c=self.calibration();history=History(c,limit=4)
        history.record(c);self.assertFalse(history.can_undo)
        for i in range(10):
            c.profile=replace(c.profile,name=f'Test {i}');history.record(c)
        count=0
        while history.can_undo:history.undo(c);count+=1
        self.assertEqual(count,3)

    def test_zoom_keeps_pixel_under_cursor_and_round_trips(self):
        v=Viewport();v.fit((800,600),(400,300))
        before=v.to_image((100,100))
        v.zoom_at((100,100),2)
        self.assertEqual(v.to_image((100,100)),before)
        self.assertEqual(v.to_canvas(before),(100,100))
        self.assertEqual(v.display_size,(800,600))

    def test_padding_rejected_clamping_pan_and_reset(self):
        v=Viewport();v.fit((800,600),(1000,300))
        with self.assertRaises(ValueError):v.to_image((800,100))
        v.zoom_at((100,100),2);v.pan(40,20)
        p=v.to_canvas((100,100));self.assertEqual(v.to_image(p),(100,100))
        self.assertEqual(v.to_image((-10000,-10000),clamp=True),(0,0))
        v.reset();self.assertEqual(v.scale,.5);self.assertEqual(v.offset,(0,0))

    def test_zoom_is_bounded_and_resize_preserves_magnification(self):
        v=Viewport();v.fit((800,600),(400,300));v.zoom_at((100,100),100)
        self.assertEqual(v.magnification,8)
        v.fit((800,600),(600,400));self.assertEqual(v.magnification,8)
        v.zoom_at((100,100),.00001);self.assertEqual(v.magnification,1)
