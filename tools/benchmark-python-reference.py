"""Same fixture workload as the Rust vision benchmark; no screen or input."""
import json
import sys
import time
from pathlib import Path
ROOT = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(ROOT))
import cv2
from metin_bot.config import Profile
from metin_bot.vision import detect_candidates, panel_score

root = ROOT / 'tests/parity'
profile = Profile.from_dict(json.loads((root / 'profiles/calibrated.json').read_text(encoding='utf-8')))
template = cv2.imread(str(root / 'frames/template.png'))
results = []
for name in ['search.png', 'panel.png']:
    frame = cv2.imread(str(root / 'frames' / name))
    for run in range(1, 4):
        samples = []
        for iteration in range(110):
            start = time.perf_counter()
            detect_candidates(frame, profile)
            panel_score(frame, template, profile.panel.region)
            if iteration >= 10:
                samples.append((time.perf_counter() - start) * 1000)
        samples.sort()
        results.append(dict(frame=name, run=run, samples=100, median_ms=samples[50], p95_ms=samples[94]))
peak = None
if sys.platform == 'win32':
    import ctypes
    from ctypes import wintypes
    class Counters(ctypes.Structure):
        _fields_ = [('cb', wintypes.DWORD), ('PageFaultCount', wintypes.DWORD)] + [(name, ctypes.c_size_t) for name in ['PeakWorkingSetSize', 'WorkingSetSize', 'QuotaPeakPagedPoolUsage', 'QuotaPagedPoolUsage', 'QuotaPeakNonPagedPoolUsage', 'QuotaNonPagedPoolUsage', 'PagefileUsage', 'PeakPagefileUsage']]
    counters = Counters()
    get_info = ctypes.WinDLL('psapi', use_last_error=True).GetProcessMemoryInfo
    get_info.argtypes = [wintypes.HANDLE, ctypes.POINTER(Counters), wintypes.DWORD]
    get_info.restype = wintypes.BOOL
    if not get_info(wintypes.HANDLE(-1), ctypes.byref(counters), ctypes.sizeof(counters)):
        raise ctypes.WinError(ctypes.get_last_error())
    peak = counters.PeakWorkingSetSize
print(json.dumps(dict(cases=results, peak_working_set_bytes=peak), indent=2))
