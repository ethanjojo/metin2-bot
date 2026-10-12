"""Exercise automatic mount declarations in the real editor, without game input."""
from pathlib import Path
import runpy
import tkinter as tk
from unittest.mock import patch
from metin_bot.runtime import Sample
from metin_bot.theme import UISettings

with patch.object(tk.Tk,'mainloop',lambda self:None):
    context=runpy.run_path(str(Path(__file__).with_name('ui_fixture.py')))
app,root=context['editor'],context['root']
try:
    app.worker.cancel_event.set();app.worker.shutdown.set();app.worker.thread.join(3)
    while not app.worker.results.empty():app.worker.results.get_nowait()
    def descendants(widget):
        for child in widget.winfo_children():
            yield child
            yield from descendants(child)
    widgets=list(descendants(root))
    assert not any(w.winfo_class()=='TButton' and w.cget('text')=='Potvrdit stav mountu' for w in widgets)
    switch=app.mount_switch
    assert not any(w.winfo_class() in ('TLabel','TCheckbutton') and w.cget('text')=='I am mounted' for w in widgets)
    assert str(switch.cget('textvariable'))==str(app.mount_status)
    before=app.settings_profile()
    requests=[]
    app.worker.confirm_mount=lambda profile,window,mounted:requests.append((profile,window,mounted)) or len(requests)
    selected=app.window_choice.current()
    app.window_choice.set('Select a game window')
    app.ui_settings=UISettings(app.ui_settings.theme,last_mounted=False)
    app.last_buff_sample=None;app.update_buff_status();root.update_idletasks()
    assert not switch.instate(['disabled'])
    switch.invoke()
    assert app.mount_status.get()=='On' and app.ui_settings.last_mounted and not requests
    assert UISettings.load(app.data_directory/'ui-settings.json').last_mounted is True
    switch.invoke()
    assert app.mount_status.get()=='Off' and not app.ui_settings.last_mounted and not requests
    app.window_choice.current(selected)
    app.ui_settings=UISettings(app.ui_settings.theme,last_mounted=False)
    app.last_buff_sample=None;app.update_buff_status();root.update_idletasks()
    assert app.mount_choice.get()==0 and not switch.instate(['alternate'])
    switch.invoke()
    assert requests[-1][2] is True and app.pending_mount_confirmation==1
    assert app.settings_profile()==before
    def acknowledge(mounted,revision):
        app.worker.results.put(Sample(None,[],0,None,'Paused','Stav mountu nastaven.',False,
            profile_id=before.id,window_hwnd=1,mounted=mounted,mount_confirmation_id=revision))
        app.poll()
    acknowledge(False,0)
    assert app.pending_mount_confirmation==1 and app.mount_choice.get()==1
    acknowledge(True,1)
    assert app.pending_mount_confirmation is None and app.mount_status.get()=='On'
    assert UISettings.load(app.data_directory/'ui-settings.json').last_mounted is True
    switch.invoke();assert requests[-1][2] is False
    acknowledge(False,2)
    assert app.mount_choice.get()==0 and app.mount_status.get()=='Off'
    assert UISettings.load(app.data_directory/'ui-settings.json').last_mounted is False
    assert app.settings_profile()==before and 'initial_mounted' not in before.to_dict()['buffs']
    print('Tk mount switch PASS: immediate commands, acknowledgement, stale sample ignored, both states, no confirmation button or profile mutation.')
finally:
    app.worker.cancel_event.set();app.worker.shutdown.set();app.worker.thread.join(3);root.destroy()
