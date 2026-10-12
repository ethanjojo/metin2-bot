"""Calibration edits an immutable profile against a frozen, local frame."""
from dataclasses import replace
from enum import Enum
import numpy as np
from .config import Profile, Region, ColorConfig, PanelConfig
from .vision import crop, pick_color, validate_frame, validate_template


class Mode(Enum):
    WORLD='world'
    MASK='mask'
    COLOR='color'
    PANEL='panel'
    TEMPLATE='template'
    BUFF_AREA='buff_area'
    BUFF_ICON='buff_icon'


def map_point(x: int, y: int, display_size: tuple[int,int], frame_size: tuple[int,int]) -> tuple[int,int]:
    w,h=display_size
    if not (0<=x<w and 0<=y<h):
        raise ValueError('Click inside the displayed frame, not in the empty space around it.')
    return min(frame_size[0]-1,int(x*frame_size[0]/w)),min(frame_size[1]-1,int(y*frame_size[1]/h))


class Calibration:
    def __init__(self, profile: Profile, template: np.ndarray | None = None):
        self.profile=profile
        self.template=template.copy() if template is not None else None
        self.frame=None

    def freeze(self, frame: np.ndarray) -> None:
        validate_frame(frame)
        size=(frame.shape[1],frame.shape[0])
        if self.profile.frame_size is not None and self.profile.frame_size!=size:
            buffs=replace(self.profile.buffs,icon_region=None,
                          skills=tuple(replace(skill,icon=None,inactive_icon=None) for skill in self.profile.buffs.skills))
            self.profile=replace(self.profile,region=None,masks=(),color=None,panel=PanelConfig(),template_file=None,template_region=None,buffs=buffs)
            self.template=None
        self.profile=replace(self.profile,frame_size=size)
        self.frame=frame.copy()

    def _frame(self) -> np.ndarray:
        if self.frame is None:raise ValueError('Capture a game frame and pause the preview first.')
        return self.frame

    def color(self, point: tuple[int,int]) -> None:
        hsv=pick_color(self._frame(),*point)
        config=replace(self.profile.color,hsv=hsv) if self.profile.color else ColorConfig(hsv)
        self.profile=replace(self.profile,color=config)

    def drag(self, mode: Mode, start: tuple[int,int], end: tuple[int,int]) -> None:
        frame=self._frame();w,h=frame.shape[1],frame.shape[0]
        x1,x2=sorted((start[0],end[0]));y1,y2=sorted((start[1],end[1]))
        if not (0<=x1<x2<w and 0<=y1<y2<h):raise ValueError('Select a rectangle inside the frame.')
        region=Region(x1/w,y1/h,x2/w,y2/h)
        self.set_region(mode,region)

    def region(self, mode: Mode, index: int | None = None) -> Region | None:
        if mode==Mode.WORLD:return self.profile.region
        if mode==Mode.PANEL:return self.profile.panel.region
        if mode==Mode.TEMPLATE:return self.profile.template_region
        if mode==Mode.BUFF_AREA:return self.profile.buffs.icon_region
        if mode==Mode.MASK and index is not None and 0<=index<len(self.profile.masks):
            return self.profile.masks[index]
        return None

    def set_region(self, mode: Mode, region: Region, index: int | None = None) -> None:
        self._frame()
        if mode==Mode.WORLD:self.profile=replace(self.profile,region=region)
        elif mode==Mode.BUFF_AREA:self.profile=replace(self.profile,buffs=replace(self.profile.buffs,icon_region=region))
        elif mode==Mode.MASK:
            masks=list(self.profile.masks)
            if index is None:masks.append(region)
            elif 0<=index<len(masks):masks[index]=region
            else:raise ValueError('The mask no longer exists.')
            self.profile=replace(self.profile,masks=tuple(masks))
        elif mode==Mode.PANEL:
            anchor=self.profile.template_region
            keep=anchor is not None and contains(region,anchor)
            self.profile=replace(self.profile,panel=replace(self.profile.panel,region=region),
                                 template_file=self.profile.template_file if keep else None,
                                 template_region=anchor if keep else None)
            if not keep:self.template=None
        elif mode==Mode.TEMPLATE:
            panel=self.profile.panel.region
            if panel is None or not contains(panel,region):
                raise ValueError('The fixed template crop must be inside the selected panel region.')
            template=crop(self._frame(),region).copy();validate_template(template)
            self.template=template
            self.profile=replace(self.profile,template_file='pending.png',template_region=region)
        else:raise ValueError('This mode uses a click rather than a rectangle.')

    def remove_region(self, mode: Mode, index: int | None = None) -> None:
        if mode==Mode.WORLD:self.profile=replace(self.profile,region=None)
        elif mode==Mode.BUFF_AREA:self.profile=replace(self.profile,buffs=replace(self.profile.buffs,icon_region=None))
        elif mode==Mode.MASK:
            if index is None or not 0<=index<len(self.profile.masks):return
            self.profile=replace(self.profile,masks=tuple(r for i,r in enumerate(self.profile.masks) if i!=index))
        elif mode==Mode.PANEL:
            self.profile=replace(self.profile,panel=replace(self.profile.panel,region=None),template_file=None,template_region=None)
            self.template=None
        elif mode==Mode.TEMPLATE:
            self.profile=replace(self.profile,template_file=None,template_region=None);self.template=None
        elif mode==Mode.COLOR:self.profile=replace(self.profile,color=None)


def contains(outer: Region, inner: Region) -> bool:
    return (outer.left<=inner.left and outer.top<=inner.top and
            outer.right>=inner.right and outer.bottom>=inner.bottom)
