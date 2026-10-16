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
