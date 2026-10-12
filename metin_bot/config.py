from dataclasses import asdict, dataclass, field, replace
import math
from uuid import UUID, uuid4
from .hotkeys import hotkey_keys


def number(value: object, name: str, low: float, high: float) -> None:
    if type(value) not in (int, float) or not math.isfinite(value) or not low <= value <= high:
        raise ValueError(f'{name}: expected a number from {low} to {high}.')


@dataclass(frozen=True)
class Region:
    left: float
    top: float
    right: float
    bottom: float

    def __post_init__(self):
        for name, value in asdict(self).items():
            number(value, name, 0, 1)
        if self.right - self.left < .001 or self.bottom - self.top < .001:
            raise ValueError('The region must have a positive width and height of at least 0.1% of the image.')

    def pixels(self, width: int, height: int) -> tuple[int, int, int, int]:
        return (min(width - 1, round(self.left * width)), min(height - 1, round(self.top * height)),
                min(width - 1, round(self.right * width)), min(height - 1, round(self.bottom * height)))


def integer(value: object, name: str, low: int, high: int) -> None:
    if type(value) is not int:
        raise ValueError(f'{name}: expected an integer.')
    number(value, name, low, high)


@dataclass(frozen=True)
class NumberRule:
    low:float
    high:float
    step:float
    integral:bool=False

    def validate(self,value:object,name:str)->None:
        (integer if self.integral else number)(value,name,self.low,self.high)


# One source of truth for profile validation and numeric editor controls.
NUMBER_RULES={
    'hue_tolerance':NumberRule(1,89,1,True),
    'saturation_tolerance':NumberRule(1,255,1,True),
    'value_tolerance':NumberRule(1,255,1,True),
    'min_area':NumberRule(4,100000,10,True),
    'min_width':NumberRule(2,1000,1,True),
    'min_height':NumberRule(2,1000,1,True),
    'stable_frames':NumberRule(2,10,1,True),
    'max_area_ratio':NumberRule(.0001,.2,.001),
    'threshold':NumberRule(.5,.99,.01),
    'sample_interval':NumberRule(.1,2,.05),
    'settle_seconds':NumberRule(.5,10,.1),
    'click_hover_seconds':NumberRule(.05,1,.05),
    'click_hold_seconds':NumberRule(.05,1,.05),
    'search_timeout':NumberRule(2,120,1),
    'acquire_timeout':NumberRule(3,120,1),
    'attack_timeout':NumberRule(10,600,1),
    'rotate_seconds':NumberRule(.05,1,.05),
    'failed_cooldown':NumberRule(1,120,1),
    'max_rotations':NumberRule(1,12,1,True),
    'wait_seconds':NumberRule(.2,30,.1),
    'interval_seconds':NumberRule(10,86400,1),
    'mount_wait_seconds':NumberRule(.2,10,.1),
    'key_hold_seconds':NumberRule(.05,.5,.05),
}


@dataclass(frozen=True)
class ColorConfig:
    hsv: tuple[int, int, int]
    hue_tolerance: int = 10
    saturation_tolerance: int = 65
    value_tolerance: int = 65
    min_area: int = 120
    min_width: int = 12
    min_height: int = 18
    max_area_ratio: float = .03
    stable_frames: int = 3

    def __post_init__(self):
        if not isinstance(self.hsv, tuple) or len(self.hsv) != 3:
            raise ValueError('hsv must contain three values.')
        for i, value in enumerate(self.hsv):
            integer(value, 'hsv', 0, 179 if i == 0 else 255)
        for name in ('hue_tolerance','saturation_tolerance','value_tolerance','min_area','stable_frames','min_width','min_height','max_area_ratio'):
            NUMBER_RULES[name].validate(getattr(self,name),name)


@dataclass(frozen=True)
class PanelConfig:
    region: Region | None = None
    threshold: float = .88
    stable_frames: int = 3

    def __post_init__(self):
        if self.region is not None and not isinstance(self.region, Region):
            raise ValueError('Invalid panel region.')
        NUMBER_RULES['threshold'].validate(self.threshold,'threshold')
        NUMBER_RULES['stable_frames'].validate(self.stable_frames,'stable_frames')


@dataclass(frozen=True)
class TimingConfig:
    click_hover_seconds: float = .15
    click_hold_seconds: float = .20
    sample_interval: float = .25
    settle_seconds: float = 1
    search_timeout: float = 5
    acquire_timeout: float = 20
    attack_timeout: float = 120
    max_rotations: int = 4
    rotate_seconds: float = .35
    failed_cooldown: float = 10

    def __post_init__(self):
        for name in ('sample_interval','settle_seconds','click_hover_seconds','click_hold_seconds','search_timeout','acquire_timeout','attack_timeout','rotate_seconds','failed_cooldown','max_rotations'):
            NUMBER_RULES[name].validate(getattr(self,name),name)


