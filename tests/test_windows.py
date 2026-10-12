import unittest
from dataclasses import replace
from types import SimpleNamespace
from unittest.mock import patch

from metin_bot.config import Region
from metin_bot.windows import GameWindow, InputDriver, WindowGeometry, WindowsAPI, SafetyError, WindowGoneError, choose_window


class FakeAPI:
    def __init__(self):
        self.geometry = WindowGeometry(100, 100, 1000, 600, True, True)
        self.held = set()
        self.events = []
        self.steps = 0
        self.cancel = False
        self.failure = False
        self.change_geometry = False
        self.position = (0, 0)

    def inspect(self, window):
        return self.geometry

    def check_input_access(self, window):
        pass

    def key_held(self, key):
        return key in self.held

    def point_clear(self, window, x, y):
        return True

    def send_key(self, window, key, down, *, alt_context=False):
        if self.failure and down:
            raise OSError('SendInput selhal')
        self.key_target=window
        self.events.append(('key', key, down))
        if down:self.held.add(key)
        else:self.held.discard(key)

    def send_mouse(self, window, point, down):
        self.mouse_target=(window,point)
        self.events.append(('mouse', down))
        if down:self.held.add(0x01)
        else:self.held.discard(0x01)

    def move(self, x, y, guard=None):
        if guard:guard()
        self.position = (x, y)
        self.steps += 1
        self.events.append(('move', x, y))
        if self.change_geometry and self.steps == 2:
            self.geometry = replace(self.geometry, left=200)

    def cursor(self):
        return self.position

    def emergency_pressed(self):
        return False


