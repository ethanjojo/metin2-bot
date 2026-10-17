use crate::{Result, require};
pub fn canonical_uuid(text: &str) -> Result<uuid::Uuid> {
    let id = uuid::Uuid::parse_str(text)
        .map_err(|_| crate::ValidationError("Invalid canonical profile UUID.".into()))?;
    require(id.to_string() == text, "Invalid canonical profile UUID.")?;
    Ok(id)
}
#[derive(Debug, Clone, Copy)]
pub struct NumberRule {
    pub name: &'static str,
    pub low: f64,
    pub high: f64,
    pub step: f64,
    pub integral: bool,
}
pub const NUMBER_RULES: &[NumberRule] = &[
    rule("puller_presses", 1., 20., 1., true),
    rule("puller_press_gap", 0.1, 5., 0.1, false),
    rule("puller_wait_seconds", 0.2, 30., 0.1, false),
    rule("puller_interval_seconds", 0.5, 86400., 0.5, false),
    rule("attack_hold_seconds", 0.2, 30., 0.2, false),
    rule("idle_search_seconds", 1., 600., 1., false),
    rule("search_attack_attempts", 1., 10., 1., true),
    rule("safety_duration_seconds", 0.5, 30., 0.5, false),
    rule("visual_timeout_seconds", 0.5, 120., 0.5, false),
    rule("visual_attempts", 1., 5., 1., true),
    rule("recovery_attempts", 1., 5., 1., true),
    rule("loot_presses", 1., 20., 1., true),
    rule("loot_delay_seconds", 0., 10., 0.1, false),
    rule("loot_interval_seconds", 0.1, 5., 0.1, false),
    rule("stop_after_seconds", 60., 86400., 60., false),
    rule("break_every_seconds", 60., 86400., 60., false),
    rule("break_seconds", 10., 14400., 10., false),
    rule("hue_tolerance", 1., 89., 1., true),
    rule("saturation_tolerance", 1., 255., 1., true),
    rule("value_tolerance", 1., 255., 1., true),
    rule("min_area", 4., 100000., 10., true),
    rule("min_width", 2., 1000., 1., true),
    rule("min_height", 2., 1000., 1., true),
    rule("stable_frames", 2., 10., 1., true),
    rule("max_area_ratio", 0.0001, 0.2, 0.001, false),
    rule("threshold", 0.5, 0.99, 0.01, false),
    rule("sample_interval", 0.1, 2., 0.05, false),
    rule("settle_seconds", 0.5, 10., 0.1, false),
    rule("click_hover_seconds", 0.05, 1., 0.05, false),
    rule("click_hold_seconds", 0.05, 1., 0.05, false),
    rule("search_timeout", 2., 120., 1., false),
    rule("acquire_timeout", 3., 120., 1., false),
    rule("attack_timeout", 10., 600., 1., false),
    rule("rotate_seconds", 0.05, 1., 0.05, false),
    rule("failed_cooldown", 1., 120., 1., false),
    rule("max_rotations", 1., 12., 1., true),
    rule("wait_seconds", 0.2, 30., 0.1, false),
    rule("interval_seconds", 10., 86400., 1., false),
    rule("mount_wait_seconds", 0.2, 10., 0.1, false),
    rule("key_hold_seconds", 0.05, 0.5, 0.05, false),
];
const fn rule(name: &'static str, low: f64, high: f64, step: f64, integral: bool) -> NumberRule {
    NumberRule {
        name,
        low,
        high,
        step,
        integral,
    }
}
pub fn number(name: &str, value: f64) -> Result<()> {
    let r = NUMBER_RULES
        .iter()
        .find(|r| r.name == name)
        .expect("known domain numeric rule");
    require(
        value.is_finite()
            && value >= r.low
            && value <= r.high
            && (!r.integral || value.fract() == 0.),
        &format!(
            "{name}: expected {}–{}{}.",
            r.low,
            r.high,
            if r.integral { " (integer)" } else { "" }
        ),
    )
}
