"""Small native controls with consistently drawn icons and keyboard hints."""
import tkinter as tk
from tkinter import ttk
from PIL import Image, ImageDraw, ImageTk


def message_line(parent,variable):
    """Stable one-line geometry, with the complete current message in a tooltip."""
    box=ttk.Frame(parent,height=24);box.pack_propagate(False)
    text=tk.StringVar(parent,value=' '.join(variable.get().split()))
    label=ttk.Label(box,textvariable=text,style='Muted.TLabel',anchor='w')
    label.pack(fill='both',expand=True)
    Tooltip(label,variable.get)
    variable.trace_add('write',lambda *args:text.set(' '.join(variable.get().split())))
    return box


class Tooltip:
    def __init__(self, widget, text):
        self.widget,self.text=widget,text
        self.pending=self.popup=None
        widget.bind('<Enter>',self.schedule,add='+')
        widget.bind('<FocusIn>',self.schedule,add='+')
        for event in ('<Leave>','<FocusOut>','<ButtonPress>','<Destroy>'):
            widget.bind(event,self.hide,add='+')

    def schedule(self,event=None):
        self.hide()
        self.pending=self.widget.after(500,self.show)

    def show(self):
        self.pending=None
        text=self.text() if callable(self.text) else self.text
        if not text:return
        self.popup=tk.Toplevel(self.widget)
        self.popup.wm_overrideredirect(True)
        self.popup.wm_geometry(f'+{self.widget.winfo_rootx()}+{self.widget.winfo_rooty()+self.widget.winfo_height()+5}')
        style=ttk.Style(self.widget)
        tk.Label(self.popup,text=text,background=style.lookup('Tooltip.TLabel','background') or '#202b3a',foreground=style.lookup('Tooltip.TLabel','foreground') or 'white',
                 font=('Segoe UI',9),padx=9,pady=6,wraplength=320,justify='left').pack()

    def hide(self,event=None):
        if self.pending is not None:
            self.widget.after_cancel(self.pending);self.pending=None
        if self.popup is not None:self.popup.destroy();self.popup=None


def icon(name,color='#34465c',size=16):
    """Author geometric icons once, antialiased to the requested display size."""
    image=Image.new('RGBA',(64,64));draw=ImageDraw.Draw(image)
    def line(points):draw.line([(x*4,y*4) for x,y in points],fill=color,width=6,joint='curve')
    if name in ('undo','redo'):
        points=[(3,7),(3,3),(7,3)]
        curve=[(3,7),(8,4),(12,6),(13,10),(11,13),(7,13)]
        if name=='redo':points=[(16-x,y) for x,y in points];curve=[(16-x,y) for x,y in curve]
        line(points);line(curve)
    elif name in ('plus','minus'):
        line([(3,8),(13,8)])
        if name=='plus':line([(8,3),(8,13)])
    elif name=='zoom':
        draw.ellipse((8,8,40,40),outline=color,width=6);line([(10,10),(14,14)])
    elif name=='fit':
        for points in [[(2,6),(2,2),(6,2)],[(10,2),(14,2),(14,6)],[(14,10),(14,14),(10,14)],[(6,14),(2,14),(2,10)]]:line(points)
    elif name=='sidebar':
        draw.rectangle((8,8,56,56),outline=color,width=6);line([(6,2),(6,14)])
    elif name=='refresh':
        draw.arc((8,8,56,56),45,315,fill=color,width=6);line([(12,2),(14,5),(10,5)])
    elif name=='save':
        line([(2,2),(12,2),(14,4),(14,14),(2,14),(2,2)])
        line([(5,2),(5,6),(11,6),(11,2)]);line([(5,14),(5,10),(11,10),(11,14)])
    elif name=='play':
        draw.polygon([(20,8),(56,32),(20,56)],fill=color)
    elif name=='pause':
        draw.rectangle((16,8,24,56),fill=color);draw.rectangle((40,8,48,56),fill=color)
    elif name=='info':
        draw.ellipse((8,8,56,56),outline=color,width=6)
        line([(8,7),(8,12)]);draw.ellipse((29,15,35,21),fill=color)
    elif name=='eye':
        line([(1,8),(5,4),(11,4),(15,8),(11,12),(5,12),(1,8)])
        draw.ellipse((24,24,40,40),outline=color,width=5)
    elif name=='sun':
        draw.ellipse((20,20,44,44),outline=color,width=5)
        for points in [[(8,1),(8,3)],[(8,13),(8,15)],[(1,8),(3,8)],[(13,8),(15,8)],
                       [(3,3),(4.5,4.5)],[(11.5,11.5),(13,13)],[(3,13),(4.5,11.5)],[(11.5,4.5),(13,3)]]:line(points)
    elif name=='moon':
        # A crescent polygon keeps its interior transparent in both palettes.
        draw.polygon([(36,5),(23,7),(13,15),(8,28),(11,41),(22,52),(37,56),(51,50),
                      (57,40),(43,42),(31,36),(25,24),(27,12)],fill=color)
    elif name=='picture':
        draw.rounded_rectangle((6,10,58,54),radius=6,outline=color,width=4)
        draw.ellipse((17,18,27,28),fill=color)
        line([(3,12),(7,8),(10,11),(12,9),(14,12)])
    return ImageTk.PhotoImage(image.resize((size,size),Image.Resampling.LANCZOS))


