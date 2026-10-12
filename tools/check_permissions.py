"""Read-only process elevation check. Does not activate windows or send input."""
import argparse
import os
from pathlib import Path
import sys

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))
from metin_bot.permissions import process_elevated


if __name__ == '__main__':
    parser = argparse.ArgumentParser(description=__doc__)
    parser.add_argument('pid', type=int, nargs='*')
    arguments = parser.parse_args()
    for pid in [os.getpid(), *arguments.pid]:
        try:
            print(f'PID {pid}: elevated={process_elevated(pid)}')
        except OSError as error:
            print(f'PID {pid}: unavailable, WinError {error.winerror}')
