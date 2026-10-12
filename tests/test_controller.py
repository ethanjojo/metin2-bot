import unittest
from dataclasses import replace
from metin_bot.config import Profile, TimingConfig
from metin_bot.controller import Controller, Observation, ActionKind
from metin_bot.vision import Candidate


class ControllerTests(unittest.TestCase):
    def setUp(self):
        self.p=Profile.create('Test');self.c=Controller(self.p);self.target=Candidate(40,50,10,10,(45,55))

    def obs(self,panel=False,candidates=None):
        return Observation(True,tuple([self.target] if candidates is None else candidates),panel)

    def seek(self):
        self.c.start(0)
        self.assertIsNone(self.c.step(0,self.obs()))
        self.assertIsNone(self.c.step(.25,self.obs()))
        return self.c.step(.5,self.obs())

    def test_stopped_and_invalid_observation(self):
        self.assertIsNone(self.c.step(0,self.obs()))
        self.c.start(0);self.c.step(0,Observation(False,(),False,'bad capture'))
        self.assertFalse(self.c.running);self.assertIn('bad capture',self.c.status.reason)

    def test_click_once_and_missing_first_panel_not_completion(self):
        action=self.seek();self.assertEqual(action.kind,ActionKind.CLICK);self.assertEqual(action.point,(45,55))
        for t in [1,2,3,4]:self.assertIsNone(self.c.step(t,self.obs()))
        self.assertTrue(self.c.running)

    def test_initial_active_panel_blocks_new_click(self):
        self.c.start(0)
        for t in [0,.25,.5,1,2]:self.assertIsNone(self.c.step(t,self.obs(True)))
        self.assertEqual(self.c.status.state,'Target active')

    def test_panel_confirmation_then_debounced_loss(self):
        self.seek()
        for t in [.75,1,1.25]:self.assertIsNone(self.c.step(t,self.obs(True)))
        self.assertEqual(self.c.status.state,'Target active')
        self.assertIsNone(self.c.step(1.5,self.obs(False)))
        self.c.step(1.75,self.obs(True))
        self.assertIsNone(self.c.step(2,self.obs(False)))
        self.assertIsNone(self.c.step(2.25,self.obs(False)))
        self.assertIsNone(self.c.step(2.5,self.obs(False)))
        self.assertIsNone(self.c.step(3,self.obs(False)))
        action=self.c.step(3.5,self.obs(False))
        self.assertEqual(action.kind,ActionKind.CLICK)

    def test_attack_timeout_never_resets_on_candidates(self):
        self.seek()
        for t in [.75,1,1.25]:self.c.step(t,self.obs(True))
        self.c.step(122,self.obs(True))
        self.assertFalse(self.c.running);self.assertIn('timed out',self.c.status.reason)

    def test_acquisition_timeout_suppresses_same_candidate(self):
        self.seek();self.assertIsNone(self.c.step(21,self.obs(False)))
        self.assertIsNone(self.c.step(21.25,self.obs(False)))
        other=Candidate(100,100,10,10,(105,105))
        action=self.c.step(21.5,self.obs(False,[other,self.target]))
        self.assertEqual(action.point,other.point)

    def test_search_and_rotation_limit(self):
        self.c.start(0)
        for t in [0,.25,.5]:self.c.step(t,self.obs(False,[]))
        for t in [6,12,18,24]:
            for preceding in [t-.5,t-.25]:self.c.step(preceding,self.obs(False,[]))
            self.assertEqual(self.c.step(t,self.obs(False,[])).kind,ActionKind.ROTATE)
            self.assertIsNone(self.c.step(t+1,self.obs(False,[])))
        for t in [29.5,29.75]:self.c.step(t,self.obs(False,[]))
        self.assertIsNone(self.c.step(30,self.obs(False,[])))
        self.assertFalse(self.c.running)

    def test_sparse_absence_is_not_completion(self):
        self.seek()
        for t in [.75,1,1.25]:self.c.step(t,self.obs(True))
        for t in [4,8,12]:self.assertIsNone(self.c.step(t,self.obs(False)))
        self.assertEqual(self.c.status.state,'Target active')

    def test_pause_prevents_actions(self):
        self.seek();self.c.pause('stop')
        self.assertIsNone(self.c.step(100,self.obs()))

    def test_completion_counts_only_confirmed_active_panel_loss_once(self):
        self.seek();self.c.step(.75,self.obs(False))
        self.assertEqual(self.c.session.completed,0)
        for t in [1,1.25,1.5]:self.c.step(t,self.obs(True))
        for t in [1.75,2,2.25,2.5,2.75]:self.c.step(t,self.obs(False))
        self.assertEqual(self.c.session.completed,1)

    def test_acquisition_failure_counts_and_remaining_matches_limit(self):
        self.seek()
        self.assertAlmostEqual(self.c.remaining(1),self.p.timing.acquire_timeout-.5)
        self.c.step(21,self.obs(False))
        self.assertEqual(self.c.session.failed,1)
        self.c.pause('stop');self.assertIsNone(self.c.remaining(30))

    def test_unstable_panel_blocks_click_and_eventually_pauses(self):
        self.c.start(0)
        for i in range(90):
            self.assertIsNone(self.c.step(i*.25,self.obs(bool(i%2))))
        self.assertFalse(self.c.running)

    def test_seeking_requires_confirmed_absence_after_panel_flicker(self):
        self.c.start(0)
        for t in [0,.25,.5]:self.c.step(t,self.obs(False,[]))
        self.c.step(.75,self.obs(True,[]))
        self.assertIsNone(self.c.step(1,self.obs(False)))
        self.assertIsNone(self.c.step(1.25,self.obs(False)))
        self.assertEqual(self.c.step(1.5,self.obs(False)).kind,ActionKind.CLICK)

    def test_seeking_does_not_rotate_on_transient_absence(self):
        self.c.start(0)
        for i in range(20):self.c.step(i*.25,self.obs(False,[]))
        self.c.step(5,self.obs(True,[]))
        self.assertIsNone(self.c.step(5.25,self.obs(False,[])))
        self.assertIsNone(self.c.step(5.5,self.obs(False,[])))
        self.assertEqual(self.c.step(5.75,self.obs(False,[])).kind,ActionKind.ROTATE)
