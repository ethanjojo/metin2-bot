use eframe::egui;
use metin_core::{Key, Shortcut, validation::NUMBER_RULES};

pub enum CapturedKey {
    Space,
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
            CapturedKey::Space => Key::Space,
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
            "puller_presses"=>"Number of item-key presses per sequence. Remaining presses are cancelled on pause or target end.",
            "puller_press_gap"=>"Wait after releasing the item key before the next press in this sequence.",
            "puller_wait_seconds"=>"Additional settling time after a completed sequence, before the repeat interval starts.",
            "puller_interval_seconds"=>"Time between repeated combat sequences, after the configured settling time. No catch-up uses after a delayed frame or pause.",
            "attack_hold_seconds"=>"Maximum time to hold the attack key. A target panel, Pause/F8, focus loss or an error releases it sooner.",
            "idle_search_seconds"=>"Time spent searching or waiting after camera rotation without a target, excluding pauses and other actions. Existing search limits still apply.",
            "search_attack_attempts"=>"Maximum search attacks without a confirmed combat. Reaching this limit or an existing search limit pauses automation.",
            "visual_timeout_seconds"=>"Maximum time to reconfirm the event before input, and to verify disappearance after the post-input wait. Must allow enough fresh image samples.",
            "visual_attempts"=>"Maximum total input attempts, including the first. Retries require the event to remain visible; exhaustion pauses automation.",
            "safety_duration_seconds"=>"The reference must match in consecutive fresh frames for this long before the rule triggers.",
            "interval_seconds"=>"Repeat interval before this action becomes due again. Periodic actions count active running time and wait until there is no target.",
            "sample_interval" => "Time between image samples. Shorter intervals use more CPU.",
            "stable_frames" => {
                "Consecutive stable frames required before accepting a detection or panel match."
            }
            "threshold" => {
                "Reference-image similarity required to confirm a visual match (0 to 1)."
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
                "Delay after input before continuing or completing the action. Elapsed delay alone does not verify a game outcome."
            }
            "mount_wait_seconds" => "Delay after a mount toggle before continuing.",
            "key_hold_seconds" => "How long the action shortcut remains pressed before release.",
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
            "loot_presses" => "Number of pickup shortcut presses after each confirmed target end. A pause cancels remaining presses.",
            "loot_delay_seconds" => "Wait after confirmed panel disappearance before the first pickup press.",
            "loot_interval_seconds" => "Wait after releasing a pickup shortcut before sending the next press. Delayed frames never cause a burst of catch-up presses.",
            _ => {
                "Edit this value by dragging or typing. The profile's numeric limits are enforced."
            }
        };
        ui.label(label).on_hover_text(hint);
        help(ui, hint);
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
/// A visible, keyboard-focusable help affordance; its tooltip never changes layout.
pub fn help(ui: &mut egui::Ui, hint: &str) -> egui::Response {
    let response = ui.add(
        egui::Button::new("")
            .frame(false)
            .min_size(egui::vec2(20., 22.)),
    );
    response.widget_info(|| {
        egui::WidgetInfo::labeled(egui::Role::Button, true, format!("Help: {hint}"))
    });
    let p = ui.painter();
    let center = response.rect.center();
    let stroke = egui::Stroke::new(1.3, ui.visuals().weak_text_color());
    p.circle_stroke(center, 6.5, stroke);
    p.circle_filled(center + egui::vec2(0., -3.), 0.9, stroke.color);
    p.line_segment(
        [center + egui::vec2(0., -0.5), center + egui::vec2(0., 4.)],
        stroke,
    );
    if response.has_focus() {
        response.clone().show_tooltip_text(hint);
    }
    response
        .on_hover_text(hint)
        .on_hover_cursor(egui::CursorIcon::Help)
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
    if key == egui::Key::Space {
        Some(CapturedKey::Space)
    } else if let Some(number) = name.strip_prefix("Num").and_then(|n| n.parse().ok()) {
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
