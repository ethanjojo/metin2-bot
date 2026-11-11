# Metin UI artwork subset

Imported from the user-supplied `metin2-ui-egui-with-icons.zip` using
`tools/import-ui-skin.py`. This deterministic subset contains the original widgets,
all 44 base skill images from both specializations of four classes, the eight class
portraits, selected Actions/Run art, and native select/scrollbar art. The mounted,
on-foot and unknown indicators retain the exact source pixels; Unknown adds a text
question mark when rendered. Periodic actions use the application's Clock symbol;
the original atlas hourglass represents waiting and breaks.

Pass the source ZIP and `--check` to the importer to verify all three generated
outputs (`widgets.png`, `manifest.json` and `catalog.rs`) without modifying them.

The manifest records every imported source atlas SHA256, the source manifest and
catalog SHA256, per-region decoded RGBA hashes, original source rectangles and
source ownership. Packing changes coordinates only, never original RGBA pixels.
The single maintained `catalog.json` defines English display labels for Warrior,
Ninja, Sura and Shaman and all 44 normal/base skill names, with verification date
and individual official English Gameforge wiki source URLs. The importer generates
typed Rust entries in `catalog.rs` and matching English manifest labels. Original
labels remain only as `source_label`/`source_label_origin` provenance in the
manifest. Skill IDs, grade-zero regions and source tokens are unchanged.
User-entered names are not translated. Grade variants, unrelated items, bitmap
fonts and other atlas content are excluded. The subset remains 116 regions in one
512 × 512 atlas.

The supplied manifest identifies the art as the local CZ 2015 client asset set.
Original game art retains its existing ownership; the export grants no new asset
license. This source note must remain with redistributed assets.

The runtime uses egui for interaction, accessible labels, text input and focus.
The PNG is uploaded once per context as straight RGBA with nearest filtering.
Skill display icons are presentation metadata and never replace captured Active
or Inactive recognition references.
