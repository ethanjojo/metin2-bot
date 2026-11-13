//! Presentation-only panel placement. Coordinates are relative to a game client.
use metin_platform::{TargetInfo, TargetToken};
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PanelOffset {
    /// 96-DPI units measured from the client's right and top edges.
    pub right: i32,
    pub top: i32,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WindowPlacementKey {
    pub process: String,
    pub title: String,
    pub instance: u16,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SavedPanelPosition {
    pub key: WindowPlacementKey,
    pub offset: PanelOffset,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expanded_size: Option<PanelSize>,
}

/// Preferred expanded extent in 96-DPI units, independent of current monitor clamps.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct PanelSize {
    pub width: u32,
    pub height: u32,
}
impl Default for PanelSize {
    fn default() -> Self {
        Self {
            width: 460,
            height: 620,
        }
    }
}

impl PanelSize {
    pub const MINIMUM: Self = Self {
        width: 400,
        height: 480,
    };

    pub fn ui_points(self, dpi: u32, pixels_per_point: f32) -> [f32; 2] {
        self.physical(dpi)
            .map(|pixels| pixels as f32 / pixels_per_point)
    }

    pub fn restored_physical(self, dpi: u32, work: [i32; 4]) -> [u32; 2] {
        let preferred = self.physical(dpi);
        let minimum = Self::MINIMUM.physical(dpi);
        clamp_size(std::array::from_fn(|i| preferred[i].max(minimum[i])), work)
    }

    pub fn from_physical(size: [u32; 2], dpi: u32) -> Self {
        let logical = |v: u32| (f64::from(v) * 96. / f64::from(dpi.max(1))).round() as u32;
        Self {
            width: logical(size[0]),
            height: logical(size[1]),
        }
    }
    pub fn physical(self, dpi: u32) -> [u32; 2] {
        [self.width, self.height]
            .map(|v| (f64::from(v) * f64::from(dpi.max(1)) / 96.).round() as u32)
    }
    pub fn valid(self) -> bool {
        (64..=100_000).contains(&self.width) && (64..=100_000).contains(&self.height)
    }
}

impl Default for PanelOffset {
    fn default() -> Self {
        Self { right: 12, top: 12 }
    }
}

pub fn placement_key(target: TargetToken, targets: &[TargetInfo]) -> Option<WindowPlacementKey> {
    let info = targets.iter().find(|info| info.token == target)?;
    let process = info.process_name.to_ascii_lowercase();
    let mut peers = targets
        .iter()
        .filter(|other| {
            other.process_name.eq_ignore_ascii_case(&info.process_name) && other.title == info.title
        })
        .map(|other| other.token)
        .collect::<Vec<_>>();
    peers.sort_by_key(|token| (token.created, token.pid, token.handle));
    Some(WindowPlacementKey {
        process,
        title: info.title.clone(),
        instance: u16::try_from(peers.iter().position(|token| *token == target)?).ok()?,
    })
}

pub fn valid_positions(positions: &[SavedPanelPosition]) -> bool {
    positions.len() <= 64
        && positions.iter().enumerate().all(|(index, item)| {
            !item.key.process.is_empty()
                && item.key.process.len() <= 4096
                && item.key.title.len() <= 4096
                && item.offset.right.unsigned_abs() <= 100_000
                && item.offset.top.unsigned_abs() <= 100_000
                && item.expanded_size.is_none_or(PanelSize::valid)
                && !positions[..index].iter().any(|other| other.key == item.key)
        })
}

pub fn remember_position(
    positions: &mut Vec<SavedPanelPosition>,
    key: WindowPlacementKey,
    offset: PanelOffset,
) -> bool {
    if let Some(existing) = positions.iter_mut().find(|item| item.key == key) {
        if existing.offset == offset {
            return false;
        }
        existing.offset = offset;
    } else {
        if positions.len() >= 64 {
            positions.remove(0);
        }
        positions.push(SavedPanelPosition {
            key,
            offset,
            expanded_size: None,
        });
    }
    true
}

/// A target without a saved preference starts from the canonical expanded extent,
/// never a preceding target, chooser, collapsed strip or asynchronous native surface.
pub fn preferred_size(
    positions: &[SavedPanelPosition],
    key: Option<&WindowPlacementKey>,
) -> PanelSize {
    key.and_then(|key| positions.iter().find(|position| &position.key == key))
        .and_then(|position| position.expanded_size)
        .unwrap_or_default()
}

pub fn remember_size(
    positions: &mut Vec<SavedPanelPosition>,
    key: WindowPlacementKey,
    size: PanelSize,
) -> bool {
    if !size.valid() {
        return false;
    }
    if !positions.iter().any(|position| position.key == key) {
        remember_position(positions, key.clone(), PanelOffset::default());
    }
    let position = positions
        .iter_mut()
        .find(|position| position.key == key)
        .expect("inserted placement");
    if position.expanded_size == Some(size) {
        return false;
    }
    position.expanded_size = Some(size);
    true
}

/// OS work-area limits affect only the current surface, never the stored preference.
pub fn clamp_size(size: [u32; 2], work: [i32; 4]) -> [u32; 2] {
    let available = [
        i64::from(work[2]) - i64::from(work[0]),
        i64::from(work[3]) - i64::from(work[1]),
    ];
    std::array::from_fn(|i| size[i].min(available[i].clamp(1, i64::from(u32::MAX)) as u32))
}

pub fn panel_offset(target: [i32; 4], panel: [i32; 4], dpi: u32) -> PanelOffset {
    let logical = |value: i64| {
        ((value as f64 * 96. / f64::from(dpi.max(1))).round() as i64).clamp(-100_000, 100_000)
            as i32
    };
    PanelOffset {
        right: logical(i64::from(target[2]) - i64::from(panel[2])),
        top: logical(i64::from(panel[1]) - i64::from(target[1])),
    }
}

pub fn place_panel(
    target: [i32; 4],
    size: [u32; 2],
    offset: PanelOffset,
    dpi: u32,
    work: [i32; 4],
) -> [i32; 2] {
    let physical = |value: i32| (f64::from(value) * f64::from(dpi.max(1)) / 96.).round() as i64;
    let x = i64::from(target[2]) - i64::from(size[0]) - physical(offset.right);
    let y = i64::from(target[1]) + physical(offset.top);
    let clamp = |value: i64, low: i32, high: i32, length: u32| {
        value.clamp(
            i64::from(low),
            (i64::from(high) - i64::from(length)).max(i64::from(low)),
        ) as i32
    };
    [
        clamp(x, work[0], work[2], size[0]),
        clamp(y, work[1], work[3], size[1]),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn new_target_uses_default_extent_instead_of_previous_target_or_chooser_surface() {
        let a = WindowPlacementKey {
            process: "game.exe".into(),
            title: "A".into(),
            instance: 0,
        };
        let b = WindowPlacementKey {
            title: "B".into(),
            ..a.clone()
        };
        let mut positions = Vec::new();
        remember_size(
            &mut positions,
            a.clone(),
            PanelSize {
                width: 700,
                height: 800,
            },
        );
        assert_eq!(
            preferred_size(&positions, Some(&a)),
            PanelSize {
                width: 700,
                height: 800
            }
        );
        // These transient surfaces are deliberately absent from the preference API.
        for current_surface in [[780, 600], [700, 800], [460, 42]] {
            assert_ne!(
                preferred_size(&positions, Some(&b)).physical(96),
                current_surface
            );
            assert_eq!(preferred_size(&positions, Some(&b)), PanelSize::default());
        }
        remember_size(
            &mut positions,
            b.clone(),
            PanelSize {
                width: 540,
                height: 700,
            },
        );
        assert_eq!(
            preferred_size(&positions, Some(&b)),
            PanelSize {
                width: 540,
                height: 700
            }
        );
        assert_eq!(preferred_size(&positions, None), PanelSize::default());
    }

    #[test]
    fn expanded_minimum_is_dip_based_instead_of_growing_to_the_saved_size_at_ui_zoom() {
        let preferred = PanelSize {
            width: 540,
            height: 700,
        };
        for dpi in [96, 144] {
            let ppp = dpi as f32 / 96. * 1.5;
            let [w, h] = PanelSize::MINIMUM.ui_points(dpi, ppp);
            assert!((w * ppp - 400. * dpi as f32 / 96.).abs() < 0.01);
            assert!((h * ppp - 480. * dpi as f32 / 96.).abs() < 0.01);
            let requested = preferred.restored_physical(dpi, [0, 0, 2560, 1600]);
            assert!(
                w * ppp < requested[0] as f32 && h * ppp < requested[1] as f32,
                "Saved540x700 must remain resizable down to400x480 DIP at150% UI zoom"
            );
        }
    }

    #[test]
    fn size_is_dpi_independent_and_clamping_never_changes_preference() {
        let size = PanelSize::from_physical([810, 1050], 144);
        assert_eq!(
            size,
            PanelSize {
                width: 540,
                height: 700
            }
        );
        assert_eq!(size.physical(192), [1080, 1400]);
        assert_eq!(
            clamp_size(size.physical(192), [-800, 0, 0, 1000]),
            [800, 1000]
        );
        assert_eq!(size.physical(96), [540, 700]);
        let key = WindowPlacementKey {
            process: "game.exe".into(),
            title: "A".into(),
            instance: 0,
        };
        let mut positions = Vec::new();
        assert!(remember_size(&mut positions, key.clone(), size));
        remember_position(
            &mut positions,
            key.clone(),
            PanelOffset { right: 24, top: 48 },
        );
        assert_eq!(positions[0].expanded_size, Some(size));
        assert!(!remember_size(&mut positions, key, size));
        assert!(valid_positions(&positions));
        positions[0].expanded_size = Some(PanelSize {
            width: 0,
            height: 700,
        });
        assert!(!valid_positions(&positions));
    }

    #[test]
    fn expanded_size_roundtrips_with_position_and_old_positions_stay_valid() {
        let old = r#"{"key":{"process":"game.exe","title":"A","instance":0},"offset":{"right":12,"top":12}}"#;
        let old: SavedPanelPosition = serde_json::from_str(old).unwrap();
        let mut value = serde_json::to_value(&old).unwrap();
        value["expanded_size"] = serde_json::json!({"width":540,"height":700});
        let saved: SavedPanelPosition = serde_json::from_value(value.clone()).unwrap();
        assert_eq!(serde_json::to_value(saved).unwrap(), value);
    }

    #[test]
    fn positions_survive_settings_roundtrip_and_do_not_mix_clients() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("ui-settings.json"), b"{}").unwrap();
        let mut settings = crate::storage::UISettings::load(dir.path()).unwrap();
        assert!(settings.panel_positions.is_empty());
        let first = WindowPlacementKey {
            process: "game.exe".into(),
            title: "First character".into(),
            instance: 0,
        };
        let second = WindowPlacementKey {
            title: "Second character".into(),
            ..first.clone()
        };
        assert!(remember_position(
            &mut settings.panel_positions,
            first.clone(),
            PanelOffset { right: 40, top: 70 }
        ));
        assert!(!remember_position(
            &mut settings.panel_positions,
            first.clone(),
            PanelOffset { right: 40, top: 70 }
        ));
        remember_position(
            &mut settings.panel_positions,
            second,
            PanelOffset {
                right: 400,
                top: 170,
            },
        );
        remember_size(
            &mut settings.panel_positions,
            first,
            PanelSize {
                width: 540,
                height: 700,
            },
        );
        settings.save(dir.path()).unwrap();
        let saved = crate::storage::UISettings::load(dir.path()).unwrap();
        assert_eq!(saved.panel_positions, settings.panel_positions);
        let mut invalid = saved.clone();
        invalid
            .panel_positions
            .push(invalid.panel_positions[0].clone());
        assert!(invalid.save(dir.path()).is_err());
        assert_eq!(
            crate::storage::UISettings::load(dir.path())
                .unwrap()
                .panel_positions,
            settings.panel_positions
        );
    }

    #[test]
    fn panel_positions_keep_negative_origin_and_clamp_disconnected_monitors() {
        let offset = PanelOffset { right: 40, top: 80 };
        assert_eq!(
            place_panel(
                [-1920, 0, 0, 1080],
                [460, 620],
                offset,
                96,
                [-1920, 0, 0, 1040]
            ),
            [-500, 80]
        );
        assert_eq!(
            place_panel(
                [0, 0, 800, 600],
                [460, 620],
                PanelOffset {
                    right: -9000,
                    top: 9000
                },
                96,
                [0, 0, 1920, 1040]
            ),
            [1460, 420]
        );
    }

    #[test]
    fn panel_offset_roundtrips_across_dpi_without_height_dependent_collapse_jump() {
        let offset = PanelOffset { right: 24, top: 32 };
        let pos = place_panel(
            [100, 200, 2100, 1400],
            [600, 800],
            offset,
            144,
            [0, 0, 2560, 1600],
        );
        assert_eq!(pos, [1464, 248]);
        assert_eq!(
            panel_offset([100, 200, 2100, 1400], [1464, 248, 2064, 1048], 144),
            offset
        );
        assert_eq!(
            place_panel(
                [100, 200, 2100, 1400],
                [600, 42],
                offset,
                144,
                [0, 0, 2560, 1600]
            ),
            pos
        );
    }

    #[test]
    fn distinct_live_clients_get_distinct_persistent_keys_without_storing_handles() {
        let a = TargetInfo {
            token: TargetToken {
                handle: 1,
                pid: 2,
                created: 3,
            },
            title: "Character".into(),
            process_name: "GAME.EXE".into(),
        };
        let mut b = a.clone();
        b.token = TargetToken {
            handle: 4,
            pid: 5,
            created: 6,
        };
        let list = vec![b.clone(), a.clone()];
        assert_ne!(placement_key(a.token, &list), placement_key(b.token, &list));
        assert_eq!(placement_key(a.token, &list).unwrap().process, "game.exe");
        assert!(
            placement_key(
                TargetToken {
                    handle: 99,
                    pid: 99,
                    created: 99
                },
                &list
            )
            .is_none()
        );
    }
}
