# -*- mode: python ; coding: utf-8 -*-
import os
from pathlib import Path

root = Path(SPECPATH).parent
metadata_dir = Path(os.environ['METINBOT_BUILD_METADATA_DIR'])

a = Analysis(
    [str(root / 'packaging_entry.py')],
    pathex=[str(root)],
    binaries=[],
    datas=[(str(metadata_dir / 'build-info.json'), '.')],
    hiddenimports=[],
    hookspath=[],
    hooksconfig={},
    runtime_hooks=[],
    excludes=[],
    noarchive=False,
)
pyz = PYZ(a.pure)
exe = EXE(
    pyz, a.scripts, [],
    exclude_binaries=True,
    name='MetinBot',
    debug=False,
    bootloader_ignore_signals=False,
    strip=False,
    upx=False,
    console=False,
    uac_admin=True,
    version=str(metadata_dir / 'version-info.txt'),
)
coll = COLLECT(exe, a.binaries, a.datas, strip=False, upx=False, name='MetinBot')
