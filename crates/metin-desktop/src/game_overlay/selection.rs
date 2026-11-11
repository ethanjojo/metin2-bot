//! Physical-pixel selection interactions shared by rendering and native messages.
use super::*;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum SelectionHit {
    Draw,
    Move,
    Resize([i8; 2]),
}

#[derive(Clone, Copy)]
struct Drag {
    start: [u32; 2],
    original: [u32; 4],
    hit: SelectionHit,
}

pub(super) struct SelectionShape {
    size: [u32; 2],
    pub pointer: [u32; 2],
    rect: Option<[u32; 4]>,
    drag: Option<Drag>,
}

impl SelectionShape {
    pub fn new(size: [u32; 2], rect: Option<[u32; 4]>) -> Self {
        Self {
            size,
            pointer: [0, 0],
            rect,
            drag: None,
        }
    }

    pub fn bounds(&self) -> Option<[u32; 4]> {
        self.rect
    }
    pub fn dragging(&self) -> bool {
        self.drag.is_some()
    }

    pub fn handles(&self) -> Option<[[u32; 2]; 8]> {
        self.rect.map(|[l, t, r, b]| {
            let x = l + (r - l) / 2;
            let y = t + (b - t) / 2;
            [
                [l, t],
                [x, t],
                [r, t],
                [r, y],
                [r, b],
                [x, b],
                [l, b],
                [l, y],
            ]
        })
    }

    pub fn hit(&self, point: [u32; 2]) -> SelectionHit {
        let Some([l, t, r, b]) = self.rect else {
            return SelectionHit::Draw;
        };
        // Keep a center grab point even for exact five-pixel references.
        let radius = ((r - l).min(b - t) / 3).clamp(1, 6);
        let handles = self.handles().expect("rectangle exists");
        for index in [0, 2, 4, 6, 1, 3, 5, 7] {
            let handle = handles[index];
            if point[0].abs_diff(handle[0]) <= radius && point[1].abs_diff(handle[1]) <= radius {
                return SelectionHit::Resize(
                    [
                        [-1, -1],
                        [0, -1],
                        [1, -1],
                        [1, 0],
                        [1, 1],
                        [0, 1],
                        [-1, 1],
                        [-1, 0],
                    ][index],
                );
            }
        }
        if point[0] >= l && point[0] <= r && point[1] >= t && point[1] <= b {
            SelectionHit::Move
        } else {
            SelectionHit::Draw
        }
    }

    pub fn cursor_hit(&self) -> SelectionHit {
        self.drag
            .map(|d| d.hit)
            .unwrap_or_else(|| self.hit(self.pointer))
    }

    pub fn down(&mut self, point: [u32; 2]) {
        self.pointer = self.clamp(point);
        let hit = self.hit(self.pointer);
        let original = if hit == SelectionHit::Draw {
            [
                self.pointer[0],
                self.pointer[1],
                self.pointer[0],
                self.pointer[1],
            ]
        } else {
            self.rect.expect("move/resize requires rectangle")
        };
        self.rect = Some(original);
        self.drag = Some(Drag {
            start: self.pointer,
            original,
            hit,
        });
    }

    fn clamp(&self, point: [u32; 2]) -> [u32; 2] {
        std::array::from_fn(|i| point[i].min(self.size[i].saturating_sub(1)))
    }

    pub fn motion(&mut self, point: [u32; 2]) {
        self.pointer = self.clamp(point);
        let Some(drag) = self.drag else { return };
        let [l, t, r, b] = drag.original;
        let delta: [i64; 2] =
            std::array::from_fn(|i| i64::from(self.pointer[i]) - i64::from(drag.start[i]));
        self.rect = Some(match drag.hit {
            SelectionHit::Draw => [
                drag.start[0].min(self.pointer[0]),
                drag.start[1].min(self.pointer[1]),
                drag.start[0].max(self.pointer[0]),
                drag.start[1].max(self.pointer[1]),
            ],
            SelectionHit::Move => {
                let dx = delta[0].clamp(
                    -i64::from(l),
                    i64::from(self.size[0].saturating_sub(1)) - i64::from(r),
                );
                let dy = delta[1].clamp(
                    -i64::from(t),
                    i64::from(self.size[1].saturating_sub(1)) - i64::from(b),
                );
                [
                    (i64::from(l) + dx) as u32,
                    (i64::from(t) + dy) as u32,
                    (i64::from(r) + dx) as u32,
                    (i64::from(b) + dy) as u32,
                ]
            }
            SelectionHit::Resize(handle) => {
                let mut a = [l, t];
                let mut z = [r, b];
                for i in 0..2 {
                    let bound = i64::from(self.size[i].saturating_sub(1));
                    if handle[i] < 0 {
                        a[i] = (i64::from(a[i]) + delta[i]).clamp(0, bound) as u32;
                    }
                    if handle[i] > 0 {
                        z[i] = (i64::from(z[i]) + delta[i]).clamp(0, bound) as u32;
                    }
                }
                [
                    a[0].min(z[0]),
                    a[1].min(z[1]),
                    a[0].max(z[0]),
                    a[1].max(z[1]),
                ]
            }
        });
    }

