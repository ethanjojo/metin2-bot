"""Application-only appearance preferences and one palette for native widgets."""
from dataclasses import dataclass, asdict
import json
from pathlib import Path
from tkinter import PhotoImage
from tkinter import ttk
from .profiles import atomic_json

PALETTES={
    'light':dict(bg='#f4f5f7',surface='#ffffff',field='#ffffff',fg='#202b3a',muted='#526072',
                 border='#c6ced9',accent='#235cba',selected='#dce9fc',preview='#e0e5ec',
                 disabled='#8793a3',success='#20704c',warning='#8c5c12',primary_fg='#ffffff',success_bg='#e5f3eb',warning_bg='#fff1d6'),
    'dark':dict(bg='#171e29',surface='#202a38',field='#111925',fg='#e5edf7',muted='#a5b4c7',
                border='#3a4a60',accent='#79aaff',selected='#293f61',preview='#101722',
                disabled='#75849a',success='#83d8ab',warning='#f0c679',primary_fg='#101722',success_bg='#19362b',warning_bg='#3e321e'),
}


@dataclass(frozen=True)
class UISettings:
    theme:str='light'
    last_mounted:bool=False

    def __post_init__(self):
        if not isinstance(self.theme,str) or self.theme not in PALETTES:raise ValueError('Invalid application theme.')
        if type(self.last_mounted) is not bool:raise ValueError('Invalid remembered mount state.')

    @classmethod
    def load(cls,path:Path):
        if not path.exists():return cls()
        try:return cls(**json.loads(path.read_text(encoding='utf-8')))
        except (TypeError,ValueError) as error:raise ValueError('Invalid application appearance settings.') from error

    def save(self,path:Path):atomic_json(path,asdict(self))


