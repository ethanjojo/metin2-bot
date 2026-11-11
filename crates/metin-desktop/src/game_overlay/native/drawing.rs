//! Frozen-source rendering; no screen capture or mutable game state is read here.
use super::*;

unsafe fn frozen(dc: HDC, selection: &Selection, pixels: &[u8], clip: Option<RECT>) {
    unsafe {
        let frame = &selection.request.frame;
        let info = BITMAPINFO {
            bmiHeader: BITMAPINFOHEADER {
                biSize: size_of::<BITMAPINFOHEADER>() as u32,
                biWidth: frame.width as i32,
                biHeight: -(frame.height as i32),
                biPlanes: 1,
                biBitCount: 32,
                biCompression: BI_RGB,
                ..zeroed()
            },
            ..zeroed()
        };
        let saved = SaveDC(dc);
        if saved == 0 {
            return;
        }
        if let Some(rect) = clip {
            IntersectClipRect(dc, rect.left, rect.top, rect.right, rect.bottom);
        }
        StretchDIBits(
            dc,
            0,
            0,
            frame.width as i32,
            frame.height as i32,
            0,
            0,
            frame.width as i32,
            frame.height as i32,
            pixels.as_ptr().cast(),
            &info,
            DIB_RGB_COLORS,
            SRCCOPY,
        );
        RestoreDC(dc, saved);
    }
}

unsafe fn clipped_text(dc: HDC, mut rect: RECT, value: &str, color: u32) {
    unsafe {
        let value: Vec<u16> = value.encode_utf16().collect();
        SetTextColor(dc, color);
        SetBkMode(dc, TRANSPARENT as i32);
        DrawTextW(
            dc,
            value.as_ptr(),
            value.len() as i32,
            &mut rect,
            DT_SINGLELINE | DT_END_ELLIPSIS | DT_VCENTER,
        );
    }
}

