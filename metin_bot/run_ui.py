"""Compact session overview shared by calibration and live preview."""
import tkinter as tk
from tkinter import ttk, messagebox
from .session import Statistics, duration
from .ui_controls import icon_button, Tooltip, message_line

TITLES={
    'Checking target':'Checking current target', 'Checking panel':'Checking target panel',
    'Acquiring target':'Waiting for target confirmation', 'Target active':'Target active',
    'Target ended':'Preparing next target', 'Searching':'Searching for a Metin',
    'Rotating':'Rotating camera', 'Armed':'Waiting for game activation',
}
COUNTDOWN_LABELS={'Checking target':'Target check','Checking panel':'Panel check',
                  'Acquiring target':'Acquiring target','Target active':'Target timeout',
                  'Target ended':'Settling','Searching':'Search timeout',
                  'Rotating':'Settling','Buffing':'Buff animation'}

def preview_status(has_frame, frozen, captured_at, now, max_age):
    if not has_frame:return 'No frame'
    if frozen:return 'Frozen frame'
    if captured_at is None or now-captured_at>max_age:return 'Stale frame'
    return 'Live preview'


def build_overview(app,parent):
    box=ttk.Frame(parent,padding=(10,5),style='Overview.TFrame')
    box.pack(fill='x',pady=(0,8))
    app.run_title=tk.StringVar(value='Stopped')
    app.run_timer=tk.StringVar(value='—')
    app.run_timer_caption=tk.StringVar(value='Next step')
    row=ttk.Frame(box,height=52,style='Overview.TFrame');row.pack(fill='x');row.pack_propagate(False)
    status=ttk.Frame(row,width=230,style='Overview.TFrame');status.pack(side='left',fill='both',expand=True)
    status.pack_propagate(False)
    ttk.Label(status,text='Bot status',style='OverviewMuted.TLabel').pack(anchor='w')
    app.run_title_label=ttk.Label(status,textvariable=app.run_title,font=('Segoe UI',11,'bold'),style='Overview.TLabel')
    app.run_title_label.pack(anchor='w',pady=(3,0))
    countdown=ttk.Frame(row,width=118,style='Overview.TFrame');countdown.pack(side='left',fill='y',padx=(8,14))
    countdown.pack_propagate(False)
    ttk.Label(countdown,textvariable=app.run_timer_caption,style='OverviewMuted.TLabel').pack(anchor='w')
    ttk.Label(countdown,textvariable=app.run_timer,font=('Segoe UI',14),style='Overview.TLabel').pack(anchor='w',pady=(2,0))
    metrics=ttk.Frame(row,style='Overview.TFrame');metrics.pack(side='right',fill='y')
    app.metric_values={}
    for key,label,explanation in (
        ('time','Active time','Active run time, excluding pauses. Independent of skill countdowns.'),
        ('completed','Targets ended','Confirmed disappearance of the target panel. This does not independently prove destruction.'),
        ('clicks','Clicks','Target clicks sent; the game panel confirms selection.'),
        ('failed','Unconfirmed','Target panel unconfirmed or target timeout reached.'),
    ):
        ttk.Separator(metrics,orient='vertical').pack(side='left',fill='y',padx=(0,12),pady=4)
        tile=ttk.Frame(metrics,width=104 if key=='time' else 94,style='Overview.TFrame');tile.pack(side='left',fill='y',padx=(0,8))
        tile.pack_propagate(False)
        caption=ttk.Label(tile,text=label,style='OverviewMuted.TLabel');caption.pack(anchor='w')
        value=tk.StringVar(value='00:00:00' if key=='time' else '0');app.metric_values[key]=value
        number=ttk.Label(tile,textvariable=value,font=('Segoe UI',15,'bold'),style='Overview.TLabel');number.pack(anchor='w',pady=(2,0))
        Tooltip(caption,explanation);Tooltip(number,explanation)
    icon_button(app,metrics,'refresh',app.reset_statistics,'Reset session statistics').pack(side='left')
    icon_button(app,metrics,'info',lambda:messagebox.showinfo('Run details',app.reason.get(),parent=parent),
                'Show the full current message').pack(side='left',padx=(4,0))
    # Reserve exactly one line; neither empty messages nor wrapped errors resize the preview.
    message_line(box,app.reason).pack(fill='x',pady=(6,0))
    ttk.Separator(parent).pack(fill='x',pady=(0,8))


def update_overview(app,state,statistics:Statistics,remaining=None):
    app.run_title.set(TITLES.get(state,state))
    app.run_timer_caption.set(COUNTDOWN_LABELS.get(state,'Next step'))
    app.run_timer.set(f'{remaining:.1f} s' if remaining is not None else '—')
    for key,value in (('time',duration(statistics.active_seconds)),('completed',statistics.completed),
                      ('clicks',statistics.clicks),('failed',statistics.failed)):
        app.metric_values[key].set(str(value))
