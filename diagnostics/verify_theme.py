"""Real-widget appearance checks without saving profiles or sending game inputs."""
from pathlib import Path
import runpy
import tkinter as tk
from tkinter import ttk
from unittest.mock import patch
import numpy as np
from metin_bot.calibration import Mode
from metin_bot.theme import UISettings,PALETTES

with patch.object(tk.Tk,'mainloop',lambda self:None):
    context=runpy.run_path(str(Path(__file__).with_name('ui_fixture.py')))
app,root=context['editor'],context['root']
try:
    app.worker.cancel_event.set();app.worker.shutdown.set();app.worker.thread.join(3)
    root.update();app.update_editor()
    profile=app.settings_profile();frame=app.calibration.frame.copy()
    dirty=app.dirty;queued=app.worker.commands.qsize()
    notebook=app.sidebar.winfo_children()[0]
    root.focus_force();notebook.select(0);root.update()
    for index in (1,2,0,1):
        x=next(x for x in range(notebook.winfo_width()) if notebook.identify(x,8)=='tab' and notebook.index('@%s,8'%x)==index)
        notebook.event_generate('<Button-1>',x=x,y=8);root.update()
        assert notebook.index('current')==index
        assert root.focus_get() is notebook
    settings_host=root.nametowidget(notebook.tabs()[1])
    settings_canvas=next(w for w in settings_host.winfo_children() if isinstance(w,tk.Canvas))
    settings_content=settings_canvas.nametowidget(settings_canvas.itemcget(1,'window'))
    root.update_idletasks()
    for field in settings_content.winfo_children():
        if isinstance(field,(ttk.Entry,ttk.Spinbox)):
            assert field.winfo_x()>=10
            assert settings_content.winfo_width()-(field.winfo_x()+field.winfo_width())>=10
    assert {Mode.WORLD,Mode.MASK,Mode.COLOR,Mode.PANEL,Mode.TEMPLATE}.issubset(app.step_photos)
    app.mount_advanced.toggle();root.update_idletasks()
    assert app.buff_fields['mount_shortcut'].get()==profile.buffs.mount_shortcut
    app.mount_advanced.toggle();root.update_idletasks()
    for label,theme in (('Tmavé','dark'),('Světlé','light')):
        if app.ui_settings.theme!=theme:app.theme_button.invoke()
        root.update_idletasks()
        assert app.theme_button.icon_name==('sun' if theme=='dark' else 'moon')
        assert UISettings.load(app.data_directory/'ui-settings.json').theme==theme
        assert app.canvas.cget('background')==PALETTES[theme]['preview']
        import ctypes
        from ctypes import wintypes as W
        user=ctypes.WinDLL('user32');user.GetAncestor.argtypes=[W.HWND,W.UINT];user.GetAncestor.restype=W.HWND
        hwnd=user.GetAncestor(root.winfo_id(),2)
        dwm=ctypes.WinDLL('dwmapi')
        dwm.DwmGetWindowAttribute.argtypes=[W.HWND,W.DWORD,W.LPVOID,W.DWORD]
        dwm.DwmGetWindowAttribute.restype=W.LONG
        # Caption/text color attributes are set-only on this Windows build.
        dark_frame=W.BOOL()
        assert dwm.DwmGetWindowAttribute(hwnd,20,ctypes.byref(dark_frame),ctypes.sizeof(dark_frame))==0
        assert bool(dark_frame.value)==(theme=='dark')
        assert ttk.Style(root).lookup('TEntry','fieldbackground')==PALETTES[theme]['field']
        style=ttk.Style(root)
        for name in ('TCombobox','TSpinbox'):
            for state in (('active',),('pressed',),('readonly','active'),('readonly','pressed')):
                assert style.lookup(name,'background',state)==PALETTES[theme]['selected']
                assert style.lookup(name,'arrowcolor',state)==PALETTES[theme]['fg']
            assert style.lookup(name,'arrowcolor',('disabled',))==PALETTES[theme]['disabled']
        for name in ('TNotebook','TNotebook.Tab'):
            for state in ((),('selected',),('active',),('disabled',)):
                for edge in ('bordercolor','lightcolor','darkcolor'):
                    assert style.lookup(name,edge,state)==PALETTES[theme]['border'],(name,state,edge)
        for orient in ('horizontal','vertical'):
            separator=ttk.Separator(root,orient=orient)
            assert (separator.winfo_reqwidth() if orient=='vertical' else separator.winfo_reqheight())==1
            separator.destroy()
        assert app.settings_profile()==profile and app.dirty==dirty
        np.testing.assert_array_equal(app.calibration.frame,frame)
        assert app.worker.commands.qsize()==queued
    mode=app.mode
    app.highlight_calibration(Mode.PANEL)
    assert app.mode==mode
    app.highlight_calibration(None)
    app.last_frame=None;app.frozen=False;app.render()
    assert app.info.get()=='' and app.canvas.find_withtag('empty-icon')
    assert app.canvas.itemcget('empty-title','text')=='No snapshot yet'
    print('Appearance PASS: themes persisted outside profiles, source pixels and timers untouched, original thumbnails, disclosure fields retained, hover preserves edit mode.')
finally:
    app.worker.cancel_event.set();app.worker.shutdown.set();app.worker.thread.join(3);root.destroy()
