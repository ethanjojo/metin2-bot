"""Explicit local profile diagnostics. Never sends game input."""
import argparse
from pathlib import Path
import sys
import cv2
sys.path.insert(0,str(Path(__file__).resolve().parent.parent))
from metin_bot.profiles import ProfileStore
from metin_bot.vision import annotate, detect_candidates, panel_score
from metin_bot.windows import WindowsAPI, choose_window


def main():
    parser=argparse.ArgumentParser(description=__doc__)
    parser.add_argument('--profile',required=True,help='Saved profile UUID.')
    parser.add_argument('--image',type=Path,help='Existing image instead of the foreground game.')
    parser.add_argument('--output',type=Path,required=True,help='Explicit local annotation path.')
    args=parser.parse_args()
    store=ProfileStore(Path(__file__).resolve().parent.parent/'profiles')
    profile,template=store.load(args.profile)
    if args.image:
        frame=cv2.imread(str(args.image))
        if frame is None:parser.error('Obrázek nelze načíst.')
    else:
        api=WindowsAPI();api.enable_dpi()
        frame=api.capture(choose_window(api.list_windows(profile.process_name)))
    if profile.frame_size is not None and (frame.shape[1],frame.shape[0])!=profile.frame_size:
        parser.error('Rozměry snímku neodpovídají kalibraci profilu.')
    candidates=detect_candidates(frame,profile)
    score=panel_score(frame,template,profile.panel.region) if template is not None and profile.panel.region else None
    args.output.parent.mkdir(parents=True,exist_ok=True)
    if not cv2.imwrite(str(args.output),annotate(frame,candidates,profile,score)):
        raise OSError('Nelze uložit anotaci.')
    print(f'Image: {frame.shape[1]}x{frame.shape[0]}; candidates: {len(candidates)}; panel score: {score}; output: {args.output.resolve()}')


if __name__=='__main__':main()
