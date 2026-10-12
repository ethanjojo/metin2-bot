"""Disposable visual fixture for status, counters and skill timers; no game input."""
from pathlib import Path
import runpy
from dataclasses import replace
import sys
import tkinter as tk
from unittest.mock import patch
import cv2
from metin_bot.config import BuffSkill,BuffIcon
from metin_bot.buff_detection import Presence
from metin_bot.runtime import Sample
from metin_bot.session import Statistics
from metin_bot.run_ui import update_overview

with patch.object(tk.Tk,'mainloop',lambda self:None):
    context=runpy.run_path(str(Path(__file__).with_name('ui_fixture.py')))
app,root=context['editor'],context['root']
app.worker.cancel_event.set();app.worker.shutdown.set();app.worker.thread.join(3)
while not app.worker.results.empty():app.worker.results.get_nowait()
active=BuffIcon.capture(cv2.imread(str(Path(__file__).parent.parent/'tests/data/hotbar-active.png')))
app.buff_list.load((BuffSkill('3',interval_seconds=14400,name='Strach'),BuffSkill('4',interval_seconds=600,name='Brnění'),BuffSkill('2',mode='inactive',icon=active,name='Čepel')))
app.buff_fields['enabled'].set(True)
app.saved_profile=app.settings_profile()
app.last_buff_sample=Sample(None,[],0,None,'Target active','Target confirmed; the ready buff will be used after it is destroyed.',True,
    profile_id=app.saved_profile.id,window_hwnd=1,buff_seconds=0,buff_skill_seconds=(8230,0,None),buff_states=(Presence.UNKNOWN,Presence.UNKNOWN,Presence.ACTIVE),mounted=True)
app.running=True;app.status.set('Target active');app.update_run_controls()
app.reason.set(app.last_buff_sample.reason)
app.update_buff_status()
update_overview(app,'Target active',Statistics(3661,128,140,12),88.5)
app.sidebar.winfo_children()[0].select(0 if '--calibration' in sys.argv else 1 if '--settings' in sys.argv else 2)
if app.ui_settings.theme!=('dark' if '--dark' in sys.argv else 'light'):app.toggle_theme()
root.title('Metin Bot — English UI test' + (' dark' if '--dark' in sys.argv else ' light'))
root.geometry('980x640+50+50' if '--compact' in sys.argv else '1280x840+50+50')
if '--empty' in sys.argv:
    app.last_frame=None;app.frozen=False;app.preview_busy=False
    app.running=False;app.status.set('Stopped');app.update_run_controls()
    app.last_buff_sample=replace(app.last_buff_sample,running=False,state='Stopped')
    app.update_buff_status();update_overview(app,'Stopped',Statistics(3661,128,140,12))
root.after(900,app.render)
root.after(90000,root.destroy)
root.mainloop()
