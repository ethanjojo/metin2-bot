"""Local profile editor and a single foreground-only game worker."""
from dataclasses import replace
from pathlib import Path
import logging
import queue
import time
import tkinter as tk
from tkinter import messagebox, simpledialog
import cv2
from PIL import Image, ImageTk
from .config import Profile, Region, BuffConfig, BuffIcon
from .vision import crop
from .profiles import ProfileStore
from .data_paths import prepare_data_directory
from .calibration import Calibration, Mode, contains
from .vision import annotate, detect_candidates, panel_score
from .windows import WindowsAPI
from .worker import Worker
from .editor_ui import build, STEP_NAMES
from .editing import Gesture, hit_handle, transform_region, handles
from .editor_tools import History, Viewport
from .run_ui import update_overview, preview_status
from .session import Statistics
from .theme import UISettings, apply_theme, PALETTES
import numpy as np

logger = logging.getLogger(__name__)

# UI fields map directly to the validated, canonical profile schema.
FIELDS = (
    ('hue_tolerance', 'Hue tolerance', 'color', int),
    ('saturation_tolerance', 'Saturation tolerance', 'color', int),
    ('value_tolerance', 'Brightness tolerance', 'color', int),
    ('min_area', 'Min. area (px)', 'color', int),
    ('min_width', 'Min. width (px)', 'color', int),
    ('min_height', 'Min. height (px)', 'color', int),
    ('max_area_ratio', 'Max. area / frame', 'color', float),
    ('threshold', 'Panel match (0–1)', 'panel', float),
    ('click_hover_seconds', 'Cursor settling (s)', 'timing', float),
    ('click_hold_seconds', 'Mouse hold (s)', 'timing', float),
    ('acquire_timeout', 'Target acquire (s)', 'timing', float),
    ('attack_timeout', 'Attack timeout (s)', 'timing', float),
    ('search_timeout', 'Search timeout (s)', 'timing', float),
    ('rotate_seconds', 'E rotation (s)', 'timing', float),
    ('max_rotations', 'Max. rotations / retries', 'timing', int),
)