def apply_theme(app,theme):
    p=PALETTES[theme];style=ttk.Style(app.root);style.theme_use('clam')
    style.configure('.',font=('Segoe UI',10),background=p['bg'],foreground=p['fg'],
                    bordercolor=p['border'],lightcolor=p['border'],darkcolor=p['border'])
    style.configure('TFrame',background=p['bg'])
    style.configure('TLabel',background=p['bg'],foreground=p['fg'])
    style.configure('Muted.TLabel',foreground=p['muted'])
    style.configure('TButton',background=p['surface'],foreground=p['fg'],padding=(10,6),borderwidth=1)
    style.map('TButton',background=[('disabled',p['bg']),('active',p['selected'])],foreground=[('disabled',p['disabled'])])
    style.configure('Icon.TButton',padding=(7,6))
    style.configure('Small.TButton',padding=(3,3))
    style.configure('Primary.TButton',background=p['accent'],foreground=p['primary_fg'],padding=(12,6))
    style.map('Primary.TButton',background=[('disabled',p['border']),('active',p['accent'])],foreground=[('disabled',p['muted'])])
    for name in ('TEntry','TCombobox','TSpinbox','Shortcut.TEntry','Capture.TEntry'):
        style.configure(name,fieldbackground=p['field'],foreground=p['fg'],insertcolor=p['fg'],padding=4)
        style.map(name,fieldbackground=[('disabled',p['bg']),('readonly',p['selected'] if name=='Capture.TEntry' else p['field'])],
                  foreground=[('disabled',p['disabled']),('readonly',p['fg'])])
    # Replace Clam's light hover/pressed arrow backgrounds with our palette.
    for name in ('TCombobox','TSpinbox'):
        style.configure(name,background=p['surface'],arrowcolor=p['muted'])
        style.map(name,background=[('disabled',p['bg']),('pressed',p['selected']),('active',p['selected'])],
              arrowcolor=[('disabled',p['disabled']),('pressed',p['fg']),('active',p['fg'])],
              bordercolor=[('focus',p['accent'])])
    for name in ('TCheckbutton','TRadiobutton'):
        style.configure(name,background=p['bg'],foreground=p['fg'],indicatorbackground=p['field'],indicatorforeground=p['fg'])
        style.map(name,background=[('active',p['bg'])],foreground=[('disabled',p['disabled'])],
                  indicatorbackground=[('selected',p['selected']),('active',p['surface'])])
    style.configure('Step.Toolbutton',padding=(6,6),background=p['bg'],foreground=p['fg'],anchor='w')
    style.map('Step.Toolbutton',background=[('selected',p['selected']),('active',p['surface'])],foreground=[('selected',p['accent'])])
    for name in ('TNotebook','TNotebook.Tab'):
        style.configure(name,bordercolor=p['border'],lightcolor=p['border'],darkcolor=p['border'])
        # Clam's tab lightcolor state map overrides inherited palette colors.
        style.map(name,**{edge:[('',p['border'])] for edge in ('bordercolor','lightcolor','darkcolor')})
    style.configure('TNotebook',background=p['bg'])
    style.configure('TNotebook.Tab',background=p['surface'],foreground=p['muted'],padding=(8,6))
    style.map('TNotebook.Tab',background=[('selected',p['selected']),('active',p['surface'])],foreground=[('disabled',p['disabled']),('selected',p['fg'])],padding=[])
    style.configure('Vertical.TScrollbar',background=p['border'],troughcolor=p['bg'],arrowcolor=p['muted'])
    style.map('Vertical.TScrollbar',background=[('active',p['muted']),('disabled',p['bg'])])
    style.configure('TSeparator',background=p['border'])
    # The native separator element draws a raised system-colored line.
    # One shared image element gives both orientations a flat palette-colored rule.
    if not hasattr(app.root,'separator_images'):app.root.separator_images={}
    separator_name='PaletteSeparator.'+theme
    if theme not in app.root.separator_images:
        line=PhotoImage(master=app.root,width=1,height=1);line.put(p['border'],to=(0,0,1,1))
        app.root.separator_images[theme]=line
        style.element_create(separator_name,'image',line,sticky='nswe')
    for orient in ('Horizontal','Vertical'):
        style.layout(orient+'.TSeparator',[(separator_name,{'sticky':'nswe'})])
    from .switch_ui import apply_switch_style
    apply_switch_style(app.root,style,theme,p)
    style.configure('Skill.TFrame',background=p['bg'],borderwidth=1,relief='solid')
    style.configure('Overview.TFrame',background=p['surface'])
    style.configure('Overview.TLabel',background=p['surface'],foreground=p['fg'])
    style.configure('OverviewMuted.TLabel',background=p['surface'],foreground=p['muted'],font=('Segoe UI',9))
    style.configure('Tooltip.TLabel',background=p['surface'],foreground=p['fg'])
    for name,key in (('Good','success'),('Warning','warning'),('Neutral','muted'),('Active','accent')):
        background=p['success_bg'] if name=='Good' else p['warning_bg'] if name=='Warning' else p['selected'] if name=='Active' else p['bg']
        style.configure(name+'.TLabel',foreground=p[key],background=background,padding=(4,2))
    app.root.configure(background=p['bg'])
    app.root.option_add('*TCombobox*Listbox.background',p['field'])
    app.root.option_add('*TCombobox*Listbox.foreground',p['fg'])
    app.root.option_add('*TCombobox*Listbox.selectBackground',p['selected'])
    app.root.option_add('*TCombobox*Listbox.selectForeground',p['fg'])
    def visit(widget):
        kind=widget.winfo_class()
        if kind=='Canvas':
            widget.configure(highlightbackground=p['border'])
            if widget is not getattr(app,'swatch',None):widget.configure(background=p['preview'] if widget is app.canvas else p['bg'])
        elif kind=='Listbox':
            widget.configure(background=p['field'],foreground=p['fg'],selectbackground=p['selected'],selectforeground=p['fg'])
        for child in widget.winfo_children():visit(child)
    visit(app.root)
    from .ui_controls import icon
    if hasattr(app,'theme_button'):
        name='moon' if theme=='light' else 'sun'
        app.theme_button.icon_name=name
        if (name,False) not in app.icons:app.icons[(name,False)]=icon(name,p['fg'])
    for key in list(app.icons):
        if isinstance(key,tuple):
            name,primary=key
            app.icons[key]=icon(name,p['primary_fg'] if primary else p['fg'])
    for widget,name,primary in app.icon_widgets:
        if not widget.winfo_exists():continue
        name=widget.icon_name
        value=app.icons[(name,primary)]
        widget.configure(image=value)
        if hasattr(widget,'normal_image'):
            widget.normal_image=value;widget.recolor_frames(p['border'],p['accent'])
    if hasattr(app,'run_title_label'):
        active=app.running or app.status.get()=='Armed'
        app.run_title_label.configure(foreground=p['accent'] if active else p['fg'])
    if hasattr(app,'zoom_label'):
        app.icons['zoom_label']=icon('zoom',p['muted']);app.zoom_label.configure(image=app.icons['zoom_label'])
    from .window_chrome import apply_title_bar
    apply_title_bar(app.root,dark=theme=='dark',background=p['bg'],foreground=p['fg'],border=p['border'])
