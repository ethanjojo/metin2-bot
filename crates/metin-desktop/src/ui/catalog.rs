//! Verified original artwork; all rendering shares the skin atlas upload.
use eframe::egui::{self, Color32, Rect};
use metin_core::SkillIconId;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Artwork {
    Ready,
    Attack,
    AutoAttack,
    Camera,
    Skills,
    Loot,
    Mount,
    Puller,
    Wait,
    Recover,
    Notice,
    Settings,
    MountUnknown,
    MountOnFoot,
    MountMounted,
}

impl Artwork {
    fn region(self) -> &'static str {
        match self {
            Self::Ready => "public/check_image",
            Self::Attack => "icon/utility/quickbar/mouseattack1",
            Self::AutoAttack => "icon/utility/quickbar/mouseauto1",
            Self::Camera => "icon/utility/quickbar/mousecamera1",
            Self::Skills => "icon/utility/quickbar/mouseskill1",
            Self::Loot => "published/native-inventory/moneyIcon",
            Self::Mount => "icon/skills/130-0",
            Self::Puller => {
                "icon/items/44bdf9d28903f2ff2b1c0557fb27eaf23b312a4a80c281d733e6694166de611c"
            }
            Self::Wait => "icon/utility/party/time",
            Self::Recover => "icon/utility/guild/refresh1",
            Self::Notice => "icon/utility/guild/notice",
            Self::Settings => "icon/utility/quickbar/system1",
            Self::MountUnknown => "pattern/horsestate/00",
            Self::MountOnFoot => "pattern/horsestate/13",
            Self::MountMounted => "pattern/horsestate/33",
        }
    }
}

pub fn paint(ui: &egui::Ui, artwork: Artwork, rect: Rect) {
    super::skin::sprite(ui, artwork.region(), rect);
    if artwork == Artwork::MountUnknown {
        ui.painter().text(
            rect.right_bottom() - egui::vec2(1., 0.),
            egui::Align2::RIGHT_BOTTOM,
            "?",
            egui::FontId::proportional((rect.height() * 0.6).max(10.)),
            Color32::from_rgb(244, 225, 163),
        );
    }
}

#[derive(Debug, Clone, Copy)]
pub struct SkillEntry {
    pub id: SkillIconId,
    pub name: &'static str,
    region: &'static str,
}

#[derive(Debug, Clone, Copy)]
pub struct SkillClass {
    pub id: &'static str,
    pub name: &'static str,
    pub portrait: &'static str,
    pub skills: &'static [SkillEntry],
}

const fn entry(id: u16, name: &'static str, region: &'static str) -> SkillEntry {
    let Some(id) = SkillIconId::from_id(id) else {
        panic!("Unsupported class skill in embedded catalog")
    };
    SkillEntry { id, name, region }
}

// Typed display metadata is generated from the single English catalog.
include!("../../assets/metin-ui/catalog.rs");

pub fn skill(id: SkillIconId) -> &'static SkillEntry {
    CLASSES
        .iter()
        .flat_map(|class| class.skills)
        .find(|entry| entry.id == id)
        .expect("validated skill icon has a complete embedded catalog entry")
}

pub fn paint_skill(ui: &egui::Ui, id: SkillIconId, rect: Rect) {
    super::skin::sprite(ui, skill(id).region, rect);
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashSet;

    #[test]
    fn complete_class_catalog_uses_verified_english_names_and_original_base_images() {
        let manifest: serde_json::Value =
            serde_json::from_slice(include_bytes!("../../assets/metin-ui/manifest.json")).unwrap();
        let mut ids = HashSet::new();
        assert_eq!(CLASSES.map(|class| class.skills.len()), [10, 10, 12, 12]);
        for class in CLASSES {
            assert!(manifest["regions"].get(class.portrait).is_some());
            for entry in class.skills {
                assert!(ids.insert(entry.id));
                let key = format!("skill/{}", entry.id.get());
                assert_eq!(manifest["icons"][&key]["label"], entry.name);
                assert_eq!(manifest["icons"][&key]["region"], entry.region);
                assert!(manifest["regions"].get(entry.region).is_some());
                assert_eq!(skill(entry.id).id, entry.id);
            }
        }
        assert_eq!(ids.len(), 44);
        for id in 0..=u16::MAX {
            assert_eq!(
                SkillIconId::from_id(id).is_some(),
                ids.iter().any(|entry| entry.get() == id)
            );
        }
    }

    #[test]
    fn activity_and_mount_artwork_resolve_without_another_texture_loader() {
        let manifest: serde_json::Value =
            serde_json::from_slice(include_bytes!("../../assets/metin-ui/manifest.json")).unwrap();
        for artwork in [
            Artwork::Ready,
            Artwork::Attack,
            Artwork::AutoAttack,
            Artwork::Camera,
            Artwork::Skills,
            Artwork::Loot,
            Artwork::Mount,
            Artwork::Puller,
            Artwork::Wait,
            Artwork::Recover,
            Artwork::Notice,
            Artwork::Settings,
            Artwork::MountUnknown,
            Artwork::MountOnFoot,
            Artwork::MountMounted,
        ] {
            assert!(manifest["regions"].get(artwork.region()).is_some());
        }
        assert_eq!(Artwork::MountOnFoot.region(), "pattern/horsestate/13");
        assert_eq!(Artwork::Wait.region(), "icon/utility/party/time");
        let ctx = egui::Context::default();
        let mut output = ctx.run_ui(Default::default(), |ui| {
            let rect = Rect::from_min_size(ui.cursor().min, egui::vec2(28., 28.));
            paint(ui, Artwork::MountUnknown, rect);
            paint_skill(ui, SkillIconId::new(96).unwrap(), rect);
        });
        assert_eq!(output.textures_delta.set.len(), 2); // egui font atlas + shared Metin atlas
        output.textures_delta.clear();
    }
}
