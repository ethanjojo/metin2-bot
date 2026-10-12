"""Palette colors for the application's native Windows title bar."""
import ctypes
from ctypes import wintypes as W
import logging
import sys
from tkinter import Tk

logger=logging.getLogger(__name__)


def apply_title_bar(root:Tk,*,dark:bool,background:str,foreground:str,border:str)->None:
    if sys.platform!='win32':return
    root.update_idletasks()
    user=ctypes.WinDLL('user32',use_last_error=True)
    user.GetAncestor.argtypes=[W.HWND,W.UINT];user.GetAncestor.restype=W.HWND
    # Tk's winfo_id is the client HWND; DWM needs the outer decorated window.
    hwnd=user.GetAncestor(root.winfo_id(),2)  # GA_ROOT
    if not hwnd:
        logger.warning('Could not find the application title-bar window.')
        return
    dwm=ctypes.WinDLL('dwmapi')
    dwm.DwmSetWindowAttribute.argtypes=[W.HWND,W.DWORD,W.LPCVOID,W.DWORD]
    dwm.DwmSetWindowAttribute.restype=W.LONG
    def set_attribute(attribute:int,value)->None:
        result=dwm.DwmSetWindowAttribute(hwnd,attribute,ctypes.byref(value),ctypes.sizeof(value))
        if result<0:logger.warning('Title-bar attribute %s unavailable (HRESULT %#x).',attribute,result & 0xffffffff)
    def color(value:str)->W.DWORD:
        red,green,blue=(int(value[i:i+2],16) for i in (1,3,5))
        return W.DWORD(red | green<<8 | blue<<16)  # COLORREF
    set_attribute(20,W.BOOL(dark))  # DWMWA_USE_IMMERSIVE_DARK_MODE
    set_attribute(34,color(border))  # DWMWA_BORDER_COLOR
    set_attribute(35,color(background))  # DWMWA_CAPTION_COLOR
    set_attribute(36,color(foreground))  # DWMWA_TEXT_COLOR
