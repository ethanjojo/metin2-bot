//! MetinBot desktop composition root.
pub mod cli;
#[cfg(target_os = "macos")]
pub mod dock_icon;
pub mod editor;
pub mod runtime;
pub mod self_test;
pub mod storage;
pub mod ui;
pub mod ui_controls;
pub const VERSION: &str = env!("METINBOT_VERSION");
pub const APP_ID: &str = env!("METINBOT_APP_ID");
pub fn app_icon_png(dark: bool) -> &'static [u8] {
    if dark {
        include_bytes!("../../../packaging/icons/metinbot-dark-256.png")
    } else {
        include_bytes!("../../../packaging/icons/metinbot-light-256.png")
    }
}
pub fn app_icon(dark: bool) -> Result<eframe::egui::IconData, String> {
    eframe::icon_data::from_png_bytes(app_icon_png(dark)).map_err(|e| e.to_string())
}
pub fn build_info() -> serde_json::Value {
    serde_json::json!({"publisher":env!("METINBOT_PUBLISHER"),"product":env!("METINBOT_PRODUCT_NAME"),"version":VERSION,"commit_sha":env!("METINBOT_COMMIT"),"working_tree_dirty":env!("METINBOT_DIRTY")=="true","target":env!("METINBOT_TARGET"),"runtime":"rust","gui":"egui"})
}

// AppKit may return associated-alpha samples even for unassociated PNG sources.
#[cfg(any(test, all(target_os = "macos", feature = "ui-smoke")))]
fn premultiplied_rgba(mut pixel: [usize; 4], unassociated: bool) -> [usize; 4] {
    if unassociated {
        let alpha = pixel[3];
        for value in &mut pixel[..3] {
            *value = (*value * alpha + 127) / 255;
        }
    }
    pixel
}

#[cfg(test)]
mod icon_tests {
    use super::premultiplied_rgba;

    #[test]
    fn appkit_associated_alpha_matches_png_without_confusing_opposite_icons() {
        let png_white = premultiplied_rgba([255, 255, 255, 5], true);
        let dock_white = premultiplied_rgba([5, 5, 5, 5], false);
        assert_eq!(png_white, dock_white);
        assert_ne!(png_white, premultiplied_rgba([0, 0, 0, 5], false));
        assert_eq!(premultiplied_rgba([255, 255, 255, 0], true), [0; 4]);
        assert_eq!(premultiplied_rgba([255, 255, 255, 255], true), [255; 4]);
    }
}
