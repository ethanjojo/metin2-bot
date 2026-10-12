"""Disposable UI verification fixture. Uses App unchanged; refuses all game input."""
from pathlib import Path
import sys
sys.path.insert(0,str(Path(__file__).resolve().parent.parent))
import tkinter as tk
from dataclasses import replace
import cv2
from metin_bot import app as module
from metin_bot.windows import GameWindow, WindowGeometry
from metin_bot.calibration import Mode

frame=cv2.imread('C:/Users/horak/AppData/Local/Temp/codex-clipboard-85e5286f-0fda-401c-8f1a-885128f14f6a.png')
icons=cv2.imread('C:/Users/horak/AppData/Local/Temp/codex-clipboard-5ebd1dbd-e5a4-4690-babb-8857743effb5.png')
if frame is None or icons is None:raise RuntimeError('Fixture images missing')
frame[30:30+icons.shape[0],50:50+icons.shape[1]]=icons

class PreviewAPI:
    def list_windows(self):return [GameWindow(1,1,'Ukázkový snímek · UI test','verzumclient_x64.exe')]
    def start_hotkeys(self):pass
    def close_hotkeys(self):pass
    def hotkey_events(self):return False,False
    def key_held(self,key):return False
    def inspect(self,window):return WindowGeometry(0,0,frame.shape[1],frame.shape[0],True,True)
    def capture(self,window):return frame.copy()
    def request_activation(self,window):return True
    def check_input_access(self,window):raise RuntimeError('UI fixture: no game input allowed')

fixture_data=Path(__file__).resolve().parent/'compact-ui-fixture-data'
root=tk.Tk();editor=module.App(root,PreviewAPI(),data_directory=fixture_data)
root.title('Metin Bot — UI test bez herních vstupů')
editor.last_frame=frame.copy();editor.calibration.freeze(frame)
editor.calibration.drag(Mode.WORLD,(30,170),(1050,740))
editor.calibration.drag(Mode.MASK,(900,20),(1050,160))
editor.calibration.color((757,285))
editor.calibration.drag(Mode.PANEL,(30,15),(250,120))
editor.calibration.drag(Mode.TEMPLATE,(50,30),(180,78))
editor.mark_dirty();editor.render_template()
editor.frozen=True
editor.begin_calibration(Mode.WORLD)
root.after(600,editor.render)
root.mainloop()
