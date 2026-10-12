import unittest
from dataclasses import replace
from metin_bot.runtime import Runtime
from metin_bot.windows import GameWindow, WindowGeometry
from fixtures import calibrated,scene
from test_windows import FakeAPI


class RuntimeAPI(FakeAPI):
    def __init__(self):
        super().__init__();self.geometry=WindowGeometry(0,0,800,600,True,True)
        self.bad_capture=False;self.panel=False;self.template=None

    def capture(self,window):
        if self.bad_capture:raise RuntimeError('Black frame')
        return scene(panel=self.panel,template=self.template)


class RuntimeTests(unittest.TestCase):
    def test_active_toggle_is_not_sent_during_real_observation_cycle(self):
        import numpy as np
        from metin_bot.config import BuffIcon,BuffConfig,BuffSkill,Region
        r,api=self.runtime()
        icon=np.random.default_rng(10).integers(0,256,(16,16,3),dtype=np.uint8)
        config=BuffConfig(enabled=True,fight_mounted=False,
            icon_region=Region(.6,0,.9,.1),skills=(BuffSkill('F1',.2,'inactive',BuffIcon.capture(icon)),BuffSkill('F2',.2)))
        original=api.capture
        def capture(window):
            frame=original(window);frame[10:26,500:516]=icon;return frame
        api.capture=capture
        r=Runtime(replace(r.profile,buffs=config),r.window,api,r.template);r.controller.buffs.mounted=False;r.driver._wait=lambda *args:None
        r.request_start();actions=[]
        for n in range(62):
            api.panel=n<48
            sample=r.sample(n*.25)
            if sample.action is not None:actions.append(sample.action.shortcut)
        self.assertIn('F2',actions)
        self.assertNotIn('F1',actions)
        self.assertFalse(r.driver.owned)

    def test_full_target_buff_remount_and_next_click_cycle(self):
        from metin_bot.config import BuffConfig, BuffSkill
        r,api=self.runtime()
        p=replace(r.profile,buffs=BuffConfig(enabled=True,mount_wait_seconds=.2,
            skills=(BuffSkill('F1',.2),BuffSkill('2',.2))))
        r=Runtime(p,r.window,api,r.template);r.controller.buffs.mounted=True;r.driver._wait=lambda *args:None
        r.request_start();actions=[]
        for n in range(66):
            api.panel=n<48
            sample=r.sample(n*.25)
            if sample.action is not None:actions.append(sample.action.kind.value)
        self.assertEqual(actions,['mount','skill','skill','mount','click'])
        self.assertTrue(sample.mounted)
        self.assertEqual(sample.statistics.completed,1)
        self.assertEqual(sample.statistics.clicks,1)
        keyboard=[event for event in api.events if event[0]=='key']
        chord=[('key',0x11,True),('key',0x47,True),('key',0x47,False),('key',0x11,False)]
        self.assertEqual(keyboard,chord+[('key',0x70,True),('key',0x70,False),
            ('key',0x32,True),('key',0x32,False)]+chord)

    def test_runtime_sends_mount_keys_and_only_then_tracks_new_state(self):
        from metin_bot.config import BuffConfig, BuffSkill
        from metin_bot.hotkeys import hotkey_keys
        r,api=self.runtime()
        p=replace(r.profile,buffs=BuffConfig(enabled=True,fight_mounted=False,skills=(BuffSkill('F1'),)))
        r=Runtime(p,r.window,api,r.template);r.controller.buffs.mounted=True;r.driver._wait=lambda *args:None
        r.request_start();r.sample(0);r.sample(.25);sample=r.sample(.5)
        self.assertEqual(sample.action.shortcut,'Ctrl+G')
        self.assertFalse(sample.mounted)
        self.assertEqual(api.events,[('key',0x11,True),('key',0x47,True),('key',0x47,False),('key',0x11,False)])

    def test_failed_mount_marks_state_unknown_and_f7_cannot_restart(self):
        from metin_bot.config import BuffConfig, BuffSkill
        r,api=self.runtime()
        p=replace(r.profile,buffs=BuffConfig(enabled=True,fight_mounted=False,skills=(BuffSkill('F1'),)))
        r=Runtime(p,r.window,api,r.template);r.controller.buffs.mounted=True;api.failure=True
        r.request_start();r.sample(0);r.sample(.25);sample=r.sample(.5)
        self.assertFalse(sample.running);self.assertIsNone(sample.mounted)
        events=list(api.events);r.request_start();sample=r.sample(.75)
        self.assertFalse(sample.running);self.assertEqual(api.events,events)

    def test_rejected_window_key_message_pauses_before_skills(self):
        from metin_bot.config import BuffConfig,BuffSkill
        r,api=self.runtime()
        p=replace(r.profile,buffs=BuffConfig(enabled=True,fight_mounted=False,skills=(BuffSkill('F1'),)))
        r=Runtime(p,r.window,api,r.template);r.controller.buffs.mounted=True;r.driver._wait=lambda *args:None
        original=api.send_key
        def dropped(window,key,down,**kwargs):
            if down and key==0x47:raise OSError('PostMessage klávesy odmítnut')
            original(window,key,down,**kwargs)
        api.send_key=dropped
        r.request_start();r.sample(0);r.sample(.25);sample=r.sample(.5)
        self.assertFalse(sample.running);self.assertIsNone(sample.mounted)
        self.assertIn('PostMessage',sample.reason)
        before=list(api.events)
        r.sample(12);r.request_start();r.sample(13)
        self.assertEqual(api.events,before)
        self.assertFalse(r.driver.owned)

    def runtime(self):
        profile,template=calibrated();api=RuntimeAPI();api.template=template
        r=Runtime(profile,GameWindow(1,2,'Game'),api,template)
        return r,api

    def test_invalid_capture_pauses(self):
        r,api=self.runtime();api.bad_capture=True;r.request_start()
        self.assertFalse(r.sample(0).running)
        self.assertIn('frame',r.controller.status.reason)

    def test_real_observations_click_once_then_wait_for_panel(self):
        r,api=self.runtime();r.request_start()
        r.sample(0);r.sample(.25);sample=r.sample(.5)
        self.assertEqual(sample.action.kind.value,'click')
        self.assertEqual(api.events.count(('mouse',True)),1)
        self.assertEqual(sample.statistics.clicks,1)
        api.panel=True
        for t in [.75,1,1.25,1.5]:r.sample(t)
        self.assertEqual(r.controller.status.state,'Target active')
        self.assertEqual(api.events.count(('mouse',True)),1)

    def test_incomplete_profile_cannot_start(self):
        r,api=self.runtime();r.profile=replace(r.profile,color=None);r.request_start()
        self.assertFalse(r.sample(0).running)
        self.assertNotIn(('mouse',True),api.events)

    def test_wrong_calibration_dimensions_pause_before_input(self):
        r,api=self.runtime();r.profile=replace(r.profile,frame_size=(900,600));r.request_start()
        self.assertFalse(r.sample(0).running)
        self.assertNotIn(('mouse',True),api.events)

    def test_geometry_change_pauses(self):
        r,api=self.runtime();r.request_start();r.sample(0)
        api.geometry=replace(api.geometry,left=10)
        self.assertFalse(r.sample(.25).running)

    def test_stop_during_action_releases_button(self):
        r,api=self.runtime()
        original=api.send_mouse
        def stop(window,point,down):
            original(window,point,down)
            if down:r.stop.set()
        api.send_mouse=stop;r.request_start()
        for t in [0,.25,.5]:r.sample(t)
        self.assertFalse(r.controller.running)
        self.assertEqual(r.driver.owned,set())
        self.assertIn(('mouse',False),api.events)
        self.assertEqual(r.session.clicks,1)

    def test_pause_reason_survives_preview_failure(self):
        r,api=self.runtime();r.cancel('F8');api.bad_capture=True
        self.assertEqual(r.sample(0).reason,'F8')

    def test_template_snapshot_is_independent(self):
        profile,template=calibrated();api=RuntimeAPI()
        r=Runtime(profile,GameWindow(1,2,'Game'),api,template)
        original=r.template.copy();template[:]=0
        self.assertTrue((r.template==original).all())

    def test_close_sets_cancellation(self):
        r,api=self.runtime();r.close();self.assertTrue(r.stop.is_set())

    def test_preview_identifies_profile_window_and_capture_time(self):
        r,api=self.runtime();sample=r.sample(42)
        self.assertEqual(sample.profile_id,r.profile.id)
        self.assertEqual(sample.window_hwnd,r.window.hwnd)
        self.assertEqual(sample.captured_at,42)

    def test_stopped_preview_detects_without_input(self):
        r,api=self.runtime()
        for t in [0,.25,.5,1]:sample=r.sample(t)
        self.assertFalse(sample.running)
        self.assertTrue(sample.candidates)
        self.assertFalse(api.events)
