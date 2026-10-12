"""Explicit, validated game shortcuts. F7/F8 stay reserved for bot control."""
MODIFIERS={'CTRL':0x11,'SHIFT':0x10,'ALT':0x12}


def hotkey_keys(shortcut:str)->tuple[int,...]:
    if not isinstance(shortcut,str):raise ValueError('The shortcut must be text.')
    parts=[part.strip().upper() for part in shortcut.split('+')]
    if not parts or not parts[-1] or len(set(parts))!=len(parts):
        raise ValueError('Invalid shortcut; use Ctrl+G, F1 or 1, for example.')
    if any(part not in MODIFIERS for part in parts[:-1]):
        raise ValueError('Allowed modifiers: Ctrl, Shift, Alt.')
    key=parts[-1]
    if len(key)==1 and key in 'ABCDEFGHIJKLMNOPQRSTUVWXYZ0123456789':code=ord(key)
    elif key.startswith('F') and key[1:].isdigit() and 1<=int(key[1:])<=12:
        code=0x70+int(key[1:])-1
    else:raise ValueError('Use a letter, digit or F1–F12.')
    if code in (0x76,0x77):raise ValueError('F7 and F8 are reserved for starting and stopping the bot.')
    return tuple(sorted(MODIFIERS[part] for part in parts[:-1]))+(code,)