    pub fn up(&mut self, point: [u32; 2]) {
        self.motion(point);
        self.drag = None;
    }
}

pub(super) fn color_candidates(
    frame: &Frame,
    profile: &metin_core::Profile,
    point: [u32; 2],
) -> Result<Vec<Candidate>, String> {
    let mut preview = profile.clone();
    let hsv = metin_vision::pick_color(frame, point).map_err(|e| e.to_string())?;
    preview.color = Some(metin_core::ColorConfig {
        hsv,
        ..preview.color.unwrap_or_default()
    });
    metin_vision::detect_candidates(frame, &preview).map_err(|e| e.to_string())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn drawn_region_can_move_and_resize_without_redrawing_it() {
        let mut selection = SelectionShape::new([200, 120], None);
        selection.down([20, 25]);
        selection.motion([80, 75]);
        selection.up([80, 75]);
        assert_eq!(selection.bounds(), Some([20, 25, 80, 75]));
        selection.down([50, 50]);
        selection.motion([65, 60]);
        selection.up([65, 60]);
        assert_eq!(selection.bounds(), Some([35, 35, 95, 85]));
        selection.down([95, 85]);
        selection.motion([130, 100]);
        selection.up([130, 100]);
        assert_eq!(selection.bounds(), Some([35, 35, 130, 100]));
    }

    #[test]
    fn movement_clamps_whole_rectangle_and_preserves_size_at_every_edge() {
        let mut selection = SelectionShape::new([100, 80], Some([20, 20, 60, 60]));
        selection.down([40, 40]);
        selection.up([0, 0]);
        assert_eq!(selection.bounds(), Some([0, 0, 40, 40]));
        selection.down([20, 20]);
        selection.up([99, 79]);
        assert_eq!(selection.bounds(), Some([59, 39, 99, 79]));
    }

    #[test]
    fn tiny_rectangles_move_from_their_center_and_resize_at_actual_edges() {
        let mut selection = SelectionShape::new([800, 600], Some([90, 90, 94, 94]));
        assert_eq!(selection.hit([92, 92]), SelectionHit::Move);
        assert_eq!(selection.hit([94, 94]), SelectionHit::Resize([1, 1]));
        selection.down([92, 92]);
        selection.up([112, 122]);
        assert_eq!(selection.bounds(), Some([110, 120, 114, 124]));
    }

    #[test]
    fn resize_crossing_an_opposite_edge_normalizes_and_drawing_outside_starts_new() {
        let mut selection = SelectionShape::new([200, 120], Some([20, 25, 80, 75]));
        selection.down([20, 25]);
        selection.up([100, 90]);
        assert_eq!(selection.bounds(), Some([80, 75, 100, 90]));
        selection.down([5, 5]);
        selection.up([9, 9]);
        assert_eq!(selection.bounds(), Some([5, 5, 9, 9]));
    }

    #[test]
    fn color_preview_matches_editor_and_runtime_vision_including_masks_and_tolerances() {
        let mut pixels = vec![30u8; 200 * 120 * 3];
        for left in [20, 70, 120] {
            for y in 20..44 {
                for x in left..left + 20 {
                    pixels[(y * 200 + x) * 3..(y * 200 + x) * 3 + 3].copy_from_slice(&[0, 255, 0]);
                }
            }
        }
        let frame = Frame::new(200, 120, pixels).unwrap();
        let mut profile = metin_core::Profile::create("Color preview parity");
        profile.region = Some(Region {
            left: 0.,
            top: 0.,
            right: 1.,
            bottom: 1.,
        });
        profile.masks.push(Region {
            left: 0.34,
            top: 0.1,
            right: 0.46,
            bottom: 0.4,
        });
        profile.color = Some(metin_core::ColorConfig {
            hue_tolerance: 4,
            saturation_tolerance: 20,
            value_tolerance: 20,
            ..Default::default()
        });
        let before = profile.clone();
        let actual = color_candidates(&frame, &profile, [30, 35]).unwrap();
        let mut editor = crate::editor::Editor::new(profile.clone(), None);
        editor.freeze(frame.clone()).unwrap();
        editor.color([30, 35]).unwrap();
        let expected = metin_vision::detect_candidates(&frame, &editor.profile).unwrap();
        assert_eq!(actual, expected);
        assert_eq!(
            actual.len(),
            2,
            "masked green object is excluded while the two others remain"
        );
        assert_eq!(
            profile, before,
            "preview must never change the real profile"
        );
        assert!(color_candidates(&frame, &profile, [200, 120]).is_err());
    }
}
