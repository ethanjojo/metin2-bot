//! Companion desktop composition root.
pub mod cli;
pub mod editor;
mod file_tree;
pub mod game_overlay;
pub mod installation;
pub mod runtime;
pub mod self_test;
pub mod storage;
pub mod ui;
pub mod ui_controls;
pub mod updates;
pub const VERSION: &str = env!("COMPANION_VERSION");
pub const PRODUCT_NAME: &str = env!("COMPANION_PRODUCT_NAME");
pub const APP_ID: &str = env!("COMPANION_APP_ID");
pub fn app_icon_png() -> &'static [u8] {
    include_bytes!("../../../packaging/icons/companion-256.png")
}
pub fn app_icon() -> Result<eframe::egui::IconData, String> {
    eframe::icon_data::from_png_bytes(app_icon_png()).map_err(|e| e.to_string())
}
pub fn build_info() -> serde_json::Value {
    serde_json::json!({"publisher":env!("COMPANION_PUBLISHER"),"product":PRODUCT_NAME,"version":VERSION,"commit_sha":env!("COMPANION_COMMIT"),"working_tree_dirty":env!("COMPANION_DIRTY")=="true","target":env!("COMPANION_TARGET"),"runtime":"rust","gui":"egui"})
}

pub mod notifications;