pub(super) unsafe fn draw_selection(dc: HDC, selection: &Selection) {
    unsafe {
        let frame = &selection.request.frame;
        frozen(dc, selection, &selection.dimmed, None);
        // Restore the primary source pixels first so they cannot erase context
        // outlines. Selection borders/handles are painted above this layer.
        if selection.request.kind == SelectionKind::Region
            && let Some([left, top, right, bottom]) = selection.shape.bounds()
        {
            frozen(
                dc,
                selection,
                &selection.dib,
                Some(RECT {
                    left: left as i32,
                    top: top as i32,
                    right: right as i32 + 1,
                    bottom: bottom as i32 + 1,
                }),
            );
        }
        draw_region_annotations(
            dc,
            &selection.request.context_regions,
            frame.width,
            frame.height,
            false,
        );
        let point = selection.picked();
        let mut details = String::new();
        if selection.request.kind == SelectionKind::Region {
            if let Some([left, top, right, bottom]) = selection.shape.bounds() {
                let rect = RECT {
                    left: left as i32,
                    top: top as i32,
                    right: right as i32 + 1,
                    bottom: bottom as i32 + 1,
                };
                outline(dc, rect, GOLD, 1);
                details = format!("{} x {} px", right - left + 1, bottom - top + 1);
                if !selection.shape.dragging() {
                    for [x, y] in selection.shape.handles().expect("selected region") {
                        let rect = RECT {
                            left: x as i32 - 2,
                            top: y as i32 - 2,
                            right: x as i32 + 3,
                            bottom: y as i32 + 3,
                        };
                        let brush = CreateSolidBrush(WHITE);
                        FillRect(dc, &rect, brush);
                        DeleteObject(brush);
                        outline(dc, rect, 0x00272421, 1);
                    }
                }
            }
        } else {
            let offset = (point[1] as usize * frame.width as usize + point[0] as usize) * 3;
            let bgr = &frame.bgr[offset..offset + 3];
            details = format!("RGB {}, {}, {}", bgr[2], bgr[1], bgr[0]);
            if selection.request.color_preview.is_some() && selection.point.is_some() {
                details = format!("{details} · {} found", selection.candidates.len());
                for candidate in &selection.candidates {
                    let rect = RECT {
                        left: candidate.x as i32,
                        top: candidate.y as i32,
                        right: (candidate.x + candidate.width) as i32,
                        bottom: (candidate.y + candidate.height) as i32,
                    };
                    frozen(dc, selection, &selection.dib, Some(rect));
                    outline(dc, rect, GREEN, 2);
                }
            }
            frozen(
                dc,
                selection,
                &selection.dib,
                Some(RECT {
                    left: point[0] as i32 - 6,
                    top: point[1] as i32 - 6,
                    right: point[0] as i32 + 7,
                    bottom: point[1] as i32 + 7,
                }),
            );
            crosshair(dc, point, GOLD);
        }
        if !selection.controls_visible {
            return;
        }
        if let Some(loupe) =
            loupe_rect(selection).filter(|_| frame.width >= 16 && frame.height >= 16)
        {
            let info = BITMAPINFO {
                bmiHeader: BITMAPINFOHEADER {
                    biSize: size_of::<BITMAPINFOHEADER>() as u32,
                    biWidth: frame.width as i32,
                    biHeight: -(frame.height as i32),
                    biPlanes: 1,
                    biBitCount: 32,
                    biCompression: BI_RGB,
                    ..zeroed()
                },
                ..zeroed()
            };
            let sx = (point[0] as i32 - 8).clamp(0, frame.width as i32 - 16);
            let sy = (point[1] as i32 - 8).clamp(0, frame.height as i32 - 16);
            // GDI addresses the source subrectangle from the DIB's bottom even
            // when a negative height makes its pixel rows top-down.
            let source_y = frame.height as i32 - sy - 16;
            SetStretchBltMode(dc, COLORONCOLOR);
            StretchDIBits(
                dc,
                loupe.left + 1,
                loupe.top + 1,
                96,
                96,
                sx,
                source_y,
                16,
                16,
                selection.dib.as_ptr().cast(),
                &info,
                DIB_RGB_COLORS,
                SRCCOPY,
            );
            outline(dc, loupe, GOLD, 1);
            let px = loupe.left + 1 + (point[0] as i32 - sx) * 6;
            let py = loupe.top + 1 + (point[1] as i32 - sy) * 6;
            outline(
                dc,
                RECT {
                    left: px,
                    top: py,
                    right: px + 6,
                    bottom: py + 6,
                },
                GOLD,
                1,
            );
        }
        if let Some(toolbar) = selection_toolbar(selection) {
            let brush = CreateSolidBrush(0x00272421);
            FillRect(dc, &toolbar, brush);
            DeleteObject(brush);
            outline(dc, toolbar, GOLD, 1);
            clipped_text(
                dc,
                RECT {
                    left: toolbar.left + 9,
                    top: toolbar.top + 3,
                    right: toolbar.right - 34,
                    bottom: toolbar.top + 26,
                },
                &selection.request.label,
                GOLD,
            );
            clipped_text(
                dc,
                RECT {
                    left: toolbar.left + 9,
                    top: toolbar.top + 28,
                    right: toolbar.right - 146,
                    bottom: toolbar.bottom - 4,
                },
                if selection.error.is_empty() {
                    &details
                } else {
                    &selection.error
                },
                WHITE,
            );
            for (right, label, color) in [
                (toolbar.right - 70, "Apply ↵", GOLD),
                (toolbar.right, "Cancel", WHITE),
            ] {
                let rect = RECT {
                    left: right - 68,
                    top: toolbar.top + 28,
                    right: right - 3,
                    bottom: toolbar.bottom - 4,
                };
                outline(dc, rect, color, 1);
                clipped_text(
                    dc,
                    RECT {
                        left: rect.left + 7,
                        ..rect
                    },
                    label,
                    color,
                );
            }
            if selection.request.kind == SelectionKind::Point {
                let offset = (point[1] as usize * frame.width as usize + point[0] as usize) * 3;
                let p = &frame.bgr[offset..offset + 3];
                let swatch = RECT {
                    left: toolbar.right - 28,
                    top: toolbar.top + 5,
                    right: toolbar.right - 8,
                    bottom: toolbar.top + 23,
                };
                let brush = CreateSolidBrush(
                    u32::from(p[2]) | (u32::from(p[1]) << 8) | (u32::from(p[0]) << 16),
                );
                FillRect(dc, &swatch, brush);
                DeleteObject(brush);
                outline(dc, swatch, WHITE, 1);
            }
        } else if !selection.shape.dragging() && selection.shape.pointer[1] + 48 < frame.height {
            let hint = if !selection.error.is_empty() {
                &selection.error
            } else if selection.request.kind == SelectionKind::Region {
                if selection.shape.bounds().is_some() {
                    "Drag inside to move · Edges to resize · Enter applies · Esc cancels"
                } else {
                    "Drag to select · Enter applies · Esc cancels"
                }
            } else {
                "Pick a pixel · Enter applies · Esc cancels"
            };
            clipped_text(
                dc,
                RECT {
                    left: 12,
                    top: frame.height as i32 - 28,
                    right: frame.width as i32 - 12,
                    bottom: frame.height as i32 - 5,
                },
                hint,
                WHITE,
            );
        }
    }
}
