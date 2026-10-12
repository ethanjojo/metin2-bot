"""Windows boundary. All input goes through the guarded InputDriver."""
import ctypes
from ctypes import wintypes as W
from dataclasses import dataclass
from pathlib import PureWindowsPath
import time
import os
import logging
from typing import Callable, Protocol

import cv2
import numpy as np
from PIL import ImageGrab

from .permissions import process_elevated

logger=logging.getLogger(__name__)


class SafetyError(RuntimeError):
    pass


class WindowGoneError(SafetyError):
    """The original HWND no longer identifies the selected game process."""


@dataclass(frozen=True)
class GameWindow:
    hwnd: int
    pid: int
    title: str
    process_name: str = ''


@dataclass(frozen=True)
class WindowGeometry:
    left: int
    top: int
    width: int
    height: int
    visible: bool
    foreground: bool


def choose_window(windows: list[GameWindow]) -> GameWindow:
    if len(windows) != 1:
        raise SafetyError(f'Select one game window; found {len(windows)}.')
    return windows[0]


class InputAPI(Protocol):
    def check_input_access(self, window: GameWindow) -> None: ...
    def inspect(self, window: GameWindow) -> WindowGeometry: ...
    def key_held(self, key: int) -> bool: ...
    def point_clear(self, window: GameWindow, x: int, y: int) -> bool: ...
    def send_key(self, window: GameWindow, key: int, down: bool, *, alt_context:bool=False) -> None: ...
    def send_mouse(self, window: GameWindow, point: tuple[int, int], down: bool) -> None: ...
    def move(self, x: int, y: int, guard: Callable[[], None] | None = None) -> None: ...
    def cursor(self) -> tuple[int, int]: ...
    def emergency_pressed(self) -> bool: ...


