"""Explicit disposable dependency/UI/storage checks; never touches a game client."""
from dataclasses import replace
import json
from pathlib import Path
import tempfile
import traceback

from .version import build_info


class NoGameAPI:
    def list_windows(self):return []
    def start_hotkeys(self):pass
    def close_hotkeys(self):pass
    def hotkey_events(self):return False,False
    def key_held(self,key):return False


def run_self_test(report:Path)->int:
    report=report.resolve();report.parent.mkdir(parents=True,exist_ok=True)
    result={'ok':False,'build':build_info(),'checks':[]}
    root=app=None
    try:
        import cv2
        import numpy as np
        import tkinter as tk
        from PIL import Image,ImageTk
        from .app import App
        from .config import Profile,Region,ColorConfig
        from .vision import detect_candidates
        from .theme import UISettings,apply_theme
        with tempfile.TemporaryDirectory(prefix='MetinBot-selftest-',dir=report.parent) as folder:
            directory=Path(folder)
            assert directory.resolve().is_relative_to(report.parent)
            root=tk.Tk();root.withdraw()
            app=App(root,NoGameAPI(),data_directory=directory)
            root.update()
            for theme in ('dark','light'):
                apply_theme(app,theme)
                root.update_idletasks()
            photo=ImageTk.PhotoImage(Image.new('RGB',(8,8),'red'),master=root)
            assert photo.width()==8
            result['checks'].append('tk-pillow-themes')
            profile=replace(Profile.create('Self-test'),region=Region(0,0,1,1),color=ColorConfig((20,180,200)))
            hsv=np.zeros((240,320,3),dtype=np.uint8)
            hsv[60:90,80:100]=(20,180,200)
            frame=cv2.cvtColor(hsv,cv2.COLOR_HSV2BGR)
            assert len(detect_candidates(frame,profile))==1
            result['checks'].append('opencv-numpy-detection')
            stored=app.store.save(profile)
            assert app.store.load(stored.id)[0]==stored
            UISettings('dark',last_mounted=True).save(directory/'ui-settings.json')
            assert UISettings.load(directory/'ui-settings.json').last_mounted
            result['checks'].append('profile-preference-roundtrip')
            app.worker.cancel_event.set();app.worker.shutdown.set();app.worker.thread.join(3)
            assert not app.worker.thread.is_alive()
            root.destroy();root=app=None
        result['ok']=True
    except Exception:
        result['error']=traceback.format_exc()
    finally:
        if app is not None:
            app.worker.cancel_event.set();app.worker.shutdown.set();app.worker.thread.join(3)
        if root is not None:root.destroy()
        report.write_text(json.dumps(result,indent=2)+'\n',encoding='utf-8')
    return 0 if result['ok'] else 1
