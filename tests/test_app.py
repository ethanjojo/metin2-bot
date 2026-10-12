import unittest
from dataclasses import replace
from pathlib import Path
from types import SimpleNamespace
from unittest.mock import patch, Mock

from metin_bot.app import App
from metin_bot.windows import GameWindow, SafetyError
from fixtures import calibrated
from metin_bot.calibration import Calibration
from metin_bot.theme import UISettings


class AppStartTests(unittest.TestCase):
    def test_statistics_reset_preserves_step_countdown_and_skill_snapshot(self):
        from metin_bot.session import Statistics
        app=self.app();app.worker.reset_statistics=Mock()
        app.status=SimpleNamespace(get=lambda:'Target active')
        app.last_statistics=Statistics(60,4,5,1)
        app.last_buff_sample=SimpleNamespace(remaining_seconds=42,buff_skill_seconds=(600,))
        snapshot=app.last_buff_sample
        app.reset_statistics()
        app.run_timer.set.assert_called_with('42.0 s')
        self.assertIs(app.last_buff_sample,snapshot)
        app.worker.reset_statistics.assert_called_once()

    def test_reset_skill_timer_uses_saved_config_and_rejects_draft(self):
        app=self.app();app.saved_profile=app.calibration.profile
        app.settings_profile=lambda:app.saved_profile
        app.selected_window=lambda:app.windows[0]
        app.last_buff_sample=SimpleNamespace(profile_id=app.saved_profile.id,window_hwnd=1)
        app.worker.reset_skill_timer=Mock()
        app.reset_skill_timer(0)
        app.worker.reset_skill_timer.assert_called_once_with(app.saved_profile,app.windows[0],0)
        app.worker.reset_skill_timer.reset_mock()
        app.settings_profile=lambda:replace(app.saved_profile,buffs=replace(app.saved_profile.buffs,fight_mounted=False))
        app.reset_skill_timer(0);app.worker.reset_skill_timer.assert_not_called()
        self.assertIn('Save',app.reason.set.call_args.args[0])

    def test_reuse_rejects_missing_or_outside_source_region_without_mutation(self):
        import numpy as np
        from metin_bot.config import BuffIcon,Region
        image=np.random.default_rng(4).integers(0,256,(25,25,3),dtype=np.uint8)
        for source_region in (None,Region(.7,.7,.8,.8)):
            app=self.app();app.render=Mock();app.update_ready=Mock();app.mark_dirty=Mock()
            app.last_frame=np.random.default_rng(8).integers(0,256,(600,800,3),dtype=np.uint8)
            app.calibration.frame=app.last_frame.copy()
            app.calibration.profile=replace(app.calibration.profile,buffs=replace(app.calibration.profile.buffs,icon_region=Region(0,0,.2,.2)))
            app.buff_list=SimpleNamespace(rows=[SimpleNamespace(icon=None,inactive_icon=BuffIcon.capture(image,source_region))],set_icon=Mock())
            app.begin_calibration=Mock()
            app.begin_buff_pick(0,True,reuse=True)
            app.buff_list.set_icon.assert_not_called();app.mark_dirty.assert_not_called()
            self.assertIsNone(app.mode)

    def test_reusing_other_icon_region_captures_current_pixels_and_roundtrips(self):
        import numpy as np
        from metin_bot.config import BuffIcon,BuffSkill,Region,Profile
        from metin_bot.vision import crop
        app=self.app();app.render=Mock();app.update_ready=Mock();app.mark_dirty=Mock()
        app.last_frame=np.random.default_rng(8).integers(0,256,(600,800,3),dtype=np.uint8)
        app.calibration.frame=app.last_frame.copy()
        region=Region(.02,.02,.05,.06)
        icon=BuffIcon.capture(np.random.default_rng(4).integers(0,256,(25,25,3),dtype=np.uint8),region)
        row=SimpleNamespace(icon=None,inactive_icon=icon)
        app.buff_list=SimpleNamespace(rows=[row],set_icon=Mock())
        app.calibration.profile=replace(app.calibration.profile,buffs=replace(app.calibration.profile.buffs,icon_region=Region(0,0,.2,.2)))
        app.begin_calibration=Mock()
        app.begin_buff_pick(0,True,reuse=True)
        args=app.buff_list.set_icon.call_args.args
        self.assertEqual((args[0],args[2]),(0,True))
        self.assertEqual(args[1].region,region)
        np.testing.assert_array_equal(args[1].decode(),crop(app.last_frame,region))
        profile=replace(app.calibration.profile,buffs=replace(app.calibration.profile.buffs,skills=(BuffSkill('2',mode='inactive',icon=args[1],inactive_icon=icon),)))
        self.assertEqual(Profile.from_dict(profile.to_dict()),profile)
        self.assertIsNone(app.buff_pick)

    def setUp(self):
        settings_save=patch.object(UISettings,'save')
        settings_save.start();self.addCleanup(settings_save.stop)
        dialog = patch('metin_bot.app.messagebox.showerror')
        dialog.start()
        self.addCleanup(dialog.stop)

    def app(self, activation_error=None):
        app = App.__new__(App)
        app.data_directory = Path('isolated-test-data')
        app.ui_settings=UISettings()
        app.windows = [GameWindow(1, 2, 'Game')]
        app.window_choice = SimpleNamespace(current=lambda: 0)
        profile, template = calibrated()
        app.calibration = Calibration(profile,template)
        app.closing=False;app.dirty=False
        app.sync_dirty=Mock()
        app.update_history_buttons=Mock()
        app.update_editor=Mock()
        app.update_run_controls=Mock();app.update_preview_status=Mock()
        app.render=Mock()
        app.running=False;app.last_capture_at=None
        app.buff_pick=None;app.hover_mode=None;app.step_buttons={}
        app.preview_after=None;app.preview_busy=False
        app.snapshot_button=Mock()
        app.pending_mount_confirmation=None
        app.mount_choice=Mock();app.mount_switch=Mock();app.mount_status=Mock();app.buff_list=Mock()
        app.update_buff_status=Mock()
        app.status=Mock();app.reason=Mock()
        from metin_bot.editor_tools import History, Viewport
        app.history=History(app.calibration);app.viewport=Viewport()
        app.dirty_text=SimpleNamespace(set=Mock())
        app.run_title=Mock();app.run_timer=Mock();app.run_timer_caption=Mock()
        app.metric_values={key:Mock() for key in ('time','completed','clicks','failed')}
        app.refresh_profiles=Mock()
        self.events = []
        app.save = lambda: self.events.append('save') or True
        def activate(window):
            self.events.append(('activate', window))
            if activation_error:
                raise activation_error
        app.api = SimpleNamespace(request_activation=activate)
        app.worker = SimpleNamespace(arm=lambda: self.events.append('arm'), pause=lambda: self.events.append('pause'))
        return app

    def test_start_activates_selected_game_before_arming(self):
        app = self.app()
        app.arm()
        self.assertEqual(self.events, ['save', ('activate', app.windows[0]), 'arm'])

    def test_buff_picker_delete_and_escape_preserve_world_calibration(self):
        from metin_bot.calibration import Mode
        app=self.app();original=app.calibration.profile
        app.mode=Mode.WORLD;app.buff_pick=0
        app.delete_selection()
        self.assertEqual(app.calibration.profile,original)
        app.draw_regions=Mock();app.gesture=Mock();app.edit_preview=Mock();app.force_new=True
        app.cancel_drag()
        self.assertIsNone(app.buff_pick);self.assertIsNone(app.mode)
        self.assertEqual(app.calibration.profile,original)

    def test_buff_area_is_independent_and_movable(self):
        import numpy as np
        from metin_bot.calibration import Mode
        from metin_bot.config import Region
        app=self.app();app.calibration.frame=np.zeros((600,800,3),dtype=np.uint8)
        original=app.calibration.profile.region
        area=Region(.01,.01,.2,.1)
        app.calibration.set_region(Mode.BUFF_AREA,area)
        self.assertEqual(app.calibration.region(Mode.BUFF_AREA),area)
        self.assertEqual(app.calibration.profile.region,original)

    def test_resize_clears_ui_buff_icons_before_dirty_profile_is_read(self):
        import numpy as np
        app=self.app();app.buff_list=Mock();app.mark_dirty=Mock()
        w,h=app.calibration.profile.frame_size
        app.freeze_frame(np.zeros((h+1,w+1,3),dtype=np.uint8))
        app.buff_list.clear_icons.assert_called_once()

    def test_icon_picker_checks_calibration_after_freezing_resized_frame(self):
        from metin_bot.config import Region
        for active in (True,False):
            app=self.app();app.last_frame=Mock();app.render=Mock()
            app.calibration.profile=replace(app.calibration.profile,buffs=replace(app.calibration.profile.buffs,icon_region=Region(0,0,.5,.2)))
            app.buff_list=SimpleNamespace(rows=[SimpleNamespace(icon=Mock())])
            def resized(mode):
                app.mode=mode
                app.calibration.profile=replace(app.calibration.profile,buffs=replace(app.calibration.profile.buffs,icon_region=None))
                app.buff_list.rows[0].icon=None
            app.begin_calibration=resized
            app.begin_buff_pick(0,active)
            self.assertIsNone(app.buff_pick)
            self.assertIsNone(app.mode)

    def test_denied_activation_never_arms(self):
        app = self.app(SafetyError('Aktivace odmítnuta'))
        with patch('metin_bot.app.messagebox.showerror') as dialog:
            app.arm()
        self.assertNotIn('arm', self.events)
        self.assertIn('pause', self.events)
        dialog.assert_called_once()

    def test_incomplete_profile_never_activates_or_arms(self):
        from metin_bot.config import Profile
        app=self.app()
        app.calibration=Calibration(Profile.create('Incomplete'))
        app.arm()
        self.assertEqual(self.events,['save'])

    def test_missing_window_never_saves_or_arms(self):
        app=self.app();app.window_choice=SimpleNamespace(current=lambda:-1)
        app.arm();self.assertEqual(self.events,[])

    def test_snapshot_loading_ends_on_success_timeout_and_activation_error(self):
        import numpy as np
        for outcome in ('success','timeout','activation_error'):
            with self.subTest(outcome=outcome):
                app=self.app(SafetyError('Denied') if outcome=='activation_error' else None)
                app.root=Mock();app.freeze_frame=Mock();app.update_ready=Mock();app.render=Mock()
                app.refresh_preview()
                if outcome=='activation_error':
                    self.assertFalse(app.preview_busy)
                else:
                    self.assertTrue(app.preview_busy)
                    app.last_frame=np.zeros((5,5,3),dtype=np.uint8) if outcome=='success' else None
                    app.preview_deadline=0
                    app.finish_preview()
                    self.assertFalse(app.preview_busy)
                app.snapshot_button.set_loading.assert_called_with(False)

    def test_snapshot_cancel_clears_callback_and_allows_another_request(self):
        app=self.app();app.root=Mock()
        app.refresh_preview();callback=app.preview_after
        app.refresh_preview()
        self.assertEqual(sum(isinstance(event,tuple) for event in self.events),1)
        app.cancel_preview()
        app.root.after_cancel.assert_called_with(callback)
        self.assertFalse(app.preview_busy);self.assertIsNone(app.preview_after)
        app.finish_preview()
        app.refresh_preview()
        self.assertTrue(app.preview_busy)

    def test_mount_confirmation_applies_without_saving_or_marking_profile_dirty(self):
        app=self.interactive();app.saved_profile=app.calibration.profile
        app.saved_template=app.calibration.template.copy()
        app.settings_profile=lambda:app.calibration.profile
        app.sync_dirty=App.sync_dirty.__get__(app)
        app.worker.confirm_mount=Mock(return_value=9)
        app.mount_choice.get.return_value=False
        app.edit_mount_state()
        self.assertEqual(app.pending_mount_confirmation,9)
        app.worker.confirm_mount.assert_called_once_with(app.calibration.profile.id,app.windows[0],False)
        self.assertFalse(app.dirty)
        self.assertNotIn('save',self.events)

    def test_mount_can_be_prepared_without_a_window_or_worker_command(self):
        app=self.app();app.saved_profile=app.calibration.profile
        app.settings_profile=lambda:app.saved_profile
        app.buff_fields={'enabled':SimpleNamespace(get=lambda:False)}
        app.window_choice=SimpleNamespace(current=lambda:-1)
        app.worker.confirm_mount=Mock()
        app.update_buff_status=App.update_buff_status.__get__(app)
        # An old attached-window sample must not overwrite an offline declaration.
        app.last_buff_sample=SimpleNamespace(buff_skill_seconds=(),buff_states=(),running=False,mounted=False)
        for mounted in (True,False):
            app.mount_choice.get.return_value=mounted
            app.edit_mount_state()
            self.assertEqual(app.ui_settings.last_mounted,mounted)
            app.mount_status.set.assert_called_with('On' if mounted else 'Off')
            app.mount_switch.state.assert_called_with(['!alternate','!disabled'])
        app.worker.confirm_mount.assert_not_called()
        self.assertIsNone(app.pending_mount_confirmation)
        self.assertFalse(app.dirty)

    def test_buff_status_uses_draft_even_when_old_worker_samples_arrive(self):
        app=self.app();saved=app.calibration.profile
        from metin_bot.config import BuffSkill
        draft=replace(saved,buffs=replace(saved.buffs,enabled=True,skills=(BuffSkill('F1'),)))
        app.saved_profile=saved;app.settings_profile=lambda:draft
        app.buff_fields={'enabled':SimpleNamespace(get=lambda:True)}
        app.last_buff_sample=SimpleNamespace(state='Stopped',buff_skill_seconds=(),buff_states=(),running=True,buff_seconds=None,mounted=None)
        App.update_buff_status(app)
        self.assertTrue(app.buff_list.update_timers.call_args.kwargs['dirty'])
        app.saved_profile=draft
        app.last_buff_sample=SimpleNamespace(state='Searching',buff_skill_seconds=(),buff_states=(),running=True,buff_seconds=42,mounted=False)
        App.update_buff_status(app)
        self.assertFalse(app.buff_list.update_timers.call_args.kwargs['dirty'])
        app.mount_status.set.assert_called_with('Off')
        draft=replace(draft,buffs=replace(draft.buffs,enabled=False))
        app.buff_fields['enabled']=SimpleNamespace(get=lambda:False)
        App.update_buff_status(app)
        self.assertTrue(app.buff_list.update_timers.call_args.kwargs['dirty'])

    def test_toggle_only_details_are_forwarded_to_skill_rows(self):
        from metin_bot.config import BuffSkill
        app=self.app()
        app.saved_profile=replace(app.calibration.profile,buffs=replace(app.calibration.profile.buffs,
                               enabled=True,skills=(BuffSkill('2',mode='inactive'),)))
        app.settings_profile=lambda:app.saved_profile
        app.buff_fields={'enabled':SimpleNamespace(get=lambda:True)}
        app.last_buff_sample=SimpleNamespace(state='Searching',buff_skill_seconds=(),buff_states=(),running=True,buff_seconds=None,mounted=False)
        App.update_buff_status(app)
        self.assertEqual(app.buff_list.update_timers.call_args.kwargs['states'],())

    def test_mount_confirmation_has_own_feedback_independent_of_buff_toggle(self):
        app=self.app();app.saved_profile=app.calibration.profile
        app.settings_profile=lambda:app.saved_profile
        app.buff_fields={'enabled':SimpleNamespace(get=lambda:False)}
        app.last_buff_sample=None;app.pending_mount_confirmation=3
        App.update_buff_status(app)
        app.mount_status.set.assert_called_with('Off')
        app.pending_mount_confirmation=None
        App.update_buff_status(app)
        app.mount_status.set.assert_called_with('Off')

    def test_known_mount_samples_are_remembered_but_unknown_keeps_last_choice(self):
        app=self.app();app.saved_profile=app.calibration.profile
        app.settings_profile=lambda:app.saved_profile
        app.buff_fields={'enabled':SimpleNamespace(get=lambda:False)}
        def sample(mounted):
            return SimpleNamespace(buff_skill_seconds=(),buff_states=(),running=False,mounted=mounted)
        app.last_buff_sample=sample(True)
        App.update_buff_status(app)
        self.assertTrue(app.ui_settings.last_mounted)
        app.mount_status.set.assert_called_with('On')
        app.last_buff_sample=sample(None)
        App.update_buff_status(app)
        app.mount_choice.set.assert_called_with(1)
        app.mount_status.set.assert_called_with('On')
        self.assertFalse(app.dirty)

    def test_run_toggle_pauses_active_or_armed_start_without_saving(self):
        app=self.app();app.status=SimpleNamespace(get=lambda:'Armed')
        app.toggle_run();self.assertEqual(self.events,['pause'])
        self.events.clear();app.running=True;app.status=SimpleNamespace(get=lambda:'Target active')
        app.toggle_run();self.assertEqual(self.events,['pause'])

    def test_run_toggle_starts_stopped_bot_via_existing_guarded_arm(self):
        app=self.app();app.status=SimpleNamespace(get=lambda:'Paused')
        app.toggle_run();self.assertEqual(self.events,['save',('activate',app.windows[0]),'arm'])

    def test_zoom_buttons_anchor_canvas_center_and_block_during_edit(self):
        app=self.interactive();app.last_frame=app.calibration.frame;app.pan_start=None
        app.canvas.winfo_width=lambda:800;app.canvas.winfo_height=lambda:600
        before=app.viewport.to_image((400,300))
        app.change_zoom(1.25)
        self.assertEqual(app.viewport.to_image((400,300)),before)
        app.gesture=object();app.change_zoom(1.25)
        self.assertEqual(app.viewport.magnification,1.25)

    def test_window_refresh_without_match_clears_running_and_armed_status(self):
        from unittest.mock import MagicMock
        from metin_bot.session import Statistics
        app=self.app();app.running=True;app.last_statistics=Statistics()
        app.worker.clear=Mock();app.api.list_windows=lambda:[]
        app.window_choice=MagicMock();app.window_choice.current.return_value=-1
        app.render=Mock()
        app.refresh_windows()
        self.assertFalse(app.running)
        app.status.set.assert_called_with('Stopped')
        app.run_title.set.assert_called_with('Stopped')
        app.update_run_controls.assert_called_once()

    def test_profile_switch_pauses_and_discards_template_and_frame(self):
        from unittest.mock import Mock
        from metin_bot.config import Profile
        app=self.app();new=Profile.create('Second')
        app.profiles=[new];app.profile_choice=SimpleNamespace(current=lambda:0)
        app.store=SimpleNamespace(load=lambda identifier:(new,None),select=Mock())
        app.last_frame=object();app.frozen=True;app.mode=object();app.drag_start=(1,2)
        app.history=__import__('metin_bot.editor_tools',fromlist=['History']).History(app.calibration)
        app.load_fields=Mock();app.refresh_windows=Mock();app.render=Mock();app.reason=SimpleNamespace(set=Mock())
        app.select_profile()
        self.assertEqual(self.events,['pause'])
        self.assertIsNone(app.calibration.template)
        self.assertIsNone(app.last_frame)
        self.assertFalse(app.frozen)
        app.refresh_windows.assert_called_once()

    def test_poll_rejects_preview_from_previous_window(self):
        import queue
        import numpy as np
        from unittest.mock import Mock
        from metin_bot.runtime import Sample
        app=self.app();app.closing=False;app.root=SimpleNamespace(after=Mock())
        app.worker.results=queue.Queue();app.worker.thread=SimpleNamespace(is_alive=lambda:True)
        sample=Sample(np.zeros((100,100,3),dtype=np.uint8),[],0,None,'old','old',False,
                      profile_id=app.calibration.profile.id,window_hwnd=999,captured_at=10)
        app.worker.results.put(sample)
        app.status=SimpleNamespace(set=Mock());app.reason=SimpleNamespace(set=Mock())
        app.last_frame=None;app.frozen=False;app.render=Mock()
        app.poll()
        self.assertIsNone(app.last_frame);app.status.set.assert_not_called()
        app.worker.results.put(replace(sample,frame=None))
        app.poll();app.status.set.assert_not_called()

    def test_fatal_worker_error_preserves_frozen_frame_but_updates_reason(self):
        import queue
        from metin_bot.runtime import Sample
        app=self.app();app.frozen=True;app.last_frame=object();before=app.last_frame
        app.root=SimpleNamespace(after=Mock());app.info=Mock();app.render=Mock()
        app.worker.thread=SimpleNamespace(is_alive=lambda:False);app.worker.results=queue.Queue()
        app.worker.results.put(Sample(None,[],0,None,'Error','Záznam diagnostiky nelze otevřít.',False))
        app.poll()
        self.assertIs(app.last_frame,before)
        app.reason.set.assert_called_with('Záznam diagnostiky nelze otevřít.')
        app.status.set.assert_called_with('Error')
        app.render.assert_not_called()

    def interactive(self):
        import numpy as np
        from metin_bot.calibration import Mode
        app=self.app();app.calibration.freeze(np.zeros((600,800,3),np.uint8))
        app.mode=Mode.WORLD;app.mask_index=None;app.force_new=False
        app.viewport.fit((800,600),(800,600));app.preview_size=(800,600);app.gesture=None;app.edit_preview=None;app.dirty=False
        app.canvas=SimpleNamespace(focus_set=Mock(),configure=Mock())
        app.sync_dirty=Mock(side_effect=lambda:setattr(app,'dirty',True))
        app.draw_regions=Mock();app.render=Mock();app.render_template=Mock();app.update_ready=Mock()
        app.reason=SimpleNamespace(set=Mock())
        return app

    def test_canvas_drag_moves_existing_area_instead_of_replacing_it(self):
        app=self.interactive();before=app.calibration.profile.region
        app.drag_begin(SimpleNamespace(x=300,y=300))
        self.assertEqual(app.gesture.handle,'move')
        app.drag_move(SimpleNamespace(x=350,y=330))
        self.assertEqual(app.calibration.profile.region,before,'Preview must not commit')
        app.drag_end(SimpleNamespace(x=350,y=330))
        after=app.calibration.profile.region
        self.assertAlmostEqual(after.right-after.left,before.right-before.left)
        self.assertAlmostEqual(after.left,before.left+50/800)
        self.assertTrue(app.dirty)

    def test_escape_discards_preview_without_changing_profile(self):
        app=self.interactive();before=app.calibration.profile
        app.drag_begin(SimpleNamespace(x=300,y=300));app.drag_move(SimpleNamespace(x=350,y=330))
        app.cancel_drag()
        self.assertEqual(app.calibration.profile,before)
        self.assertIsNone(app.gesture);self.assertIsNone(app.edit_preview)

    def test_resizing_selected_mask_does_not_append_duplicate(self):
        from metin_bot.calibration import Mode
        from metin_bot.config import Region
        from dataclasses import replace
        app=self.interactive();app.mode=Mode.MASK;app.mask_index=0
        app.calibration.profile=replace(app.calibration.profile,masks=(Region(.2,.2,.5,.5),))
        app.drag_begin(SimpleNamespace(x=400,y=300));app.drag_end(SimpleNamespace(x=440,y=330))
        self.assertEqual(len(app.calibration.profile.masks),1)
        self.assertEqual(app.calibration.profile.masks[0],Region(.2,.2,.55,.55))

    def test_new_mask_draws_preview_and_commits_without_index(self):
        from metin_bot.calibration import Mode
        app=self.interactive();app.mode=Mode.MASK;app.frozen=True
        app.canvas=SimpleNamespace(focus_set=Mock(),delete=Mock(),create_rectangle=Mock(),
                                   create_text=Mock(return_value=1),bbox=Mock(return_value=(0,0,50,15)),tag_lower=Mock())
        app.draw_regions=App.draw_regions.__get__(app)
        app.drag_begin(SimpleNamespace(x=500,y=300))
        app.drag_move(SimpleNamespace(x=650,y=400))
        self.assertEqual(len(app.calibration.profile.masks),0)
        app.drag_end(SimpleNamespace(x=650,y=400))
        self.assertEqual(len(app.calibration.profile.masks),1)

    def test_collapsing_new_rectangle_does_not_commit_previous_preview(self):
        app=self.interactive();app.force_new=True;before=app.calibration.profile
        app.drag_begin(SimpleNamespace(x=300,y=300));app.drag_move(SimpleNamespace(x=350,y=330))
        app.drag_end(SimpleNamespace(x=300,y=300))
        self.assertEqual(app.calibration.profile,before)

    def test_cancel_unsaved_changes_does_not_save(self):
        app=self.app();app.dirty=True;app.root=Mock()
        with patch('metin_bot.app.messagebox.askyesnocancel',return_value=None):
            self.assertFalse(app.confirm_unsaved('zavřením'))
        self.assertEqual(self.events,['pause'])

    def test_save_failure_keeps_editor_open(self):
        app=self.app();app.dirty=True;app.root=Mock();app.save=Mock(return_value=False)
        with patch('metin_bot.app.messagebox.askyesnocancel',return_value=True):
            self.assertFalse(app.confirm_unsaved('zavřením'))
        app.save.assert_called_once()

    def test_discard_confirmation_allows_leaving_without_saving(self):
        app=self.app();app.dirty=True;app.root=Mock()
        with patch('metin_bot.app.messagebox.askyesnocancel',return_value=False):
            self.assertTrue(app.confirm_unsaved('zavřením'))
        self.assertEqual(self.events,['pause'])

    def test_close_cancel_does_not_shut_worker_down(self):
        app=self.app();app.confirm_unsaved=Mock(return_value=False)
        app.worker.shutdown=Mock();app.close()
        self.assertFalse(app.closing);app.worker.shutdown.set.assert_not_called()

    def test_switch_cancel_restores_original_profile_choice(self):
        from metin_bot.config import Profile
        app=self.app();other=Profile.create('Second')
        app.profiles=[app.calibration.profile,other]
        app.profile_choice=Mock();app.profile_choice.current.return_value=1
        app.confirm_unsaved=Mock(return_value=False)
        original=app.calibration
        app.select_profile()
        self.assertIs(app.calibration,original)
        app.profile_choice.current.assert_called_with(0)

    def test_save_during_switch_selects_loaded_profile_in_selector(self):
        from metin_bot.config import Profile
        app=self.app();other=Profile.create('Second')
        app.profiles=[app.calibration.profile,other]
        app.profile_choice=Mock();app.profile_choice.current.return_value=1
        app.confirm_unsaved=Mock(side_effect=lambda action:app.save())
        app.store=SimpleNamespace(load=lambda identifier:(other,None),select=Mock())
        app.load_fields=Mock();app.refresh_windows=Mock();app.render=Mock()
        app.reason=SimpleNamespace(set=Mock())
        app.select_profile()
        self.assertEqual(app.calibration.profile.id,other.id)
        app.refresh_profiles.assert_called_with(other.id)

    def test_undo_back_to_saved_state_clears_dirty_and_redo_restores_it(self):
        app=self.interactive();app.sync_dirty=App.sync_dirty.__get__(app)
        app.saved_profile=app.calibration.profile;app.saved_template=app.calibration.template.copy()
        app.settings_profile=lambda:app.calibration.profile
        app.drag_begin(SimpleNamespace(x=300,y=300));app.drag_end(SimpleNamespace(x=350,y=330))
        self.assertTrue(app.dirty)
        app.undo();self.assertFalse(app.dirty)
        app.redo();self.assertTrue(app.dirty)

    def test_zoomed_picker_uses_same_client_coordinates(self):
        app=self.interactive();app.viewport.zoom_at((200,200),2)
        point=app.viewport.to_canvas((100,150))
        self.assertEqual(app.point(SimpleNamespace(x=point[0],y=point[1])),(100,150))
