"""Geometry shared by canvas hover, drag preview and committed calibration."""
from dataclasses import dataclass
from .config import Region
from .calibration import Mode


@dataclass(frozen=True)
class Gesture:
    mode: Mode
    index: int | None
    handle: str
    start: tuple[int, int]
    original: Region | None


def handles(region: Region, size: tuple[int, int]) -> dict[str, tuple[float, float]]:
    w, h = size
    l, t, r, b = region.left*w, region.top*h, region.right*w, region.bottom*h
    x, y = (l+r)/2, (t+b)/2
    return {'nw':(l,t), 'n':(x,t), 'ne':(r,t), 'e':(r,y),
            'se':(r,b), 's':(x,b), 'sw':(l,b), 'w':(l,y)}


def hit_handle(region: Region, point: tuple[float, float], size: tuple[int, int],
               tolerance: float = 8) -> str | None:
    x, y = point
    for handle, (hx, hy) in handles(region, size).items():
        if abs(x-hx) <= tolerance and abs(y-hy) <= tolerance:
            return handle
    w, h = size
    if region.left*w <= x <= region.right*w and region.top*h <= y <= region.bottom*h:
        return 'move'
    return None


def transform_region(region: Region, start: tuple[int, int], end: tuple[int, int],
                     size: tuple[int, int], handle: str) -> Region:
    w, h = size
    dx, dy = (end[0]-start[0])/w, (end[1]-start[1])/h
    l, t, r, b = region.left, region.top, region.right, region.bottom
    if handle == 'move':
        dx = max(-l, min(1-r, dx)); dy = max(-t, min(1-b, dy))
        return Region(l+dx, t+dy, r+dx, b+dy)
    if handle not in handles(region, size):
        raise ValueError('Invalid region handle.')
    min_w, min_h = max(.002, 4/w), max(.002, 4/h)
    if 'w' in handle:l = max(0, min(r-min_w, l+dx))
    if 'e' in handle:r = min(1, max(l+min_w, r+dx))
    if 'n' in handle:t = max(0, min(b-min_h, t+dy))
    if 's' in handle:b = min(1, max(t+min_h, b+dy))
    return Region(l,t,r,b)
