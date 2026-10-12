"""A centered, theme-aware empty state; the canvas keeps owning preview layout."""
from .theme import PALETTES
from .ui_controls import icon


def render_empty_preview(app):
    canvas=app.canvas;p=PALETTES[app.ui_settings.theme]
    width=max(1,canvas.winfo_width());height=max(1,canvas.winfo_height())
    x,y=width/2,height/2
    if app.preview_busy:
        title='Capturing snapshot…';text='Waiting for a new snapshot from the selected game window.'
    elif app.selected_window() is None:
        title='Select a game window';text='Select a game window above, then click Snapshot.'
    else:
        title='No snapshot yet';text='Click Snapshot to preview the game for calibration.'
    key='empty-preview-'+app.ui_settings.theme
    if key not in app.icons:app.icons[key]=icon('picture',p['muted'],size=48)
    canvas.create_image(x,y-42,image=app.icons[key],anchor='center',tags='empty-icon')
    canvas.create_text(x,y+8,text=title,anchor='center',fill=p['fg'],font=('Segoe UI',14,'bold'),tags='empty-title')
    canvas.create_text(x,y+36,text=text,anchor='n',justify='center',width=min(420,max(80,width-40)),
                       fill=p['muted'],font=('Segoe UI',10),tags='empty-description')
