"""Built-in display labels remain English while original artwork keeps its identity."""
import hashlib
import importlib.util
import json
from pathlib import Path
import tempfile
import unittest

ROOT = Path(__file__).resolve().parents[1]
ASSETS = ROOT / "crates/metin-desktop/assets/metin-ui"
SPEC = importlib.util.spec_from_file_location("import_ui_skin", ROOT / "tools/import-ui-skin.py")
IMPORTER = importlib.util.module_from_spec(SPEC)
SPEC.loader.exec_module(IMPORTER)

# Normal/base-grade names checked against each official English Gameforge skill
# page, reached from Warrior, Ninja, Sura and Shaman on 2026-11-11.
ENGLISH_SKILLS = {
    1: "Three-Way Cut", 2: "Sword Spin", 3: "Berserk", 4: "Aura of the Sword", 5: "Dash",
    16: "Spirit Strike (W)", 17: "Bash", 18: "Stump", 19: "Strong Body", 20: "Sword Strike",
    31: "Ambush", 32: "Fast Attack", 33: "Rolling Dagger", 34: "Stealth", 35: "Poisonous Cloud",
    46: "Repetitive Shot", 47: "Arrow Shower", 48: "Fire Arrow", 49: "Feather Walk", 50: "Poison Arrow",
    61: "Finger Strike", 62: "Dragon Swirl", 63: "Enchanted Blade", 64: "Fear", 65: "Enchanted Armour", 66: "Dispel",
    76: "Dark Strike", 77: "Flame Strike", 78: "Flame Spirit", 79: "Dark Protection", 80: "Spirit Strike (S)", 81: "Dark Orb",
    91: "Flying Talisman", 92: "Shooting Dragon", 93: "Dragon's Roar", 94: "Blessing", 95: "Reflect", 96: "Dragon's Strength",
    106: "Lightning Throw", 107: "Summon Lightning", 108: "Lightning Claw", 109: "Cure", 110: "Swiftness", 111: "Attack Up",
}


class UiCatalogTests(unittest.TestCase):
    def setUp(self):
        self.manifest = json.loads((ASSETS / "manifest.json").read_text(encoding="utf-8"))

    def test_all_existing_class_skill_ids_have_official_english_base_names(self):
        skills = {int(key.removeprefix("skill/")): icon["label"]
                  for key, icon in self.manifest["icons"].items()
                  if key.startswith("skill/") and key != "skill/130"}
        self.assertEqual(skills, ENGLISH_SKILLS)

    def test_original_czech_names_are_provenance_only(self):
        icon = self.manifest["icons"]["skill/4"]
        self.assertEqual(icon.get("source_label"), "Aura meče")
        self.assertEqual(icon.get("source_label_origin"), "original_czech_skill_catalog")
        self.assertEqual(icon["label"], "Aura of the Sword")
        self.assertEqual(icon["label_origin"], "official_english_gameforge_wiki")

    def test_translation_preserves_atlas_pixels_and_all_116_regions(self):
        self.assertEqual(self.manifest["size"], [512, 512])
        self.assertEqual(len(self.manifest["regions"]), 116)
        self.assertEqual(hashlib.sha256((ASSETS / "widgets.png").read_bytes()).hexdigest(),
                         "0e3463aef03b57e12e815165958a5bc64c08202576ec38a71c72e62d80da738e")

    def test_runtime_catalog_is_regenerated_from_the_same_english_metadata(self):
        display = IMPORTER.load_display_catalog(ASSETS / "catalog.json")
        self.assertEqual([item["name"] for item in display["classes"]],
                         ["Warrior", "Ninja", "Sura", "Shaman"])
        self.assertEqual(IMPORTER.rust_catalog(display, self.manifest["icons"]),
                         (ASSETS / "catalog.rs").read_bytes())

    def test_importer_rejects_accidental_non_english_class_labels(self):
        display = json.loads((ASSETS / "catalog.json").read_text(encoding="utf-8"))
        display["classes"][0]["name"] = "Válečník"
        with tempfile.TemporaryDirectory() as temporary:
            path = Path(temporary) / "catalog.json"
            path.write_text(json.dumps(display), encoding="utf-8")
            with self.assertRaisesRegex(ValueError, "English class label"):
                IMPORTER.load_display_catalog(path)


if __name__ == "__main__":
    unittest.main()
