//! One bounded, self-contained profile document. Image names are never extracted.
use super::*;
use base64::{Engine, engine::general_purpose::STANDARD};
use std::io::Read;

const PACKAGE_SCHEMA: u32 = 1;
const MAX_PACKAGE_BYTES: usize = 16 * 1024 * 1024;
const MAX_TEMPLATE_BYTES: usize = 8 * 1024 * 1024;
const MAX_TEMPLATE_PIXELS: u64 = 16 * 1024 * 1024;

#[derive(Clone)]
pub struct ProfilePackage {
    profile: Profile,
    template: Option<Frame>,
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PortableDocument {
    schema: u32,
    profile: Profile,
    template_png: Option<String>,
}

fn decode_template(bytes: &[u8]) -> Result<Frame> {
    if bytes.len() > MAX_TEMPLATE_BYTES
        || bytes.len() < 24
        || !bytes.starts_with(b"\x89PNG\r\n\x1a\n")
    {
        return Err(invalid("Profile reference must be a PNG of at most 8 MiB."));
    }
    let width = u32::from_be_bytes(bytes[16..20].try_into().expect("checked PNG header"));
    let height = u32::from_be_bytes(bytes[20..24].try_into().expect("checked PNG header"));
    if u64::from(width) * u64::from(height) > MAX_TEMPLATE_PIXELS {
        return Err(invalid("Profile reference exceeds the 16 megapixel limit."));
    }
    let image = metin_vision::decode_png(bytes)?;
    metin_vision::validate_template(&image)?;
    Ok(image)
}

impl ProfilePackage {
    fn validate(&self) -> Result<()> {
        self.profile.validate()?;
        if self.profile.name.chars().any(char::is_control) {
            return Err(invalid("Profile names cannot contain control characters."));
        }
        if self.profile.template_file.is_some() {
            return Err(invalid(
                "Portable profiles cannot reference external image paths.",
            ));
        }
        validate_icons(&self.profile)?;
        if let Some(template) = &self.template {
            if u64::from(template.width) * u64::from(template.height) > MAX_TEMPLATE_PIXELS {
                return Err(invalid("Profile reference exceeds the 16 megapixel limit."));
            }
            metin_vision::validate_template(template)?;
            if let Some([width, height]) = self.profile.frame_size
                && (template.width > width || template.height > height)
            {
                return Err(invalid(
                    "Profile reference does not fit its calibrated game size.",
                ));
            }
        }
        Ok(())
    }

    pub fn profile_name(&self) -> &str {
        &self.profile.name
    }

    pub fn frame_size(&self) -> Option<[u32; 2]> {
        self.profile.frame_size
    }

    pub fn image_count(&self) -> usize {
        usize::from(self.template.is_some())
            + self
                .profile
                .buffs
                .skills
                .iter()
                .map(|buff| {
                    usize::from(buff.icon.is_some()) + usize::from(buff.inactive_icon.is_some())
                })
                .sum::<usize>()
            + self
                .profile
                .safety
                .rules
                .iter()
                .filter(|rule| rule.reference.is_some())
                .count()
    }

    /// Parse once, keep validated owned contents for the import preview, then
    /// import those exact contents even if the source document is later changed.
    pub fn read(path: &Path) -> Result<Self> {
        reject_link(path)?;
        let file = File::open(path)?;
        if file.metadata()?.len() > MAX_PACKAGE_BYTES as u64 {
            return Err(invalid("Profile package exceeds the 16 MiB limit."));
        }
        let mut bytes = Vec::new();
        file.take((MAX_PACKAGE_BYTES + 1) as u64)
            .read_to_end(&mut bytes)?;
        if bytes.len() > MAX_PACKAGE_BYTES {
            return Err(invalid("Profile package exceeds the 16 MiB limit."));
        }
        let mut document: PortableDocument = serde_json::from_slice(&bytes)?;
        if document.schema != PACKAGE_SCHEMA {
            return Err(invalid("Unsupported profile package version."));
        }
        // Normalize under the source profile before import assigns a new parent ID.
        document.profile.normalize_entity_ids();
        let template = document
            .template_png
            .map(|encoded| {
                if encoded.len() > MAX_TEMPLATE_BYTES.div_ceil(3) * 4 {
                    return Err(invalid("Profile reference exceeds the 8 MiB limit."));
                }
                let png = STANDARD
                    .decode(encoded)
                    .map_err(|_| invalid("Invalid Base64 profile reference."))?;
                decode_template(&png)
            })
            .transpose()?;
        let package = Self {
            profile: document.profile,
            template,
        };
        package.validate()?;
        Ok(package)
    }