class LoadingButton(ttk.Button):
    """Animate the existing icon slot, keeping the label and geometry stable."""
    def __init__(self,parent,**kwargs):
        super().__init__(parent,**kwargs)
        self.normal_image=self.cget('image')
        self.animation=None;self.frame_index=0
        self.recolor_frames('#d5dce6','#16498e')
        self.bind('<Destroy>',self._stop_animation,add='+')

    def recolor_frames(self,border,accent):
        self.frames=[]
        for angle in range(0,360,45):
            image=Image.new('RGBA',(64,64));draw=ImageDraw.Draw(image)
            draw.ellipse((8,8,56,56),outline=border,width=6)
            draw.arc((8,8,56,56),angle,angle+100,fill=accent,width=7)
            self.frames.append(ImageTk.PhotoImage(image.resize((16,16),Image.Resampling.LANCZOS),master=self))
        if self.animation is not None:self.configure(image=self.frames[self.frame_index])

    def _stop_animation(self,event=None):
        if self.animation is not None:
            self.after_cancel(self.animation);self.animation=None

    def _animate(self):
        self.configure(image=self.frames[self.frame_index])
        self.frame_index=(self.frame_index+1)%len(self.frames)
        self.animation=self.after(90,self._animate)

    def set_loading(self,loading:bool):
        self._stop_animation()
        if loading:
            self.state(['disabled']);self.frame_index=0;self._animate()
        else:
            self.configure(image=self.normal_image);self.state(['!disabled'])


def icon_button(app,parent,name,command,tip,text='',primary=False,loading=False):
    key=(name,primary)
    if key not in app.icons:app.icons[key]=icon(name,'white' if primary else '#34465c')
    button_type=LoadingButton if loading else ttk.Button
    button=button_type(parent,image=app.icons[key],text=text,compound='left',command=command,
                      width=0,style='Primary.TButton' if primary else 'Icon.TButton')
    Tooltip(button,tip)
    button.icon_name=name
    if not hasattr(app,'icon_widgets'):app.icon_widgets=[]
    app.icon_widgets.append((button,name,primary))
    return button


class Disclosure(ttk.Frame):
    """A local layout choice; collapsing never edits its configuration values."""
    def __init__(self,parent,title,expanded=False):
        super().__init__(parent)
        self.title=title;self.expanded=expanded
        self.button=ttk.Button(self,command=self.toggle)
        self.button.pack(fill='x')
        self.content=ttk.Frame(self,padding=(0,6))
        self.refresh()

    def toggle(self):self.expanded=not self.expanded;self.refresh()

    def refresh(self):
        self.button.configure(text=('▾ ' if self.expanded else '▸ ')+self.title)
        if self.expanded:self.content.pack(fill='x')
        else:self.content.pack_forget()
