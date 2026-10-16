use eframe::egui;
use metin_core::{Key, Shortcut, validation::NUMBER_RULES};

pub enum CapturedKey {
    Letter(char),
    Digit(u8),
    Function(u8),
}
pub fn capture_shortcut(
    key: CapturedKey,
    ctrl: bool,
    shift: bool,
    alt: bool,
) -> Result<String, String> {
    let shortcut = Shortcut {
        ctrl,
        shift,
        alt,
        key: match key {
            CapturedKey::Letter(c) => Key::Letter(c),
            CapturedKey::Digit(n) => Key::Digit(n),
            CapturedKey::Function(n) => Key::Function(n),
        },
    };
    Shortcut::parse(&shortcut.to_string())
        .map(|s| s.to_string())
        .map_err(|e| e.to_string())
}
pub fn step_number(name: &str, value: f64, direction: i32) -> Result<f64, String> {
    let rule = NUMBER_RULES
        .iter()
        .find(|r| r.name == name)
        .ok_or("Unknown numeric rule.")?;
    Ok((value + rule.step * f64::from(direction)).clamp(rule.low, rule.high))
}
pub fn numeric(ui: &mut egui::Ui, label: &str, name: &str, value: &mut f64) -> bool {
    let rule = NUMBER_RULES
        .iter()
        .find(|r| r.name == name)
        .expect("known domain rule");
    ui.horizontal(|ui| {
        ui.label(label);
        ui.add(
            egui::DragValue::new(value)
                .range(rule.low..=rule.high)
                .speed(rule.step)
                .max_decimals(if rule.integral { 0 } else { 4 }),
        )
        .changed()
    })
    .inner
}
pub fn numeric_u32(ui: &mut egui::Ui, label: &str, name: &str, value: &mut u32) -> bool {
    let mut n = f64::from(*value);
    let changed = numeric(ui, label, name, &mut n);
    if changed {
        *value = n as u32;
    }
    changed
}
pub fn egui_key(key: egui::Key) -> Option<CapturedKey> {
    let name = format!("{key:?}");
    if let Some(number) = name.strip_prefix("Num").and_then(|n| n.parse().ok()) {
        Some(CapturedKey::Digit(number))
    } else if let Some(number) = name.strip_prefix('F').and_then(|n| n.parse().ok()) {
        Some(CapturedKey::Function(number))
    } else if name.len() == 1 {
        Some(CapturedKey::Letter(name.chars().next()?))
    } else {
        None
    }
}
