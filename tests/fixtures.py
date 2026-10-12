from dataclasses import replace
import cv2
import numpy as np
from metin_bot.config import Profile, ColorConfig, PanelConfig, Region


def calibrated(size=(800,600)):
    template=np.random.default_rng(4).integers(0,255,(10,20,3),dtype=np.uint8)
    p=replace(Profile.create('Test'), region=Region(.1,.2,.9,.9),color=ColorConfig((0,200,200)),
              panel=PanelConfig(Region(0,0,.4,.15)),frame_size=size,template_file='fixture.png')
    return p,template


def scene(size=(800,600),panel=False,template=None):
    width,height=size
    frame=np.full((height,width,3),160,np.uint8)
    frame[height//2:height//2+20,width//2:width//2+20]=cv2.cvtColor(np.uint8([[[0,200,200]]]),cv2.COLOR_HSV2BGR)[0,0]
    if panel:frame[10:20,10:30]=template
    return frame
