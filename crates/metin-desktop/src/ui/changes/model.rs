use super::*;
use metin_core::{BuffIcon, Region, SkillIconId};
use metin_platform::Frame;
use uuid::Uuid;

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) enum Subject {
    Search,
    Color,
    Hp,
    Frame,
    Area(Option<Uuid>),
    Buff(Option<Uuid>),
    BuffArea,
    Mount,
    Loot,
    Periodic(Option<Uuid>),
    Puller,
    OutOfTarget,
    Rule(Option<Uuid>),
    Safety(SafetyPage),
    Timing,
}
impl Subject {
    pub(super) fn group(self) -> &'static str {
        match self {
            Self::Search | Self::Color | Self::Hp | Self::Frame | Self::Area(_) => "Recognition",
            Self::Buff(_) | Self::BuffArea => "Buffs",
            Self::Mount => "Mount",
            Self::Loot => "Loot",
            Self::Periodic(_) => "Periodic actions",
            Self::Puller => "Auto-puller",
            Self::OutOfTarget => "Out-of-target attack",
            Self::Rule(_) => "Visual rules",
            Self::Safety(_) => "Safety",
            Self::Timing => "Timing",
        }
    }
}

#[derive(Clone, PartialEq)]
pub(super) enum Preview {
    Color([u16; 3]),
    Icon(SkillIconId),
    Capture(BuffIcon),
    Frame(Frame),
}

#[derive(Clone, PartialEq)]
pub(super) struct Value {
    pub text: String,
    pub preview: Option<Preview>,
}
impl Value {
    pub(super) fn text(s: impl Into<String>) -> Self {
        Self {
            text: s.into(),
            preview: None,
        }
    }
    pub(super) fn illustrated(s: impl Into<String>, preview: Preview) -> Self {
        Self {
            text: s.into(),
            preview: Some(preview),
        }
    }
}

#[derive(Clone)]
pub(super) struct Field {
    pub label: String,
    pub before: Value,
    pub after: Value,
}
impl Field {
    pub(super) fn retained_seconds(
        label: &str,
        before: f64,
        after: f64,
        active: [bool; 2],
    ) -> Self {
        let value = |s: f64, used: bool| {
            let mut value = seconds(s);
            if !used {
                value.text.push_str(" (inactive setting)");
            }
            value
        };
        Self {
            label: label.into(),
            before: value(before, active[0]),
            after: value(after, active[1]),
        }
    }
}

#[derive(Clone)]
pub(in crate::ui) struct ProfileChange {
    pub(super) subject: Subject,
    pub(super) field: Field,
    pub(super) details: Vec<Field>,
}

/// Accumulates changes for one identifiable editor subject.
pub(super) struct Fields<'a> {
    rows: &'a mut Vec<ProfileChange>,
    subject: Subject,
    name: Option<String>,
    active: bool,
}
impl<'a> Fields<'a> {
    pub(super) fn new(rows: &'a mut Vec<ProfileChange>, subject: Subject) -> Self {
        Self {
            rows,
            subject,
            name: None,
            active: true,
        }
    }
    pub(super) fn named(mut self, name: &str) -> Self {
        self.name = Some(name.to_owned());
        self
    }
    pub(super) fn active(&mut self, active: bool) {
        self.active = active;
    }
    pub(super) fn field(&mut self, label: &str, before: Value, after: Value) {
        if before != after {
            self.compound(label, before, after, Vec::new());
        }
    }
    pub(super) fn compound(
        &mut self,
        label: &str,
        before: Value,
        after: Value,
        details: Vec<Field>,
    ) {
        let mut label = self
            .name
            .as_ref()
            .map_or_else(|| label.to_owned(), |name| format!("{name} · {label}"));
        if !self.active {
            label.push_str(" (inactive setting)");
        }
        self.rows.push(ProfileChange {
            subject: self.subject,
            field: Field {
                label,
                before,
                after,
            },
            details,
        });
    }
    pub(super) fn text(&mut self, label: &str, before: &str, after: &str) {
        let value = |s: &str| Value::text(if s.is_empty() { "Not set" } else { s });
        if before != after {
            self.compound(label, value(before), value(after), Vec::new());
        }
    }
    pub(super) fn toggle(&mut self, label: &str, before: bool, after: bool) {
        self.field(label, toggle(before), toggle(after));
    }
    pub(super) fn seconds(&mut self, label: &str, before: f64, after: f64) {
        self.field(label, seconds(before), seconds(after));
    }
    pub(super) fn count(&mut self, label: &str, before: u32, after: u32) {
        self.field(
            label,
            Value::text(before.to_string()),
            Value::text(after.to_string()),
        );
    }
}
pub(super) fn toggle(enabled: bool) -> Value {
    Value::text(if enabled { "On" } else { "Off" })
}
pub(super) fn seconds(n: f64) -> Value {
    Value::text(format!("{n} s"))
}
pub(super) fn frames(n: u32) -> Value {
    Value::text(format!("{n} {}", if n == 1 { "frame" } else { "frames" }))
}
pub(super) fn dimensions([w, h]: [u32; 2]) -> String {
    format!("{w} × {h} px")
}

