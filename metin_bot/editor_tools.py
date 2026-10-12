"""Bounded calibration history and one coordinate transform for the preview."""
from dataclasses import dataclass
import math
import numpy as np
from .calibration import Calibration
from .config import Profile


@dataclass(frozen=True)
class Snapshot:
    profile: Profile
    template: np.ndarray | None


class History:
    def __init__(self, calibration: Calibration, limit: int = 32):
        self.limit=limit
        self.states=[]
        self.index=-1
        self.record(calibration)

    @property
    def can_undo(self):return self.index>0

    @property
    def can_redo(self):return self.index<len(self.states)-1

    def record(self, calibration: Calibration):
        previous=self.states[self.index] if self.index>=0 else None
        same_template=previous is not None and (
            previous.template is None and calibration.template is None or
            previous.template is not None and calibration.template is not None and
            np.array_equal(previous.template,calibration.template))
        if previous and previous.profile==calibration.profile and same_template:return
        template=previous.template if same_template else (
            calibration.template.copy() if calibration.template is not None else None)
        if template is not None:template.flags.writeable=False
        self.states=self.states[:self.index+1]
        self.states.append(Snapshot(calibration.profile,template))
        self.states=self.states[-self.limit:]
        self.index=len(self.states)-1

    def _restore(self, calibration):
        state=self.states[self.index]
        calibration.profile=state.profile
        calibration.template=state.template.copy() if state.template is not None else None

    def undo(self, calibration):
        if self.can_undo:self.index-=1;self._restore(calibration)

    def redo(self, calibration):
        if self.can_redo:self.index+=1;self._restore(calibration)


class Viewport:
    def __init__(self):
        self.image_size=(1,1)
        self.canvas_size=(1,1)
        self.base_scale=1.
        self.magnification=1.
        self.offset=(0.,0.)

    @property
    def scale(self):return self.base_scale*self.magnification

    @property
    def display_size(self):
        return tuple(max(1,round(v*self.scale)) for v in self.image_size)

    def fit(self, image_size, canvas_size):
        if image_size!=self.image_size:self.reset()
        self.image_size=image_size
        self.canvas_size=canvas_size
        self.base_scale=min(canvas_size[0]/image_size[0],canvas_size[1]/image_size[1])

    def reset(self):
        self.magnification=1.
        self.offset=(0.,0.)

    def to_image(self, point, clamp=False):
        x,y=((point[i]-self.offset[i])/self.scale for i in (0,1))
        w,h=self.image_size
        if not clamp and not (0<=x<w and 0<=y<h):
            raise ValueError('Click inside the frame, not in the empty space around it.')
        return min(w-1,max(0,math.floor(x+1e-8))),min(h-1,max(0,math.floor(y+1e-8)))

    def to_canvas(self, point):
        return tuple(point[i]*self.scale+self.offset[i] for i in (0,1))

    def zoom_at(self, point, factor):
        old_scale=self.scale
        anchored=tuple((point[i]-self.offset[i])/old_scale for i in (0,1))
        self.magnification=max(1.,min(8.,self.magnification*factor))
        self.offset=tuple(point[i]-anchored[i]*self.scale for i in (0,1))
        if self.magnification==1:self.offset=(0.,0.)

    def pan(self, dx, dy):
        self.offset=(self.offset[0]+dx,self.offset[1]+dy)
