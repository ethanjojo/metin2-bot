"""One writable user-data location, with an atomic copy of older local data."""
import json
import logging
import os
from pathlib import Path
import shutil
import sys
import tempfile

from .profiles import ProfileStore
from .theme import UISettings


logger = logging.getLogger(__name__)
DATA_ENTRIES = ('profiles', 'ui-settings.json', 'config.json', 'config.queue-backup.json')


def default_data_directory() -> Path:
    """Fail explicitly when Windows has not supplied its per-user data directory."""
    location = os.environ.get('LOCALAPPDATA')
    if not location or not Path(location).is_absolute():
        raise RuntimeError('LOCALAPPDATA must contain an absolute Windows user-data path.')
    return Path(location) / 'MetinBot'


def legacy_data_directory() -> Path:
    """Frozen builds import beside the executable, never from PyInstaller _MEIPASS."""
    if getattr(sys, 'frozen', False):
        return Path(sys.executable).resolve().parent
    return Path(__file__).resolve().parent.parent


def _validate_data(directory: Path) -> None:
    if directory.is_symlink() or not directory.is_dir():
        raise ValueError(f'Application data must be a directory: {directory}')
    if (directory / 'config.json').exists() and (directory / 'config.queue-backup.json').exists():
        raise ValueError(f'Both legacy configuration and its backup exist in {directory}; '
                         'resolve the conflict before starting the application.')
    for name in DATA_ENTRIES:
        path = directory / name
        if path.is_symlink():
            raise ValueError(f'Application data cannot contain symbolic links: {path}')
        if not path.exists():
            continue
        if name == 'profiles':
            if not path.is_dir():
                raise ValueError(f'Profile storage must be a directory: {path}')
            for child in path.rglob('*'):
                if child.is_symlink():
                    raise ValueError(f'Profile storage cannot contain symbolic links: {child}')
            for child in path.iterdir():
                if child.is_dir() and not (child / 'profile.json').is_file():
                    raise ValueError(f'Incomplete profile directory: {child}')
            store = ProfileStore(path)
            store.list()
            store.selected()
        elif name == 'ui-settings.json':
            UISettings.load(path)
        else:
            values = json.loads(path.read_text(encoding='utf-8'))
            if not isinstance(values, dict):
                raise ValueError(f'Legacy configuration must be a JSON object: {path}')


def prepare_data_directory(data_directory: Path | None = None, *,
                           legacy_directory: Path | None = None) -> Path:
    """Return canonical storage, importing only when it has no existing contents.

    An injected directory never discovers real legacy data automatically. Tests
    and UI fixtures can opt into migration using an explicit legacy_directory.
    Existing storage is validated and wins as a whole: no merging or overwriting.
    Copy/validation failures leave originals and destination intact; a fresh
    adjacent staging directory allows the next launch to retry the whole copy.
    """
    destination = Path(data_directory) if data_directory is not None else default_data_directory()
    source = (Path(legacy_directory) if legacy_directory is not None else
              legacy_data_directory() if data_directory is None else None)
    if destination.is_symlink() or (source is not None and source.is_symlink()):
        raise ValueError('Application data directories cannot be symbolic links.')
    destination = destination.resolve()
    if source is not None:
        source = source.resolve()
    if destination.exists():
        _validate_data(destination)
        if any(destination.iterdir()):
            if source is not None and source != destination and any((source / name).exists() for name in DATA_ENTRIES):
                logger.info('Keeping existing application data at %s; legacy data at %s is unchanged.',
                            destination, source)
            return destination
    if source is None or source == destination or not any((source / name).exists() for name in DATA_ENTRIES):
        destination.mkdir(parents=True, exist_ok=True)
        return destination
    # Validation is read-only; no legacy profile conversion touches the originals.
    _validate_data(source)
    destination.parent.mkdir(parents=True, exist_ok=True)
    staging = Path(tempfile.mkdtemp(prefix='.MetinBot-migration-', dir=destination.parent))
    try:
        for name in DATA_ENTRIES:
            original = source / name
            if original.is_dir():
                shutil.copytree(original, staging / name)
            elif original.exists():
                shutil.copy2(original, staging / name)
        _validate_data(staging)
        # rmdir refuses anything but an empty directory; rename refuses existing
        # destinations on Windows. A concurrently created store is never replaced.
        if destination.exists():
            destination.rmdir()
        staging.rename(destination)
        logger.info('Copied legacy application data from %s to %s.', source, destination)
    finally:
        if staging.exists():
            shutil.rmtree(staging)
    return destination
