"""Native editor layout; App owns calibration and worker snapshots."""
import tkinter as tk
from tkinter import ttk
from .calibration import Mode
from .run_ui import build_overview
from .ui_controls import icon_button, icon, Tooltip, message_line, Disclosure
from .keybind_entry import KeybindEntry
from .buff_ui import BuffList
from .config import NUMBER_RULES
from .number_input import number_input

STEP_NAMES={Mode.WORLD:'Search area',Mode.MASK:'Excluded areas',Mode.COLOR:'Metin color',Mode.PANEL:'Target panel',Mode.TEMPLATE:'Fixed crop',Mode.BUFF_AREA:'Active buff area',Mode.BUFF_ICON:'Buff icon'}

def scroll_content(parent):
    host=ttk.Frame(parent);canvas=tk.Canvas(host,highlightthickness=0,width=220)
    bar=ttk.Scrollbar(host,orient='vertical',command=canvas.yview);canvas.configure(yscrollcommand=bar.set)
    bar.pack(side='right',fill='y');canvas.pack(side='left',fill='both',expand=True)
    content=ttk.Frame(canvas,padding=(10,10));item=canvas.create_window(0,0,window=content,anchor='nw')
    def update_scroll(event=None):
        width=canvas.winfo_width();height=canvas.winfo_height()
        canvas.itemconfigure(item,width=width)
        content_height=content.winfo_reqheight()
        canvas.configure(scrollregion=(0,0,width,max(height,content_height)))
        if content_height<=height:
            canvas.yview_moveto(0);bar.state(['disabled'])
        else:bar.state(['!disabled'])
    content.bind('<Configure>',update_scroll)
    canvas.bind('<Configure>',update_scroll)
    # Off-screen canvas windows may not emit Configure when their requested size
    # shrinks. Watch their children too, after Tk has recalculated requested sizes.
    watched=set()
    pending=None
    def schedule_update(event=None):
        nonlocal pending
        if pending is None:pending=canvas.after_idle(flush_update)
    def flush_update():
        nonlocal pending
        pending=None;watch(content);update_scroll()
    def wheel(event):
        if content.winfo_reqheight()>canvas.winfo_height():
            canvas.yview_scroll(-int(event.delta/120),'units')
        return 'break'
    def watch(widget):
        if widget not in watched:
            watched.add(widget)
            widget.bind('<Configure>',schedule_update,add='+')
            widget.bind('<MouseWheel>',wheel)
        for child in widget.winfo_children():watch(child)
    def mapped(event=None):
        watch(content);schedule_update()
    def destroyed(event):
        nonlocal pending
        if pending is not None:canvas.after_cancel(pending);pending=None
    host.bind('<Map>',mapped)
    canvas.bind('<MouseWheel>',wheel)
    canvas.bind('<Destroy>',destroyed)
    return host,content,canvas