class App:
    def __init__(self, root: tk.Tk, api: WindowsAPI, *, data_directory: Path | None = None):
        self.root, self.api = root, api
        self.data_directory = prepare_data_directory(data_directory)
        self.ui_settings=UISettings.load(self.data_directory/'ui-settings.json')
        self.hover_mode=None
        self.store = ProfileStore(self.data_directory / 'profiles')
        self.store.migrate_legacy(self.data_directory / 'config.json')
        self.profiles = self.store.list()
        if not self.profiles:
            self.profiles = [self.store.save(Profile.create('Verzum'))]
        identifier = self.store.selected() or self.profiles[0].id
        profile, template = self.store.load(identifier)
        self.calibration = Calibration(profile, template)
        self.worker = Worker(api)
        self.windows = []
        self.last_frame = None
        self.frozen = False
        self.mode = None
        self.preview_size = (1, 1)
        self.photo = self.template_photo = None
        self.closing = False
        self.mask_index = None
        self.gesture = None
        self.edit_preview = None
        self.force_new = False
        self.dirty = False
        self.history = History(self.calibration)
        self.viewport = Viewport()
        self.pan_start = None
        self.preview_cache = None
        self.saved_profile = profile
        self.last_statistics=Statistics()
        self.running=False
        self.last_capture_at=None
        self.preview_busy=False
        self.preview_after=None
        self.pending_mount_confirmation=None
        self.buff_pick=None
        self.last_buff_sample=None
        self.saved_template = template.copy() if template is not None else None
        build(self, root, profile, FIELDS)
        apply_theme(self,self.ui_settings.theme)
        self.refresh_profiles(identifier)
        self.load_fields()
        self.refresh_windows()
        root.after(80,self.poll)

    def selected_window(self):
        index=self.window_choice.current()
        return self.windows[index] if 0<=index<len(self.windows) else None

    def toggle_theme(self):
        settings=replace(self.ui_settings,theme='dark' if self.ui_settings.theme=='light' else 'light')
        try:settings.save(self.data_directory/'ui-settings.json')
        except OSError as error:
            self.reason.set(f'Could not save the theme: {error}');return
        self.ui_settings=settings;apply_theme(self,settings.theme);self.render()

    def highlight_calibration(self,mode):
        self.hover_mode=mode
        self.draw_regions()

    def toggle_run(self):
        if self.running or self.status.get()=='Armed':self.worker.pause()
        else:self.arm()

    def update_run_controls(self):
        active=self.running or self.status.get()=='Armed'
        name='pause' if active else 'play'
        from .ui_controls import icon
        key=(name,True)
        self.start_button.icon_name=name
        if key not in self.icons:self.icons[key]=icon(name,PALETTES[self.ui_settings.theme]['primary_fg'])
        enabled=active or (self.calibration.profile.ready and
                          self.calibration.template is not None and self.selected_window())
        self.start_button.configure(text='Pause' if active else 'Start',image=self.icons[key],
                                    state='normal' if enabled else 'disabled')
        palette=PALETTES[self.ui_settings.theme]
        self.run_title_label.configure(foreground=palette['accent'] if active else palette['fg'])

    def toggle_sidebar(self):
        self.cancel_drag()
        if self.sidebar.winfo_manager():self.sidebar.pack_forget()
        else:self.sidebar.pack(side='left',fill='y',padx=(0,12),before=self.preview_panel)

    def change_zoom(self,factor):
        if self.last_frame is None or self.gesture is not None or self.pan_start is not None:return
        self.viewport.zoom_at((self.canvas.winfo_width()/2,self.canvas.winfo_height()/2),factor)
        self.render()

    def update_preview_status(self):
        source=self.calibration.frame if self.frozen else self.last_frame
        label=preview_status(source is not None,self.frozen,self.last_capture_at,time.monotonic(),
                             max(1.,self.calibration.profile.timing.sample_interval*3))
        self.preview_state.set(label)

    def reset_statistics(self):
        self.worker.reset_statistics()
        self.last_statistics=Statistics()
        sample=self.last_buff_sample
        update_overview(self,self.status.get(),self.last_statistics,sample.remaining_seconds if sample is not None else None)

    def refresh_profiles(self, identifier):
        self.profiles=self.store.list()
        self.profile_choice['values']=[p.name for p in self.profiles]
        self.profile_choice.current(next(i for i,p in enumerate(self.profiles) if p.id==identifier))

    def load_fields(self):
        from .config import ColorConfig
        p=self.calibration.profile
        self.pending_mount_confirmation=None
        self.last_buff_sample=None
        self.name.set(p.name)
        for key,label,group,cast in FIELDS:
            config=getattr(p,group) or ColorConfig((0,0,0))
            self.entries[key].set(str(getattr(config,key)))
        for key,variable in self.buff_fields.items():variable.set(getattr(p.buffs,key))
        self.buff_list.load(p.buffs.skills)
        self.update_buff_status()
        self.render_template()
        self.update_ready()

    def update_ready(self):
        p=self.calibration.profile
        missing=[label for label,exists in [('search area',p.region),('color',p.color),('panel',p.panel.region),
                                           ('template',self.calibration.template is not None),('frame',p.frame_size) ] if not exists]
        try:
            if not self.settings_profile().buffs.ready:missing.append('buff icons')
        except ValueError:pass
        self.ready_text.set('Still needed: '+', '.join(missing) if missing else 'Calibration complete')
        self.update_editor()

    def select_profile(self,event=None):
        p=self.profiles[self.profile_choice.current()]
        if p.id==self.calibration.profile.id:return
        if not self.confirm_unsaved('switching profiles'):
            self.profile_choice.current(next(i for i,p in enumerate(self.profiles) if p.id==self.calibration.profile.id))
            return
        self.load_profile(p)

    def load_profile(self,p):
        self.cancel_preview()
        self.worker.pause()
        profile,template=self.store.load(p.id)
        self.store.select(p.id)
        self.calibration=Calibration(profile,template)
        self.last_statistics=Statistics()
        self.status.set('Stopped')
        update_overview(self,'Stopped',Statistics())
        self.refresh_profiles(profile.id)
        self.last_frame=None; self.frozen=False; self.mode=None;self.running=False;self.last_capture_at=None
        self.reset_edit(); self.viewport.reset(); self.history=History(self.calibration)
        self.saved_profile=profile; self.saved_template=template.copy() if template is not None else None
        self.dirty=False; self.dirty_text.set('Profile saved')
        self.load_fields(); self.refresh_windows(); self.render()
        self.reason.set('Profile loaded. Calibration and undo history belong to this profile.')

    def new_profile(self):
        if not self.confirm_unsaved('creating a profile'):return
        name=simpledialog.askstring('New profile','Server / profile name:',parent=self.root)
        if not name:return
        try:
            p=self.store.save(Profile.create(name))
            self.load_profile(p)
        except Exception as error:messagebox.showerror('Profile',str(error))

    def refresh_windows(self):
        self.pending_mount_confirmation=None
        self.last_buff_sample=None
        self.update_buff_status()
        self.cancel_preview()
        self.worker.clear()
        self.running=False;self.status.set('Stopped')
        update_overview(self,'Stopped',self.last_statistics)
        self.reason.set('Select a game window to preview and start.')
        try:
            self.windows=self.api.list_windows()
            self.window_choice['values']=[f'{w.title} · {w.process_name} · PID {w.pid}' for w in self.windows]
            matches=[i for i,w in enumerate(self.windows) if w.process_name.casefold()==self.calibration.profile.process_name.casefold()]
            if len(matches)==1:
                self.window_choice.current(matches[0]); self.select_window()
            else:
                self.window_choice.set('Select a game window')
                self.last_frame=None; self.frozen=False; self.mode=None; self.calibration.frame=None
                self.reset_edit();self.update_editor();self.render()
        except Exception as error:messagebox.showerror('Game windows',str(error))
        self.update_run_controls()

    def select_window(self,event=None):
        self.pending_mount_confirmation=None
        self.last_buff_sample=None
        self.update_buff_status()
        self.cancel_preview()
        self.worker.pause()
        self.running=False;self.status.set('Stopped')
        window=self.selected_window()
        if window:
            self.last_frame=None; self.frozen=False; self.mode=None;self.running=False;self.last_capture_at=None
            self.reset_edit()
            self.calibration.frame=None
            self.worker.configure(self.calibration.profile,window,self.calibration.template,restore_mounted=self.ui_settings.last_mounted)
            self.update_editor()
            self.render()

    def settings_profile(self):
        p=self.calibration.profile
        changes={group:{} for group in ('color','panel','timing')}
        for key,label,group,cast in FIELDS:
            changes[group][key]=cast(self.entries[key].get())
        window=self.selected_window()
        buffs=BuffConfig(
            icon_region=p.buffs.icon_region,
            **{key:(variable.get() if key in ('enabled','fight_mounted','mount_shortcut') else float(variable.get()))
               for key,variable in self.buff_fields.items()},
            skills=self.buff_list.skills())
        return replace(p,name=self.name.get(),process_name=window.process_name if window else p.process_name,
                       color=replace(p.color,**changes['color']) if p.color else None,
                       panel=replace(p.panel,**changes['panel']),timing=replace(p.timing,**changes['timing']),buffs=buffs)

    def save(self):
        self.worker.pause()
        try:
            p=self.store.save(self.settings_profile(),self.calibration.template)
            self.calibration.profile=p
            self.store.select(p.id); self.refresh_profiles(p.id)
            window=self.selected_window()
            if window:self.worker.configure(p,window,self.calibration.template,restore_mounted=self.ui_settings.last_mounted)
            self.saved_profile=p; self.saved_template=self.calibration.template.copy() if self.calibration.template is not None else None
            self.dirty=False; self.dirty_text.set('Profile saved')
            self.last_buff_sample=None
            self.update_buff_status()
            self.update_ready(); self.reason.set('Profile saved. Click Start or press F7 in the game.')
            return True
        except Exception as error:
            messagebox.showerror('Profile settings',str(error));return False

    def arm(self):
        window=self.selected_window()
        if window is None:
            messagebox.showerror('Start','Select a game window.');return
        if self.save():
            if not self.calibration.profile.ready or self.calibration.template is None:
                messagebox.showerror('Start','Complete all five calibration steps.');return
            self.cancel_preview()
            try:
                accepted=self.api.request_activation(window)
                logger.info('Foreground activation request accepted=%s',accepted)
                self.frozen=False;self.mode=None;self.reset_edit();self.update_editor();self.worker.arm()
            except Exception as error:
                self.worker.pause();messagebox.showerror('Game activation',str(error))

    def edit_mount_state(self):
        mounted=bool(self.mount_choice.get())
        self.remember_mount(mounted)
        window=self.selected_window()
        if window is None:
            self.pending_mount_confirmation=None
            self.update_buff_status()
            self.reason.set('Mount state set. It will be used when you select a game window.')
            return
        self.pending_mount_confirmation=self.worker.confirm_mount(self.calibration.profile.id,window,mounted)
        self.update_buff_status()
        self.reason.set('Applying mount state. The bot will pause; resume with Start or F7.')

    def remember_mount(self,mounted:bool):
        if mounted==self.ui_settings.last_mounted:return
        self.ui_settings=replace(self.ui_settings,last_mounted=mounted)
        try:self.ui_settings.save(self.data_directory/'ui-settings.json')
        except OSError as error:
            logger.warning('Could not save the last mount state: %s',error)
            self.reason.set('Could not save the mount state for the next launch. The current session is unchanged.')

    def update_buff_status(self):
        """Draft settings and pending mount declaration take precedence over samples."""
        enabled=self.buff_fields['enabled'].get()
        try:changed=self.settings_profile().buffs!=self.saved_profile.buffs
        except ValueError:changed=True
        sample=self.last_buff_sample
        self.buff_list.update_timers(sample.buff_skill_seconds if sample is not None else (),enabled=enabled,dirty=changed,
                                     running=sample.running if sample is not None else False,
                                     states=sample.buff_states if sample is not None else (),can_reset=sample is not None,
                                     has_region=self.calibration.profile.buffs.icon_region is not None)
        if self.selected_window() is not None and self.pending_mount_confirmation is None and sample is not None and sample.mounted is not None:
            self.remember_mount(sample.mounted)
        self.mount_choice.set(int(self.ui_settings.last_mounted))
        text='On' if self.ui_settings.last_mounted else 'Off'
        self.mount_status.set(text)
        self.mount_switch.state(['!alternate','!disabled'])

    def reset_skill_timer(self,index):
        window=self.selected_window()
        try:
            if self.settings_profile().buffs!=self.saved_profile.buffs:
                raise ValueError('Save your buff settings first.')
            sample=self.last_buff_sample
            if window is None or sample is None or sample.profile_id!=self.saved_profile.id or sample.window_hwnd!=window.hwnd:
                raise ValueError('Connect a game window for this profile first.')
            self.worker.reset_skill_timer(self.saved_profile,window,index)
            self.reason.set(f'Timer reset requested for skill {index+1}. It will be used after the target ends or the bot resumes.')
        except ValueError as error:self.reason.set(str(error))

    def cancel_preview(self):
        if self.preview_after is not None:
            self.root.after_cancel(self.preview_after);self.preview_after=None
        self.preview_busy=False
        self.snapshot_button.set_loading(False)

    def refresh_preview(self):
        if self.preview_busy:return
        self.viewport.reset()
        window=self.selected_window()
        if window is None:return
        self.worker.pause();self.frozen=False;self.mode=None;self.reset_edit();self.update_editor()
        # Wait for a NEW capture before bringing this editor back, never reuse an old frame.
        self.last_frame=None
        self.preview_busy=True
        self.snapshot_button.set_loading(True)
        self.render()
        try:
            self.api.request_activation(window)
            self.preview_requested=time.monotonic()
            self.preview_deadline=self.preview_requested+4
            self.preview_after=self.root.after(100,self.finish_preview)
        except Exception as error:
            self.cancel_preview();self.render();messagebox.showerror('Preview',str(error))

    def freeze_frame(self,frame):
        before=self.calibration.profile
        self.calibration.freeze(frame)
        if before.frame_size is not None and before.frame_size!=self.calibration.profile.frame_size:
            self.buff_list.clear_icons()
        if before.frame_size!=self.calibration.profile.frame_size:self.history=History(self.calibration)
        if before!=self.calibration.profile:self.mark_dirty()

    def finish_preview(self):
        self.preview_after=None
        if self.closing or not self.preview_busy:return
        if self.last_frame is not None:
            self.cancel_preview()
            self.freeze_frame(self.last_frame);self.frozen=True
            self.root.lift();self.root.focus_force()
            self.reason.set('Frame frozen. Select a calibration step.')
            self.update_ready();self.render();return
        if time.monotonic()<self.preview_deadline:
            self.preview_after=self.root.after(100,self.finish_preview)
        else:
            self.cancel_preview()
            self.reason.set('Frame unavailable. Activate the game manually and check that the game and bot have matching permissions.')
            self.render()

    def live_preview(self):
        self.cancel_preview()
        self.worker.pause();self.frozen=False;self.mode=None;self.reset_edit();self.update_editor()
        self.calibration_hint.set('Live preview updates while the selected game is in the foreground.')
        self.render()

    def mark_dirty(self):
        self.worker.pause()
        self.history.record(self.calibration)
        self.sync_dirty()
        self.update_history_buttons()

    def sync_dirty(self):
        try:
            profile=self.settings_profile()
            same_profile=replace(profile,template_file=None)==replace(self.saved_profile,template_file=None)
            same_template=(self.calibration.template is None and self.saved_template is None or
                           self.calibration.template is not None and self.saved_template is not None and
                           np.array_equal(self.calibration.template,self.saved_template))
            self.dirty=not (same_profile and same_template)
        except ValueError:self.dirty=True
        self.dirty_text.set('Unsaved changes' if self.dirty else 'Profile saved')
        self.update_buff_status()

    def confirm_unsaved(self,action):
        self.sync_dirty()
        if not self.dirty:return True
        self.worker.pause()
        answer=messagebox.askyesnocancel('Unsaved changes',
            f'Save profile changes before {action}?\n\nYes = save · No = discard · Cancel = stay in the editor.',parent=self.root)
        if answer is None:return False
        return self.save() if answer else True

    def update_history_buttons(self):
        self.undo_button.configure(state='normal' if self.history.can_undo else 'disabled')
        self.redo_button.configure(state='normal' if self.history.can_redo else 'disabled')

    def restore_history(self,redo=False,event=None):
        if event is not None and event.widget.winfo_class() in ('TEntry','Entry'):return
        self.cancel_drag()
        self.worker.pause()
        if redo:self.history.redo(self.calibration)
        else:self.history.undo(self.calibration)
        if self.mask_index is not None and self.mask_index>=len(self.calibration.profile.masks):self.mask_index=None
        self.sync_dirty();self.render_template();self.update_ready();self.render()
        self.reason.set('Edit redone.' if redo else 'Last edit undone.')
        return 'break'

    def undo(self,event=None):return self.restore_history(event=event)
    def redo(self,event=None):return self.restore_history(redo=True,event=event)

    def zoom(self,event):
        if self.gesture is not None or self.pan_start is not None:return 'break'
        if self.last_frame is None:return 'break'
        self.viewport.zoom_at((event.x,event.y),1.25 if event.delta>0 else .8)
        self.render();return 'break'

    def reset_zoom(self):
        self.cancel_drag();self.viewport.reset();self.render()

    def pan_begin(self,event):
        if self.gesture is None:self.pan_start=(event.x,event.y)

    def pan_move(self,event):
        if self.pan_start is None:return
        x,y=self.pan_start;self.viewport.pan(event.x-x,event.y-y)
        self.pan_start=(event.x,event.y);self.render()

    def pan_end(self,event):self.pan_start=None


    def reset_edit(self):
        self.buff_pick=None
        self.gesture=None
        self.edit_preview=None
        self.force_new=False
        self.mask_index=None

    def prepare_buff_edit(self):
        self.worker.pause()
        if self.buff_pick is not None:self.cancel_drag()

    def begin_buff_pick(self,index,active=True,reuse=False):
        if self.last_frame is None:
            self.reason.set('Capture a frame with the desired buff state first.');return
        self.begin_calibration(Mode.BUFF_AREA if index==-1 else Mode.BUFF_ICON)
        error=None
        if index!=-1 and self.calibration.profile.buffs.icon_region is None:
            error='Select the separate active buff area first.'
        if error:
            self.mode=None;self.reason.set(error);self.update_editor();self.render();return
        if reuse and index>=0:
            row=self.buff_list.rows[index]
            source=row.inactive_icon if active else row.icon
            try:
                if source is None or source.region is None:
                    raise ValueError('The other sample has no saved area. Select it again by dragging.')
                self.capture_buff_icon(index,active,source.region)
            except ValueError as error:
                self.mode=None;self.reason.set(str(error));self.update_editor();self.render()
            return
        self.buff_pick=(index,active) if index>=0 else None;self.force_new=index>=0
        self.reason.set('Select the buff icon area or hotbar where both states will be compared.' if index==-1 else
                        f'Skill {index+1}: select the active icon including its highlight, without a tooltip.' if active else
                        f'Skill {index+1}: drag to select the inactive icon, without a tooltip.')
        self.update_editor()
        self.render()

    def capture_buff_icon(self,index,active,region):
        if not contains(self.calibration.profile.buffs.icon_region,region):
            raise ValueError('The icon must be inside the separate active buff area.')
        self.buff_list.set_icon(index,BuffIcon.capture(crop(self.calibration.frame,region).copy(),region),active)
        self.buff_pick=None;self.mode=None;self.force_new=False
        self.mark_dirty();self.update_ready();self.render()
        self.reason.set('Buff sample added to the draft. Save the profile to use it.')

    def begin_calibration(self, mode):
        if self.last_frame is None:
            self.active_mode.set(self.mode.value if self.mode else '')
            self.reason.set('Click Capture first.');return
        self.worker.pause()
        if not self.frozen:self.freeze_frame(self.last_frame)
        self.frozen=True
        self.mode=mode
        self.reset_edit()
        if mode==Mode.MASK and self.calibration.profile.masks:self.mask_index=0
        self.update_ready();self.render()
        self.canvas.focus_set()

    def new_region(self):
        if self.mode is None:
            self.reason.set('Select an area to edit on the left.');return
        self.force_new=True
        if self.mode==Mode.MASK:self.mask_index=None
        self.update_editor()
        self.calibration_hint.set('Drag a new rectangle. Escape cancels the selection.' if self.mode!=Mode.COLOR else 'Click a new Metin color.')
        self.canvas.configure(cursor='crosshair');self.draw_regions()

    def delete_selection(self):
        if self.mode is None or self.buff_pick is not None:return
        before=self.calibration.profile
        self.calibration.remove_region(self.mode,self.mask_index)
        if self.mode==Mode.MASK:self.mask_index=None
        if before!=self.calibration.profile:self.mark_dirty()
        self.render_template();self.update_ready();self.render()

    def select_mask(self,event=None):
        selected=self.mask_list.curselection()
        if selected:
            self.mask_index=selected[0];self.force_new=False
            self.update_editor();self.draw_regions()

    def point(self,event,clamp=False):
        frame=self.calibration.frame
        if frame is None:raise ValueError('Capture a calibration frame first.')
        return self.viewport.to_image((event.x,event.y),clamp=clamp)

    def hit_region(self,region,event):
        offset=self.viewport.offset
        return hit_handle(region,(event.x-offset[0],event.y-offset[1]),self.viewport.display_size)

    def target_at(self,event):
        if self.buff_pick is not None:return None,None,None
        if self.force_new or self.mode in (None,Mode.COLOR):return None,None,None
        if self.mode==Mode.MASK:
            masks=self.calibration.profile.masks
            order=list(reversed(range(len(masks))))
            if self.mask_index in order:order.remove(self.mask_index);order.insert(0,self.mask_index)
            for index in order:
                handle=self.hit_region(masks[index],event)
                if handle:return masks[index],index,handle
            return None,None,None
        region=self.calibration.region(self.mode)
        handle=self.hit_region(region,event) if region else None
        return region,None,handle

    def drag_begin(self,event):
        if self.mode is None:return
        self.canvas.focus_set()
        try:
            point=self.point(event)
            if self.mode==Mode.COLOR:
                self.calibration.color(point);self.mark_dirty();self.update_ready();self.render()
                self.reason.set('Color selected. Green rectangles show Metin candidates.');return
            region,index,handle=self.target_at(event)
            self.mask_index=index if self.mode==Mode.MASK else None
            self.gesture=Gesture(self.mode,index,handle or 'new',point,region if handle else None)
            self.update_editor()
        except ValueError as error:self.reason.set(str(error))

    def drag_move(self,event):
        if self.gesture is None:return
        try:
            end=self.point(event,clamp=True)
            gesture=self.gesture
            frame=self.calibration.frame;size=(frame.shape[1],frame.shape[0])
            if gesture.original is not None:
                self.edit_preview=transform_region(gesture.original,gesture.start,end,size,gesture.handle)
            else:
                x1,x2=sorted((gesture.start[0],end[0]));y1,y2=sorted((gesture.start[1],end[1]))
                self.edit_preview=Region(x1/size[0],y1/size[1],x2/size[0],y2/size[1])
            self.draw_regions()
        except ValueError:
            self.edit_preview=None
            self.draw_regions()

    def drag_end(self,event):
        if self.gesture is None:return
        self.drag_move(event)
        gesture=self.gesture;region=self.edit_preview
        self.gesture=None;self.edit_preview=None
        if region is not None:
            try:
                if self.buff_pick is not None:
                    index,active=self.buff_pick
                    self.capture_buff_icon(index,active,region);return
                before=self.calibration.profile
                self.calibration.set_region(gesture.mode,region,gesture.index)
                if gesture.mode==Mode.MASK and gesture.index is None:self.mask_index=len(self.calibration.profile.masks)-1
                self.force_new=False
                if before!=self.calibration.profile:self.mark_dirty()
                self.render_template();self.update_ready()
                self.reason.set('Area updated. Save the profile when calibration is ready.')
            except ValueError as error:self.reason.set(str(error))
        self.render()

    def cancel_drag(self,event=None):
        self.gesture=None;self.edit_preview=None;self.force_new=False
        if self.buff_pick is not None:self.buff_pick=None;self.mode=None
        self.update_editor();self.draw_regions()

    def hover(self,event):
        if self.gesture is not None:return
        _,_,handle=self.target_at(event)
        cursors={'nw':'size_nw_se','se':'size_nw_se','ne':'size_ne_sw','sw':'size_ne_sw',
                 'n':'sb_v_double_arrow','s':'sb_v_double_arrow','e':'sb_h_double_arrow',
                 'w':'sb_h_double_arrow','move':'fleur'}
        self.canvas.configure(cursor=cursors.get(handle,'crosshair' if self.mode or self.force_new else 'arrow'))

    def update_editor(self):
        p=self.calibration.profile
        from .calibration_ui import update_previews
        update_previews(self)
        self.buff_area_text.set('Active buff area selected' if p.buffs.icon_region else 'Area not selected')
        self.update_history_buttons()
        self.active_mode.set(self.mode.value if self.mode else '')
        self.active_title.set(STEP_NAMES[self.mode] if self.mode else 'Game preview')
        hints={Mode.WORLD:'Drag inside the area to move it; drag a handle to resize.',
               Mode.MASK:'Select a mask in the list or frame. Drag it or resize its handles.',
               Mode.COLOR:'Click the stone surface. Green rectangles show detected candidates.',
               Mode.PANEL:'Select the target panel area. Moving it outside the template clears the template.',
               Mode.TEMPLATE:'Select an icon or close button without HP or names. Move or resize the sample.',
               Mode.BUFF_AREA:'Buff icon area or hotbar: draw a new area, drag inside it, or resize its handles.',
               Mode.BUFF_ICON:'Select the stable part of an icon inside the buff area.'}
        self.calibration_hint.set(hints.get(self.mode,'Capture a frame, then select a calibration step on the left.'))
        if self.buff_pick is not None:
            index,active=self.buff_pick
            self.active_title.set(f'Skill {index+1} · '+('active icon' if active else 'inactive icon'))
            self.calibration_hint.set('Select the active appearance including its indicator, without a tooltip.' if active else
                                     'Drag to select the inactive appearance, without a tooltip.')
        complete={Mode.WORLD:p.region is not None,Mode.MASK:bool(p.masks),Mode.COLOR:p.color is not None,
                  Mode.PANEL:p.panel.region is not None,Mode.TEMPLATE:self.calibration.template is not None}
        for mode,label in STEP_NAMES.items():
            if mode not in self.step_text:continue
            suffix=f' ({len(p.masks)})' if mode==Mode.MASK else ''
            self.step_text[mode].set(('✓ ' if complete[mode] else '○ ')+label+suffix)
        region=self.calibration.region(self.mode,self.mask_index) if self.mode and self.buff_pick is None else None
        if region and p.frame_size:
            x1,y1,x2,y2=region.pixels(*p.frame_size)
            name=f'Mask {self.mask_index+1}' if self.mode==Mode.MASK else STEP_NAMES[self.mode]
            self.selection_info.set(f'Selected: {name}\n{x2-x1+1} × {y2-y1+1} px · position {x1}, {y1}')
        elif self.mode==Mode.COLOR:self.selection_info.set('Selected: Metin color')
        else:self.selection_info.set('Drag a new area on the frame.' if self.mode else 'No area selected')
        if self.buff_pick is None and self.mode in (Mode.WORLD,Mode.MASK,Mode.PANEL,Mode.TEMPLATE,Mode.BUFF_AREA):self.region_actions.pack(fill='x',pady=(0,8))
        else:self.region_actions.pack_forget()
        if self.mode==Mode.MASK:
            self.mask_box.pack(fill='x',after=self.mask_anchor,pady=(0,6))
            self.mask_list.delete(0,'end')
            for i in range(len(p.masks)):self.mask_list.insert('end',f'Mask {i+1}')
            if self.mask_index is not None:self.mask_list.selection_set(self.mask_index)
        else:self.mask_box.pack_forget()
        if self.mode==Mode.COLOR:self.color_box.pack(fill='x',pady=(4,8))
        else:self.color_box.pack_forget()
        if self.mode in (Mode.PANEL,Mode.TEMPLATE):self.template_box.pack(fill='x',pady=(4,8))
        else:self.template_box.pack_forget()
        self.color_text.set(f'Selected color · HSV {p.color.hsv}' if p.color else 'Metin color not selected')
        color='#ffffff'
        if p.color:
            import numpy as np
            rgb=cv2.cvtColor(np.uint8([[p.color.hsv]]),cv2.COLOR_HSV2RGB)[0,0]
            color='#'+''.join(f'{int(v):02x}' for v in rgb)
        self.swatch.configure(background=color)
        self.update_run_controls()

    def draw_regions(self):
        self.canvas.delete('areas')
        if not self.frozen or self.calibration.frame is None:return
        p=self.calibration.profile;w,h=self.viewport.display_size
        ox,oy=self.viewport.offset
        regions=[(Mode.WORLD,None,p.region),(Mode.PANEL,None,p.panel.region),
                 (Mode.TEMPLATE,None,p.template_region),(Mode.BUFF_AREA,None,p.buffs.icon_region),*[(Mode.MASK,i,r) for i,r in enumerate(p.masks)]]
        if self.edit_preview is not None and self.gesture:
            g=self.gesture
            regions=[entry for entry in regions if (entry[0],entry[1])!=(g.mode,g.index)]
            regions.append((g.mode,g.index,self.edit_preview))
        regions.sort(key=lambda entry:entry[0]==self.mode and (entry[0]!=Mode.MASK or entry[1]==self.mask_index))
        for mode,index,region in regions:
            if region is None:continue
            selected=mode==self.mode and (mode!=Mode.MASK or index==self.mask_index)
            if self.gesture and mode==self.gesture.mode and index==self.gesture.index:selected=True
            highlighted=mode==self.hover_mode
            color='#e09c32' if highlighted and not selected else '#235cba' if selected else '#526072'
            coords=(region.left*w+ox,region.top*h+oy,min(w-1,region.right*w)+ox,min(h-1,region.bottom*h)+oy)
            self.canvas.create_rectangle(*coords,outline=color,width=2 if selected or highlighted else 1,dash=() if selected or highlighted else (5,3),tags='areas')
            label=('New mask' if index is None else f'Mask {index+1}') if mode==Mode.MASK else STEP_NAMES[mode]
            if self.buff_pick is not None and selected:label=f'Skill {self.buff_pick[0]+1} · '+('active' if self.buff_pick[1] else 'inactive')
            tx,ty=coords[0]+5,coords[1]+5
            text=self.canvas.create_text(tx,ty,anchor='nw',text=label,fill='white',font=('Segoe UI',9,'bold'),tags='areas')
            bbox=self.canvas.bbox(text)
            backing=self.canvas.create_rectangle(bbox[0]-3,bbox[1]-2,bbox[2]+3,bbox[3]+2,fill=color,outline='',tags='areas')
            self.canvas.tag_lower(backing,text)
            if selected:
                for hx,hy in handles(region,(w,h)).values():
                    hx=min(w-4,max(4,hx))+ox;hy=min(h-4,max(4,hy))+oy
                    self.canvas.create_rectangle(hx-4,hy-4,hx+4,hy+4,fill='white',outline=color,width=2,tags='areas')

    def render_template(self):
        template=self.calibration.template
        if template is None:
            self.template_photo=None;self.template_label.configure(image='',text='No template yet');return
        image=Image.fromarray(cv2.cvtColor(template,cv2.COLOR_BGR2RGB));image.thumbnail((260,45))
        self.template_photo=ImageTk.PhotoImage(image)
        self.template_label.configure(image=self.template_photo,text='')

    def render(self):
        self.canvas.delete('all')
        source=self.calibration.frame if self.frozen else self.last_frame
        self.update_preview_status()
        if source is None:
            from .preview_ui import render_empty_preview
            self.info.set('')
            render_empty_preview(self);return
        try:
            p=self.settings_profile()
            detections=self.show_detections.get()
            key=(id(source),p,id(self.calibration.template),detections)
            if self.preview_cache is None or self.preview_cache[0]!=key:
                candidates=detect_candidates(source,p)
                score=panel_score(source,self.calibration.template,p.panel.region) if self.calibration.template is not None and p.panel.region else None
                frame=annotate(source,candidates,p,score,regions=False) if detections else source
                image=Image.fromarray(cv2.cvtColor(frame,cv2.COLOR_BGR2RGB))
                self.preview_cache=(key,image,len(candidates),score)
            _,image,count,score=self.preview_cache
            panel='not set' if score is None else f'{score:.3f} / threshold {p.panel.threshold:.2f}'
            self.info.set(f'{count} candidates · panel match: {panel}')
            available=(max(10,self.canvas.winfo_width()),max(10,self.canvas.winfo_height()))
            self.viewport.fit((source.shape[1],source.shape[0]),available)
            self.preview_size=self.viewport.display_size
            self.zoom_text.set(f'{self.viewport.magnification*100:.0f} %')
            scale=self.viewport.scale;ox,oy=self.viewport.offset
            visible=image.transform(available,Image.Transform.AFFINE,(1/scale,0,-ox/scale,0,1/scale,-oy/scale),
                                    Image.Resampling.BILINEAR,fillcolor=PALETTES[self.ui_settings.theme]['preview'])
            self.photo=ImageTk.PhotoImage(visible)
            self.canvas.create_image(0,0,anchor='nw',image=self.photo)
            self.draw_regions()
        except ValueError as error:self.info.set(str(error))

    def poll(self):
        if self.closing:return
        try:
            sample=self.worker.results.get_nowait()
            window=self.selected_window()
            if sample.profile_id is not None and (window is None or sample.profile_id!=self.calibration.profile.id or sample.window_hwnd!=window.hwnd or sample.captured_at<getattr(self,'preview_requested',0)):
                self.root.after(80,self.poll);return
            self.status.set(sample.state)
            self.running=sample.running
            self.update_run_controls()
            if sample.profile_id is not None:self.last_statistics=sample.statistics
            update_overview(self,sample.state,getattr(self,'last_statistics',Statistics()),sample.remaining_seconds)
            self.reason.set(sample.reason)
            self.last_buff_sample=sample
            if self.pending_mount_confirmation is not None and sample.mount_confirmation_id>=self.pending_mount_confirmation:
                self.pending_mount_confirmation=None
            self.update_buff_status()
            if not self.frozen:
                if sample.frame is not None:
                    self.last_capture_at=sample.captured_at
                    self.last_frame=sample.frame.copy();self.render()
        except queue.Empty:pass
        self.update_preview_status()
        if self.worker.thread.is_alive():self.root.after(80,self.poll)

    def close(self):
        if self.closing:return
        if not self.confirm_unsaved('closing the application'):return
        self.cancel_preview()
        self.closing=True;self.worker.cancel_event.set();self.worker.shutdown.set()
        self.status.set('Stopping…');self._finish_close()

    def _finish_close(self):
        if self.worker.thread.is_alive():self.root.after(30,self._finish_close)
        else:self.root.destroy()


def run(*,data_directory:Path|None=None):
    api=WindowsAPI();api.enable_dpi();root=tk.Tk()
    try:App(root,api,data_directory=data_directory)
    except Exception:
        root.destroy();raise
    root.mainloop()