class InputDriver:
    def __init__(self, api: InputAPI):
        self.api = api
        self.owned: set[int] = set()
        self._mouse_target: tuple[GameWindow, tuple[int, int]] | None = None
        self._key_target: GameWindow | None = None
        self._expected_cursor: tuple[int, int] | None = None

    def _guard(self, window: GameWindow, baseline: WindowGeometry, cancelled: Callable[[], bool],
               point: tuple[int, int] | None = None) -> None:
        if cancelled() or self.api.key_held(0x77) or self.api.emergency_pressed():
            raise SafetyError('Emergency stop (F8).')
        current = self.api.inspect(window)
        if not current.visible or not current.foreground:
            raise SafetyError('The game must be visible and in the foreground. Run paused.')
        if current != baseline:
            raise SafetyError('The window geometry changed. Check the region and start again.')
        if point and not self.api.point_clear(window, *point):
            raise SafetyError('Another window overlaps the selection area.')
        if self._expected_cursor is not None:
            actual = self.api.cursor()
            if any(abs(a - b) > 3 for a, b in zip(actual, self._expected_cursor)):
                raise SafetyError('Manual mouse movement interrupted selection. Press F7 to resume.')

    def _begin(self, window: GameWindow, cancelled: Callable[[], bool]) -> WindowGeometry:
        if self.owned:raise SafetyError('Previous inputs must be released before the next action.')
        self._expected_cursor = None
        baseline = self.api.inspect(window)
        self.api.check_input_access(window)
        self._guard(window, baseline, cancelled)
        for key in (0x01, 0x02, 0x10, 0x11, 0x12, 0xA2, 0xA3, 0x51, 0x45, 0x57, 0x41, 0x53, 0x44):
            if self.api.key_held(key):
                raise SafetyError('Release the mouse buttons and control keys before starting the bot.')
        self._key_target=window
        return baseline

    def _wait(self, seconds: float, window: GameWindow, baseline: WindowGeometry,
              cancelled: Callable[[], bool], point: tuple[int, int] | None = None) -> None:
        end = time.monotonic() + seconds
        while time.monotonic() < end:
            self._guard(window, baseline, cancelled, point)
            time.sleep(min(.01, max(0, end - time.monotonic())))
        self._guard(window, baseline, cancelled, point)

    def _down(self, key: int) -> None:
        # Track before sending: an unknown outcome must still be released.
        alt_context=0x12 in self.owned
        self.owned.add(key)
        if self._key_target is None:raise SafetyError('The original keyboard input window is missing.')
        self.api.send_key(self._key_target,key, True,alt_context=alt_context)

    def release_owned(self) -> None:
        errors = []
        for key in sorted(self.owned,reverse=True):
            try:
                if key == 0x01:
                    if self._mouse_target is None:
                        raise SafetyError('The original window for releasing the mouse button is missing.')
                    window,point=self._mouse_target
                    self.api.send_mouse(window,point,False)
                    self._mouse_target=None
                else:
                    if self._key_target is None:raise SafetyError('The original window for releasing the key is missing.')
                    self.api.send_key(self._key_target,key, False,alt_context=0x12 in self.owned)
                self.owned.remove(key)
            except (OSError, SafetyError) as error:
                errors.append(str(error))
        if errors:
            raise SafetyError('Failed to release inputs: ' + '; '.join(errors))
        self._expected_cursor = None
        self._key_target=None

    def click(self, window: GameWindow, point: tuple[int,int], cancelled: Callable[[], bool],
              on_sent: Callable[[], None] | None = None, *, hover_seconds: float = .15,
              hold_seconds: float = .20) -> None:
        baseline = self._begin(window, cancelled)
        if len(point) != 2 or any(type(v) is not int for v in point) or not (0 <= point[0] < baseline.width and 0 <= point[1] < baseline.height):
            raise SafetyError('The click point is outside the client area.')
        target = (baseline.left + point[0], baseline.top + point[1])
        try:
            self._guard(window, baseline, cancelled, target)
            self.api.move(*target,guard=lambda:self._guard(window,baseline,cancelled,target))
            self._expected_cursor = target
            logger.info('Click client=%s screen=%s hover=%.3fs hold=%.3fs',point,target,hover_seconds,hold_seconds)
            self._wait(hover_seconds, window, baseline, cancelled, target)
            # Release always addresses this original window, even after focus loss.
            self._mouse_target=(window,point)
            self.owned.add(0x01)
            self.api.send_mouse(window,point,True)
            if on_sent is not None:on_sent()
            self._wait(hold_seconds, window, baseline, cancelled, target)
            logger.info('Mouse down posted to hwnd=%s; target panel must confirm selection',window.hwnd)
        finally:
            self.release_owned()
            logger.info('Owned mouse button released')

    def rotate(self, window: GameWindow, seconds: float, cancelled: Callable[[], bool]) -> None:
        baseline = self._begin(window, cancelled)
        try:
            self._down(0x45)
            self._wait(seconds, window, baseline, cancelled)
        finally:
            self.release_owned()

    def shortcut(self,window:GameWindow,keys:tuple[int,...],seconds:float,cancelled:Callable[[],bool])->None:
        baseline=self._begin(window,cancelled)
        if any(self.api.key_held(key) for key in keys):raise SafetyError('Release the skill or mount shortcut keys.')
        try:
            logger.info('Shortcut keys=%s hold=%.3fs',keys,seconds)
            for key in keys[:-1]:
                self._guard(window,baseline,cancelled);self._down(key)
            # Let the client's input polling observe modifiers before the main key.
            if len(keys)>1:self._wait(.1,window,baseline,cancelled)
            self._guard(window,baseline,cancelled);self._down(keys[-1])
            self._wait(seconds,window,baseline,cancelled)
            if len(keys)>1:
                self.api.send_key(window,keys[-1],False,alt_context=0x12 in self.owned);self.owned.remove(keys[-1])
                self._wait(.05,window,baseline,cancelled)
        finally:self.release_owned()


