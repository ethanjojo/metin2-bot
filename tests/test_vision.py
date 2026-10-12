import unittest
from dataclasses import replace
import cv2
import numpy as np
from metin_bot.config import Profile, ColorConfig, PanelConfig, Region
from metin_bot.vision import Candidate, CandidateTracker, detect_candidates, panel_score, pick_color, validate_template


class VisionTests(unittest.TestCase):
    def profile(self):
        return replace(Profile.create('Test'), region=Region(0, 0, 1, 1), color=ColorConfig((0, 200, 200), min_area=10))

    def frame(self):
        return np.zeros((200, 200, 3), np.uint8)

    def paint(self, frame, bounds, hsv=(0,200,200)):
        x,y,w,h=bounds
        frame[y:y+h,x:x+w]=cv2.cvtColor(np.uint8([[hsv]]),cv2.COLOR_HSV2BGR)[0,0]

    def test_hue_wrap_and_blob_point(self):
        frame=self.frame();self.paint(frame,(40,40,20,20),(179,200,200))
        found=detect_candidates(frame,self.profile())
        self.assertEqual(len(found),1)
        self.assertTrue(40<=found[0].point[0]<60 and 40<=found[0].point[1]<60)

    def test_masks_and_world_exclude_ui_after_morphology(self):
        frame=self.frame();self.paint(frame,(40,40,20,20));self.paint(frame,(140,140,20,20))
        profile=replace(self.profile(),region=Region(0,0,.5,.5),masks=(Region(.15,.15,.35,.35),))
        self.assertEqual(detect_candidates(frame,profile),[])

    def test_minimum_and_maximum_area(self):
        frame=self.frame();self.paint(frame,(10,10,2,2));self.paint(frame,(50,50,80,80))
        self.assertEqual(detect_candidates(frame,self.profile()),[])

    def test_color_picker_reads_body(self):
        frame=self.frame();self.paint(frame,(40,40,20,20),(100,170,180))
        h,s,v=pick_color(frame,50,50)
        self.assertEqual(h,100);self.assertLessEqual(abs(s-170),1);self.assertEqual(v,180)

    def test_minimum_dimensions_reject_small_objects_even_with_enough_area(self):
        frame=self.frame();self.paint(frame,(30,30,11,10));self.paint(frame,(90,90,30,35))
        p=replace(self.profile(),color=replace(self.profile().color,min_width=24,min_height=24))
        found=detect_candidates(frame,p)
        self.assertEqual(len(found),1)
        self.assertEqual((found[0].width,found[0].height),(30,35))

    def test_click_point_on_detected_pixel_for_hollow_shape(self):
        frame=self.frame();self.paint(frame,(40,40,20,20));frame[45:55,45:55]=0
        found=detect_candidates(frame,self.profile())
        x,y=found[0].point
        self.assertTrue(np.any(frame[y,x]))

    def test_stable_candidates_and_stale_gap(self):
        tracker=CandidateTracker(3,.25)
        c=Candidate(30,40,10,10,(35,45))
        self.assertEqual(tracker.update([c],0),[])
        self.assertEqual(tracker.update([c],.25),[])
        self.assertEqual(tracker.update([c],.5),[c])
        self.assertEqual(tracker.update([c],3),[])

    def test_matching_panel_inside_roi_only(self):
        rng=np.random.default_rng(1);template=rng.integers(0,255,(12,20,3),dtype=np.uint8)
        frame=self.frame();frame[20:32,40:60]=template
        self.assertGreater(panel_score(frame,template,Region(.1,.05,.4,.25)),.99)
        self.assertLess(panel_score(frame,template,Region(.5,.5,.9,.9)),.5)

    def test_invalid_template_rejected(self):
        for image in [np.zeros((10,10,3),np.uint8),np.ones((2,20,3),np.uint8)]:
            with self.assertRaises(ValueError):validate_template(image)

    def test_empty_frame_rejected(self):
        with self.assertRaises(ValueError):detect_candidates(np.zeros((0,0,3),np.uint8),self.profile())
