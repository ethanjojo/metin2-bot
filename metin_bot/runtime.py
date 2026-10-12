"""One worker owns this runtime; only cancellation crosses threads."""
from dataclasses import dataclass
import logging
import threading
import time
import numpy as np
from .config import Profile
from .controller import Action, ActionKind, Controller, Observation
from .vision import Candidate, CandidateTracker, detect_candidates, panel_score, validate_template
from .windows import GameWindow, InputDriver, SafetyError, WindowsAPI, WindowGeometry
from .session import Session, Statistics
from .hotkeys import hotkey_keys
from .buff_detection import BuffDetector, Presence

logger=logging.getLogger(__name__)
logger.addHandler(logging.NullHandler())


@dataclass(frozen=True)
class Sample:
    frame: np.ndarray | None
    candidates: list[Candidate]
    stable_candidates: int
    panel_score: float | None
    state: str
    reason: str
    running: bool
    action: Action | None = None
    profile_id: str | None = None
    window_hwnd: int | None = None
    captured_at: float = 0
    statistics: Statistics = Statistics()
    remaining_seconds: float | None = None
    buff_seconds: float | None = None
    mounted: bool | None = None
    mount_confirmation_id: int = 0
    buff_skill_seconds: tuple[float|None,...] = ()
    buff_states: tuple[Presence,...] = ()


class Runtime:
    def __init__(self, profile: Profile, window: GameWindow, api: WindowsAPI, template: np.ndarray | None = None,
                 *, session: Session | None = None):
        self.profile,self.window,self.api=profile,window,api
        self.template=template.copy() if template is not None else None
        if self.template is not None:
            validate_template(self.template);self.template.flags.writeable=False
        self.session=session if session is not None else Session()
        self.controller=Controller(profile,self.session);self.driver=InputDriver(api)
        self.tracker=CandidateTracker(profile.color.stable_frames if profile.color else 3,profile.timing.sample_interval)
        self.stop=threading.Event();self._pending_start=False;self._geometry=None
        self._sample_time=None;self._wall_time=None
        self.buff_detector=BuffDetector(profile.buffs,profile.frame_size,profile.timing.sample_interval)

    def _logical_now(self):
        if self._sample_time is None:return None
        return self._sample_time+max(0,time.monotonic()-self._wall_time)

    def request_start(self) -> None:
        self.stop.clear();self._pending_start=True

    def cancel(self,reason: str) -> None:
        logger.info('Pause: %s',reason)
        self.session.pause(self._logical_now())
        self.stop.set();self._pending_start=False;self.controller.pause(reason,self._logical_now())
        self.tracker.reset();self.driver.release_owned()
        self.buff_detector.reset()

    def close(self) -> None:
        self.cancel('Application closed.')

    def sample(self,now: float) -> Sample:
        self._sample_time=now;self._wall_time=time.monotonic()
        self.session.advance(now,self.controller.running)
        frame=None;candidates=[];stable=[];score=None;action=None;states=()
        try:
            if self.stop.is_set() and (self.controller.running or self._pending_start):
                self.cancel('Stopped (F8 / pause).')
            geometry=self.api.inspect(self.window)
            if self.controller.running and geometry!=self._geometry:
                raise SafetyError('Window or focus changed. Bot paused.')
            frame=self.api.capture(self.window)
            size=(frame.shape[1],frame.shape[0])
            if self._pending_start:
                if not self.profile.ready or self.template is None:
                    raise SafetyError('Incomplete profile: select a search area, color, target panel and stable template.')
                if size!=self.profile.frame_size:
                    raise SafetyError('Game size does not match profile calibration. Recalibrate the profile.')
                self.api.check_input_access(self.window)
                self._geometry=geometry;self.tracker.reset();self.controller.start(now);self._pending_start=False
                self.session.was_running=self.controller.running
            if self.controller.running and size!=self.profile.frame_size:
                raise SafetyError('Frame size changed. Bot paused.')
            if self.profile.frame_size is None or size==self.profile.frame_size:
                candidates=detect_candidates(frame,self.profile)
                stable=self.tracker.update(candidates,now)
                if self.template is not None and self.profile.panel.region:
                    score=panel_score(frame,self.template,self.profile.panel.region)
            states=self.buff_detector.observe(frame,now) if geometry.foreground and self.controller.running else ()
            action=self.controller.step(now,Observation(True,tuple(stable),score is not None and score>=self.profile.panel.threshold,buffs=states))
            if action:
                logger.info('Action %s requested %s',action.kind.value,action.point)
                if action.kind==ActionKind.CLICK:
                    self.driver.click(self.window,action.point,self.stop.is_set,on_sent=self.session.clicked,
                                      hover_seconds=self.profile.timing.click_hover_seconds,
                                      hold_seconds=self.profile.timing.click_hold_seconds)
                elif action.kind==ActionKind.ROTATE:
                    self.driver.rotate(self.window,self.profile.timing.rotate_seconds,self.stop.is_set)
                else:
                    self.driver.shortcut(self.window,hotkey_keys(action.shortcut),self.profile.buffs.key_hold_seconds,self.stop.is_set)
                    self.controller.action_completed(self._logical_now())
                logger.info('Action %s completed',action.kind.value);self.tracker.reset()
            if self.stop.is_set() and (self.controller.running or self._pending_start):
                self.cancel('Stopped (F8 / pause).')
        except Exception as error:
            if action is not None and action.kind==ActionKind.MOUNT:self.controller.buffs.mounted=None
            active=self.controller.running or self._pending_start or bool(self.driver.owned)
            reason=str(error) or type(error).__name__
            if active:
                logger.exception('Runtime failed')
                try:self.cancel(reason)
                except Exception as cleanup:self.controller.pause(f'{reason}; input release failed: {cleanup}')
            else:
                if self.controller.status.state=='Stopped':self.controller.pause(reason)
                self.tracker.reset()
        status=self.controller.status
        self.session.advance(self._logical_now(),self.controller.running)
        return Sample(frame,candidates,len(stable),score,status.state,status.reason,self.controller.running,action,self.profile.id,self.window.hwnd,now,
                      self.session.snapshot(),self.controller.remaining(now),
                      self.controller.buffs.remaining(now,paused=not self.controller.running),
                      self.controller.buffs.mounted,
                      buff_skill_seconds=self.controller.buffs.skill_remaining(now,paused=not self.controller.running),
                      buff_states=states if self.controller.running else ())
