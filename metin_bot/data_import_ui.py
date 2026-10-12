"""First installed launch can locate an existing source deployment explicitly."""
from pathlib import Path
import tkinter as tk
from tkinter import filedialog,messagebox


class ImportCancelled(Exception):
    pass


def choose_legacy_directory()->Path|None:
    root=tk.Tk();root.withdraw()
    try:
        answer=messagebox.askyesnocancel('First launch',
            'Import profiles and settings from an existing Metin Bot folder?\n\n'
            'Choose Yes to select your old project folder. Original files will be kept.\n'
            'Choose No to start with a new profile.',parent=root)
        if answer is None:raise ImportCancelled()
        if not answer:return None
        folder=filedialog.askdirectory(title='Select the old Metin Bot project or data folder',mustexist=True,parent=root)
        if not folder:raise ImportCancelled()
        path=Path(folder)
        return path
    finally:root.destroy()
