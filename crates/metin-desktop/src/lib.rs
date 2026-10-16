//! MetinBot desktop composition root.
pub mod cli;
pub mod editor;
pub mod runtime;
pub mod self_test;
pub mod storage;
pub mod ui;
pub mod ui_controls;
pub const VERSION: &str = env!("METINBOT_VERSION");
pub fn build_info() -> serde_json::Value {
    serde_json::json!({"version":VERSION,"commit_sha":env!("METINBOT_COMMIT"),"working_tree_dirty":env!("METINBOT_DIRTY")=="true","target":env!("METINBOT_TARGET"),"runtime":"rust","gui":"egui"})
}
