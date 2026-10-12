"""One entry point for source execution and the packaged Windows application."""
import argparse
import logging
import sys
from pathlib import Path
from .version import build_info


def main(argv:list[str]|None=None)->int:
    parser=argparse.ArgumentParser(description='Metin Bot — local color and target-panel assistant')
    parser.add_argument('--version',action='version',version=build_info()['version'])
    parser.add_argument('--data-dir',type=Path,help='Explicit isolated data directory; disables legacy auto-discovery.')
    parser.add_argument('--import-data',type=Path,help='Copy profiles/settings from an old project into an empty data directory.')
    parser.add_argument('--log',type=Path,help='Override the local diagnostic log path (no screenshots).')
    parser.add_argument('--self-test',action='store_true',help='Test bundled dependencies/UI/storage without game input.')
    parser.add_argument('--self-test-report',type=Path,help='Required JSON output for --self-test.')
    args=parser.parse_args(argv)
    if args.self_test:
        if not args.self_test_report or args.data_dir or args.log or args.import_data:
            parser.error('--self-test requires --self-test-report and cannot use --data-dir/--log/--import-data')
        from .self_test import run_self_test
        return run_self_test(args.self_test_report)
    if args.self_test_report:parser.error('--self-test-report requires --self-test')
    from .data_paths import prepare_data_directory,default_data_directory,legacy_data_directory,DATA_ENTRIES
    from .data_import_ui import choose_legacy_directory,ImportCancelled
    from .app import run
    try:
        destination=args.data_dir or default_data_directory()
        legacy=args.import_data
        empty=not destination.exists() or not any(destination.iterdir())
        if legacy is None and args.data_dir is None and getattr(sys,'frozen',False) and empty:
            adjacent=legacy_data_directory()
            if not any((adjacent/name).exists() for name in DATA_ENTRIES):
                legacy=choose_legacy_directory()
        if legacy is not None:
            if not legacy.is_dir() or not any((legacy/name).exists() for name in DATA_ENTRIES):
                raise ValueError('The import folder contains no Metin Bot profiles or settings.')
            if not empty:raise ValueError('The data directory already contains files. Import requires an empty directory; existing profiles are never overwritten.')
        directory=prepare_data_directory(args.data_dir,legacy_directory=legacy)
        log=args.log or directory/'logs'/'run.log'
        log.parent.mkdir(parents=True,exist_ok=True)
        logging.basicConfig(filename=log,encoding='utf-8',level=logging.INFO,
                            format='%(asctime)s %(name)s %(message)s',force=True)
        logging.info('MetinBot %s commit=%s',build_info()['version'],build_info()['commit_sha'])
        run(data_directory=directory)
        return 0
    except ImportCancelled:
        return 0
    except Exception as error:
        logging.exception('Application startup failed')
        from tkinter import messagebox
        messagebox.showerror('Unable to start Metin Bot',str(error))
        return 1
