"""Dynamic skill drafts; moving a row keeps its complete configuration together."""
from dataclasses import dataclass, replace
import tkinter as tk
from tkinter import ttk
import cv2
from PIL import Image, ImageTk
from .config import BuffSkill, BuffIcon, NUMBER_RULES
from .number_input import number_input, set_number_rule
from .keybind_entry import KeybindEntry
from .ui_controls import Tooltip
from .session import duration
from .buff_detection import Presence

MODES={'Refresh periodically':'refresh','Activate when inactive':'inactive'}
INTERVAL_UNITS={'s':1,'min':60}


@dataclass(eq=False)
class SkillRow:
    frame:ttk.Frame
    name:tk.StringVar
    shortcut:tk.StringVar
    wait:tk.StringVar
    mode:tk.StringVar
    icon:BuffIcon|None
    label:ttk.Label
    number:ttk.Label
    up:ttk.Button
    down:ttk.Button
    inactive_icon:BuffIcon|None
    inactive_label:ttk.Label
    icon_box:ttk.Frame
    interval:tk.StringVar
    interval_unit:str
    unit_choice:ttk.Combobox
    interval_box:ttk.Frame
    interval_entry:ttk.Spinbox
    timer_label:ttk.Label
    state_label:ttk.Label
    timer_box:ttk.Frame
    reset_button:ttk.Button
    reuse_active:ttk.Button
    reuse_inactive:ttk.Button
    clear_active:ttk.Button
    clear_inactive:ttk.Button
    photo:ImageTk.PhotoImage|None=None
    inactive_photo:ImageTk.PhotoImage|None=None