def build(app,root,profile,fields):
    root.title('Metin Bot');root.geometry('1280x840+50+50');root.minsize(980,640)
    root.protocol('WM_DELETE_WINDOW',app.close)
    ttk.Style(root).theme_use('clam')
    app.icons={}
    for name,value in [('status','Stopped'),('reason','Select a game window and capture a snapshot.'),('name',profile.name),
                       ('ready_text',''),('dirty_text','Profile saved'),('active_mode',''),('active_title','Game preview'),
                       ('calibration_hint','Capture a snapshot and select a calibration step.'),('selection_info','No area selected'),
                       ('color_text',''),('preview_state','No snapshot'),('zoom_text','100 %'),('info','The snapshot stays on this computer.'),
                       ('mount_status','')]:
        setattr(app,name,tk.StringVar(value=value))
    app.show_detections=tk.BooleanVar(value=True)
    outer=ttk.Frame(root,padding=12);outer.pack(fill='both',expand=True)
    top=ttk.Frame(outer);top.pack(fill='x',pady=(0,8))
    ttk.Label(top,text='Profile',style='Muted.TLabel').pack(side='left',padx=(0,6))
    app.profile_choice=ttk.Combobox(top,state='readonly',width=14);app.profile_choice.pack(side='left')
    app.profile_choice.bind('<<ComboboxSelected>>',app.select_profile)
    icon_button(app,top,'plus',app.new_profile,'Create a new profile').pack(side='left',padx=(4,14))
    ttk.Label(top,text='Window',style='Muted.TLabel').pack(side='left',padx=(0,6))
    app.window_choice=ttk.Combobox(top,state='readonly',width=18);app.window_choice.pack(side='left',fill='x',expand=True)
    app.window_choice.bind('<<ComboboxSelected>>',app.select_window)
    icon_button(app,top,'refresh',app.refresh_windows,'Refresh game window list').pack(side='left',padx=4)
    icon_button(app,top,'save',app.save,'Save profile',text='Save').pack(side='left',padx=(4,6))
    app.start_button=icon_button(app,top,'play',app.toggle_run,'F7 start / pause in game · F8 stop',text='Start',primary=True)
    app.start_button.pack(side='left')
    app.theme_button=icon_button(app,top,'moon',app.toggle_theme,
        lambda:'Switch to dark theme' if app.ui_settings.theme=='light' else 'Switch to light theme')
    app.theme_button.pack(side='left',padx=(8,0))
    build_overview(app,outer)
    body=ttk.Frame(outer);body.pack(fill='both',expand=True)
    app.sidebar=ttk.Frame(body,width=280);app.sidebar.pack(side='left',fill='y',padx=(0,12));app.sidebar.pack_propagate(False)
    notebook=ttk.Notebook(app.sidebar);notebook.pack(fill='both',expand=True)
    # Focus the tab strip before Tk's class binding; ActivateTab then skips
    # its automatic focusFirst/traverseTo (which selects an entry's text).
    notebook.bind('<Button-1>',lambda event:notebook.focus_set())
    calibration_host,calibration,calibration_scroll=scroll_content(notebook)
    advanced_host,advanced,advanced_scroll=scroll_content(notebook)
    notebook.add(calibration_host,text='Calibration');notebook.add(advanced_host,text='Settings')
    buff_host,buff_panel,_=scroll_content(notebook);notebook.add(buff_host,text='Buffs')
    app.buff_fields={}
    def switch(key,text,tip,parent=buff_panel):
        variable=tk.BooleanVar(value=getattr(profile.buffs,key));app.buff_fields[key]=variable
        control=ttk.Checkbutton(parent,text=text,variable=variable,style='Switch.TCheckbutton',
            command=app.mark_dirty)
        control.pack(anchor='w',pady=4);Tooltip(control,tip)
    switch('enabled','Automatic buffs','Refresh skills at their intervals; check toggle skill icons between Metins.')
    switch('fight_mounted','Fight mounted','When disabled, the character stays on foot after buffing.')
    ttk.Separator(buff_panel).pack(fill='x',pady=(10,8))
    mount_box=ttk.Frame(buff_panel)
    mount_box.pack(fill='x',pady=(8,10))
    ttk.Label(mount_box,text='Current mount state',font=('Segoe UI',10,'bold')).pack(anchor='w')
    app.mount_choice=tk.IntVar(value=int(app.ui_settings.last_mounted))
    app.mount_switch=ttk.Checkbutton(mount_box,textvariable=app.mount_status,style='Switch.TCheckbutton',variable=app.mount_choice,onvalue=1,offvalue=0,
                    command=app.edit_mount_state)
    app.mount_switch.pack(anchor='w',pady=4)
    Tooltip(app.mount_switch,'On means mounted; Off means on foot. You can set this before selecting a game window. The last state is remembered across launches. Correct it if you change mounts manually.')
    ttk.Separator(buff_panel).pack(fill='x',pady=(0,8))
    app.mount_advanced=Disclosure(buff_panel,'Mount settings')
    app.mount_advanced.pack(fill='x',pady=(0,8))
    mount_settings=app.mount_advanced.content
    for key,label in (('mount_shortcut','Mount / dismount'),
                      ('mount_wait_seconds','Mount wait (s)'),('key_hold_seconds','Key hold (s)')):
        ttk.Label(mount_settings,text=label,wraplength=190).pack(anchor='w')
        variable=tk.StringVar(value=str(getattr(profile.buffs,key)));app.buff_fields[key]=variable
        if key=='mount_shortcut':
            entry=KeybindEntry(mount_settings,variable=variable,on_change=app.mark_dirty,on_begin=app.worker.pause,
                on_error=app.reason.set,allow_empty=False,width=20)
        else:
            entry=number_input(mount_settings,variable=variable,rule=NUMBER_RULES[key],on_change=app.mark_dirty,width=20)
        entry.pack(fill='x',pady=(3,8))
    ttk.Label(buff_panel,text='Skill order',font=('Segoe UI',10,'bold')).pack(anchor='w',pady=(6,3))
    ttk.Label(buff_panel,text='Click a shortcut and press the keys. The adjacent field is the wait after the skill (s). Delete clears the shortcut.',wraplength=190,style='Muted.TLabel').pack(fill='x',pady=(0,6))
    ttk.Button(buff_panel,text='Edit active buff area',command=lambda:app.begin_buff_pick(-1)).pack(fill='x',pady=(2,5))
    app.buff_area_text=tk.StringVar(value='Area not set')
    ttk.Label(buff_panel,textvariable=app.buff_area_text,wraplength=190,style='Muted.TLabel').pack(fill='x')
    app.buff_list=BuffList(buff_panel,on_change=app.mark_dirty,on_begin=app.prepare_buff_edit,
                           on_error=app.reason.set,on_pick=app.begin_buff_pick,on_reset=app.reset_skill_timer)
    ttk.Label(buff_panel,text='For example, F1, F2 or 1, 2. F7/F8 control the bot. Mount state is inferred from sent shortcuts, without image detection.',wraplength=190,style='Muted.TLabel').pack(fill='x',pady=(8,0))
    app.step_text={};app.step_buttons={};app.step_photos={};app.step_preview_key=None
    for mode,label in STEP_NAMES.items():
        if mode in (Mode.BUFF_AREA,Mode.BUFF_ICON):continue
        text=tk.StringVar(value=label);app.step_text[mode]=text
        button=ttk.Radiobutton(calibration,textvariable=text,variable=app.active_mode,value=mode.value,style='Step.Toolbutton',
                        command=lambda m=mode:app.begin_calibration(m))
        button.pack(fill='x',pady=2);app.step_buttons[mode]=button
        button.bind('<Enter>',lambda event,m=mode:app.highlight_calibration(m))
        button.bind('<Leave>',lambda event:app.highlight_calibration(None))
        Tooltip(button,'Click to edit. Hover to highlight the area in the preview; the thumbnail comes from the game snapshot.')
    ttk.Separator(calibration).pack(fill='x',pady=10)
    app.context_box=ttk.Frame(calibration);app.context_box.pack(fill='x')
    ttk.Label(app.context_box,textvariable=app.selection_info,wraplength=190).pack(fill='x')
    ttk.Label(app.context_box,textvariable=app.calibration_hint,wraplength=190,style='Muted.TLabel').pack(fill='x',pady=(8,10))
    app.region_actions=ttk.Frame(app.context_box)
    ttk.Button(app.region_actions,text='New area',command=app.new_region).pack(side='left')
    ttk.Button(app.region_actions,text='Delete',command=app.delete_selection).pack(side='left',padx=(4,0))
    app.mask_anchor=app.region_actions;app.mask_box=ttk.Frame(app.context_box)
    ttk.Label(app.mask_box,text='Masks',style='Muted.TLabel').pack(anchor='w',pady=(0,5))
    app.mask_list=tk.Listbox(app.mask_box,height=4,exportselection=False,borderwidth=1,highlightthickness=0,font=('Segoe UI',10))
    app.mask_list.pack(fill='x');app.mask_list.bind('<<ListboxSelect>>',app.select_mask)
    app.color_box=ttk.Frame(app.context_box)
    app.swatch=tk.Canvas(app.color_box,height=20,width=32,highlightthickness=1)
    app.swatch.pack(side='left',padx=(0,8));ttk.Label(app.color_box,textvariable=app.color_text,wraplength=165,style='Muted.TLabel').pack(side='left')
    app.template_box=ttk.Frame(app.context_box)
    ttk.Label(app.template_box,text='Panel template',style='Muted.TLabel').pack(anchor='w',pady=(0,5))
    app.template_label=ttk.Label(app.template_box,text='Not set yet');app.template_label.pack(anchor='w')
    ttk.Label(calibration,textvariable=app.ready_text,wraplength=205,style='Muted.TLabel').pack(fill='x',pady=(16,0))
    ttk.Label(advanced,text='Profile name').grid(row=0,column=0,columnspan=2,sticky='w')
    advanced.columnconfigure(0,weight=1)
    name_entry=ttk.Entry(advanced,textvariable=app.name,width=22);name_entry.grid(row=1,column=0,columnspan=2,sticky='ew',pady=(4,12))
    name_entry.bind('<KeyRelease>',lambda e:app.mark_dirty())
    app.entries={};row=2;previous_group=None
    for key,label,group,cast in fields:
        if group!=previous_group:
            ttk.Label(advanced,text={'color':'Metin detection','panel':'Panel recognition','timing':'Run timing'}[group],font=('Segoe UI',10,'bold')).grid(row=row,column=0,columnspan=2,sticky='w',pady=(12,5))
            row+=1;previous_group=group
        ttk.Label(advanced,text=label,wraplength=138).grid(row=row,column=0,sticky='w',pady=4)
        variable=tk.StringVar();app.entries[key]=variable
        def step():app.mark_dirty();app.render()
        entry=number_input(advanced,variable=variable,rule=NUMBER_RULES[key],on_change=step,width=7);entry.grid(row=row,column=1,sticky='e',padx=(6,0),pady=4)
        entry.bind('<KeyRelease>',lambda e:app.mark_dirty())
        entry.bind('<FocusOut>',lambda e:app.render());entry.bind('<Return>',lambda e:app.render());row+=1
    app.preview_panel=right=ttk.Frame(body);right.pack(side='left',fill='both',expand=True)
    toolbar=ttk.Frame(right);toolbar.pack(fill='x',pady=(0,8))
    app.sidebar_button=icon_button(app,toolbar,'sidebar',app.toggle_sidebar,'Show / hide calibration');app.sidebar_button.pack(side='left',padx=(0,8))
    ttk.Label(toolbar,textvariable=app.active_title,font=('Segoe UI',11,'bold')).pack(side='left')
    app.preview_state_label=ttk.Label(toolbar,textvariable=app.preview_state,style='Muted.TLabel');app.preview_state_label.pack(side='left',padx=10)
    app.snapshot_button=icon_button(app,toolbar,'refresh',app.refresh_preview,
        lambda:'Waiting for a new game snapshot…' if app.preview_busy else 'Capture a new snapshot and return to the editor',text='Snapshot',loading=True)
    app.snapshot_button.pack(side='right')
    icon_button(app,toolbar,'eye',app.live_preview,'Live preview while the game is in the foreground',text='Live').pack(side='right',padx=5)
    tools=ttk.Frame(right);tools.pack(fill='x',pady=(0,8))
    app.undo_button=icon_button(app,tools,'undo',app.undo,'Undo · Ctrl+Z');app.undo_button.pack(side='left')
    app.redo_button=icon_button(app,tools,'redo',app.redo,'Redo · Ctrl+Y');app.redo_button.pack(side='left',padx=(4,12))
    ttk.Checkbutton(tools,text='Detections',style='Switch.TCheckbutton',variable=app.show_detections,command=app.render).pack(side='left')
    icon_button(app,tools,'fit',app.reset_zoom,'Fit the full snapshot').pack(side='right',padx=(8,0))
    icon_button(app,tools,'plus',lambda:app.change_zoom(1.25),'Zoom in').pack(side='right')
    ttk.Label(tools,textvariable=app.zoom_text,width=7,anchor='center').pack(side='right')
    icon_button(app,tools,'minus',lambda:app.change_zoom(.8),'Zoom out').pack(side='right')
    app.icons['zoom_label']=icon('zoom')
    app.zoom_label=zoom_label=ttk.Label(tools,image=app.icons['zoom_label'],text=' Zoom',compound='left',style='Muted.TLabel');zoom_label.pack(side='right',padx=(0,8))
    Tooltip(zoom_label,'Scroll to zoom around the cursor; drag with the middle mouse button to pan the snapshot.')
    app.canvas=tk.Canvas(right,highlightthickness=0,takefocus=True);app.canvas.pack(fill='both',expand=True)
    app.canvas.bind('<Configure>',lambda e:app.render())
    for event,command in [('<ButtonPress-1>',app.drag_begin),('<B1-Motion>',app.drag_move),('<ButtonRelease-1>',app.drag_end),
                          ('<Motion>',app.hover),('<Escape>',app.cancel_drag),('<MouseWheel>',app.zoom),
                          ('<ButtonPress-2>',app.pan_begin),('<B2-Motion>',app.pan_move),('<ButtonRelease-2>',app.pan_end)]:app.canvas.bind(event,command)
    app.canvas.bind('<Delete>',lambda e:app.delete_selection());root.bind('<Control-z>',app.undo);root.bind('<Control-y>',app.redo)
    message_line(right,app.info).pack(fill='x',pady=(6,0))
    footer=ttk.Frame(outer);footer.pack(fill='x',pady=(8,0))
    ttk.Label(footer,textvariable=app.dirty_text,style='Muted.TLabel').pack(side='left')
    ttk.Label(footer,text='F7 start / pause in game · F8 stop',style='Muted.TLabel').pack(side='right')
