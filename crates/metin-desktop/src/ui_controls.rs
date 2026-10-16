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
        let hint = match name {
            "sample_interval" => "Time between image samples. Shorter intervals use more CPU.",
            "stable_frames" => {
                "Consecutive stable frames required before accepting a detection or panel match."
            }
            "threshold" => {
                "Reference-image similarity required to confirm the target panel (0 to 1)."
            }
            "acquire_timeout" => {
                "Maximum time to confirm the clicked target. Must allow enough stable samples."
            }
            "attack_timeout" => "Maximum time spent attacking one target before abandoning it.",
            "search_timeout" => "How long to search before attempting camera rotation.",
            "settle_seconds" => {
                "Wait after the target panel disappears before looking for another target."
            }
            "wait_seconds" => {
                "Delay after sending this skill, before continuing the buff sequence."
            }
            "mount_wait_seconds" => "Delay after a mount toggle before continuing.",
            "key_hold_seconds" => "How long each buff or mount shortcut remains pressed.",
            "click_hover_seconds" => {
                "Wait for the cursor to settle over a candidate before clicking."
            }
            "click_hold_seconds" => "How long the target-selection mouse button remains pressed.",
            "rotate_seconds" => "Duration of each camera rotation during target search.",
            "failed_cooldown" => "How long a failed candidate position is avoided.",
            "max_rotations" => {
                "Maximum camera rotations or retries before ending this search attempt."
            }
            "max_area_ratio" => "Largest candidate area as a fraction of the full frame.",
            "hue_tolerance" => {
                "Allowed difference from the picked HSV hue, including hue wraparound."
            }
            "saturation_tolerance" => "Allowed difference from the picked HSV saturation.",
            "value_tolerance" => "Allowed difference from the picked HSV brightness.",
            "min_area" => "Minimum candidate area in pixels.",
            "min_width" => "Minimum candidate width in pixels.",
            "min_height" => "Minimum candidate height in pixels.",
            _ => {
                "Edit this value by dragging or typing. The profile's numeric limits are enforced."
            }
        };
        ui.label(label).on_hover_text(hint);
        ui.add_space((ui.available_width() - 90.).max(0.));
        ui.add(
            egui::DragValue::new(value)
                .range(rule.low..=rule.high)
                .speed(rule.step)
                .max_decimals(if rule.integral { 0 } else { 4 }),
        )
        .on_hover_text(format!(
            "{hint}\nAllowed range: {}–{}.",
            rule.low, rule.high
        ))
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

/// OpenCV HSV is computed on 8-bit sRGB. egui Hsva uses linear RGB;
/// route the picker through sRGB to avoid displaying a brighter sampled color.
pub fn sampled_hsv_to_srgb(hsv: [u16; 3]) -> [u8; 3] {
    egui::ecolor::rgb_from_hsv((
        hsv[0] as f32 / 180.,
        hsv[1] as f32 / 255.,
        hsv[2] as f32 / 255.,
    ))
    .map(|v| (v * 255.).round().clamp(0., 255.) as u8)
}
pub fn sampled_srgb_to_hsv(rgb: [u8; 3]) -> [u16; 3] {
    let (h, s, v) = egui::ecolor::hsv_from_rgb(rgb.map(|v| f32::from(v) / 255.));
    [
        (h * 180.).round().clamp(0., 179.) as u16,
        (s * 255.).round().clamp(0., 255.) as u16,
        (v * 255.).round().clamp(0., 255.) as u16,
    ]
}
