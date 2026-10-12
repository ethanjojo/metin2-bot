import threading
import time
import unittest

import numpy as np

from metin_bot.worker import Worker
from fixtures import calibrated, scene
from metin_bot.windows import GameWindow, WindowGeometry
from test_windows import FakeAPI


class WorkerAPI(FakeAPI):
    def __init__(self):
        super().__init__()
        self.entered = threading.Event()
        self.released = threading.Event()
        self.captured = threading.Event()
        self.start_requested = threading.Event()
        self.registered = False
        self.focused = True
        self.profile, self.template = calibrated()

    def start_hotkeys(self):
        self.registered = True

    def close_hotkeys(self):
        self.registered = False

    def hotkey_events(self):
        if self.start_requested.is_set():
            self.start_requested.clear()
            return (True, False)
        return (False, False)

    def inspect(self, window):
        return WindowGeometry(0, 0, 800, 600, True, self.focused)

    def capture(self, window):
        self.captured.set()
        return scene()

    def key_held(self, key):
        return super().key_held(key)

    def send_mouse(self, window, point, down):
        super().send_mouse(window, point, down)
        if down:
            self.entered.set()
        else:
            self.released.set()



class WorkerTests(unittest.TestCase):
    def setUp(self):
        # Previous UI tests create disposable Tcl interpreters. Finalize their
        # unreachable callback cycles on the GUI thread before workers run GC.
        import gc
        gc.collect()

    def test_skill_timer_reset_is_identity_bound_and_does_not_reset_statistics(self):
        from dataclasses import replace
        from metin_bot.config import BuffConfig,BuffSkill
        api=WorkerAPI();api.profile=replace(api.profile,buffs=BuffConfig(enabled=True,
            skills=(BuffSkill('3',interval_seconds=10),BuffSkill('4',interval_seconds=30))))
        worker=Worker(api);window=GameWindow(1,2,'Game')
        def after(requested):
            deadline=time.monotonic()+3
            while time.monotonic()<deadline:
                s=worker.results.get(timeout=3)
                if s.captured_at>=requested:return s
            self.fail('No fresh snapshot')
        try:
            worker.configure(api.profile,window,api.template);after(0)
            requested=time.monotonic();worker.reset_skill_timer(api.profile,GameWindow(1,99,'Wrong PID'),0)
            self.assertEqual(after(requested).buff_skill_seconds,(None,None))
            requested=time.monotonic();worker.reset_skill_timer(api.profile,window,0)
            sample=after(requested)
            self.assertEqual(sample.buff_skill_seconds,(0,None))
            self.assertFalse(sample.running);self.assertEqual(sample.statistics.clicks,0)
            changed=replace(api.profile,buffs=replace(api.profile.buffs,skills=tuple(reversed(api.profile.buffs.skills))))
            requested=time.monotonic();worker.reset_skill_timer(changed,window,1)
            self.assertEqual(after(requested).buff_skill_seconds,(0,None))
        finally:
            worker.cancel_event.set();worker.shutdown.set();worker.thread.join(2)

    def test_mount_confirmation_rejects_stale_window_and_preserves_timer(self):
        from metin_bot.config import BuffConfig,BuffSkill
        from dataclasses import replace
        api=WorkerAPI()
        api.profile=replace(api.profile,buffs=BuffConfig(enabled=True,skills=(BuffSkill('F1',interval_seconds=10),)))
        worker=Worker(api);window=GameWindow(1,2,'Game')
        def matching(predicate):
            deadline=time.monotonic()+3
            while time.monotonic()<deadline:
                sample=worker.results.get(timeout=3)
                if predicate(sample):return sample
            self.fail('Expected confirmation sample')
        try:
            worker.configure(api.profile,window,api.template,restore_mounted=True)
            matching(lambda s:s.profile_id is not None)
            requested=time.monotonic()
            stale=worker.confirm_mount(api.profile.id,GameWindow(1,99,'Reused HWND'),False)
            sample=matching(lambda s:s.captured_at>=requested)
            self.assertNotEqual(sample.mount_confirmation_id,stale)
            self.assertTrue(sample.mounted)
            accepted=worker.confirm_mount(api.profile.id,window,False)
            confirmed=matching(lambda s:s.mount_confirmation_id==accepted)
            self.assertFalse(confirmed.mounted)
            self.assertFalse(confirmed.running)
            self.assertEqual(confirmed.buff_seconds,sample.buff_seconds)
            with self.assertRaises(ValueError):worker.confirm_mount(api.profile.id,window,1)
            requested=time.monotonic()
            worker.configure(api.profile,window,api.template,restore_mounted=True)
            unchanged=matching(lambda s:s.captured_at>=requested)
            self.assertFalse(unchanged.mounted)
            worker.configure(api.profile,GameWindow(3,4,'New session'),api.template,restore_mounted=False)
            fresh=matching(lambda s:s.window_hwnd==3)
            self.assertFalse(fresh.mounted)
            self.assertEqual(fresh.mount_confirmation_id,0)
        finally:
            worker.cancel_event.set();worker.shutdown.set();worker.thread.join(2)

    def test_buff_countdown_and_mount_survive_save_refresh_and_disable(self):
        from metin_bot.config import BuffConfig, BuffSkill
        from dataclasses import replace
        api=WorkerAPI()
        api.profile=replace(api.profile,buffs=BuffConfig(enabled=True,fight_mounted=False,
            skills=(BuffSkill('F1',interval_seconds=10),)))
        worker=Worker(api);window=GameWindow(1,2,'Game')
        def after_request(predicate,requested):
            deadline=time.monotonic()+3
            while time.monotonic()<deadline:
                sample=worker.results.get(timeout=3)
                if sample.captured_at>=requested and predicate(sample):return sample
            self.fail('Expected buff state snapshot')
        try:
            worker.configure(api.profile,window,api.template)
            self.assertTrue(api.captured.wait(1))
            requested=time.monotonic();identifier=worker.confirm_mount(api.profile.id,window,True)
            after_request(lambda s:s.mount_confirmation_id==identifier,requested)
            requested=time.monotonic();api.start_requested.set()
            after_request(lambda s:s.mounted is False,requested)
            worker.pause();requested=time.monotonic()
            paused=after_request(lambda s:not s.running,requested)
            self.assertLess(paused.buff_seconds,10)
            changed=replace(api.profile,name='Renamed',buffs=replace(api.profile.buffs,
                skills=(replace(api.profile.buffs.skills[0],name='Strach'),)))
            requested=time.monotonic();worker.configure(changed,window,api.template)
            saved=after_request(lambda s:not s.running,requested)
            self.assertFalse(saved.mounted)
            self.assertAlmostEqual(saved.buff_seconds,paused.buff_seconds,delta=.2)
            worker.clear();requested=time.monotonic();worker.configure(changed,window,api.template)
            refreshed=after_request(lambda s:not s.running,requested)
            self.assertFalse(refreshed.mounted)
            self.assertAlmostEqual(refreshed.buff_seconds,paused.buff_seconds,delta=.2)
            disabled=replace(changed,buffs=replace(changed.buffs,enabled=False))
            requested=time.monotonic();worker.configure(disabled,window,api.template)
            after_request(lambda s:not s.running,requested)
            requested=time.monotonic();worker.configure(changed,window,api.template)
            self.assertFalse(after_request(lambda s:not s.running,requested).mounted)
        finally:
            worker.cancel_event.set();worker.shutdown.set();worker.thread.join(2)

    def test_uncertain_mount_survives_refresh_until_explicit_confirmation(self):
        from metin_bot.config import BuffConfig, BuffSkill
        from dataclasses import replace
        api=WorkerAPI();api.failure=True
        api.profile=replace(api.profile,buffs=BuffConfig(enabled=True,fight_mounted=False,skills=(BuffSkill('F1',interval_seconds=10),)))
        worker=Worker(api);window=GameWindow(1,2,'Game')
        def matching(predicate,requested):
            deadline=time.monotonic()+3
            while time.monotonic()<deadline:
                sample=worker.results.get(timeout=3)
                if sample.captured_at>=requested and predicate(sample):return sample
            self.fail('Expected mount state snapshot')
        try:
            worker.configure(api.profile,window,api.template)
            self.assertTrue(api.captured.wait(1))
            requested=time.monotonic();identifier=worker.confirm_mount(api.profile.id,window,True)
            matching(lambda s:s.mount_confirmation_id==identifier,requested)
            requested=time.monotonic();api.start_requested.set()
            matching(lambda s:s.state=='Paused' and s.mounted is None,requested)
            worker.clear();requested=time.monotonic();worker.configure(api.profile,window,api.template)
            self.assertIsNone(matching(lambda s:not s.running,requested).mounted)
            requested=time.monotonic();identifier=worker.confirm_mount(api.profile.id,window,False)
            confirmed=matching(lambda s:s.mount_confirmation_id==identifier,requested)
            self.assertFalse(confirmed.mounted)
            self.assertFalse(confirmed.running)
            worker.configure(api.profile,window,api.template)
            requested=time.monotonic()
            self.assertFalse(matching(lambda s:not s.running,requested).mounted)
        finally:
            worker.cancel_event.set();worker.shutdown.set();worker.thread.join(2)

    def test_session_survives_save_but_resets_for_new_window(self):
        api=WorkerAPI();worker=Worker(api);window=GameWindow(1,2,'Game')
        def sample_matching(predicate):
            deadline=time.monotonic()+2
            while time.monotonic()<deadline:
                sample=worker.results.get(timeout=2)
                if predicate(sample):return sample
            self.fail('Expected session snapshot was not published')
        try:
            worker.configure(api.profile,window,api.template)
            self.assertTrue(api.captured.wait(1));api.start_requested.set()
            sample_matching(lambda s:s.statistics.clicks==1)
            worker.configure(api.profile,window,api.template)
            sample=sample_matching(lambda s:not s.running)
            self.assertEqual(sample.statistics.clicks,1)
            worker.reset_statistics()
            sample_matching(lambda s:s.statistics.clicks==0)
            worker.configure(api.profile,window,api.template)
            api.start_requested.set()
            sample_matching(lambda s:s.statistics.clicks==1)
            worker.configure(api.profile,GameWindow(9,2,'Other'),api.template)
            sample=sample_matching(lambda s:s.window_hwnd==9)
            self.assertEqual(sample.statistics.clicks,0)
        finally:
            worker.cancel_event.set();worker.shutdown.set();worker.thread.join(2)

    def test_armed_start_waits_for_stable_foreground(self):
        api = WorkerAPI()
        api.focused = False
        worker = Worker(api)
        try:
            worker.configure(api.profile, GameWindow(1, 2, 'Game'), api.template)
            self.assertTrue(api.captured.wait(1))
            worker.arm()
            time.sleep(3.1)
            api.focused = True
            self.assertFalse(api.entered.wait(.2), 'Started while foreground was still settling')
            self.assertTrue(api.entered.wait(2))
        finally:
            worker.cancel_event.set()
            worker.shutdown.set()
            worker.thread.join(2)

    def test_close_during_input_terminates_and_releases(self):
        api = WorkerAPI()
        worker = Worker(api)
        try:
            worker.configure(api.profile, GameWindow(1, 2, 'Game'), api.template)
            self.assertTrue(api.captured.wait(1))
            api.start_requested.set()
            self.assertTrue(api.entered.wait(2), 'Worker did not enter an input action')
            worker.cancel_event.set()
            worker.shutdown.set()
            worker.thread.join(1)
            self.assertFalse(worker.thread.is_alive())
            self.assertTrue(api.released.is_set())
            self.assertFalse(api.registered)
        finally:
            worker.cancel_event.set()
            worker.shutdown.set()
            worker.thread.join(2)

    def test_clearing_window_detaches_runtime_before_f7(self):
        api=WorkerAPI();worker=Worker(api)
        try:
            worker.configure(api.profile,GameWindow(1,2,'Game'),api.template)
            self.assertTrue(api.captured.wait(1))
            worker.clear()
            time.sleep(.15)
            api.start_requested.set()
            self.assertFalse(api.entered.wait(1))
        finally:
            worker.cancel_event.set();worker.shutdown.set();worker.thread.join(2)

    def test_f8_dominates_simultaneous_f7_event(self):
        api=WorkerAPI()
        def events():
            if api.start_requested.is_set():
                api.start_requested.clear();return True,True
            return False,False
        api.hotkey_events=events
        worker=Worker(api)
        try:
            worker.configure(api.profile,GameWindow(1,2,'Game'),api.template)
            self.assertTrue(api.captured.wait(1));api.start_requested.set()
            self.assertFalse(api.entered.wait(1.5),'F7 restarted after F8')
        finally:
            worker.cancel_event.set();worker.shutdown.set();worker.thread.join(2)