class MOUSEINPUT(ctypes.Structure):
    _fields_ = [('dx', W.LONG), ('dy', W.LONG), ('mouseData', W.DWORD), ('dwFlags', W.DWORD),
                ('time', W.DWORD), ('dwExtraInfo', ctypes.c_size_t)]


class KEYBDINPUT(ctypes.Structure):
    _fields_ = [('wVk', W.WORD), ('wScan', W.WORD), ('dwFlags', W.DWORD),
                ('time', W.DWORD), ('dwExtraInfo', ctypes.c_size_t)]


class HARDWAREINPUT(ctypes.Structure):
    _fields_ = [('uMsg', W.DWORD), ('wParamL', W.WORD), ('wParamH', W.WORD)]


class INPUTUNION(ctypes.Union):
    _fields_ = [('mi', MOUSEINPUT), ('ki', KEYBDINPUT), ('hi', HARDWAREINPUT)]


class INPUT(ctypes.Structure):
    _anonymous_ = ('data',)
    _fields_ = [('type', W.DWORD), ('data', INPUTUNION)]


class WindowsAPI:
    def __init__(self):
        self.user = ctypes.WinDLL('user32', use_last_error=True)
        self.kernel = ctypes.WinDLL('kernel32', use_last_error=True)
        self.callback_type = ctypes.WINFUNCTYPE(W.BOOL, W.HWND, W.LPARAM)
        self._start_pending = False
        self._stop_pending = False
        self._registered_hotkeys: list[int] = []
        signatures = {
            'EnumWindows': ([self.callback_type, W.LPARAM], W.BOOL),
            'IsWindow': ([W.HWND], W.BOOL), 'IsWindowVisible': ([W.HWND], W.BOOL),
            'IsIconic': ([W.HWND], W.BOOL), 'GetForegroundWindow': ([], W.HWND),
            'GetClientRect': ([W.HWND, ctypes.POINTER(W.RECT)], W.BOOL),
            'ClientToScreen': ([W.HWND, ctypes.POINTER(W.POINT)], W.BOOL),
            'GetWindowThreadProcessId': ([W.HWND, ctypes.POINTER(W.DWORD)], W.DWORD),
            'GetWindowTextLengthW': ([W.HWND], ctypes.c_int),
            'GetWindowTextW': ([W.HWND, W.LPWSTR, ctypes.c_int], ctypes.c_int),
            'GetAsyncKeyState': ([ctypes.c_int], ctypes.c_short),
            'WindowFromPoint': ([W.POINT], W.HWND), 'GetAncestor': ([W.HWND, W.UINT], W.HWND),
            'GetSystemMetrics': ([ctypes.c_int], ctypes.c_int),
            'MapVirtualKeyW': ([W.UINT, W.UINT], W.UINT),
            'GetCursorPos': ([ctypes.POINTER(W.POINT)], W.BOOL),
            'SetForegroundWindow': ([W.HWND], W.BOOL),
            'ShowWindow': ([W.HWND, ctypes.c_int], W.BOOL),
            'SendInput': ([W.UINT, ctypes.POINTER(INPUT), ctypes.c_int], W.UINT),
            'PostMessageW': ([W.HWND, W.UINT, W.WPARAM, W.LPARAM], W.BOOL),
            'SetProcessDpiAwarenessContext': ([W.HANDLE], W.BOOL),
            'RegisterHotKey': ([W.HWND, ctypes.c_int, W.UINT, W.UINT], W.BOOL),
            'UnregisterHotKey': ([W.HWND, ctypes.c_int], W.BOOL),
            'PeekMessageW': ([ctypes.POINTER(W.MSG), W.HWND, W.UINT, W.UINT, W.UINT], W.BOOL),
        }
        for name, (arguments, result) in signatures.items():
            function = getattr(self.user, name)
            function.argtypes, function.restype = arguments, result
        self.kernel.OpenProcess.argtypes = [W.DWORD, W.BOOL, W.DWORD]
        self.kernel.OpenProcess.restype = W.HANDLE
        self.kernel.QueryFullProcessImageNameW.argtypes = [W.HANDLE, W.DWORD, W.LPWSTR, ctypes.POINTER(W.DWORD)]
        self.kernel.QueryFullProcessImageNameW.restype = W.BOOL
        self.kernel.CloseHandle.argtypes = [W.HANDLE]
        self.kernel.CloseHandle.restype = W.BOOL

    def enable_dpi(self) -> None:
        if not self.user.SetProcessDpiAwarenessContext(ctypes.c_void_p(-4)):
            code = ctypes.get_last_error()
            if code != 5:  # Access denied means the process awareness was already set.
                raise ctypes.WinError(code)

    def start_hotkeys(self) -> None:
        # Register and pump on the same worker thread; NOREPEAT avoids key-hold toggles.
        for identifier, key in ((7, 0x76), (8, 0x77)):
            if not self.user.RegisterHotKey(None, identifier, 0x4000, key):
                self.close_hotkeys()
                raise SafetyError('Another application is using F7/F8. Free these shortcuts and restart the bot.')
            self._registered_hotkeys.append(identifier)

    def close_hotkeys(self) -> None:
        for identifier in self._registered_hotkeys:
            self.user.UnregisterHotKey(None, identifier)
        self._registered_hotkeys.clear()

    def _pump_hotkeys(self) -> None:
        message = W.MSG()
        while self.user.PeekMessageW(ctypes.byref(message), None, 0x0312, 0x0312, 1):
            if message.wParam == 7:
                self._start_pending = True
            elif message.wParam == 8:
                self._stop_pending = True

    def hotkey_events(self) -> tuple[bool, bool]:
        self._pump_hotkeys()
        result = self._start_pending, self._stop_pending
        self._start_pending = self._stop_pending = False
        return result

    def emergency_pressed(self) -> bool:
        self._pump_hotkeys()
        return self._stop_pending

    def list_windows(self, process_name: str | None = None) -> list[GameWindow]:
        windows = []
        @self.callback_type
        def visit(hwnd, _):
            if not self.user.IsWindowVisible(hwnd):
                return True
            pid = W.DWORD()
            self.user.GetWindowThreadProcessId(hwnd, ctypes.byref(pid))
            handle = self.kernel.OpenProcess(0x1000, False, pid.value)
            if not handle:
                return True
            try:
                buffer = ctypes.create_unicode_buffer(32768)
                size = W.DWORD(len(buffer))
                if not self.kernel.QueryFullProcessImageNameW(handle, 0, buffer, ctypes.byref(size)):
                    return True
                basename = PureWindowsPath(buffer.value).name
                if process_name and basename.lower() != process_name.lower():
                    return True
                length = self.user.GetWindowTextLengthW(hwnd)
                title = ctypes.create_unicode_buffer(length + 1)
                self.user.GetWindowTextW(hwnd, title, len(title))
                rect = W.RECT()
                if self.user.GetClientRect(hwnd, ctypes.byref(rect)) and rect.right > 100 and rect.bottom > 100:
                    windows.append(GameWindow(int(hwnd), pid.value, title.value, basename))
            finally:
                self.kernel.CloseHandle(handle)
            return True
        if not self.user.EnumWindows(visit, 0):
            raise ctypes.WinError(ctypes.get_last_error())
        return windows

    def inspect(self, window: GameWindow) -> WindowGeometry:
        pid = W.DWORD()
        self.user.GetWindowThreadProcessId(window.hwnd, ctypes.byref(pid))
        if not self.user.IsWindow(window.hwnd) or pid.value != window.pid:
            raise WindowGoneError('The selected game window no longer exists.')
        rect, point = W.RECT(), W.POINT(0, 0)
        if not self.user.GetClientRect(window.hwnd, ctypes.byref(rect)) or not self.user.ClientToScreen(window.hwnd, ctypes.byref(point)):
            raise ctypes.WinError(ctypes.get_last_error())
        return WindowGeometry(point.x, point.y, rect.right, rect.bottom,
                              bool(self.user.IsWindowVisible(window.hwnd) and not self.user.IsIconic(window.hwnd)),
                              self.user.GetForegroundWindow() == window.hwnd)

    def capture(self, window: GameWindow) -> np.ndarray:
        geometry = self.inspect(window)
        if not geometry.visible or not geometry.foreground or geometry.width < 100 or geometry.height < 100:
            raise SafetyError('Bring the game to the foreground for the live preview.')
        # Refuse even small overlay windows before capturing another application's pixels.
        for fx in (.1, .5, .9):
            for fy in (.1, .5, .9):
                if not self.point_clear(window, geometry.left + int(geometry.width * fx), geometry.top + int(geometry.height * fy)):
                    raise SafetyError('Another window overlaps the game area.')
        image = ImageGrab.grab(bbox=(geometry.left, geometry.top, geometry.left + geometry.width,
                                   geometry.top + geometry.height), all_screens=True)
        if self.inspect(window) != geometry:
            raise SafetyError('The window or focus changed during capture.')
        frame = cv2.cvtColor(np.asarray(image), cv2.COLOR_RGB2BGR)
        if frame.shape[:2] != (geometry.height, geometry.width) or np.max(frame) < 8:
            raise SafetyError('The game frame is invalid or black.')
        return frame

    def check_input_access(self, window: GameWindow) -> None:
        try:
            bot_elevated = process_elevated(os.getpid())
            game_elevated = process_elevated(window.pid)
        except OSError as error:
            raise SafetyError(f'Cannot verify game privileges: {error}') from error
        if game_elevated and not bot_elevated:
            raise SafetyError('The game is running as administrator, but the bot is not. Close the bot, open run.cmd '
                              'and confirm the Windows administrator prompt manually. '
                              'Alternatively, run both the game and the bot without elevated privileges.')

    def request_activation(self, window: GameWindow) -> bool:
        # Run from the user's Start click while our GUI owns focus.
        # A click into the world would select a target, so use window activation.
        self.inspect(window)
        self.check_input_access(window)
        if self.user.IsIconic(window.hwnd):
            self.user.ShowWindow(window.hwnd, 9)  # SW_RESTORE
        # Cross-process activation can complete asynchronously. Worker observes
        # actual foreground and stability before any game input.
        return bool(self.user.SetForegroundWindow(window.hwnd))

    def key_held(self, key: int) -> bool:
        return bool(self.user.GetAsyncKeyState(key) & 0x8000)

    def point_clear(self, window: GameWindow, x: int, y: int) -> bool:
        hit = self.user.WindowFromPoint(W.POINT(x, y))
        return bool(hit and self.user.GetAncestor(hit, 2) == window.hwnd)

    def _send(self, *events: INPUT) -> None:
        batch = (INPUT * len(events))(*events)
        ctypes.set_last_error(0)
        sent = self.user.SendInput(len(events), batch, ctypes.sizeof(INPUT))
        if sent != len(events):
            code = ctypes.get_last_error()
            raise SafetyError(f'SendInput rejected: {sent}/{len(events)} events, WinError {code}. '
                              'Check the privilege levels of the client and bot; Windows may not report the cause.')

    def send_key(self, window:GameWindow,key:int,down:bool,*,alt_context:bool=False)->None:
        try:geometry=self.inspect(window)
        except WindowGoneError:
            if down:raise
            logger.info('Key release retired: original hwnd=%s no longer exists',window.hwnd)
            return
        if down and (not geometry.visible or not geometry.foreground):
            raise SafetyError('The game must be visible and in the foreground before pressing a key.')
        if type(key) is not int or not 1<=key<=254:raise SafetyError('Invalid key code.')
        scan=self.user.MapVirtualKeyW(key,0)
        if not scan:raise SafetyError(f'Key {key:#x} has no valid scan code.')
        flags=1|((scan&0xff)<<16)
        if key in (0xA3,0xA5):flags|=1<<24
        if not down:flags|=(1<<30)|(1<<31)
        if alt_context:flags|=1<<29
        system_key=alt_context or key in (0x12,0xA4,0xA5)
        message=(0x0104 if down else 0x0105) if system_key else (0x0100 if down else 0x0101)
        ctypes.set_last_error(0)
        if not self.user.PostMessageW(window.hwnd,message,key,flags):
            raise SafetyError(f'PostMessage key event rejected: hwnd={window.hwnd}, vk={key:#x}, WinError {ctypes.get_last_error()}.')
        logger.info('Key message queued hwnd=%s vk=%#x scan=%#x down=%s; client response unverified',window.hwnd,key,scan,down)

    def send_mouse(self, window: GameWindow, point: tuple[int, int], down: bool) -> None:
        try:
            geometry=self.inspect(window)  # Checks HWND against the original PID.
        except WindowGoneError:
            if down:raise
            # Window-local state ceased to exist; never release into a reused HWND.
            logger.info('Mouse release retired: original hwnd=%s no longer exists',window.hwnd)
            return
        if len(point)!=2 or any(type(v) is not int or not 0<=v<=32767 for v in point):
            raise SafetyError('Invalid client coordinates for the mouse message.')
        if down:
            if not geometry.visible or not geometry.foreground:
                raise SafetyError('The game must be visible and in the foreground before pressing a mouse button.')
            if point[0]>=geometry.width or point[1]>=geometry.height:
                raise SafetyError('The click point is outside the client area.')
        message=0x0201 if down else 0x0202  # WM_LBUTTONDOWN / WM_LBUTTONUP
        coordinates=point[0]|(point[1]<<16)
        ctypes.set_last_error(0)
        if not self.user.PostMessageW(window.hwnd,message,1 if down else 0,coordinates):
            code=ctypes.get_last_error()
            raise SafetyError(f'PostMessage mouse event rejected: hwnd={window.hwnd}, '
                              f'message={message:#x}, WinError {code}.')
        logger.info('Mouse message posted hwnd=%s message=%#x client=%s',window.hwnd,message,point)

    def move(self, x: int, y: int, guard: Callable[[], None] | None = None) -> None:
        left,top,width,height=(self.user.GetSystemMetrics(index) for index in (76,77,78,79))
        if not left<=x<left+width or not top<=y<top+height:
            raise SafetyError('Coordinates are outside the virtual desktop.')
        # Relative motion reaches clients consuming mouse deltas. Measure each
        # step rather than assuming Windows acceleration is 1:1.
        deadline = time.monotonic() + .5
        gain=2.;previous=None;last_step=None
        while True:
            if guard:guard()
            actual = self.cursor()
            if abs(actual[0] - x) <= 3 and abs(actual[1] - y) <= 3:
                return
            if time.monotonic() >= deadline:
                raise SafetyError(f'SendInput movement not confirmed: cursor {actual}, expected {(x, y)}. '
                                  'Selection stopped; check for blocked input or manual mouse movement.')
            dx,dy=x-actual[0],y-actual[1]
            if previous is not None:
                moved=max(abs(actual[i]-previous[i]) for i in (0,1))
                gain=max(gain,min(8.,moved/max(abs(v) for v in last_step)+.25))
            scale=min(1/gain,64/max(abs(dx),abs(dy)))
            last_step=(round(dx*scale),round(dy*scale))
            # At least one pixel must be sent when a correction remains.
            if last_step==(0,0):last_step=(1 if dx>0 else -1,0) if abs(dx)>=abs(dy) else (0,1 if dy>0 else -1)
            previous=actual
            self._send(INPUT(type=0,mi=MOUSEINPUT(*last_step,0,0x0001,0,0)))
            time.sleep(.005)

    def cursor(self) -> tuple[int, int]:
        point = W.POINT()
        if not self.user.GetCursorPos(ctypes.byref(point)):
            raise SafetyError(f'GetCursorPos failed: {ctypes.WinError(ctypes.get_last_error())}')
        return point.x, point.y
