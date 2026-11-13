//! Cached presentation of differences between the canonical saved and edited profiles.
use super::*;
use serde_json::Value;

#[derive(Clone)]
pub(super) struct ProfileChange {
    label: String,
    before: String,
    after: String,
    action: SetupAction,
    buff: Option<usize>,
    path: Vec<String>,
}

#[derive(Clone)]
struct CachedChanges {
    profile: uuid::Uuid,
    revision: u64,
    unsaved_new: bool,
    rows: Arc<Vec<ProfileChange>>,
}

fn short_value(value: &Value, image: bool) -> String {
    if value.is_null() {
        return "Not set".into();
    }
    if image {
        return "Captured image".into();
    }
    match value {
        Value::Bool(on) => if *on { "On" } else { "Off" }.into(),
        Value::String(text) => {
            let mut chars = text.chars();
            let visible = chars.by_ref().take(80).collect::<String>();
            if chars.next().is_some() {
                format!("{visible}…")
            } else if visible.is_empty() {
                "Empty".into()
            } else {
                visible
            }
        }
        Value::Number(n) => n
            .as_f64()
            .filter(|value| value.fract() == 0.)
            .map_or_else(|| n.to_string(), |value| format!("{value:.0}")),
        Value::Array(_) | Value::Object(_) => "Configured".into(),
        Value::Null => unreachable!(),
    }
}

fn destination(path: &[String]) -> (SetupAction, Option<usize>) {
    let key = path.first().map(String::as_str).unwrap_or_default();
    let action = match key {
        "buffs"
            if path.get(1).is_some_and(|key| {
                matches!(
                    key.as_str(),
                    "fight_mounted" | "mount_shortcut" | "mount_wait_seconds"
                )
            }) =>
        {
            SetupAction::Actions(ActionsPage::Mount)
        }
        "buffs" => SetupAction::Actions(ActionsPage::Buffs),
        "loot" => SetupAction::Actions(ActionsPage::Loot),
        "periodic_actions" => SetupAction::Actions(ActionsPage::Periodic),
        "puller" => SetupAction::Actions(ActionsPage::Puller),
        "out_of_target" => SetupAction::Actions(ActionsPage::OutOfTarget),
        "safety" if path.get(1).is_some_and(|key| key == "rules") => SetupAction::VisualRules,
        "safety" => SetupAction::Safety,
        "timing" => SetupAction::Settings,
        _ => SetupAction::Calibration,
    };
    let buff = if key == "buffs" && path.get(1).is_some_and(|key| key == "skills") {
        path.get(2).and_then(|index| index.parse().ok())
    } else {
        None
    };
    (action, buff)
}

fn field_label(path: &[String], profile: &Profile) -> String {
    let name = path
        .last()
        .map(String::as_str)
        .unwrap_or("Profile")
        .replace('_', " ");
    let field = match name.as_str() {
        "interval seconds" => "Interval".into(),
        "png" => "Reference image".into(),
        "color preset id" => "Metin preset".into(),
        _ => {
            let mut chars = name.chars();
            chars.next().map_or_else(String::new, |first| {
                first.to_uppercase().collect::<String>() + chars.as_str()
            })
        }
    };
    let (section, buff) = destination(path);
    let item_name = match path.first().map(String::as_str) {
        Some("periodic_actions") => path
            .get(1)
            .and_then(|index| index.parse::<usize>().ok())
            .and_then(|index| profile.periodic_actions.get(index))
            .map(|action| action.name.as_str()),
        Some("masks") => path
            .get(1)
            .and_then(|index| index.parse::<usize>().ok())
            .and_then(|index| profile.masks.get(index))
            .map(|area| area.name.as_str()),
        Some("safety") if path.get(1).is_some_and(|key| key == "rules") => path
            .get(2)
            .and_then(|index| index.parse::<usize>().ok())
            .and_then(|index| profile.safety.rules.get(index))
            .map(|rule| rule.name.as_str()),
        _ => None,
    };
    if let Some(name) = item_name {
        return format!("{name} · {field}");
    }
    if let Some(index) = buff {
        let label = profile
            .buffs
            .skills
            .get(index)
            .map(|skill| skill.name.clone())
            .filter(|name| !name.is_empty())
            .unwrap_or_else(|| format!("Buff {}", index + 1));
        return format!("{label} · {field}");
    }
    let section = match section {
        SetupAction::Actions(ActionsPage::Buffs) => "Buffs",
        SetupAction::Actions(ActionsPage::Mount) => "Mount",
        SetupAction::Actions(ActionsPage::Loot) => "Loot",
        SetupAction::Actions(ActionsPage::Periodic) => "Periodic actions",
        SetupAction::Actions(ActionsPage::Puller) => "Auto-puller",
        SetupAction::Actions(ActionsPage::OutOfTarget) => "Out-of-target attack",
        SetupAction::VisualRules => "Visual rules",
        SetupAction::Safety => "Safety",
        SetupAction::Settings => "Settings",
        _ => "Recognition",
    };
    format!("{section} · {field}")
}

