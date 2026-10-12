"""Worker-owned session counters; immutable snapshots cross the UI boundary."""
from dataclasses import dataclass


@dataclass(frozen=True)
class Statistics:
    active_seconds: float = 0
    completed: int = 0
    clicks: int = 0
    failed: int = 0


class Session:
    def __init__(self):
        self.reset()

    def reset(self):
        self.active_seconds=0.;self.completed=self.clicks=self.failed=0
        self.last_time=None;self.was_running=False

    def advance(self,now,running):
        if self.was_running and self.last_time is not None:
            self.active_seconds+=max(0,now-self.last_time)
        self.last_time=now;self.was_running=running

    def pause(self,now=None):
        if now is not None:self.advance(now,False)
        self.was_running=False

    def clicked(self):self.clicks+=1

    def snapshot(self):
        return Statistics(self.active_seconds,self.completed,self.clicks,self.failed)


def duration(seconds):
    seconds=max(0,int(seconds))
    hours,remainder=divmod(seconds,3600);minutes,seconds=divmod(remainder,60)
    return f'{hours:02}:{minutes:02}:{seconds:02}'