/// Scaling the shortest decimal string preserves distinct stored thresholds without binary rounding noise.
pub(super) fn percent(fraction: f64) -> String {
    let decimal = fraction.to_string();
    let (whole, decimals) = decimal.split_once('.').unwrap_or((&decimal, ""));
    let mut digits = format!("{whole}{decimals:0<2}");
    let point = whole.len() + 2;
    if digits.len() > point {
        digits.insert(point, '.');
    }
    let result = digits.trim_start_matches('0');
    if result.is_empty() || result.starts_with('.') {
        format!("0{result}%")
    } else {
        format!("{result}%")
    }
}
#[derive(Clone, Copy)]
pub(super) struct Geometry(pub [Option<[u32; 2]>; 2]);
impl Geometry {
    pub(super) fn bounds(&self, side: usize, region: Option<Region>, missing: &str) -> String {
        let Some(r) = region else {
            return missing.to_owned();
        };
        if let Some([w, h]) = self.0[side]
            && r.validate().is_ok()
        {
            let [l, t, right, bottom] = r.pixels(w, h);
            format!(
                "{} at ({l}, {t})",
                dimensions([right - l + 1, bottom - t + 1])
            )
        } else {
            format!(
                "({}, {}) to ({}, {}) of frame",
                r.left, r.top, r.right, r.bottom
            )
        }
    }
    pub(super) fn region(
        &self,
        f: &mut Fields<'_>,
        label: &str,
        a: Option<Region>,
        b: Option<Region>,
        missing: &str,
    ) {
        f.field(
            label,
            Value::text(self.bounds(0, a, missing)),
            Value::text(self.bounds(1, b, missing)),
        );
    }
    pub(super) fn capture(&self, side: usize, icon: Option<&BuffIcon>) -> Value {
        match icon {
            None => Value::text("Not captured"),
            Some(icon) => {
                // Crop equality is expressed in source pixels by the caption.
                // Only the PNG payload participates in preview equality.
                let preview = BuffIcon {
                    png: icon.png.clone(),
                    region: None,
                };
                let crop = self.bounds(side, icon.region, "source area unavailable");
                Value::illustrated(format!("Captured · {crop}"), Preview::Capture(preview))
            }
        }
    }
}

pub(super) enum Entity<'a, T> {
    Added(&'a T),
    Removed(&'a T),
    Both(&'a T, &'a T),
}
/// Product lists are bounded (at most thirty items). Preserve their visible order.
pub(super) fn entities<'a, T>(
    before: &'a [T],
    after: &'a [T],
    id: impl Fn(&T) -> Uuid,
) -> Vec<Entity<'a, T>> {
    let mut items: Vec<_> = after
        .iter()
        .map(
            |current| match before.iter().find(|saved| id(saved) == id(current)) {
                Some(saved) => Entity::Both(saved, current),
                None => Entity::Added(current),
            },
        )
        .collect();
    items.extend(
        before
            .iter()
            .filter(|saved| !after.iter().any(|current| id(saved) == id(current)))
            .map(Entity::Removed),
    );
    items
}
pub(super) fn membership(
    rows: &mut Vec<ProfileChange>,
    subject: Subject,
    label: &str,
    name: &str,
    added: bool,
) {
    let (before, after) = if added {
        ("Not in profile", name)
    } else {
        (name, "Removed")
    };
    Fields::new(rows, subject).named(name).compound(
        label,
        Value::text(before),
        Value::text(after),
        Vec::new(),
    );
}
