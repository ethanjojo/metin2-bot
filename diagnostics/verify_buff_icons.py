"""Verify both icon pickers in the actual editor; no game input or profile writes."""
from pathlib import Path
from dataclasses import replace
from types import SimpleNamespace
import runpy
import tkinter as tk
from unittest.mock import patch
import numpy as np
from metin_bot.config import BuffSkill, Region, Profile
from metin_bot.calibration import Mode

with patch.object(tk.Tk, 'mainloop', lambda self: None):
    context=runpy.run_path(str(Path(__file__).with_name('ui_fixture.py')))
app,root=context['editor'],context['root']
try:
    app.buff_list.load((BuffSkill('2',mode='inactive'),))
    app.calibration.set_region(Mode.BUFF_AREA,Region(0,0,.2,.2))
    app.calibration.frame[:100,:100]=np.random.default_rng(12).integers(0,256,(100,100,3),dtype=np.uint8)
    root.update();app.render()
    def drag(active,start,end):
        app.begin_buff_pick(0,active)
        def event(point):
            x,y=app.viewport.to_canvas(point)
            return SimpleNamespace(x=x,y=y)
        app.drag_begin(event(start));app.drag_end(event(end));root.update()
        assert app.buff_pick is None,app.reason.get()
    # Start with inactive; its free rectangle must not require an active image.
    drag(False,(20,20),(44,44))
    row=app.buff_list.rows[0]
    first=row.inactive_icon
    assert first.decode().shape==(25,25,3)
    app.calibration.frame[20:45,20:45]=np.random.default_rng(15).integers(0,256,(25,25,3),dtype=np.uint8)
    row.reuse_active.invoke();root.update()
    assert row.icon.region==first.region and row.inactive_icon==first
    # Independent active selection may choose another area and dimensions.
    drag(True,(50,50),(79,79))
    assert row.icon.decode().shape==(30,30,3) and row.inactive_icon==first
    source=row.icon
    app.calibration.frame[50:80,50:80]=np.random.default_rng(18).integers(0,256,(30,30,3),dtype=np.uint8)
    row.reuse_inactive.invoke();root.update()
    assert row.inactive_icon.region==source.region and row.icon==source
    profile=replace(app.calibration.profile,buffs=replace(app.calibration.profile.buffs,skills=app.buff_list.skills()))
    assert Profile.from_dict(profile.to_dict())==profile
    inactive=row.inactive_icon
    row.clear_active.invoke();root.update()
    assert row.icon is None and row.inactive_icon==inactive
    row.clear_inactive.invoke();root.update()
    assert row.icon is None and row.inactive_icon is None
    print('Tk icon editor PASS: free rectangles, reuse both directions, independent deletion, profile roundtrip; no game input or saves.')
finally:
    app.worker.cancel_event.set();app.worker.shutdown.set()
    app.worker.thread.join(timeout=3);root.destroy()
