"""Record image calls made by the original vision/buff tests, without game access."""
import sys,json,hashlib,unittest
from pathlib import Path
from dataclasses import asdict,replace
ROOT=Path(__file__).resolve().parents[1]
sys.path[:0]=[str(ROOT),str(ROOT/'tests')]
import cv2
import metin_bot.vision as vision
import metin_bot.buff_detection as buffs
from metin_bot.config import Profile
OUT=ROOT/'tests/parity/vision'
(OUT/'frames').mkdir(parents=True,exist_ok=True)
calls=[];detectors=[];trackers=[]
def image(frame):
    name=hashlib.sha256(bytes(str(frame.shape),'ascii')+frame.tobytes()).hexdigest()+'.png'
    path=OUT/'frames'/name
    if not path.exists():
        if not cv2.imwrite(str(path),frame):raise RuntimeError('Cannot save fixture')
    return name
original_detect=vision.detect_candidates
def detect(frame,profile):
    result=original_detect(frame,profile)
    calls.append(dict(kind='detect',frame=image(frame),profile=profile.to_dict(),expected=[asdict(c) for c in result]))
    return result
vision.detect_candidates=detect
original_score=vision.panel_score
def score(frame,template,region):
    result=original_score(frame,template,region)
    calls.append(dict(kind='panel',frame=image(frame),template=image(template),region=asdict(region),expected=result))
    return result
vision.panel_score=score
original_pick=vision.pick_color
def pick(frame,x,y):
    result=original_pick(frame,x,y)
    calls.append(dict(kind='color',frame=image(frame),point=[x,y],expected=result))
    return result
vision.pick_color=pick
OriginalDetector=buffs.BuffDetector
class Detector(OriginalDetector):
    def __init__(self,config,frame_size,sample_interval=.25):
        super().__init__(config,frame_size,sample_interval)
        self.record=dict(profile=replace(Profile.create('Detector fixture'),buffs=config).to_dict(),size=frame_size,interval=sample_interval,events=[])
        detectors.append(self.record)
    def observe(self,frame,now):
        result=super().observe(frame,now)
        self.record['events'].append(dict(frame=image(frame),now=now,expected=[s.value for s in result]))
        return result
buffs.BuffDetector=Detector
OriginalTracker=vision.CandidateTracker
class Tracker(OriginalTracker):
    def __init__(self,required,interval):
        super().__init__(required,interval)
        self.record=dict(required=required,interval=interval,events=[]);trackers.append(self.record)
    def update(self,candidates,now):
        result=super().update(candidates,now)
        self.record['events'].append(dict(candidates=[asdict(c) for c in candidates],now=now,expected=[asdict(c) for c in result]))
        return result
vision.CandidateTracker=Tracker
suite=unittest.TestSuite(unittest.defaultTestLoader.loadTestsFromName(name) for name in ['test_vision','test_buff_detection','test_hotbar_buffs'])
result=unittest.TextTestRunner(verbosity=1).run(suite)
if not result.wasSuccessful():raise SystemExit(1)
(OUT/'expected.json').write_text(json.dumps(dict(tests=result.testsRun,calls=calls,detectors=detectors,trackers=trackers),indent=2,ensure_ascii=False,allow_nan=False)+'\n',encoding='utf-8')
print(f'{result.testsRun} original tests; {len(calls)} vision calls; {len(detectors)} buff detectors; {sum(len(d["events"]) for d in detectors)} observations')
