"""Small calibration previews from original pixels, never detection overlays."""
import cv2
import numpy as np
from PIL import Image,ImageTk
from .calibration import Mode
from .vision import crop


def update_previews(app):
    if not app.step_buttons:return
    p=app.calibration.profile;frame=app.calibration.frame
    key=(id(frame),p.region,p.masks,app.mask_index,p.panel.region,id(app.calibration.template),p.color.hsv if p.color else None)
    if key==app.step_preview_key:return
    app.step_preview_key=key;app.step_photos={}
    for mode,button in app.step_buttons.items():
        source=None
        if mode==Mode.TEMPLATE:source=app.calibration.template
        elif mode==Mode.COLOR and p.color:
            source=cv2.cvtColor(np.full((24,36,3),p.color.hsv,dtype=np.uint8),cv2.COLOR_HSV2BGR)
        elif frame is not None:
            index=app.mask_index if app.mask_index is not None and 0<=app.mask_index<len(p.masks) else 0
            region=p.region if mode==Mode.WORLD else p.panel.region if mode==Mode.PANEL else p.masks[index] if mode==Mode.MASK and p.masks else None
            if region is not None:source=crop(frame,region)
        if source is not None:
            value=Image.fromarray(cv2.cvtColor(source,cv2.COLOR_BGR2RGB));value.thumbnail((36,28))
            photo=ImageTk.PhotoImage(value);app.step_photos[mode]=photo
            button.configure(image=photo,compound='left')
        else:button.configure(image='')