@dataclass(frozen=True)
class BuffIcon:
    png:str
    region:Region|None=None

    def decode(self):
        import base64
        import cv2
        import numpy as np
        from .vision import validate_template
        if not isinstance(self.png,str) or len(self.png)>100000:raise ValueError('Invalid buff icon.')
        try:data=base64.b64decode(self.png,validate=True)
        except (ValueError,TypeError) as error:raise ValueError('Invalid buff icon.') from error
        if data[:8]!=b'\x89PNG\r\n\x1a\n':raise ValueError('The icon must be a PNG image.')
        # Check PNG dimensions before allocation by the decoder.
        if len(data)<24 or not all(5<=int.from_bytes(data[a:a+4],'big')<=128 for a in (16,20)):
            raise ValueError('Select an icon with dimensions of 5–128 pixels.')
        frame=cv2.imdecode(np.frombuffer(data,np.uint8),cv2.IMREAD_COLOR)
        if frame is None:raise ValueError('Cannot load the icon.')
        validate_template(frame)
        return frame

    def __post_init__(self):
        if self.region is not None and not isinstance(self.region,Region):raise ValueError('Invalid buff template region.')
        self.decode()

    @classmethod
    def capture(cls,frame,region=None):
        import base64
        import cv2
        ok,data=cv2.imencode('.png',frame)
        if not ok:raise ValueError('Cannot save the icon.')
        return cls(base64.b64encode(data).decode('ascii'),region)


@dataclass(frozen=True)
class BuffSkill:
    shortcut:str
    wait_seconds:float=1.5
    mode:str='refresh'
    icon:BuffIcon|None=None
    inactive_icon:BuffIcon|None=None
    interval_seconds:float|None=None
    name:str=''

    def __post_init__(self):
        if not isinstance(self.name,str) or len(self.name)>60 or any(ord(c)<32 for c in self.name):
            raise ValueError('The skill name must contain at most 60 characters and no control characters.')
        object.__setattr__(self,'name',self.name.strip())
        hotkey_keys(self.shortcut)
        NUMBER_RULES['wait_seconds'].validate(self.wait_seconds,'Wait after skill')
        if self.mode not in ('refresh','inactive'):raise ValueError('Invalid buff mode.')
        if self.mode=='refresh':
            if self.interval_seconds is None:object.__setattr__(self,'interval_seconds',120)
            NUMBER_RULES['interval_seconds'].validate(self.interval_seconds,'Skill interval')
        elif self.interval_seconds is not None:raise ValueError('A toggle skill does not have an interval.')
        for icon in (self.icon,self.inactive_icon):
            if icon is not None and not isinstance(icon,BuffIcon):raise ValueError('Invalid buff icon.')


@dataclass(frozen=True)
class BuffConfig:
    enabled:bool=False
    fight_mounted:bool=True
    mount_shortcut:str='Ctrl+G'
    mount_wait_seconds:float=1.5
    key_hold_seconds:float=.15
    skills:tuple[BuffSkill,...]=()
    icon_region:Region|None=None

    def __post_init__(self):
        for name in ('enabled','fight_mounted'):
            if type(getattr(self,name)) is not bool:raise ValueError(f'{name}: expected a boolean.')
        hotkey_keys(self.mount_shortcut)
        NUMBER_RULES['mount_wait_seconds'].validate(self.mount_wait_seconds,'Wait after mounting or dismounting')
        NUMBER_RULES['key_hold_seconds'].validate(self.key_hold_seconds,'Skill key hold duration')
        if not isinstance(self.skills,tuple) or len(self.skills)>8 or any(not isinstance(s,BuffSkill) for s in self.skills):
            raise ValueError('Configure at most eight skills.')
        if self.enabled and not self.skills:raise ValueError('Configure at least one skill for buffing.')
        if self.icon_region is not None and not isinstance(self.icon_region,Region):raise ValueError('Invalid buff icon region.')
        codes=[hotkey_keys(s.shortcut) for s in self.skills]
        if len(set(codes))!=len(codes):raise ValueError('Each skill must have a unique shortcut.')
        if hotkey_keys(self.mount_shortcut) in codes:raise ValueError('The mount and skill shortcuts must be different.')

    @property
    def ready(self):
        return not self.enabled or all(s.mode=='refresh' or (s.icon is not None and self.icon_region is not None) for s in self.skills)

    @property
    def has_refresh(self):return any(s.mode=='refresh' for s in self.skills)

    @property
    def execution_config(self):
        """Display names do not change scheduling or invalidate countdowns."""
        return replace(self,skills=tuple(replace(s,name='') for s in self.skills))

    @property
    def has_toggles(self):return any(s.mode=='inactive' for s in self.skills)


