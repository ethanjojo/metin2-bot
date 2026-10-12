"""Conservative active-buff detection, restricted to the calibrated icon strip."""
from enum import Enum
import cv2
import numpy as np
from .vision import crop,validate_frame


class Presence(Enum):
    ACTIVE='active'
    ABSENT='absent'
    UNKNOWN='unknown'


class BuffDetector:
    def __init__(self,config,frame_size,sample_interval=.25):
        self.config=config;self.frame_size=frame_size
        self.max_gap=max(1.,sample_interval*1.5)
        self.templates=[s.icon.decode() if s.icon is not None else None for s in config.skills]
        self.inactive_templates=[s.inactive_icon.decode() if s.inactive_icon is not None else None for s in config.skills]
        self.reset()

    def reset(self):
        self.absent=[0]*len(self.templates);self.last=None

    @staticmethod
    def _score(area,template):
        if template is None or any(a<b for a,b in zip(area.shape[:2],template.shape[:2])):return None
        score=float(np.max(cv2.matchTemplate(area,template,cv2.TM_CCOEFF_NORMED)))
        return score if np.isfinite(score) else None

    def observe(self,frame,now):
        validate_frame(frame)
        fresh=self.last is not None and 0<now-self.last<=self.max_gap
        if not fresh:self.absent=[0]*len(self.templates)
        self.last=now
        unknown=tuple(Presence.UNKNOWN for _ in self.templates)
        if self.config.icon_region is None or (frame.shape[1],frame.shape[0])!=self.frame_size:
            self.absent=[0]*len(self.templates);return unknown
        area=crop(frame,self.config.icon_region)
        if np.std(area)<5:
            self.absent=[0]*len(self.templates);return unknown
        states=[]
        for i,template in enumerate(self.templates):
            state=Presence.UNKNOWN
            score=self._score(area,template);inactive=self.inactive_templates[i]
            inactive_score=self._score(area,inactive) if inactive is not None else None
            active_match=score is not None and score>=.90
            absent_match=score is not None and score<=.65
            if inactive is not None:
                # Both known appearances must be distinguishable. A missing or
                # occluded slot is not evidence of the inactive hotbar state.
                active_match=active_match and inactive_score is not None and score-inactive_score>=.08
                absent_match=score is not None and inactive_score is not None and inactive_score>=.90 and inactive_score-score>=.08
            if active_match:state=Presence.ACTIVE
            elif absent_match:
                self.absent[i]+=1
                if self.absent[i]>=3:state=Presence.ABSENT
            if not absent_match:self.absent[i]=0
            states.append(state)
        return tuple(states)
