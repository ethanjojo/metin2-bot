//! Reusable named color snapshots. Profiles keep their own runtime configuration.
use super::*;
use metin_core::ColorConfig;
use std::{collections::HashSet, io::Read};

const CATALOG_FILE: &str = "metin-presets.json";
const CATALOG_SCHEMA: u32 = 1;
const MAX_PRESETS: usize = 128;
const MAX_CATALOG_BYTES: usize = 1024 * 1024;

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct MetinPreset {
    #[serde(deserialize_with = "deserialize_id")]
    pub id: Uuid,
    pub name: String,
    #[serde(deserialize_with = "deserialize_color")]
    pub color: ColorConfig,
}

fn deserialize_id<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<Uuid, D::Error> {
    let id = String::deserialize(deserializer)?;
    metin_core::validation::canonical_uuid(&id).map_err(serde::de::Error::custom)
}

fn deserialize_color<'de, D: serde::Deserializer<'de>>(
    deserializer: D,
) -> std::result::Result<ColorConfig, D::Error> {
    let value = serde_json::Value::deserialize(deserializer)?;
    if value.get("hsv").is_none() {
        return Err(serde::de::Error::custom(
            "Color configuration requires HSV values.",
        ));
    }
    serde_json::from_value(value).map_err(serde::de::Error::custom)
}

#[derive(Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct PresetCatalog {
    schema: u32,
    presets: Vec<MetinPreset>,
}

fn validate_name(name: &str) -> Result<()> {
    if name.trim().is_empty() || name.chars().count() > 80 || name.chars().any(char::is_control) {
        return Err(invalid(
            "Preset names must contain 1–80 characters without control characters.",
        ));
    }
    Ok(())
}

fn validate_catalog(catalog: &PresetCatalog) -> Result<()> {
    if catalog.schema != CATALOG_SCHEMA {
        return Err(invalid("Unsupported Metin preset catalog version."));
    }
    if catalog.presets.len() > MAX_PRESETS {
        return Err(invalid("Store at most 128 Metin presets."));
    }
    let mut ids = HashSet::new();
    let mut names = HashSet::new();
    let mapper = icu_casemap::CaseMapper::new();
    for preset in &catalog.presets {
        validate_name(&preset.name)?;
        preset.color.validate()?;
        if !ids.insert(preset.id)
            || !names.insert(mapper.fold_string(preset.name.trim()).into_owned())
        {
            return Err(invalid("Metin preset identities and names must be unique."));
        }
    }
    Ok(())
}

pub(super) fn read_catalog(root: &Path) -> Result<Vec<MetinPreset>> {
    let path = root.join(CATALOG_FILE);
    if !present(&path)? {
        return Ok(Vec::new());
    }
    reject_link(&path)?;
    let file = File::open(&path)?;
    let metadata = file.metadata()?;
    if !metadata.is_file() || metadata.len() > MAX_CATALOG_BYTES as u64 {
        return Err(invalid(
            "Metin preset catalog must be a regular file of at most 1 MiB.",
        ));
    }
    let mut bytes = Vec::new();
    file.take((MAX_CATALOG_BYTES + 1) as u64)
        .read_to_end(&mut bytes)?;
    if bytes.len() > MAX_CATALOG_BYTES {
        return Err(invalid("Metin preset catalog exceeds the 1 MiB limit."));
    }
    let catalog: PresetCatalog = serde_json::from_slice(&bytes)?;
    validate_catalog(&catalog)?;
    Ok(catalog.presets)
}

impl ProfileStore {
    pub fn metin_presets(&self) -> Result<Vec<MetinPreset>> {
        read_catalog(&self.root)
    }

    /// Commit the catalog first; callers apply this snapshot to their draft only on success.
    pub fn save_metin_preset(
        &mut self,
        id: Option<Uuid>,
        name: &str,
        color: &ColorConfig,
    ) -> Result<MetinPreset> {
        if name.chars().any(char::is_control) {
            return Err(invalid("Preset names cannot contain control characters."));
        }
        let name = name.trim();
        validate_name(name)?;
        color.validate()?;
        let mut presets = self.metin_presets()?;
        let preset = MetinPreset {
            id: id.unwrap_or_else(Uuid::new_v4),
            name: name.to_owned(),
            color: color.clone(),
        };
        if id.is_some() {
            let existing = presets
                .iter_mut()
                .find(|entry| entry.id == preset.id)
                .ok_or_else(|| invalid("The Metin preset no longer exists."))?;
            *existing = preset.clone();
        } else {
            presets.push(preset.clone());
        }
        self.write_metin_presets(presets)?;
        Ok(preset)
    }

    pub fn delete_metin_preset(&mut self, id: Uuid) -> Result<()> {
        let mut presets = self.metin_presets()?;
        let index = presets
            .iter()
            .position(|entry| entry.id == id)
            .ok_or_else(|| invalid("The Metin preset no longer exists."))?;
        presets.remove(index);
        self.write_metin_presets(presets)
    }

    fn write_metin_presets(&self, presets: Vec<MetinPreset>) -> Result<()> {
        let catalog = PresetCatalog {
            schema: CATALOG_SCHEMA,
            presets,
        };
        validate_catalog(&catalog)?;
        let mut bytes = serde_json::to_vec_pretty(&catalog)?;
        bytes.push(b'\n');
        if bytes.len() > MAX_CATALOG_BYTES {
            return Err(invalid("Metin preset catalog exceeds the 1 MiB limit."));
        }
        let path = self.root.join(CATALOG_FILE);
        if present(&path)? {
            reject_link(&path)?;
        }
        atomic_write(&path, &bytes)
    }
}