    pub fn write(&self, path: &Path) -> Result<()> {
        self.validate()?;
        let template_png = self
            .template
            .as_ref()
            .map(|template| {
                let png = metin_vision::encode_png(template)?;
                if png.len() > MAX_TEMPLATE_BYTES {
                    return Err(invalid("Profile reference exceeds the 8 MiB limit."));
                }
                Ok(STANDARD.encode(png))
            })
            .transpose()?;
        let document = PortableDocument {
            schema: PACKAGE_SCHEMA,
            profile: self.profile.clone(),
            template_png,
        };
        let bytes = serde_json::to_vec_pretty(&document)?;
        if bytes.len() > MAX_PACKAGE_BYTES {
            return Err(invalid("Profile package exceeds the 16 MiB limit."));
        }
        if present(path)? {
            reject_link(path)?;
        }
        atomic_write(path, &bytes)
    }
}

impl ProfileStore {
    pub fn export_profile(&self, id: Uuid) -> Result<ProfilePackage> {
        let (mut profile, template) = self.load(id)?;
        profile.template_file = None;
        let package = ProfilePackage { profile, template };
        package.validate()?;
        Ok(package)
    }

    /// Save as a new store entry, without selecting it or copying session state.
    pub fn import_profile(&mut self, package: &ProfilePackage) -> Result<Profile> {
        package.validate()?;
        let mut profile = package.profile.clone();
        loop {
            profile.id = Uuid::new_v4();
            if !present(&self.folder(profile.id)?)? {
                break;
            }
        }
        Ok(self.save(&profile, package.template.as_ref())?.profile)
    }

