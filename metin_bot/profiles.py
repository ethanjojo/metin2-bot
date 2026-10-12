"""One canonical local profile store, with atomic JSON commits."""
from dataclasses import replace
import json
import os
from pathlib import Path
from uuid import UUID, uuid4

import cv2
import numpy as np

from .config import Profile, Region
from .vision import validate_template


def atomic_json(path: Path, data: dict) -> None:
    temporary = path.with_suffix(path.suffix + '.tmp')
    try:
        temporary.write_text(json.dumps(data, indent=2, ensure_ascii=False) + '\n', encoding='utf-8')
        os.replace(temporary, path)
    finally:
        temporary.unlink(missing_ok=True)


class ProfileStore:
    def __init__(self, root: Path):
        self.root = root
        self.root.mkdir(parents=True, exist_ok=True)

    def _folder(self, identifier: str) -> Path:
        if not isinstance(identifier, str) or str(UUID(identifier)) != identifier:
            raise ValueError('Invalid profile UUID.')
        folder = self.root / identifier
        if folder.is_symlink() or not folder.resolve().is_relative_to(self.root.resolve()):
            raise ValueError('The profile is outside the store.')
        return folder

    def load(self, identifier: str) -> tuple[Profile, np.ndarray | None]:
        folder = self._folder(identifier)
        profile = Profile.from_dict(json.loads((folder / 'profile.json').read_text(encoding='utf-8')))
        if profile.id != identifier:
            raise ValueError('The profile identity does not match its folder.')
        template = None
        if profile.template_file:
            asset = folder / profile.template_file
            if asset.is_symlink():
                raise ValueError('The template must not be a symbolic link.')
            if not asset.is_file():
                raise ValueError('The panel template is missing.')
            template = cv2.imread(str(asset))
            if template is None:
                raise ValueError('The panel template is missing or is not a valid image.')
            validate_template(template)
        return profile, template

    def list(self) -> list[Profile]:
        result = []
        for path in self.root.iterdir():
            if path.is_dir() and (path / 'profile.json').exists():
                result.append(self.load(path.name)[0])
        return sorted(result, key=lambda p: (p.name.casefold(), p.id))

    def save(self, profile: Profile, template: np.ndarray | None = None) -> Profile:
        folder = self._folder(profile.id)
        folder.mkdir(exist_ok=True)
        old = None
        if (folder / 'profile.json').exists():
            old = self.load(profile.id)[0].template_file
        created = None
        try:
            if template is not None:
                validate_template(template)
                created = folder / f'{uuid4()}.png'
                if not cv2.imwrite(str(created), template):
                    raise OSError('Cannot save the template.')
                profile = replace(profile, template_file=created.name)
            elif profile.template_file:
                asset = folder / profile.template_file
                if asset.is_symlink() or not asset.is_file():
                    raise ValueError('The panel template is missing.')
            atomic_json(folder / 'profile.json', profile.to_dict())
        except Exception:
            if created:
                created.unlink(missing_ok=True)
            raise
        if old and old != profile.template_file:
            (folder / old).unlink(missing_ok=True)
        return profile

    def select(self, identifier: str) -> None:
        self.load(identifier)
        atomic_json(self.root / 'selection.json', {'profile_id': identifier})

    def selected(self) -> str | None:
        path = self.root / 'selection.json'
        if not path.exists():
            return None
        data = json.loads(path.read_text(encoding='utf-8'))
        if set(data) != {'profile_id'}:
            raise ValueError('Invalid profile preference.')
        self.load(data['profile_id'])
        return data['profile_id']

    def migrate_legacy(self, path: Path) -> Profile | None:
        if not path.exists():
            return None
        values = json.loads(path.read_text(encoding='utf-8'))
        backup = path.with_suffix('.queue-backup.json')
        if backup.exists():
            raise ValueError('A backup of the old configuration already exists; migration cannot be repeated.')
        profile = Profile.create('Verzum')
        # Deterministic ID also makes recovery after interrupted migration idempotent.
        from uuid import uuid5, NAMESPACE_URL
        profile = replace(profile, id=str(uuid5(NAMESPACE_URL, str(path.resolve()))),
                          process_name=values.get('process_name', profile.process_name),
                          region=Region(**values['region']) if values.get('region') else None)
        if not (self._folder(profile.id) / 'profile.json').exists():
            self.save(profile)
        else:
            profile = self.load(profile.id)[0]
        self.select(profile.id)
        path.replace(backup)
        return profile