@dataclass(frozen=True)
class Profile:
    id: str
    name: str
    process_name: str = 'verzumclient_x64.exe'
    region: Region | None = None
    masks: tuple[Region, ...] = ()
    color: ColorConfig | None = None
    panel: PanelConfig = field(default_factory=PanelConfig)
    timing: TimingConfig = field(default_factory=TimingConfig)
    buffs: BuffConfig = field(default_factory=BuffConfig)
    frame_size: tuple[int, int] | None = None
    template_file: str | None = None
    template_region: Region | None = None

    def __post_init__(self):
        if not isinstance(self.id, str) or str(UUID(self.id)) != self.id:
            raise ValueError('Invalid profile UUID.')
        if not isinstance(self.name, str) or not self.name.strip() or len(self.name) > 80:
            raise ValueError('The profile name must contain 1 to 80 characters.')
        if not isinstance(self.process_name, str) or not self.process_name.lower().endswith('.exe') or any(c in self.process_name for c in '/\\'):
            raise ValueError('process_name must be an .exe filename without a path.')
        if self.region is not None and not isinstance(self.region, Region):
            raise ValueError('Invalid world region.')
        if self.template_region is not None and not isinstance(self.template_region, Region):
            raise ValueError('Invalid template region.')
        if not isinstance(self.masks, tuple) or len(self.masks) > 30 or any(not isinstance(r, Region) for r in self.masks):
            raise ValueError('Invalid masks.')
        if self.color is not None and not isinstance(self.color, ColorConfig):
            raise ValueError('Invalid color.')
        if not isinstance(self.panel, PanelConfig) or not isinstance(self.timing, TimingConfig):
            raise ValueError('Invalid configuration.')
        if not isinstance(self.buffs,BuffConfig):raise ValueError('Invalid buff configuration.')
        if self.frame_size is not None:
            if not isinstance(self.frame_size, tuple) or len(self.frame_size) != 2:
                raise ValueError('Invalid calibration frame size.')
            for value in self.frame_size:
                integer(value, 'frame_size', 100, 16384)
        if self.template_file is not None:
            if not isinstance(self.template_file, str) or not self.template_file.endswith('.png') or any(c in self.template_file for c in '/\\:') or '..' in self.template_file:
                raise ValueError('Invalid template path.')
        if self.timing.acquire_timeout <= (self.panel.stable_frames + 1) * self.timing.sample_interval * 1.5:
            raise ValueError('The confirmation timeout is too short for the required number of frames.')

    @classmethod
    def create(cls, name: str) -> 'Profile':
        return cls(str(uuid4()), name)

    @property
    def ready(self) -> bool:
        return all((self.region, self.color, self.panel.region, self.frame_size, self.template_file,self.buffs.ready))

    def to_dict(self) -> dict:
        return asdict(self)

    @classmethod
    def from_dict(cls, data: dict) -> 'Profile':
        if not isinstance(data, dict):
            raise ValueError('The profile must be a JSON object.')
        values = dict(data)
        try:
            if values.get('region') is not None:
                values['region'] = Region(**values['region'])
            if values.get('template_region') is not None:
                values['template_region'] = Region(**values['template_region'])
            values['masks'] = tuple(Region(**r) for r in values.get('masks', []))
            if values.get('color') is not None:
                color = dict(values['color']); color['hsv'] = tuple(color['hsv'])
                values['color'] = ColorConfig(**color)
            panel = dict(values.get('panel', {}))
            if panel.get('region') is not None:
                panel['region'] = Region(**panel['region'])
            values['panel'] = PanelConfig(**panel)
            values['timing'] = TimingConfig(**values.get('timing', {}))
            buffs=dict(values.get('buffs',{}))
            # Current mount state belongs to the session, never a saved profile.
            buffs.pop('initial_mounted',None)
            legacy_interval=buffs.pop('interval_seconds',120)
            number(legacy_interval,'Legacy buff interval',10,86400)
            skills=[]
            for source in buffs.get('skills',[]):
                skill=dict(source)
                if skill.get('mode','refresh')=='refresh':skill.setdefault('interval_seconds',legacy_interval)
                for key in ('icon','inactive_icon'):
                    if skill.get(key) is not None:
                        sample=dict(skill[key])
                        if sample.get('region') is not None:sample['region']=Region(**sample['region'])
                        skill[key]=BuffIcon(**sample)
                skills.append(BuffSkill(**skill))
            buffs['skills']=tuple(skills)
            if buffs.get('icon_region') is not None:buffs['icon_region']=Region(**buffs['icon_region'])
            values['buffs']=BuffConfig(**buffs)
            if values.get('frame_size') is not None:
                values['frame_size'] = tuple(values['frame_size'])
            return cls(**values)
        except (TypeError, KeyError) as error:
            raise ValueError(f'Invalid profile: {error}') from error
