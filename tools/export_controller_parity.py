"""Capture real reference controller tests as portable behavioral replays."""
import sys
from pathlib import Path
ROOT=Path(__file__).resolve().parents[1]
sys.path[:0]=[str(ROOT),str(ROOT/'tests')]
import json
import unittest
import types
from dataclasses import asdict
import metin_bot.controller as module

Original=module.Controller
scenarios=[]

class RecordingController(Original):
    def __init__(self,profile,session=None):
        super().__init__(profile,session)
        self.record={'profile':profile.to_dict(),'events':[]}
        scenarios.append(self.record)
        self.inside=False
        original_reset=self.buffs.reset_timer
        def reset_timer(cycle,index,now,*,paused):
            original_reset(index,now,paused=paused)
            self.record['events'].append({'kind':'reset_timer','index':index,'now':now,'paused':paused})
        self.buffs.reset_timer=types.MethodType(reset_timer,self.buffs)

    def expected(self,now):
        return dict(running=self.running,phase=getattr(self,'phase',None).value if hasattr(self,'phase') else None,
                    mounted=self.buffs.mounted,stats=asdict(self.session.snapshot()),
                    timers=self.buffs.skill_remaining(now,paused=not self.running))

    def start(self,now):
        event={'kind':'start','now':now,'mounted':self.buffs.mounted}
        self.inside=True
        try:super().start(now)
        finally:self.inside=False
        event['expected']=self.expected(now);self.record['events'].append(event)

    def pause(self,reason,now=None):
        external=not self.inside
        effective=now if now is not None else max(0.,self.last_observation)
        super().pause(reason,now)
        if external:self.record['events'].append(dict(kind='pause',now=effective,reason=reason,expected=self.expected(effective)))

    def step(self,now,observation):
        event={'kind':'step','now':now,'mounted':self.buffs.mounted,
               'observation':dict(ready=observation.ready,candidates=[asdict(c) for c in observation.candidates],panel_present=observation.panel_present,reason=observation.reason,buffs=[s.value for s in observation.buffs])}
        self.inside=True
        try:action=super().step(now,observation)
        finally:self.inside=False
        event['action']=None if action is None else dict(kind=action.kind.value,point=action.point,shortcut=action.shortcut)
        event['expected']=self.expected(now);self.record['events'].append(event)
        return action

    def action_completed(self,now):
        super().action_completed(now)
        self.record['events'].append(dict(kind='ack',now=now,expected=self.expected(now)))

module.Controller=RecordingController
suite=unittest.TestSuite(unittest.defaultTestLoader.loadTestsFromName(name) for name in ['test_controller','test_buffs','test_buff_scheduling'])
result=unittest.TextTestRunner(verbosity=1).run(suite)
if not result.wasSuccessful():raise SystemExit(1)
path=ROOT/'tests/parity/traces/reference-controller-tests.json'
path.parent.mkdir(parents=True,exist_ok=True)
path.write_text(json.dumps({'tests':result.testsRun,'scenarios':scenarios},indent=2,ensure_ascii=False,allow_nan=False)+'\n',encoding='utf-8')
print(f'{result.testsRun} original tests, {len(scenarios)} controller replays, {sum(len(s["events"]) for s in scenarios)} events')