fn compare(
    before: &Value,
    after: &Value,
    path: &mut Vec<String>,
    profile: &Profile,
    rows: &mut Vec<ProfileChange>,
) {
    if before == after {
        return;
    }
    match (before, after) {
        (Value::Object(old), Value::Object(new)) => {
            let keys = old
                .keys()
                .chain(new.keys())
                .collect::<std::collections::BTreeSet<_>>();
            for key in keys {
                path.push(key.clone());
                compare(
                    old.get(key).unwrap_or(&Value::Null),
                    new.get(key).unwrap_or(&Value::Null),
                    path,
                    profile,
                    rows,
                );
                path.pop();
            }
        }
        (Value::Array(old), Value::Array(new)) => {
            for index in 0..old.len().max(new.len()) {
                path.push(index.to_string());
                compare(
                    old.get(index).unwrap_or(&Value::Null),
                    new.get(index).unwrap_or(&Value::Null),
                    path,
                    profile,
                    rows,
                );
                path.pop();
            }
        }
        _ => {
            let (action, buff) = destination(path);
            let image = path.last().is_some_and(|key| key == "png");
            let units = path.last().is_some_and(|key| key.ends_with("_seconds"))
                || (path.first().is_some_and(|key| key == "timing")
                    && path.last().is_some_and(|key| key != "max_rotations"));
            let format = |value: &Value| {
                if path.first().is_some_and(|key| key == "color_preset_id") {
                    return if value.is_null() {
                        "Custom"
                    } else {
                        "Named preset"
                    }
                    .to_owned();
                }
                let text = short_value(value, image);
                if units && value.is_number() {
                    format!("{text} s")
                } else {
                    text
                }
            };
            rows.push(ProfileChange {
                label: field_label(path, profile),
                before: format(before),
                after: format(after),
                action,
                buff,
                path: path.clone(),
            });
        }
    }
}

pub(super) fn profile_changes(
    before: &Profile,
    after: &Profile,
    image_changed: bool,
    unsaved_new: bool,
) -> Vec<ProfileChange> {
    let mut rows = Vec::new();
    // These values are produced only from typed Profile data. No JSON from a file
    // is interpreted here; validation and persistence remain owned by ProfileStore.
    if let (Ok(before), Ok(after_value)) =
        (serde_json::to_value(before), serde_json::to_value(after))
    {
        compare(&before, &after_value, &mut Vec::new(), after, &mut rows);
    }
    if image_changed {
        rows.push(ProfileChange {
            label: "Target HP reference image".into(),
            before: "Saved image".into(),
            after: "Updated image".into(),
            action: SetupAction::Calibration,
            buff: None,
            path: vec!["template_file".into()],
        });
    }
    if unsaved_new {
        rows.insert(
            0,
            ProfileChange {
                label: "New profile".into(),
                before: "Not saved".into(),
                after: after.name.clone(),
                action: SetupAction::Calibration,
                buff: None,
                path: vec!["name".into()],
            },
        );
    }
    rows
}

impl MetinApp {
    fn current_changes(&self, context: &egui::Context) -> Arc<Vec<ProfileChange>> {
        let id = egui::Id::unique("profile-change-summary");
        let unsaved_new = !self
            .profiles
            .iter()
            .any(|profile| profile.id == self.editor.profile.id);
        let cache = context.data(|data| data.get_temp::<CachedChanges>(id));
        if let Some(cache) = cache
            && cache.profile == self.editor.profile.id
            && cache.revision == self.editor.change_id()
            && cache.unsaved_new == unsaved_new
        {
            return cache.rows;
        }
        let rows = Arc::new(profile_changes(
            self.editor.saved_profile(),
            &self.editor.profile,
            self.editor.template_changed(),
            unsaved_new,
        ));
        context.data_mut(|data| {
            data.insert_temp(
                id,
                CachedChanges {
                    profile: self.editor.profile.id,
                    revision: self.editor.change_id(),
                    unsaved_new,
                    rows: rows.clone(),
                },
            )
        });
        rows
    }

    fn changes_caption(&self, ctx: &egui::Context) -> String {
        let count = self.current_changes(ctx).len();
        if ctx.content_rect().width() < 380. {
            count.to_string()
        } else {
            format!("{count} changes")
        }
    }
    pub(super) fn changes_width(&self, ui: &egui::Ui) -> f32 {
        if !self.has_unsaved_profile() {
            return 0.;
        }
        ui.painter()
            .layout_no_wrap(
                self.changes_caption(ui.ctx()),
                egui::FontId::proportional(13.),
                egui::Color32::WHITE,
            )
            .size()
            .x
            + 20.
            + ui.spacing().item_spacing.x
    }