    pub fn duplicate_profile(&mut self, id: Uuid) -> Result<Profile> {
        let mut package = self.export_profile(id)?;
        package.profile.name =
            package.profile.name.chars().take(73).collect::<String>() + " (copy)";
        self.import_profile(&package)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn sample() -> Frame {
        Frame::new(10, 10, (0..300).map(|n| (n % 251) as u8).collect()).unwrap()
    }

    #[test]
    fn legacy_package_normalizes_before_copy_identity_and_never_changes_the_source() {
        let root = tempfile::tempdir().unwrap();
        let mut store = ProfileStore::open(root.path()).unwrap();
        let path = root.path().join("legacy.companion-profile");
        let mut document = serde_json::json!({"schema":1,"profile":Profile::create("Legacy package"),"template_png":null});
        document["profile"]["buffs"]["skills"] = serde_json::json!([{"shortcut":"F1"}]);
        document["profile"]["masks"] =
            serde_json::json!([{"left":0.1,"top":0.2,"right":0.3,"bottom":0.4}]);
        let source = document.to_string();
        fs::write(&path, &source).unwrap();
        let first = ProfilePackage::read(&path).unwrap();
        let second = ProfilePackage::read(&path).unwrap();
        assert_eq!(first.profile, second.profile);
        let encoded = serde_json::to_value(&first.profile).unwrap();
        assert!(encoded["buffs"]["skills"][0]["id"].is_string());
        assert!(encoded["masks"][0]["id"].is_string());
        let copy_one = store.import_profile(&first).unwrap();
        let copy_two = store.import_profile(&second).unwrap();
        assert_ne!(copy_one.id, copy_two.id);
        assert_ne!(copy_one.id, first.profile.id);
        assert_eq!(copy_one.buffs, first.profile.buffs);
        assert_eq!(copy_two.masks, first.profile.masks);
        assert_eq!(fs::read_to_string(&path).unwrap(), source);
        let exported = store.export_profile(copy_one.id).unwrap();
        assert_eq!(exported.profile.buffs, first.profile.buffs);
        assert_eq!(exported.profile.masks, first.profile.masks);
    }

    #[test]
    fn portable_profile_roundtrip_keeps_images_and_settings_with_fresh_identity() {
        let root = tempfile::tempdir().unwrap();
        let mut store = ProfileStore::open(root.path()).unwrap();
        let mut profile = Profile::create("Example profile");
        profile.frame_size = Some([800, 600]);
        profile.buffs.skills.push(metin_core::BuffSkill::default());
        profile.buffs.skills[0].icon = Some(metin_core::BuffIcon {
            png: STANDARD.encode(metin_vision::encode_png(&sample()).unwrap()),
            region: None,
        });
        profile.buffs.skills[0].inactive_icon = profile.buffs.skills[0].icon.clone();
        let mut rule = metin_core::SafetyRule::new("Recognize dialog");
        rule.reference = profile.buffs.skills[0].icon.clone();
        profile.safety.rules.push(rule);
        profile.masks.push(metin_core::ExcludedArea::new(
            "Minimap",
            metin_core::Region {
                left: 0.8,
                top: 0.,
                right: 1.,
                bottom: 0.2,
            },
        ));
        profile
            .periodic_actions
            .push(metin_core::PeriodicAction::new("Use item"));
        profile.puller.presses = 2;
        let saved = store.save(&profile, Some(&sample())).unwrap().profile;
        store.select(saved.id).unwrap();
        let source = store.load(saved.id).unwrap();
        let package = store.export_profile(saved.id).unwrap();
        assert_eq!(package.image_count(), 4);
        let file = root.path().join("one.companion-profile");
        package.write(&file).unwrap();
        let read = ProfilePackage::read(&file).unwrap();
        assert_eq!(read.profile_name(), saved.name);
        assert_eq!(read.frame_size(), Some([800, 600]));
        let imported = store.import_profile(&read).unwrap();
        assert_ne!(imported.id, saved.id);
        let mut comparable = imported.clone();
        comparable.id = saved.id;
        comparable.template_file = saved.template_file.clone();
        assert_eq!(comparable, saved);
        assert_eq!(store.load(imported.id).unwrap().1, source.1);
        assert_eq!(store.load(saved.id).unwrap(), source);
        assert_eq!(store.selected().unwrap(), Some(saved.id));
    }

    #[test]
    fn duplicate_profile_has_independent_assets_and_bounded_copy_name() {
        let root = tempfile::tempdir().unwrap();
        let mut store = ProfileStore::open(root.path()).unwrap();
        let source = store
            .save(&Profile::create("é".repeat(80)), Some(&sample()))
            .unwrap()
            .profile;
        let copy = store.duplicate_profile(source.id).unwrap();
        assert_ne!(copy.id, source.id);
        assert_ne!(copy.template_file, source.template_file);
        assert!(copy.name.ends_with(" (copy)"));
        assert_eq!(copy.name.chars().count(), 80);
        assert_eq!(store.load(source.id).unwrap().0, source);
        assert_eq!(store.list().unwrap().len(), 2);
    }

    #[test]
    fn portable_profile_rejects_external_paths_unknown_fields_and_huge_images() {
        let root = tempfile::tempdir().unwrap();
        let path = root.path().join("invalid.companion-profile");
        let mut doc =
            serde_json::json!({"schema":1,"profile":Profile::create("Safe"),"template_png":null});
        for field in ["../escape.png", "C:\\escape.png", "plain.png"] {
            doc["profile"]["template_file"] = field.into();
            fs::write(&path, serde_json::to_vec(&doc).unwrap()).unwrap();
            assert!(ProfilePackage::read(&path).is_err());
        }
        doc["profile"]["template_file"] = serde_json::Value::Null;
        doc["runtime"] = serde_json::json!({"mounted":true,"timers":[0]});
        fs::write(&path, serde_json::to_vec(&doc).unwrap()).unwrap();
        assert!(ProfilePackage::read(&path).is_err());
        doc.as_object_mut().unwrap().remove("runtime");
        let mut bytes = metin_vision::encode_png(&sample()).unwrap();
        bytes[16..20].copy_from_slice(&16384_u32.to_be_bytes());
        bytes[20..24].copy_from_slice(&16384_u32.to_be_bytes());
        doc["template_png"] = STANDARD.encode(bytes).into();
        fs::write(&path, serde_json::to_vec(&doc).unwrap()).unwrap();
        assert!(
            ProfilePackage::read(&path)
                .err()
                .expect("malicious image must fail")
                .to_string()
                .contains("pixel")
        );
    }

    #[test]
    fn portable_profile_rejects_oversize_document_and_invalid_png_without_creating_profile() {
        let root = tempfile::tempdir().unwrap();
        let mut store = ProfileStore::open(root.path()).unwrap();
        let path = root.path().join("oversize.companion-profile");
        let file = File::create(&path).unwrap();
        file.set_len((MAX_PACKAGE_BYTES + 1) as u64).unwrap();
        assert!(ProfilePackage::read(&path).is_err());
        let doc = serde_json::json!({"schema":1,"profile":Profile::create("Safe"),"template_png":STANDARD.encode(b"not a png")});
        fs::write(&path, serde_json::to_vec(&doc).unwrap()).unwrap();
        assert!(ProfilePackage::read(&path).is_err());
        assert!(store.list().unwrap().is_empty());
        let package = ProfilePackage {
            profile: Profile::create("Bad\nname"),
            template: None,
        };
        assert!(store.import_profile(&package).is_err());
        assert!(store.list().unwrap().is_empty());
    }
}
