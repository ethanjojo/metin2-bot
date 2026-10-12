from dataclasses import replace
import logging
import queue
import threading
import time
from .config import Profile
from .runtime import Runtime, Sample
from .windows import GameWindow, WindowsAPI
from .session import Session
from .buffs import BuffCycle

logger = logging.getLogger(__name__)

class Worker:
    def __init__(self, api: WindowsAPI):
        self.api = api
        self.commands: queue.Queue = queue.Queue()
        self.results: queue.Queue[Sample] = queue.Queue(maxsize=1)
        self.shutdown = threading.Event()
        self.cancel_event = threading.Event()
        self.mount_confirmation_id=0
        self.thread = threading.Thread(target=self._run, name='metin-runtime', daemon=False)
        self.thread.start()

    def configure(self, profile: Profile, window: GameWindow, template=None,*,restore_mounted:bool|None=None) -> None:
        if restore_mounted is not None and type(restore_mounted) is not bool:raise ValueError('Invalid remembered mount state.')
        self.cancel_event.set()
        self.commands.put(('configure', (profile, window, template.copy() if template is not None else None,restore_mounted)))

    def confirm_mount(self,profile_id:str,window:GameWindow,mounted:bool)->int:
        if type(mounted) is not bool:raise ValueError('Invalid mount state.')
        self.cancel_event.set()
        self.mount_confirmation_id+=1
        identifier=self.mount_confirmation_id
        self.commands.put(('confirm_mount',((profile_id,window.hwnd,window.pid),mounted,identifier)))
        return identifier

    def clear(self) -> None:
        self.cancel_event.set()
        self.commands.put(('clear', None))

    def pause(self) -> None:
        self.cancel_event.set()
        self.commands.put(('pause', None))

    def arm(self) -> None:
        self.commands.put(('arm', None))

    def reset_statistics(self) -> None:
        self.commands.put(('reset_statistics', None))

    def reset_skill_timer(self,profile:Profile,window:GameWindow,index:int)->None:
        if not profile.buffs.enabled or type(index) is not int or not 0<=index<len(profile.buffs.skills) or profile.buffs.skills[index].mode!='refresh':
            raise ValueError('Select an enabled regular skill with an interval.')
        self.commands.put(('reset_skill_timer',((profile.id,window.hwnd,window.pid),profile.buffs,index)))

    def _publish(self, sample: Sample) -> None:
        try:
            self.results.get_nowait()
        except queue.Empty:
            pass
        self.results.put_nowait(sample)

    def _run(self) -> None:
        runtime = None
        session=Session();session_key=None
        buffs=None;buffs_key=None;buffs_config=None
        mount_confirmation_id=0
        next_sample = 0.0
        armed_until = 0.0
        arm_ready = 0.0
        focused_since = None
        try:
            self.api.start_hotkeys()
            logger.info('Worker started; F7/F8 registered')
            while not self.shutdown.is_set():
                try:
                    command, payload = self.commands.get_nowait()
                except queue.Empty:
                    command = None
                if command == 'configure':
                    logger.info('Configuration applied')
                    if runtime:
                        runtime.close()
                    profile, window, template,restore_mounted = payload
                    key=(profile.id,window.hwnd,window.pid)
                    if key!=session_key:session=Session();session_key=key
                    if key!=buffs_key:
                        buffs=BuffCycle(profile.buffs)
                        buffs.mounted=restore_mounted
                        mount_confirmation_id=0
                    elif buffs_config.execution_config!=profile.buffs.execution_config:
                        mounted=buffs.mounted
                        buffs=BuffCycle(profile.buffs);buffs.mounted=mounted
                    else:buffs.config=profile.buffs
                    buffs_key=key;buffs_config=profile.buffs
                    runtime = Runtime(profile, window, self.api, template, session=session)
                    runtime.controller.buffs=buffs
                    runtime.stop = self.cancel_event
                    self.cancel_event.clear()
                    armed_until = 0
                    next_sample = 0
                elif command == 'clear':
                    if runtime:runtime.close()
                    runtime = None
                    armed_until = 0
                    focused_since = None
                elif command == 'confirm_mount':
                    key,mounted,identifier=payload
                    if key==buffs_key:
                        if runtime:runtime.cancel('Mount state updated. Resume with Start or F7.')
                        buffs.mounted=mounted
                        mount_confirmation_id=identifier
                        armed_until=0;next_sample=0
                        logger.info('Mount state confirmed: mounted=%s request=%s',mounted,identifier)
                elif command == 'reset_statistics':
                    session.reset()
                    if runtime:session.advance(time.monotonic(),runtime.controller.running)
                    next_sample=0
                elif command == 'reset_skill_timer':
                    key,config,index=payload
                    if key==buffs_key and config==buffs_config:
                        running=runtime is not None and runtime.controller.running
                        buffs.reset_timer(index,time.monotonic(),paused=not running)
                        if running:runtime.controller.attempted_skills.discard(index)
                        next_sample=0
                        logger.info('Skill timer reset: index=%s',index)
                elif command == 'pause' and runtime:
                    runtime.cancel('Paused by user.')
                    armed_until = 0
                elif command == 'arm' and runtime:
                    logger.info('Start armed')
                    armed_until = time.monotonic() + 12
                    arm_ready = time.monotonic() + 3
                    self.cancel_event.clear()
                now = time.monotonic()
                f7_edge, f8 = self.api.hotkey_events()
                f8 = f8 or self.api.key_held(0x77)
                if runtime:
                    if f8 or self.shutdown.is_set():
                        runtime.cancel('Emergency stop (F8).')
                        armed_until = 0
                    try:
                        focused = self.api.inspect(runtime.window).foreground
                    except Exception:
                        focused = False
                    if focused:
                        if focused_since is None:
                            focused_since = now
                    else:
                        focused_since = None
                    if f7_edge and not f8 and focused and self.commands.empty() and not self.shutdown.is_set():
                        logger.info('F7 in game; running=%s', runtime.controller.running)
                        armed_until = 0
                        if runtime.controller.running:
                            runtime.cancel('Paused (F7).')
                        else:
                            runtime.request_start()
                        next_sample = 0
                    if armed_until:
                        if now > armed_until:
                            runtime.cancel('The game did not reach stable foreground focus within 12 s. Bot did not start. Activate it manually and press F7.')
                            armed_until = 0
                        elif now >= arm_ready and focused and now - focused_since >= .5 and not self.cancel_event.is_set():
                            logger.info('Armed start accepted')
                            runtime.request_start()
                            armed_until = 0
                            next_sample = 0
                    if runtime.controller.running and not focused:
                        runtime.cancel('Game lost focus. Press F7 in the game to resume.')
                        next_sample = 0
                    if now >= next_sample:
                        sample = runtime.sample(now)
                        if armed_until:
                            sample = replace(sample, state='Armed', reason='Waiting for stable game focus. Start delay: 3 s; timeout: 12 s.')
                        self._publish(replace(sample,mount_confirmation_id=mount_confirmation_id))
                        next_sample = now + runtime.profile.timing.sample_interval
                self.shutdown.wait(.02)
        except Exception as error:
            self._publish(Sample(None, [], 0, None, 'Error', str(error), False))
        finally:
            if runtime:
                try:
                    runtime.close()
                except Exception as error:
                    self._publish(Sample(None, [], 0, None, 'Release error', str(error), False))
            self.api.close_hotkeys()
