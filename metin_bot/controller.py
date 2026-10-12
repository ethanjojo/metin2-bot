from dataclasses import dataclass
from enum import Enum
from .config import Profile
from .vision import Candidate
from .session import Session
from .buffs import BuffCycle
from .buff_detection import Presence


class ActionKind(Enum):
    CLICK='click'
    ROTATE='rotate'
    MOUNT='mount'
    SKILL='skill'


@dataclass(frozen=True)
class Action:
    kind: ActionKind
    point: tuple[int,int] | None = None
    shortcut: str | None = None


@dataclass(frozen=True)
class Observation:
    ready: bool
    candidates: tuple[Candidate,...]
    panel_present: bool
    reason: str = ''
    buffs: tuple[Presence,...] = ()


@dataclass(frozen=True)
class Status:
    state: str
    reason: str


class Phase(Enum):
    INITIAL='initial'
    SEEKING='seeking'
    ACQUIRING='acquiring'
    ACTIVE='active'
    SETTLING='settling'
    BUFFING='buffing'


class Controller:
    def __init__(self, profile: Profile, session: Session | None = None):
        self.profile=profile
        self.session=session if session is not None else Session()
        self.buffs=BuffCycle(profile.buffs)
        self.running=False
        self.status=Status('Stopped','Calibrate the profile and start the bot.')

    def start(self, now: float) -> None:
        if not self.profile.buffs.ready:
            self.pause('Select an active icon and buff area for toggle skills.');return
        if self.profile.buffs.enabled and self.buffs.mounted is None:
            self.pause('Mount state is unknown. Set it with the switch in Buffs, then start again.');return
        self.buffs.resume(now)
        self.running=True;self.phase=Phase.INITIAL;self.since=now
        self.present=self.absent=0;self.last_observation=float('-inf')
        self.rotations=self.failures=0;self.failed=[];self.clicked=None
        self.uncertain_since=None
        self.attempted_skills=set()
        self.status=Status('Checking target','Checking whether a target panel is already present.')

    def pause(self, reason: str, now: float | None = None) -> None:
        if self.running:
            self.buffs.pause(now if now is not None else max(0.,self.last_observation))
        self.session.pause()
        self.running=False;self.status=Status('Paused',reason)

    def remaining(self,now):
        if not self.running:return None
        if self.phase==Phase.BUFFING:return max(0.,self.buffs.wait_until-now)
        timing=self.profile.timing
        limit={Phase.INITIAL:timing.acquire_timeout,Phase.SEEKING:timing.search_timeout,
               Phase.ACQUIRING:timing.acquire_timeout,Phase.ACTIVE:timing.attack_timeout,
               Phase.SETTLING:timing.settle_seconds}[self.phase]
        return max(0.,limit-(now-self.since))

    def _active(self, now: float) -> None:
        self.phase=Phase.ACTIVE;self.since=now;self.rotations=self.failures=0
        self.status=Status('Target active','Panel confirmed; waiting for the target to end.')

    def step(self, now: float, observation: Observation) -> Action | None:
        if not self.running:return None
        if not observation.ready:
            self.pause(observation.reason or 'Invalid frame.');return None
        if self.phase==Phase.BUFFING:
            if observation.panel_present:
                self.pause('A target panel appeared while buffing. Check the game before resuming.',now);return None
            return self._buff_step(now,observation.buffs)
        if not 0<now-self.last_observation<=max(1.,self.profile.timing.sample_interval*1.5):
            self.present=self.absent=0
        self.last_observation=now
        if observation.panel_present:self.present+=1;self.absent=0
        else:self.absent+=1;self.present=0
        required=self.profile.panel.stable_frames
        timing=self.profile.timing
        if self.phase not in (Phase.ACTIVE,Phase.ACQUIRING):
            if self.present < required and self.absent < required:
                if self.uncertain_since is None:self.uncertain_since=now
                if now-self.uncertain_since >= timing.acquire_timeout:
                    self.pause('Target panel detection is uncertain. Check the template.');return None
            else:self.uncertain_since=None
        if self.phase==Phase.ACTIVE:
            buff_remaining=self.buffs.remaining(now)
            if buff_remaining is not None:
                detail='Scheduled buffs are waiting for the target to end.' if buff_remaining==0 else f'Scheduled buffs in {buff_remaining:.0f} s.'
                self.status=Status('Target active','Panel confirmed; waiting for the target to end. '+detail)
            elif self.profile.buffs.enabled and self.profile.buffs.has_toggles:
                self.status=Status('Target active','Panel confirmed; toggle buffs will be checked before the next Metin.')
            if now-self.since>=timing.attack_timeout:
                self.session.failed+=1
                self.pause('Target timed out. Check the attack or an obstruction.');return None
            if self.absent>=required:
                self.session.completed+=1
                self.phase=Phase.SETTLING;self.since=now
                self.status=Status('Target ended','Panel disappeared; letting the frame settle before the next target.')
            return None
        if self.phase==Phase.ACQUIRING:
            if self.present>=required:self._active(now)
            elif now-self.since>=timing.acquire_timeout:
                self.session.failed+=1
                self.failures+=1
                if self.failures>=timing.max_rotations:
                    self.pause('Target panel was not confirmed within the click retry limit.');return None
                self.failed.append((self.clicked,now+timing.failed_cooldown))
                self.phase=Phase.SEEKING;self.since=now
                self.status=Status('Searching','Click did not confirm a panel; temporarily skipping this point.')
            return None
        # A panel appearing while searching/settling always blocks a new click.
        if observation.panel_present:
            if self.present>=required:self._active(now)
            return None
        if self.phase==Phase.INITIAL:
            initial_frames=max(required,3) if self.profile.buffs.enabled else required
            if self.absent<initial_frames:return None
            self.phase=Phase.SEEKING
        if self.phase==Phase.SETTLING:
            if now-self.since<timing.settle_seconds:return None
            self.phase=Phase.SEEKING;self.since=now
        if self.absent<required:
            self.status=Status('Checking panel','Waiting for confirmed target panel absence.')
            return None
        if self.buffs.needed(now,observation.buffs,attempted_skills=self.attempted_skills):
            self.buffs.begin(now,observation.buffs,attempted_skills=self.attempted_skills)
            self.phase=Phase.BUFFING
            return self._buff_step(now,observation.buffs)
        self.failed=[(point,until) for point,until in self.failed if now<until]
        candidates=[c for c in observation.candidates if not any(
            abs(c.point[0]-p[0])<=max(15,c.width) and abs(c.point[1]-p[1])<=max(15,c.height)
            for p,until in self.failed)]
        if candidates:
            point=candidates[0].point;self.clicked=point
            self.attempted_skills=set()
            self.phase=Phase.ACQUIRING;self.since=now;self.present=self.absent=0
            self.status=Status('Acquiring target','Click sent; waiting for a stable target panel.')
            return Action(ActionKind.CLICK,point)
        self.status=Status('Searching','Waiting for a stable color candidate in the search area.')
        if now-self.since>=timing.search_timeout:
            if self.rotations>=timing.max_rotations:
                self.pause('No Metin found within the search limit.');return None
            self.rotations+=1;self.phase=Phase.SETTLING;self.since=now;self.failed=[]
            self.status=Status('Rotating',f'Search {self.rotations}/{timing.max_rotations}.')
            return Action(ActionKind.ROTATE)
        return None

    def _buff_step(self,now,states=()):
        step=self.buffs.step(now,states)
        if step is not None:
            if step.skill_index is not None:
                self.attempted_skills.add(step.skill_index)
            self.status=Status('Buffing',step.label+'; waiting for the animation to finish.')
            kind=ActionKind.MOUNT if step.mounted_after is not None else ActionKind.SKILL
            return Action(kind,shortcut=step.shortcut)
        if not self.buffs.active:
            self.phase=Phase.SETTLING;self.since=now
            self.present=self.absent=0
            skipped=' Skipped skills: '+', '.join(self.buffs.skipped)+'.' if self.buffs.skipped else ''
            self.status=Status('Target ended','Buffing complete; letting the frame settle before selecting a target.'+skipped)
        return None

    def action_completed(self,now:float):
        if self.phase==Phase.BUFFING:self.buffs.acknowledge(now)