    pub(super) fn changes_badge(&mut self, ui: &mut egui::Ui) {
        if !self.has_unsaved_profile() {
            return;
        }
        let rows = self.current_changes(ui.ctx());
        let response =
            widgets::plain_button(ui, self.changes_caption(ui.ctx())).on_hover_text(format!(
                "{} unsaved changes. Review before Save; Revert restores the saved profile.",
                rows.len()
            ));
        response.widget_info(|| {
            egui::WidgetInfo::labeled(
                egui::Role::Button,
                true,
                format!("{} unsaved profile changes", rows.len()),
            )
        });
        let mut selected = None;
        let popup = egui::Popup::from_toggle_button_response(&response)
            .frame(skin::popup_frame(ui))
            .width(330_f32.min(ui.ctx().content_rect().width() - 24.));
        #[cfg(feature = "ui-smoke")]
        let popup = if ui
            .ctx()
            .data(|data| data.get_temp::<bool>(egui::Id::unique("qol-changes-fixture")))
            .unwrap_or(false)
        {
            popup.open(true)
        } else {
            popup
        };
        popup.show(|ui| {
            ui.strong("Unsaved changes");
            ui.separator();
            egui::ScrollArea::vertical()
                .max_height(300.)
                .show(ui, |ui| {
                    for row in rows.iter() {
                        if widgets::plain_button(ui, &row.label).clicked() {
                            selected = Some(row.clone());
                            ui.close();
                        }
                        ui.add(
                            egui::Label::new(
                                egui::RichText::new(format!("{} to {}", row.before, row.after))
                                    .small(),
                            )
                            .wrap(),
                        );
                        ui.add_space(5.);
                    }
                });
            ui.small("Save and Revert are in the profile bar.");
        });
        if let Some(row) = selected {
            self.open_profile_change(&row);
        }
    }
    pub(super) fn open_profile_change(&mut self, row: &ProfileChange) {
        self.open_setup(row.action);
        match row.action {
            SetupAction::Settings => {
                self.settings_page = SettingsPage::Combat;
            }
            SetupAction::Safety => {
                self.safety_page = match row.path.get(1).map(String::as_str) {
                    Some("sound" | "system_notification") => SafetyPage::Notifications,
                    Some("recovery_enabled" | "recovery_attempts" | "recovery_wait_seconds") => {
                        SafetyPage::Recovery
                    }
                    _ => SafetyPage::Schedule,
                }
            }
            SetupAction::Actions(ActionsPage::Periodic) => {
                self.selected_periodic = row
                    .path
                    .get(1)
                    .and_then(|index| index.parse::<usize>().ok())
                    .and_then(|index| self.editor.profile.periodic_actions.get(index))
                    .map(|action| action.id)
            }
            SetupAction::VisualRules => {
                if let Some(index) = row
                    .path
                    .get(2)
                    .and_then(|index| index.parse::<usize>().ok())
                    .filter(|index| *index < self.editor.profile.safety.rules.len())
                {
                    self.selected_safety = index;
                    self.visual_detail = true;
                } else {
                    self.visual_detail = false;
                }
            }
            _ => {}
        }
        if let Some(index) = row.buff
            && index < self.editor.profile.buffs.skills.len()
        {
            self.selected_skill = index;
            self.buff_detail = true;
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn recognition_changes_open_recognition_and_timing_opens_settings() {
        for key in ["color", "panel", "color_preset_id"] {
            let path = vec![key.to_owned(), "stable_frames".to_owned()];
            assert!(
                matches!(destination(&path).0, SetupAction::Calibration),
                "{key}"
            );
        }
        assert!(matches!(
            destination(&["timing".to_owned(), "sample_interval".to_owned()]).0,
            SetupAction::Settings
        ));
    }

    #[test]
    fn preset_provenance_summary_uses_product_labels_instead_of_storage_ids() {
        let before = Profile::create("Main");
        let mut after = before.clone();
        after.color_preset_id = Some(uuid::Uuid::new_v4());
        let rows = profile_changes(&before, &after, false, false);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].label, "Recognition · Metin preset");
        assert_eq!(rows[0].before, "Custom");
        assert_eq!(rows[0].after, "Named preset");
    }

    #[test]
    fn summary_routes_buff_interval_and_hides_image_bytes() {
        let mut before = Profile::create("Main");
        let skill = metin_core::BuffSkill {
            name: "Blade".into(),
            interval_seconds: Some(120.),
            ..Default::default()
        };
        before.buffs.skills.push(skill);
        let mut after = before.clone();
        after.buffs.skills[0].interval_seconds = Some(180.);
        let rows = profile_changes(&before, &after, true, false);
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].buff, Some(0));
        assert_eq!(rows[0].before, "120 s");
        assert!(rows[0].label.contains("Blade"));
        assert_eq!(rows[0].after, "180 s");
        assert_eq!(
            short_value(&Value::String("SECRET IMAGE BYTES".into()), true),
            "Captured image"
        );
    }
}
