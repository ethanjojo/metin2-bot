"""Local shortcut recording; the profile variable never contains UI prompts."""
import tkinter as tk
from tkinter import ttk
from .hotkeys import hotkey_keys
from .ui_controls import Tooltip

MODIFIER_NAMES={'Control_L','Control_R','Shift_L','Shift_R','Alt_L','Alt_R','ISO_Level3_Shift'}


def captured_shortcut(keysym:str,keycode:int,state:int)->str|None:
    if keysym in MODIFIER_NAMES:return None
    # Windows virtual key codes retain the physical number/letter with Shift or
    # a Czech layout; event.char/keysym may instead contain ! or an accented letter.
    if 48<=keycode<=57 or 65<=keycode<=90:key=chr(keycode)
    elif 0x70<=keycode<=0x7B:key=f'F{keycode-0x70+1}'
    else:key=keysym.upper()
    modifiers=[]
    if state&4:modifiers.append('Ctrl')
    if state&1:modifiers.append('Shift')
    if state&0x20000:modifiers.append('Alt')
    shortcut='+'.join(modifiers+[key])
    hotkey_keys(shortcut)
    return shortcut


class KeybindEntry(ttk.Entry):
    def __init__(self,parent,*,variable,on_change,on_begin,on_error,allow_empty=True,**kwargs):
        self.variable=variable;self.on_change=on_change;self.on_begin=on_begin;self.on_error=on_error
        self.allow_empty=allow_empty;self.capturing=False
        self.display=tk.StringVar(parent)
        super().__init__(parent,textvariable=self.display,state='readonly',style='Shortcut.TEntry',**kwargs)
        self.trace=variable.trace_add('write',self.sync_display)
        self.sync_display()
        self.bind('<Button-1>',self.begin)
        self.bind('<FocusIn>',self.begin)
        self.bind('<FocusOut>',self.cancel)
        self.bind('<KeyPress>',self.key_press)
        self.bind('<Destroy>',self.destroyed)
        Tooltip(self,lambda:(self.variable.get() or 'Not set')+' · Click and press a shortcut. Esc cancels.'+
                (' Delete/Backspace clears it.' if self.allow_empty else ' A mount shortcut is required.'))

    def sync_display(self,*args):
        if not self.capturing:self.display.set(self.variable.get() or 'Click and press')

    def begin(self,event=None):
        if not self.capturing:
            self.capturing=True;self.on_begin()
        self.focus_set();self.configure(style='Capture.TEntry')
        self.display.set('Press a shortcut…')
        return 'break'

    def cancel(self,event=None):
        self.capturing=False;self.configure(style='Shortcut.TEntry');self.sync_display()

    def commit(self,shortcut):
        previous=self.variable.get()
        self.cancel();self.variable.set(shortcut)
        if previous!=shortcut:self.on_change()

    def key_press(self,event):
        if event.keysym in ('Tab','ISO_Left_Tab'):
            self.cancel();return None
        if event.keysym=='Escape':self.cancel();return 'break'
        if event.keysym in ('BackSpace','Delete'):
            if self.allow_empty:self.commit('')
            else:self.on_error('The mount shortcut cannot be cleared; press a new combination.')
            return 'break'
        if not self.capturing:
            if event.keysym in ('Return','space'):self.begin()
            return 'break'
        try:
            shortcut=captured_shortcut(event.keysym,event.keycode,event.state)
            if shortcut is not None:self.commit(shortcut)
        except ValueError as error:self.on_error(str(error))
        return 'break'

    def destroyed(self,event):
        self.variable.trace_remove('write',self.trace)
