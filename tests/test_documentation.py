"""Keep published documentation links and captured assets usable."""
from html.parser import HTMLParser
import hashlib
from pathlib import Path
import json
import re
import struct
import unittest
from urllib.parse import unquote, urlsplit
import zlib

ROOT = Path(__file__).resolve().parents[1]


class Links(HTMLParser):
    def __init__(self):
        super().__init__()
        self.targets = []

    def handle_starttag(self, tag, attrs):
        for key, value in attrs:
            if key in ("src", "href") and value:
                self.targets.append(value)
            elif key == "srcset" and value:
                self.targets.extend(item.strip().split()[0] for item in value.split(","))


class DocumentationTests(unittest.TestCase):
    def test_local_markdown_and_html_links_resolve_inside_repository(self):
        public_docs = sorted(path for path in (ROOT / "docs").rglob("*.md")
                             if not path.is_relative_to(ROOT / "docs/superpowers"))
        for path in [ROOT / "README.md", ROOT / "CHANGELOG.md", *public_docs]:
            with self.subTest(document=path.relative_to(ROOT)):
                content = path.read_text(encoding="utf-8")
                parser = Links()
                parser.feed(content)
                targets = parser.targets + re.findall(r"!?\[[^\]]*\]\(([^\s)]+)(?:\s+[^)]*)?\)", content)
                for target in targets:
                    parsed = urlsplit(target)
                    if parsed.scheme or parsed.netloc or not parsed.path:
                        continue
                    resolved = (path.parent / unquote(parsed.path)).resolve()
                    self.assertTrue(resolved.is_relative_to(ROOT), target)
                    self.assertTrue(resolved.exists(), f"{path.name}: {target}")

    def test_documented_screenshots_are_complete_pngs(self):
        sizes = {name: (460, 620) for name in (
            "calibration", "actions", "run", "settings", "safety", "puller",
            "out-of-target", "periodic", "visual-rules", "action-catalog",
            "buff-detail", "buff-icon-picker", "run-setup", "run-break",
            "profile-menu", "profile-rename", "overlay-controls", "overlay-controls-off", "size-review",
            "updates", "update-available", "update-download", "update-ready", "update-error",
            "startup-update-check", "startup-update-available", "startup-update-download", "startup-update-error",
            "update-installing", "update-driver", "update-checking", "update-complete", "update-install-error",
        )}
        sizes["window-chooser"] = (780, 600)
        sizes["event-history"] = (400, 500)
        sizes["collapsed"] = (460, 42)
        seen_images = {}
        for name, expected_size in sizes.items():
            with self.subTest(screenshot=name):
                path = ROOT / "docs/assets" / f"{name}.png"
                self.assertTrue(path.is_file(), f"Missing documentation capture: {name}")
                data = path.read_bytes()
                digest = hashlib.sha256(data).hexdigest()
                self.assertNotIn(digest, seen_images,
                                 f"{name} reuses another view: {seen_images.get(digest)}")
                seen_images[digest] = name
                self.assertEqual(data[:8], b"\x89PNG\r\n\x1a\n")
                self.assertGreater(len(data), 1_000 if expected_size[1] < 100 else 10_000)
                offset, ended = 8, False
                while offset < len(data):
                    length = int.from_bytes(data[offset:offset + 4], "big")
                    tag = data[offset + 4:offset + 8]
                    payload = data[offset + 8:offset + 8 + length]
                    crc = int.from_bytes(data[offset + 8 + length:offset + 12 + length], "big")
                    self.assertEqual(zlib.crc32(tag + payload), crc)
                    if tag == b"IHDR":
                        self.assertEqual(struct.unpack(">II", payload[:8]), expected_size)
                    offset += length + 12
                    if tag == b"IEND":
                        ended = True
                        break
                self.assertTrue(ended)
                self.assertEqual(offset, len(data))

    def test_example_profile_uses_neutral_defaults(self):
        example = json.loads((ROOT / "profile.example.json").read_text(encoding="utf-8"))
        self.assertEqual(example["name"], "Main profile")
        self.assertNotIn("process_name", example)
        periodic = example["periodic_actions"][0]
        self.assertFalse(periodic["enabled"])
        self.assertEqual(periodic["first_run"], "after_interval")
        self.assertEqual(periodic["input"]["kind"], "key")
        self.assertNotIn("timing", periodic)
        visual = example["safety"]["rules"][0]
        self.assertFalse(visual["enabled"])
        self.assertIsNone(visual["reference"])
        self.assertEqual(visual["execution"]["input"], {"kind": "click", "region": None})
        self.assertEqual(visual["execution"]["verification"], "reference_gone")
        self.assertEqual(visual["execution"]["after_success"], "pause")


if __name__ == "__main__":
    unittest.main()
