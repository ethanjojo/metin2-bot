"""A worker-owned buff sequence, acknowledged only after each key is released."""
from dataclasses import dataclass
from .config import BuffConfig
from .hotkeys import hotkey_keys
from .buff_detection import Presence


@dataclass(frozen=True)
class BuffStep:
    shortcut:str
    wait_seconds:float
    label:str
    mounted_after:bool|None=None
    skill_index:int|None=None


class BuffCycle:
    def __init__(self,config:BuffConfig):
        self.config=config;self.mounted:bool|None=None
        self.due:dict[int,float]={};self.paused_remaining:dict[int,float]|None=None
        self.steps:list[BuffStep]=[];self.pending:BuffStep|None=None
        self.wait_until=0.;self.active=False
        self.skipped=[]

    def resume(self,now):
        if self.paused_remaining is not None:self.due={i:now+left for i,left in self.paused_remaining.items()}
        for i,s in enumerate(self.config.skills):
            if s.mode=='refresh':self.due.setdefault(i,now)
        self.paused_remaining=None

    def pause(self,now):
        self.paused_remaining={i:max(0.,due-now) for i,due in self.due.items()}
        self.steps=[];self.pending=None;self.active=False

    def skill_remaining(self,now,*,paused=False):
        values=(self.paused_remaining or {}) if paused else {i:max(0.,due-now) for i,due in self.due.items()}
        return tuple(values.get(i) if self.config.enabled and s.mode=='refresh' else None
                     for i,s in enumerate(self.config.skills))

    def remaining(self,now,*,paused=False):
        values=[v for v in self.skill_remaining(now,paused=paused) if v is not None]
        return min(values) if values else None

    def reset_timer(self,index:int,now:float,*,paused:bool):
        if type(index) is not int or not 0<=index<len(self.config.skills) or self.config.skills[index].mode!='refresh':
            raise ValueError('Only regular skill countdowns can be reset.')
        self.due[index]=now
        if paused:
            if self.paused_remaining is None:self.paused_remaining={}
            self.paused_remaining[index]=0.

    def _skill_indices(self,now,states,attempted_skills):
        return [i for i,s in enumerate(self.config.skills) if i not in attempted_skills and (
                (s.mode=='refresh' and i in self.due and now>=self.due[i]) or
                (s.mode=='inactive' and i<len(states) and states[i]==Presence.ABSENT))]

    def needed(self,now,states:tuple[Presence,...]=(),*,attempted_skills=frozenset()):
        return self.config.enabled and (self.mounted!=self.config.fight_mounted or bool(self._skill_indices(now,states,attempted_skills)))

    def begin(self,now,states:tuple[Presence,...]=(),*,attempted_skills=frozenset()):
        self.skipped=[]
        self.steps=[];self.pending=None;self.active=True;self.wait_until=now
        indices=self._skill_indices(now,states,attempted_skills)
        if any(self.config.skills[i].mode=='refresh' for i in indices):
            self.skipped=[f'{i+1}: '+('already active' if i<len(states) and states[i]==Presence.ACTIVE else 'uncertain detection')
                          for i,s in enumerate(self.config.skills) if s.mode=='inactive' and i not in indices and i not in attempted_skills]
        mounted=self.mounted
        if mounted and (indices or not self.config.fight_mounted):
            self.steps.append(BuffStep(self.config.mount_shortcut,self.config.mount_wait_seconds,'Dismounting',False))
            mounted=False
        self.steps.extend(BuffStep(self.config.skills[i].shortcut,self.config.skills[i].wait_seconds,
                                  f'{self.config.skills[i].name or f"Buff {i+1}"}: {self.config.skills[i].shortcut}',skill_index=i) for i in indices)
        if not mounted and self.config.fight_mounted:
            self.steps.append(BuffStep(self.config.mount_shortcut,self.config.mount_wait_seconds,'Mounting',True))

    def step(self,now,states:tuple[Presence,...]=())->BuffStep|None:
        if self.pending is not None or now<self.wait_until:return None
        while self.steps:
            step=self.steps[0];index=step.skill_index
            if index is None or self.config.skills[index].mode=='refresh':break
            state=states[index] if index<len(states) else Presence.UNKNOWN
            if state==Presence.ABSENT:break
            self.skipped.append(f'{index+1}: '+('already active' if state==Presence.ACTIVE else 'uncertain detection'))
            self.steps.pop(0)
        if not self.steps:
            self.active=False;return None
        self.pending=self.steps.pop(0)
        return self.pending

    def acknowledge(self,now):
        if self.pending is None:raise RuntimeError('No pending buff step.')
        if self.pending.mounted_after is not None:self.mounted=self.pending.mounted_after
        index=self.pending.skill_index
        if index is not None and self.config.skills[index].mode=='refresh':
            self.due[index]=now+self.pending.wait_seconds+self.config.skills[index].interval_seconds
        self.wait_until=now+self.pending.wait_seconds;self.pending=None