class WindowsTests(unittest.TestCase):
    def setUp(self):
        self.api = FakeAPI()
        self.window = GameWindow(1, 123, 'Verzum')
        self.driver = InputDriver(self.api)
        self.region = Region(.1, .1, .8, .8)

    def test_multiple_windows_require_selection(self):
        with self.assertRaises(SafetyError):
            choose_window([self.window, GameWindow(2, 456, 'Verzum')])

    def test_mount_chord_releases_key_before_modifier(self):
        waits=[]
        self.driver._wait=lambda seconds,*args:waits.append((seconds,list(self.api.events)))
        self.driver.shortcut(self.window,(0x11,0x47),.15,lambda:False)
        self.assertEqual(self.api.events,[('key',0x11,True),('key',0x47,True),
                                         ('key',0x47,False),('key',0x11,False)])
        self.assertFalse(self.driver.owned)
        self.assertEqual(waits[0],(.1,[('key',0x11,True)]))
        self.assertEqual(waits[1][0],.15)
        self.assertEqual(waits[2][1][-1],('key',0x47,False))

    def test_keyboard_messages_address_window_with_scan_and_release_flags(self):
        api=WindowsAPI.__new__(WindowsAPI);events=[]
        api.inspect=lambda window:self.api.geometry
        api.user=SimpleNamespace(MapVirtualKeyW=lambda key,mode:0x1d if key==0x11 else 0x22,
                                 PostMessageW=lambda *args:events.append(args) or True)
        api.send_key(self.window,0x11,True);api.send_key(self.window,0x47,True)
        api.send_key(self.window,0x47,False);api.send_key(self.window,0x11,False)
        self.assertEqual(events,[(1,0x100,0x11,1|(0x1d<<16)),(1,0x100,0x47,1|(0x22<<16)),
                                (1,0x101,0x47,1|(0x22<<16)|0xc0000000),
                                (1,0x101,0x11,1|(0x1d<<16)|0xc0000000)])

    def test_unobservable_global_key_state_does_not_reject_window_messages(self):
        self.api.key_held=lambda key:False
        self.driver._wait=lambda *args:None
        self.driver.shortcut(self.window,(0x11,0x47),.15,lambda:False)
        self.assertEqual(self.api.key_target,self.window)
        self.assertEqual(self.api.events,[('key',0x11,True),('key',0x47,True),
                                         ('key',0x47,False),('key',0x11,False)])

    def test_failed_key_message_reports_os_error(self):
        api=WindowsAPI.__new__(WindowsAPI)
        api.inspect=lambda window:self.api.geometry
        api.user=SimpleNamespace(MapVirtualKeyW=lambda *args:0x22,PostMessageW=lambda *args:False)
        with patch('metin_bot.windows.ctypes.get_last_error',return_value=5):
            with self.assertRaisesRegex(SafetyError,'PostMessage.*WinError 5'):
                api.send_key(self.window,0x47,True)

    def test_alt_chord_uses_system_messages_and_context_bit(self):
        api=WindowsAPI.__new__(WindowsAPI);events=[]
        api.inspect=lambda window:self.api.geometry
        api.user=SimpleNamespace(MapVirtualKeyW=lambda *args:0x22,PostMessageW=lambda *args:events.append(args) or True)
        api.send_key(self.window,0x12,True)
        api.send_key(self.window,0x47,True,alt_context=True)
        api.send_key(self.window,0x47,False,alt_context=True)
        self.assertEqual([e[1] for e in events],[0x104,0x104,0x105])
        self.assertEqual(events[0][3]&(1<<29),0)
        self.assertTrue(events[1][3]&(1<<29))

    def test_keyboard_release_stays_with_original_window_after_focus_loss(self):
        api=WindowsAPI.__new__(WindowsAPI);events=[]
        api.inspect=lambda window:replace(self.api.geometry,foreground=False)
        api.user=SimpleNamespace(MapVirtualKeyW=lambda *args:0x22,PostMessageW=lambda *args:events.append(args) or True)
        with self.assertRaises(SafetyError):api.send_key(self.window,0x47,True)
        api.send_key(self.window,0x47,False)
        self.assertEqual(events[0][:3],(1,0x101,0x47))

    def test_keyboard_release_does_not_address_reused_hwnd(self):
        api=WindowsAPI.__new__(WindowsAPI)
        api.inspect=lambda window:(_ for _ in ()).throw(WindowGoneError('reused'))
        api.user=SimpleNamespace(PostMessageW=lambda *args:self.fail('Posted to reused HWND'))
        api.send_key(self.window,0x47,False)
        with self.assertRaises(WindowGoneError):api.send_key(self.window,0x47,True)

    def test_shortcut_cancellation_releases_all_keys_and_sends_no_more(self):
        self.driver._wait=lambda *args:(_ for _ in ()).throw(SafetyError('F8'))
        with self.assertRaises(SafetyError):self.driver.shortcut(self.window,(0x11,0x47),.15,lambda:False)
        self.assertEqual(self.api.events,[('key',0x11,True),('key',0x11,False)])
        self.assertFalse(self.driver.owned)

    def test_focus_loss_between_modifier_and_key_blocks_chord(self):
        original=self.api.send_key
        def send(window,key,down,**kwargs):
            original(window,key,down,**kwargs)
            if down:self.api.geometry=replace(self.api.geometry,foreground=False)
        self.api.send_key=send
        with self.assertRaises(SafetyError):self.driver.shortcut(self.window,(0x11,0x47),.15,lambda:False)
        self.assertEqual(self.api.events,[('key',0x11,True),('key',0x11,False)])

    def test_cancel_after_primary_release_still_releases_modifier(self):
        waits=[]
        def wait(seconds,*args):
            waits.append(seconds)
            if len(waits)==3:raise SafetyError('F8')
        self.driver._wait=wait
        with self.assertRaises(SafetyError):
            self.driver.shortcut(self.window,(0x11,0x47),.15,lambda:False)
        self.assertEqual(self.api.events,[('key',0x11,True),('key',0x47,True),
                                         ('key',0x47,False),('key',0x11,False)])
        self.assertFalse(self.driver.owned)

    def test_focus_loss_blocks_input(self):
        self.api.geometry = replace(self.api.geometry, foreground=False)
        with self.assertRaises(SafetyError):
            self.driver.click(self.window, (100, 60), lambda: False)
        self.assertEqual(self.api.events, [])

    def test_unobservable_button_state_does_not_abort_accepted_click(self):
        self.api.key_held=lambda key:False
        waits=[]
        self.driver._wait=lambda seconds,*args:waits.append(seconds)
        sent=[]
        self.driver.click(self.window,(100,60),lambda:False,on_sent=lambda:sent.append(True))
        self.assertEqual(waits,[.15,.2])
        self.assertEqual(sent,[True])
        self.assertEqual(self.api.events[-1],('mouse',False))
        self.assertEqual(self.driver.owned,set())

    def test_click_uses_configured_hover_and_hold(self):
        waits=[]
        self.driver._wait=lambda seconds,*args:waits.append(seconds)
        self.driver.click(self.window,(100,60),lambda:False,hover_seconds=.15,hold_seconds=.2)
        self.assertEqual(waits,[.15,.2])

    def test_mouse_messages_address_window_with_client_coordinates(self):
        api=WindowsAPI.__new__(WindowsAPI);events=[]
        api.inspect=lambda window:self.api.geometry
        api.user=SimpleNamespace(PostMessageW=lambda *args:events.append(args) or True)
        api.send_mouse(self.window,(400,300),True)
        api.send_mouse(self.window,(400,300),False)
        self.assertEqual(events,[(1,0x0201,1,400|(300<<16)),(1,0x0202,0,400|(300<<16))])

    def test_failed_mouse_message_reports_os_error(self):
        api=WindowsAPI.__new__(WindowsAPI)
        api.inspect=lambda window:self.api.geometry
        api.user=SimpleNamespace(PostMessageW=lambda *args:False)
        with patch('metin_bot.windows.ctypes.get_last_error',return_value=5):
            with self.assertRaisesRegex(SafetyError,'PostMessage.*WinError 5'):
                api.send_mouse(self.window,(400,300),True)

    def test_mouse_down_rejects_outside_or_unfocused_window(self):
        api=WindowsAPI.__new__(WindowsAPI);events=[]
        api.inspect=lambda window:self.api.geometry
        api.user=SimpleNamespace(PostMessageW=lambda *args:events.append(args) or True)
        for point in [(-1,20),(1000,20),(20,600),(32768,20)]:
            with self.assertRaises(SafetyError):api.send_mouse(self.window,point,True)
        self.api.geometry=replace(self.api.geometry,foreground=False)
        with self.assertRaises(SafetyError):api.send_mouse(self.window,(10,20),True)
        self.assertEqual(events,[])

    def test_mouse_up_uses_original_window_after_focus_loss(self):
        def wait(seconds,*args):
            if ('mouse',True) in self.api.events:
                self.api.geometry=replace(self.api.geometry,foreground=False)
                raise SafetyError('focus lost')
        self.driver._wait=wait
        with self.assertRaisesRegex(SafetyError,'focus lost'):
            self.driver.click(self.window,(100,60),lambda:False)
        self.assertEqual(self.api.mouse_target,(self.window,(100,60)))
        self.assertEqual(self.api.events[-1],('mouse',False))

    def test_destroyed_window_retires_mouse_ownership_without_posting_to_replacement(self):
        messages=[]
        api=WindowsAPI.__new__(WindowsAPI)
        api.user=SimpleNamespace(PostMessageW=lambda *args:messages.append(args) or True)
        api.inspect=lambda window:(_ for _ in ()).throw(WindowGoneError('window replaced'))
        self.api.send_mouse=api.send_mouse
        self.driver.owned.add(0x01)
        self.driver._mouse_target=(self.window,(100,60))
        self.driver.release_owned()
        self.driver.release_owned()  # Closing/reconfiguring this runtime is safe.
        self.assertEqual(self.driver.owned,set())
        self.assertIsNone(self.driver._mouse_target)
        self.assertEqual(messages,[])
        with self.assertRaises(WindowGoneError):api.send_mouse(self.window,(100,60),True)

    def test_mouse_up_is_posted_even_if_original_window_loses_focus(self):
        api=WindowsAPI.__new__(WindowsAPI);messages=[]
        api.inspect=lambda window:replace(self.api.geometry,foreground=False,visible=False)
        api.user=SimpleNamespace(PostMessageW=lambda *args:messages.append(args) or True)
        api.send_mouse(self.window,(100,60),False)
        self.assertEqual(messages,[(1,0x0202,0,100|(60<<16))])

    def test_geometry_change_interrupts_click(self):
        original = self.api.move
        def changed(x,y,guard=None):
            original(x,y,guard)
            self.api.geometry = replace(self.api.geometry, left=200)
        self.api.move = changed
        with self.assertRaises(SafetyError):
            self.driver.click(self.window, (100, 60), lambda: False)
        self.assertNotIn(('mouse', True), self.api.events)
        self.assertEqual(self.driver.owned, set())

    def test_user_held_input_blocks_action(self):
        self.api.held.add(0xA2)
        with self.assertRaises(SafetyError):
            self.driver.click(self.window, (100, 60), lambda: False)
        self.assertEqual(self.api.events, [])

    def test_cancel_releases_owned_inputs(self):
        with self.assertRaises(SafetyError):
            self.driver.click(self.window, (100, 60), lambda: any(e == ('mouse', True) for e in self.api.events))
        self.assertEqual(self.driver.owned, set())
        self.assertEqual(self.api.events[-2:], [('mouse', True), ('mouse', False)])

    def test_sendinput_failure_releases_inputs(self):
        original = self.api.send_mouse
        def fail_down(window, point, down):
            if down:
                raise OSError('SendInput selhal')
            original(window, point, down)
        self.api.send_mouse = fail_down
        with self.assertRaises(OSError):
            self.driver.click(self.window, (100, 60), lambda: False)
        self.assertIn(('mouse', False), self.api.events)
        self.assertEqual(self.driver.owned, set())

    def test_relative_motion_corrects_acceleration_on_negative_monitor(self):
        api=WindowsAPI.__new__(WindowsAPI);position=[100,100];events=[]
        api.cursor=lambda:tuple(position)
        api.user=SimpleNamespace(GetSystemMetrics=lambda index:{76:-1920,77:0,78:3840,79:1080}[index])
        def send(event):
            events.append(event);position[0]+=event.mi.dx*2;position[1]+=event.mi.dy*2
        api._send=send;api.move(-500,300)
        self.assertTrue(all(abs(a-b)<=3 for a,b in zip(position,(-500,300))))
        self.assertTrue(all(e.mi.dwFlags==1 for e in events))

    def test_successful_click_releases_inputs(self):
        self.driver.click(self.window, (100, 60), lambda: False)
        self.assertEqual(self.driver.owned, set())
        self.assertEqual(self.api.events[0], ('move', 200, 160))
        self.assertEqual(self.api.events[-2:], [('mouse', True), ('mouse', False)])

    def test_cancel_rotation_releases_key(self):
        def stop():
            return any(event == ('key', 0x45, True) for event in self.api.events)
        with self.assertRaises(SafetyError):
            self.driver.rotate(self.window, .35, stop)
        self.assertEqual(self.api.events[-1], ('key', 0x45, False))

    def test_physical_mouse_movement_interrupts_before_button_down(self):
        original = self.api.move
        def move_then_physical(x,y,guard=None):
            original(x,y,guard)
            self.api.position = (1500, 100)
        self.api.move = move_then_physical
        with self.assertRaises(SafetyError):
            self.driver.click(self.window, (100, 60), lambda: False)
        self.assertNotIn(('mouse', True), self.api.events)
        self.assertEqual(self.driver.owned, set())

    def test_move_waits_for_delivered_sendinput_position(self):
        api = WindowsAPI.__new__(WindowsAPI)
        positions = iter([(10, 20), (10, 20), (400, 300)])
        events = []
        api.cursor = lambda: next(positions)
        api._send = lambda event: events.append(event)
        api.user = SimpleNamespace(SetCursorPos=lambda x, y: True, GetSystemMetrics=lambda index: {76: 0, 77: 0, 78: 1920, 79: 1080}[index])
        api.move(400, 300)
        self.assertGreaterEqual(len(events), 1)
        self.assertTrue(all(e.mi.dwFlags==0x0001 for e in events))
        self.assertTrue(all(abs(e.mi.dx)<=64 and abs(e.mi.dy)<=64 for e in events))

    def test_relative_motion_checks_guard_before_every_event(self):
        api=WindowsAPI.__new__(WindowsAPI);position=[0,0];events=[]
        api.cursor=lambda:tuple(position)
        api.user=SimpleNamespace(GetSystemMetrics=lambda index:{76:0,77:0,78:1920,79:1080}[index])
        def send(event):
            events.append(event);position[0]+=event.mi.dx;position[1]+=event.mi.dy
        def guard():
            if events:raise SafetyError('focus lost')
        api._send=send
        with self.assertRaisesRegex(SafetyError,'focus lost'):api.move(600,400,guard=guard)
        self.assertEqual(len(events),1)

    def test_undelivered_move_stops_before_click(self):
        api = WindowsAPI.__new__(WindowsAPI)
        api.cursor = lambda: (10, 20)
        api._send = lambda event: None
        api.user = SimpleNamespace(SetCursorPos=lambda x, y: True, GetSystemMetrics=lambda index: {76: 0, 77: 0, 78: 1920, 79: 1080}[index])
        with self.assertRaisesRegex(SafetyError, 'SendInput.*cursor'):
            api.move(400, 300)

    def test_activation_request_does_not_require_immediate_foreground(self):
        api = WindowsAPI.__new__(WindowsAPI)
        api.inspect = lambda window: WindowGeometry(0, 0, 800, 600, True, False)
        api.check_input_access = lambda window: None
        requests = []
        api.user = SimpleNamespace(IsIconic=lambda hwnd: False,
                                   SetForegroundWindow=lambda hwnd: requests.append(hwnd) or True)
        self.assertTrue(api.request_activation(self.window))
        self.assertEqual(requests, [self.window.hwnd])

    def test_refused_activation_request_reports_false_without_input(self):
        api = WindowsAPI.__new__(WindowsAPI)
        api.inspect = lambda window: WindowGeometry(0, 0, 800, 600, True, False)
        api.check_input_access = lambda window: None
        api.user = SimpleNamespace(IsIconic=lambda hwnd: False, SetForegroundWindow=lambda hwnd: False)
        self.assertFalse(api.request_activation(self.window))

    def test_elevated_game_requires_elevated_bot(self):
        api = WindowsAPI.__new__(WindowsAPI)
        with patch('metin_bot.windows.process_elevated', side_effect=[False, True], create=True):
            with self.assertRaisesRegex(SafetyError, 'administrator'):
                api.check_input_access(self.window)

    def test_equal_privileges_allow_input(self):
        api = WindowsAPI.__new__(WindowsAPI)
        with patch('metin_bot.windows.process_elevated', return_value=False, create=True):
            api.check_input_access(self.window)
