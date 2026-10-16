"""Build sanitized, deterministic migration fixtures; never reads user profiles."""
from __future__ import annotations
import ast
from dataclasses import asdict, replace
import json
from pathlib import Path
import statistics
import subprocess
import sys
import time

ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
import cv2
import numpy as np
from metin_bot.config import Profile, NUMBER_RULES, ColorConfig, PanelConfig, Region
from metin_bot.vision import detect_candidates, panel_score
from metin_bot.controller import Controller, Observation

GROUPS = {
    'app': ('P01 P08 P15 P27', 'metin-desktop/tests/ui_state.rs'),
    'config': ('P26', 'metin-core/tests/profile.rs'),
    'profiles': ('P01 P26 P27', 'metin-desktop/tests/storage.rs'),
    'data_paths': ('P27', 'metin-desktop/tests/import.rs'),
    'cli': ('P28', 'metin-desktop/tests/cli.rs'),
    'vision': ('P04 P05 P06', 'metin-vision/tests/parity.rs'),
    'controller': ('P07 P08 P09 P22', 'metin-core/tests/controller.rs'),
    'buffs': ('P18 P22', 'metin-core/tests/buffs.rs'),
    'buff_detection': ('P21', 'metin-vision/tests/parity.rs'),
    'hotbar_buffs': ('P20 P21', 'metin-vision/tests/parity.rs'),
    'buff_scheduling': ('P19 P22', 'metin-core/tests/buffs.rs'),
    'skill_intervals': ('P19 P24', 'metin-core/tests/buffs.rs'),
    'worker': ('P08 P09 P11 P23 P24 P25', 'metin-desktop/tests/worker.rs'),
    'runtime': ('P03 P07 P10 P22 P23', 'metin-desktop/tests/runtime.rs'),
    'windows': ('P02 P10 P11 P29', 'metin-platform/tests/windows_input.rs'),
    'calibration': ('P03 P12 P20', 'metin-desktop/tests/editor.rs'),
    'editing': ('P12 P13', 'metin-desktop/tests/editor.rs'),
    'editor_tools': ('P13 P14', 'metin-desktop/tests/history.rs'),
    'preview_ui': ('P03 P15', 'metin-desktop/tests/ui_state.rs'),
    'buff_ui': ('P17 P19 P20 P23', 'metin-desktop/tests/ui_state.rs'),
    'keybind_entry': ('P18', 'metin-desktop/tests/keybind.rs'),
    'number_input': ('P16', 'metin-desktop/tests/numeric.rs'),
    'session': ('P25', 'metin-core/tests/session.rs'),
    'run_ui': ('P25', 'metin-desktop/tests/ui_state.rs'),
    'theme': ('P16', 'metin-desktop/tests/ui_state.rs'),
    'switch_ui': ('P23', 'metin-desktop/tests/ui_state.rs'),
    'sidebar_scroll': ('P15', 'metin-desktop/tests/ui_state.rs'),
    'build': ('P28 P29', 'metin-desktop/tests/self_test.rs'),
    'launcher': ('P29', 'metin-desktop/tests/release.rs'),
    'release': ('P30', 'metin-desktop/tests/release.rs'),
    'gitignore': ('P31', 'metin-desktop/tests/release.rs'),
}

def write(path, value):
    path.parent.mkdir(parents=True, exist_ok=True)
    path.write_text(json.dumps(value, ensure_ascii=False, indent=2, allow_nan=False)+'\n', encoding='utf-8')

def main():
    out = ROOT/'tests/parity'
    tests=[]
    for file in sorted((ROOT/'tests').glob('test_*.py')):
        group=file.stem.removeprefix('test_')
        ids,target=GROUPS[group]
        tree=ast.parse(file.read_text(encoding='utf-8-sig'))
        for cls in [n for n in tree.body if isinstance(n,ast.ClassDef)]:
            for n in cls.body:
                if isinstance(n,ast.FunctionDef) and n.name.startswith('test_'):
                    tests.append(dict(source=f'{file.stem}.{cls.name}.{n.name}', parity=ids.split(), target='crates/'+target, status='pending'))
    commit=subprocess.check_output(['git','rev-parse','HEAD'],cwd=ROOT,text=True).strip()
    write(out/'manifest.json', dict(baseline_commit=commit, python=sys.version.split()[0], opencv=cv2.__version__,numpy=np.__version__, test_count=len(tests), tests=tests))
    write(out/'profiles/default.json', Profile.create('Fixture').to_dict() | {'id':'8591a09f-25dc-4ca4-b858-cafe68231dfc'})
    write(out/'profiles/rules.json', {k:asdict(v) for k,v in NUMBER_RULES.items()})
    rng=np.random.default_rng(4)
    template=rng.integers(0,255,(10,20,3),dtype=np.uint8)
    profile=replace(Profile.create('Fixture'), id='8591a09f-25dc-4ca4-b858-cafe68231dfc', region=Region(.1,.2,.9,.9),color=ColorConfig((0,200,200)),panel=PanelConfig(Region(0,0,.4,.15)),frame_size=(800,600),template_file='template.png')
    write(out/'profiles/calibrated.json', profile.to_dict())
    (out/'frames').mkdir(parents=True,exist_ok=True)
    cv2.imwrite(str(out/'frames/template.png'),template)
    cases=[]
    for panel in (False,True):
        frame=np.full((600,800,3),160,np.uint8)
        frame[300:320,400:420]=cv2.cvtColor(np.uint8([[[0,200,200]]]),cv2.COLOR_HSV2BGR)[0,0]
        if panel:frame[10:20,10:30]=template
        filename='panel.png' if panel else 'search.png'
        cv2.imwrite(str(out/'frames'/filename),frame)
        timings=[]
        for _ in range(30):
            start=time.perf_counter(); candidates=detect_candidates(frame,profile);score=panel_score(frame,template,profile.panel.region);timings.append((time.perf_counter()-start)*1000)
        cases.append(dict(frame=filename,candidates=[asdict(c) for c in candidates],panel_score=score,detection_ms_median=statistics.median(timings)))
    write(out/'frames/expected.json', cases)
    c=Controller(profile);c.start(0.0);trace=[]
    for now,present in [(0.,False),(.25,False),(.5,False),(.75,True),(1.,True),(1.25,True),(1.5,False),(1.75,False),(2.,False),(3.,False),(3.25,False),(3.5,False)]:
        action=c.step(now,Observation(True,tuple(detect_candidates(frame,profile)),present))
        trace.append(dict(now=now,panel=present,action=None if action is None else dict(kind=action.kind.value,point=action.point,shortcut=action.shortcut),phase=c.phase.value,running=c.running,stats=asdict(c.session.snapshot())))
    write(out/'traces/target-cycle.json',trace)
    print(json.dumps(dict(tests=len(tests),fixtures=len(cases),commit=commit)))

if __name__=='__main__':main()
