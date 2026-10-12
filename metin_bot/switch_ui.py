"""Image indicator for native ttk switches; keyboard and variable handling stay native."""
from PIL import Image,ImageDraw,ImageTk


def apply_switch_style(root,style,theme,palette):
    if not hasattr(root,'switch_images'):root.switch_images={}
    name='Switch.'+theme
    if theme not in root.switch_images:
        def picture(state,disabled=False):
            image=Image.new('RGBA',(176,96));draw=ImageDraw.Draw(image)
            track=palette['bg'] if disabled else palette['accent'] if state=='on' else palette['warning_bg'] if state=='unknown' else palette['border']
            knob=palette['disabled'] if disabled else palette['warning'] if state=='unknown' else palette['field']
            draw.rounded_rectangle((4,12,148,84),radius=36,fill=track,outline=palette['border'],width=4)
            x={'off':40,'on':112,'unknown':76}[state]
            draw.ellipse((x-26,22,x+26,74),fill=knob)
            return ImageTk.PhotoImage(image.resize((44,24),Image.Resampling.LANCZOS),master=root)
        off,on,unknown,disabled_off,disabled_on,disabled_unknown=(picture(state,disabled) for disabled in (False,True) for state in ('off','on','unknown'))
        root.switch_images[theme]=(off,on,unknown,disabled_off,disabled_on,disabled_unknown)
        style.element_create(name,'image',off,('disabled selected',disabled_on),('disabled alternate',disabled_unknown),
                             ('disabled',disabled_off),('alternate',unknown),('selected',on))
    style.layout('Switch.TCheckbutton',[('Checkbutton.padding',{'sticky':'nswe','children':[
        (name,{'side':'left','sticky':''}),
        ('Checkbutton.focus',{'side':'left','sticky':'w','children':[('Checkbutton.label',{'sticky':'nswe'})]})]})])
    style.configure('Switch.TCheckbutton',padding=(0,3),background=palette['bg'],foreground=palette['fg'],focuscolor=palette['accent'])
    style.map('Switch.TCheckbutton',background=[('active',palette['bg'])],foreground=[('disabled',palette['disabled'])])
