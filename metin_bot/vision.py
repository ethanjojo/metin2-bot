from dataclasses import dataclass
import cv2
import numpy as np
from .config import Profile, Region


@dataclass(frozen=True)
class Candidate:
    x: int
    y: int
    width: int
    height: int
    point: tuple[int, int]


def validate_frame(frame: np.ndarray) -> None:
    if frame.dtype != np.uint8 or frame.ndim != 3 or frame.shape[2] != 3 or not frame.size:
        raise ValueError('Expected a nonempty uint8 BGR frame.')


def validate_template(template: np.ndarray) -> None:
    validate_frame(template)
    if min(template.shape[:2]) < 5 or np.std(cv2.cvtColor(template, cv2.COLOR_BGR2GRAY)) < 5:
        raise ValueError('Select a distinctive crop of at least 5 × 5 pixels, not a solid color area.')


def crop(frame: np.ndarray, region: Region) -> np.ndarray:
    x1,y1,x2,y2=region.pixels(frame.shape[1],frame.shape[0])
    return frame[y1:y2+1,x1:x2+1]


def pick_color(frame: np.ndarray, x: int, y: int) -> tuple[int,int,int]:
    validate_frame(frame)
    if not 0 <= x < frame.shape[1] or not 0 <= y < frame.shape[0]:
        raise ValueError('The color picker is outside the frame.')
    patch=frame[max(0,y-1):y+2,max(0,x-1):x+2]
    # Median BGR avoids the 179/0 discontinuity of a naive median hue.
    pixel=np.median(patch.reshape(-1,3),axis=0).astype(np.uint8).reshape(1,1,3)
    return tuple(int(v) for v in cv2.cvtColor(pixel,cv2.COLOR_BGR2HSV)[0,0])


def detect_candidates(frame: np.ndarray, profile: Profile) -> list[Candidate]:
    validate_frame(frame)
    if not profile.color or not profile.region:
        return []
    c=profile.color
    hsv=cv2.cvtColor(frame,cv2.COLOR_BGR2HSV).astype(np.int16)
    diff=np.abs(hsv[:,:,0]-c.hsv[0]);hue=np.minimum(diff,180-diff)
    selected=((hue<=c.hue_tolerance) & (np.abs(hsv[:,:,1]-c.hsv[1])<=c.saturation_tolerance) &
              (np.abs(hsv[:,:,2]-c.hsv[2])<=c.value_tolerance)).astype(np.uint8)*255
    allowed=np.zeros(selected.shape,np.uint8)
    x1,y1,x2,y2=profile.region.pixels(frame.shape[1],frame.shape[0])
    allowed[y1:y2+1,x1:x2+1]=255
    exclusions=list(profile.masks)
    if profile.panel.region:
        exclusions.append(profile.panel.region)
    for region in exclusions:
        a,b,d,e=region.pixels(frame.shape[1],frame.shape[0]);allowed[b:e+1,a:d+1]=0
    selected=cv2.bitwise_and(selected,allowed)
    joined=cv2.morphologyEx(selected,cv2.MORPH_CLOSE,np.ones((3,3),np.uint8))
    joined=cv2.bitwise_and(joined,allowed)
    count,labels,stats,centers=cv2.connectedComponentsWithStats(joined)
    found=[]
    max_area=frame.shape[0]*frame.shape[1]*c.max_area_ratio
    for label in range(1,count):
        x,y,w,h,area=(int(v) for v in stats[label])
        if w<c.min_width or h<c.min_height or not c.min_area<=area<=max_area or not .2<=w/h<=5:
            continue
        ys,xs=np.where((labels[y:y+h,x:x+w]==label) & (selected[y:y+h,x:x+w]>0))
        if not len(xs):continue
        cx,cy=centers[label];index=int(np.argmin((xs+x-cx)**2+(ys+y-cy)**2))
        found.append(Candidate(x,y,w,h,(int(xs[index]+x),int(ys[index]+y))))
    center=((x1+x2)/2,(y1+y2)/2)
    return sorted(found,key=lambda c: ((c.point[0]-center[0])**2+(c.point[1]-center[1])**2,c.x,c.y))


def panel_score(frame: np.ndarray, template: np.ndarray, region: Region) -> float:
    validate_frame(frame);validate_template(template)
    area=crop(frame,region)
    if template.shape[0]>area.shape[0] or template.shape[1]>area.shape[1]:
        raise ValueError('The template is larger than the panel search region.')
    match=cv2.matchTemplate(cv2.cvtColor(area,cv2.COLOR_BGR2GRAY),
                            cv2.cvtColor(template,cv2.COLOR_BGR2GRAY),cv2.TM_CCOEFF_NORMED)
    return float(np.max(match))


class CandidateTracker:
    def __init__(self, stable_frames: int, sample_interval: float):
        self.required=stable_frames
        self.max_gap=max(1.,sample_interval*1.5)
        self.reset()

    def reset(self) -> None:
        self.tracks=[]
        self.last_time=float('-inf')

    def update(self, candidates: list[Candidate], now: float) -> list[Candidate]:
        previous=list(self.tracks) if 0<now-self.last_time<=self.max_gap else []
        self.last_time=now;tracks=[]
        for c in candidates:
            matches=[(i,old,hits) for i,(old,hits) in enumerate(previous)
                     if abs(old.point[0]-c.point[0])<=max(10,old.width*.5) and
                     abs(old.point[1]-c.point[1])<=max(10,old.height*.5)]
            if matches:
                i,old,hits=min(matches,key=lambda m: abs(m[1].point[0]-c.point[0])+abs(m[1].point[1]-c.point[1]))
                previous.pop(i);tracks.append((c,min(hits+1,self.required)))
            else:tracks.append((c,1))
        self.tracks=tracks
        return [c for c,hits in tracks if hits>=self.required]


def annotate(frame: np.ndarray, candidates: list[Candidate], profile: Profile, score: float | None = None,
             *, regions: bool = True) -> np.ndarray:
    result=frame.copy()
    for region,color in [(profile.region,(255,180,70)),(profile.panel.region,(255,90,220)),
                          *((r,(80,80,255)) for r in profile.masks)]:
        if regions and region:
            x1,y1,x2,y2=region.pixels(result.shape[1],result.shape[0])
            cv2.rectangle(result,(x1,y1),(x2,y2),color,2)
    for i,c in enumerate(candidates,1):
        cv2.rectangle(result,(c.x,c.y),(c.x+c.width,c.y+c.height),(80,255,80),2)
        cv2.drawMarker(result,c.point,(80,255,80),cv2.MARKER_CROSS,12,2)
        cv2.putText(result,str(i),(c.x,c.y-4),cv2.FONT_HERSHEY_SIMPLEX,.5,(80,255,80),1)
    return result
