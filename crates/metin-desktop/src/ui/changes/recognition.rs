use super::*;
use crate::storage::MetinPreset;
use metin_core::{ColorConfig, Region};
use metin_platform::Frame;

fn color_values(c: &ColorConfig) -> [(&'static str, Value); 8] {
    let [h, s, v] = c.hsv;
    [
        (
            "Target color",
            Value::illustrated(format!("HSV {h}, {s}, {v}"), Preview::Color(c.hsv)),
        ),
        ("Hue tolerance", Value::text(c.hue_tolerance.to_string())),
        (
            "Saturation tolerance",
            Value::text(c.saturation_tolerance.to_string()),
        ),
        (
            "Brightness tolerance",
            Value::text(c.value_tolerance.to_string()),
        ),
        (
            "Minimum candidate area",
            Value::text(format!("{} px²", c.min_area)),
        ),
        (
            "Minimum candidate size",
            Value::text(dimensions([c.min_width, c.min_height])),
        ),
        ("Maximum frame area", Value::text(percent(c.max_area_ratio))),
        ("Stable detections", frames(c.stable_frames)),
    ]
}
fn color(rows: &mut Vec<ProfileChange>, a: &Profile, b: &Profile, presets: &[MetinPreset]) {
    if a.color == b.color && a.color_preset_id == b.color_preset_id {
        return;
    }
    let preset = |p: &Profile| {
        let name = match p.color_preset_id {
            Some(id) => presets
                .iter()
                .find(|v| v.id == id)
                .map_or("Unavailable preset", |v| v.name.as_str()),
            None if p.color.is_none() => "Not configured",
            None => "Custom",
        };
        Value::text(name)
    };
    let f = &mut Fields::new(rows, Subject::Color);
    let old = a.color.as_ref().map(color_values);
    let new = b.color.as_ref().map(color_values);
    let mut details = Vec::new();
    // A named selection takes the copied settings with it. Detaching by editing
    // a value is implicit; explicitly detaching with identical values is not.
    let switch = a.color_preset_id != b.color_preset_id
        && (b.color_preset_id.is_some() || a.color == b.color);
    if let Some(fields) = new.as_ref().or(old.as_ref()) {
        for (i, (label, _)) in fields.iter().enumerate() {
            let value = |v: &Option<[(&str, Value); 8]>| {
                v.as_ref()
                    .map_or_else(|| Value::text("Not configured"), |items| items[i].1.clone())
            };
            let (before, after) = (value(&old), value(&new));
            if old.is_none() || new.is_none() || switch {
                if before != after {
                    details.push(Field {
                        label: (*label).into(),
                        before,
                        after,
                    });
                }
            } else {
                f.field(label, before, after);
            }
        }
    }
    if switch {
        f.compound("Metin preset", preset(a), preset(b), details);
    } else if old.is_none() || new.is_none() {
        // The color itself is visible without expanding the remaining calibration.
        if !details.is_empty() {
            let first = details.remove(0);
            f.compound("Metin detection", first.before, first.after, details);
        }
    }
}
pub(super) fn compare(
    rows: &mut Vec<ProfileChange>,
    a: &Profile,
    b: &Profile,
    images: [Option<&Frame>; 2],
    presets: &[MetinPreset],
    g: Geometry,
) {
    let area = |side: usize, r: Option<Region>| {
        let bounds = g.bounds(side, r, "Not selected");
        let full = r.is_some_and(|r| match g.0[side] {
            Some([w, h]) if w > 0 && h > 0 => r.pixels(w, h) == [0, 0, w - 1, h - 1],
            _ => r.left == 0. && r.top == 0. && r.right == 1. && r.bottom == 1.,
        });
        if full {
            Value::text(format!("Full window · {bounds}"))
        } else {
            Value::text(bounds)
        }
    };
    if g.bounds(0, a.region, "Not selected") != g.bounds(1, b.region, "Not selected") {
        Fields::new(rows, Subject::Search).field(
            "Search area",
            area(0, a.region),
            area(1, b.region),
        );
    }
    color(rows, a, b, presets);
    let f = &mut Fields::new(rows, Subject::Hp);
    let hp = |side: usize, p: &Profile| {
        let crop = g.bounds(side, p.template_region, "source area unavailable");
        match images[side] {
            Some(image) => Value::illustrated(
                format!(
                    "Image {} · Source {crop}",
                    dimensions([image.width, image.height])
                ),
                Preview::Frame(image.clone()),
            ),
            None if p.template_region.is_some() => Value::text(format!("No image · {crop}")),
            None => Value::text("Not captured"),
        }
    };
    f.field("HP panel reference", hp(0, a), hp(1, b));
    f.field(
        "HP match threshold",
        Value::text(percent(a.panel.threshold)),
        Value::text(percent(b.panel.threshold)),
    );
    f.field(
        "Stable HP matches",
        frames(a.panel.stable_frames),
        frames(b.panel.stable_frames),
    );
    let size =
        |s: Option<[u32; 2]>| Value::text(s.map_or_else(|| "Not calibrated".into(), dimensions));
    Fields::new(rows, Subject::Frame).field(
        "Calibrated frame size",
        size(a.frame_size),
        size(b.frame_size),
    );
    for item in entities(&a.masks, &b.masks, |area| area.id) {
        match item {
            Entity::Added(v) => membership(
                rows,
                Subject::Area(Some(v.id)),
                "Added excluded area",
                &v.name,
                true,
            ),
            Entity::Removed(v) => membership(
                rows,
                Subject::Area(None),
                "Removed excluded area",
                &v.name,
                false,
            ),
            Entity::Both(x, y) => {
                let f = &mut Fields::new(rows, Subject::Area(Some(y.id))).named(&y.name);
                f.text("Area name", &x.name, &y.name);
                g.region(
                    f,
                    "Excluded area",
                    Some(x.region),
                    Some(y.region),
                    "Not selected",
                );
            }
        }
    }
}