class BuffList:
    def __init__(self,parent,*,on_change,on_begin,on_error,on_pick,on_reset=None):
        self.on_change=on_change;self.on_begin=on_begin;self.on_error=on_error;self.on_pick=on_pick
        self.on_reset=on_reset
        self.rows:list[SkillRow]=[]
        self.host=ttk.Frame(parent);self.host.pack(fill='x')
        self.add_button=ttk.Button(parent,text='+ Add skill',command=self.add)
        self.add_button.pack(fill='x',pady=(6,4))

    def load(self,skills):
        for row in self.rows:row.frame.destroy()
        self.rows=[]
        for skill in skills:self._append(skill)
        self._layout()

    def skills(self):
        return tuple(BuffSkill(row.shortcut.get().strip(),float(row.wait.get()),MODES[row.mode.get()],row.icon,row.inactive_icon,
                               float(row.interval.get())*INTERVAL_UNITS[row.interval_unit] if MODES[row.mode.get()]=='refresh' else None,row.name.get())
                     for row in self.rows if row.shortcut.get().strip())

    def add(self):
        if len(self.rows)>=8:return
        self.on_begin();self._append(None);self._layout();self.on_change()

    def remove(self,index):
        self.on_begin();self.rows.pop(index).frame.destroy();self._layout();self.on_change()

    def move(self,index,delta):
        other=index+delta
        if not 0<=other<len(self.rows):return
        self.on_begin();self.rows[index],self.rows[other]=self.rows[other],self.rows[index]
        self._layout();self.on_change()

    def pick(self,row,active=True,reuse=False):
        if reuse:self.on_pick(self.rows.index(row),active,True)
        else:self.on_pick(self.rows.index(row),active)

    def set_interval_unit(self,row,unit):
        if unit not in INTERVAL_UNITS:raise ValueError('Invalid interval unit.')
        value=row.interval.get().strip()
        if value:
            try:value=format(float(value)*INTERVAL_UNITS[row.interval_unit]/INTERVAL_UNITS[unit],'.17g')
            except ValueError:
                row.unit_choice.set(row.interval_unit)
                self.on_error('Enter a valid interval value first.')
                return
        row.interval.set(value);row.interval_unit=unit;row.unit_choice.set(unit)
        rule=NUMBER_RULES['interval_seconds'];scale=INTERVAL_UNITS[unit]
        set_number_rule(row.interval_entry,replace(rule,low=rule.low/scale,high=rule.high/scale))
        # This changes presentation only; the canonical duration stays in seconds.

    def set_icon(self,index,icon,active=True):
        row=self.rows[index]
        other=row.inactive_icon if active else row.icon
        if icon is not None and other is not None and icon.png==other.png:
            raise ValueError('Both templates are identical. Change the buff state and refresh the snapshot before selecting the second icon.')
        if active:row.icon=icon
        else:row.inactive_icon=icon
        self._thumbnail(row)

    def clear_icons(self):
        for row in self.rows:row.icon=None;row.inactive_icon=None;self._thumbnail(row)

    def _thumbnail(self,row):
        for value,photo_name,label in ((row.icon,'photo',row.label),(row.inactive_icon,'inactive_photo',row.inactive_label)):
            photo=None
            if value:
                image=Image.fromarray(cv2.cvtColor(value.decode(),cv2.COLOR_BGR2RGB));image.thumbnail((28,28))
                photo=ImageTk.PhotoImage(image)
            setattr(row,photo_name,photo)
            label.configure(image=photo or '',text='' if photo else 'Missing' if photo_name=='photo' else '—')
        row.reuse_active.configure(state='normal' if row.inactive_icon and row.inactive_icon.region else 'disabled')
        row.reuse_inactive.configure(state='normal' if row.icon and row.icon.region else 'disabled')
        row.clear_active.configure(state='normal' if row.icon else 'disabled')
        row.clear_inactive.configure(state='normal' if row.inactive_icon else 'disabled')

    def _mode_changed(self,row):
        if MODES[row.mode.get()]=='inactive':
            row.interval_box.pack_forget();row.timer_box.pack_forget();row.icon_box.pack(fill='x',pady=(4,0))
            row.state_label.configure(text='Check between Metins')
        else:
            row.icon_box.pack_forget();row.interval_box.pack(fill='x',before=row.state_label,pady=(4,0))
            row.timer_box.pack(fill='x',before=row.state_label,pady=(4,0))
            row.state_label.configure(text='After starting');row.timer_label.configure(text='—')

    def update_timers(self,values,*,enabled,dirty,running=False,states=(),can_reset=True,has_region=True):
        index=0
        for row in self.rows:
            has_key=bool(row.shortcut.get().strip())
            value=values[index] if has_key and index<len(values) else None
            presence=states[index] if has_key and index<len(states) else None
            if has_key:index+=1
            refresh=MODES[row.mode.get()]=='refresh'
            text=({Presence.ACTIVE:'Active',Presence.ABSENT:'Inactive · awaiting use',Presence.UNKNOWN:'Uncertain detection'}.get(presence,'Check between Metins')
                  if not refresh else 'After starting' if value is None else 'Ready to use' if value==0 else 'Countdown running' if running else 'Countdown paused')
            if not refresh and (row.icon is None or not has_region):text='Calibration missing'
            if not has_key:text='Set a shortcut'
            elif dirty:text='Unsaved changes'
            elif not enabled:text='Disabled'
            row.state_label.configure(text=text,style='Warning.TLabel' if dirty or text in ('Uncertain detection','Set a shortcut','Calibration missing') else
                                      'Good.TLabel' if text in ('Active','Ready to use') else 'Active.TLabel' if running and enabled else 'Neutral.TLabel')
            row.timer_label.configure(text=duration(value) if refresh and value is not None and enabled and not dirty else '—')
            row.reset_button.configure(state='normal' if self.on_reset is not None and can_reset and enabled and has_key and refresh and not dirty else 'disabled')

    def _layout(self):
        for index,row in enumerate(self.rows):
            row.frame.pack_forget();row.frame.pack(fill='x',pady=(6,4))
            row.number.configure(text=row.name.get().strip() or f'Skill {index+1}')
            row.up.configure(state='normal' if index else 'disabled')
            row.down.configure(state='normal' if index<len(self.rows)-1 else 'disabled')
        self.add_button.configure(state='normal' if len(self.rows)<8 else 'disabled')

    def _append(self,skill):
        frame=ttk.Frame(self.host,padding=(7,8),style='Skill.TFrame')
        header=ttk.Frame(frame);header.pack(fill='x')
        number=ttk.Label(header,font=('Segoe UI',10,'bold'),width=10,anchor='w');number.pack(side='left')
        up=ttk.Button(header,text='↑',width=2,style='Small.TButton');up.pack(side='right')
        down=ttk.Button(header,text='↓',width=2,style='Small.TButton');down.pack(side='right')
        delete=ttk.Button(header,text='×',width=2,style='Small.TButton');delete.pack(side='right',padx=2)
        Tooltip(up,'Move skill up');Tooltip(down,'Move skill down');Tooltip(delete,'Remove skill')
        name=tk.StringVar(value=skill.name if skill else '')
        Tooltip(number,lambda:name.get().strip() or 'Unnamed skill')
        ttk.Label(frame,text='Skill name',style='Muted.TLabel').pack(anchor='w',pady=(5,2))
        name_entry=ttk.Entry(frame,textvariable=name);name_entry.pack(fill='x',pady=(0,4))
        Tooltip(name_entry,'Optional name, such as Fear or Enchanted Blade. Saved with the full skill configuration.')
        shortcut=tk.StringVar(value=skill.shortcut if skill else '')
        wait=tk.StringVar(value=str(skill.wait_seconds if skill else 1.5))
        mode=tk.StringVar(value=next(label for label,value in MODES.items() if value==(skill.mode if skill else 'refresh')))
        inputs=ttk.Frame(frame);inputs.pack(fill='x',pady=4)
        KeybindEntry(inputs,variable=shortcut,on_change=self.on_change,on_begin=self.on_begin,
                     on_error=self.on_error,width=10).pack(side='left',fill='x',expand=True)
        entry=number_input(inputs,variable=wait,rule=NUMBER_RULES['wait_seconds'],on_change=self.on_change,width=5);entry.pack(side='right',padx=(5,0))
        Tooltip(entry,'Wait after the skill, in seconds')
        choice=ttk.Combobox(frame,textvariable=mode,state='readonly',values=tuple(MODES),width=22)
        choice.pack(fill='x')
        Tooltip(choice,'Refresh: at the configured interval. Activate: check between Metins, independently of the interval; only when inactivity is confirmed.')
        seconds=skill.interval_seconds if skill and skill.interval_seconds is not None else 120
        unit='min' if seconds>=60 and seconds%60==0 else 's'
        interval=tk.StringVar(value=format(seconds/INTERVAL_UNITS[unit],'.17g'))
        interval_box=ttk.Frame(frame)
        ttk.Label(interval_box,text='Interval').pack(side='left')
        unit_choice=ttk.Combobox(interval_box,state='readonly',values=tuple(INTERVAL_UNITS),width=4)
        unit_choice.set(unit);unit_choice.pack(side='right')
        rule=NUMBER_RULES['interval_seconds'];scale=INTERVAL_UNITS[unit]
        interval_entry=number_input(interval_box,variable=interval,rule=replace(rule,low=rule.low/scale,high=rule.high/scale),on_change=self.on_change,width=8)
        interval_entry.pack(side='right',padx=4)
        Tooltip(interval_entry,'Interval in seconds or minutes, from 10 seconds to 24 hours. The countdown starts after use and the wait; pausing stops it.')
        Tooltip(unit_choice,'Changing the unit converts the value and preserves the interval duration.')
        timer_box=ttk.Frame(frame)
        ttk.Label(timer_box,text='Next use',style='Muted.TLabel').pack(side='left')
        reset_button=ttk.Button(timer_box,text='↻',width=2,state='disabled',style='Small.TButton');reset_button.pack(side='right')
        timer_label=ttk.Label(timer_box,text='—',font=('Segoe UI',10,'bold'));timer_label.pack(side='right',padx=4)
        Tooltip(reset_button,'Reset this countdown only. The skill is used after the current target, or after resuming if paused. Profile settings stay unchanged.')
        state_label=ttk.Label(frame,style='Muted.TLabel',wraplength=190);state_label.pack(fill='x',pady=(3,0))
        icons=ttk.Frame(frame)
        active_row=ttk.Frame(icons);active_row.pack(fill='x')
        pick=ttk.Button(active_row,text='Active icon');pick.pack(side='left')
        label=ttk.Label(active_row,style='Muted.TLabel');label.pack(side='left',padx=4)
        clear_active=ttk.Button(active_row,text='×',width=2,style='Small.TButton');clear_active.pack(side='right')
        reuse_active=ttk.Button(icons,text='Reuse inactive area')
        reuse_active.pack(fill='x',pady=(3,5))
        inactive_row=ttk.Frame(icons);inactive_row.pack(fill='x',pady=(4,0))
        inactive_pick=ttk.Button(inactive_row,text='Inactive icon');inactive_pick.pack(side='left')
        inactive_label=ttk.Label(inactive_row,style='Muted.TLabel');inactive_label.pack(side='left',padx=4)
        clear=ttk.Button(inactive_row,text='×',width=2,style='Small.TButton');clear.pack(side='right')
        reuse_inactive=ttk.Button(icons,text='Reuse active area');reuse_inactive.pack(fill='x',pady=(3,0))
        Tooltip(pick,'Drag over the active icon in the current snapshot, including its activity indicator.')
        Tooltip(inactive_pick,'Drag over the inactive icon in the current snapshot. Without it, detection checks whether the active icon disappears.')
        Tooltip(reuse_active,'Capture the active appearance from the current snapshot in the saved inactive template area. Activate the buff and refresh the snapshot first.')
        Tooltip(reuse_inactive,'Capture the inactive appearance from the current snapshot in the saved active template area. Deactivate the buff and refresh the snapshot first.')
        Tooltip(clear_active,'Delete the active template and keep the inactive one. Without an active template, recalibration is required.')
        Tooltip(clear,'Clear the inactive template and detect disappearance of the active icon')
        row=SkillRow(frame,name,shortcut,wait,mode,skill.icon if skill else None,label,number,up,down,
                     skill.inactive_icon if skill else None,inactive_label,icons,interval,unit,unit_choice,interval_box,interval_entry,timer_label,state_label,timer_box,reset_button,
                     reuse_active,reuse_inactive,clear_active,clear)
        unit_choice.bind('<<ComboboxSelected>>',lambda event:self.set_interval_unit(row,unit_choice.get()))
        def rename(event):self._layout();self.on_change()
        name_entry.bind('<KeyRelease>',rename)
        def reset_timer():
            index=sum(bool(item.shortcut.get().strip()) for item in self.rows[:self.rows.index(row)])
            if self.on_reset is not None:self.on_reset(index)
        reset_button.configure(command=reset_timer)
        up.configure(command=lambda:self.move(self.rows.index(row),-1))
        down.configure(command=lambda:self.move(self.rows.index(row),1))
        delete.configure(command=lambda:self.remove(self.rows.index(row)))
        pick.configure(command=lambda:self.pick(row))
        inactive_pick.configure(command=lambda:self.pick(row,False))
        reuse_active.configure(command=lambda:self.pick(row,True,True))
        reuse_inactive.configure(command=lambda:self.pick(row,False,True))
        def clear_icon(active):
            self.on_begin();self.set_icon(self.rows.index(row),None,active);self.on_change()
        clear.configure(command=lambda:clear_icon(False))
        clear_active.configure(command=lambda:clear_icon(True))
        def mode_changed(event):self._mode_changed(row);self.on_change()
        choice.bind('<<ComboboxSelected>>',mode_changed)
        self.rows.append(row);self._thumbnail(row);self._mode_changed(row)
