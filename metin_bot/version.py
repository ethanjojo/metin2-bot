"""Release metadata bundled alongside the executable's runtime files."""
import json
from pathlib import Path
import sys


def build_info()->dict:
    if getattr(sys,'frozen',False):
        path=Path(sys._MEIPASS)/'build-info.json'
        return json.loads(path.read_text(encoding='utf-8'))
    version=(Path(__file__).resolve().parent.parent/'VERSION').read_text(encoding='utf-8').strip()
    return {'version':version+'-dev','commit_sha':'unbuilt'}
