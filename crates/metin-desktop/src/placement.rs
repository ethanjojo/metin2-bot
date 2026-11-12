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
        positions.push(SavedPanelPosition { key, offset });
    }
    true
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
