use clap::Parser;
use std::path::PathBuf;
#[derive(Debug, Parser)]
#[command(
    name = "metinbot",
    version=crate::VERSION,
    about = "Local color and target-panel assistant"
)]
pub struct Cli {
    #[arg(long, conflicts_with = "self_test")]
    pub data_dir: Option<PathBuf>,
    #[arg(long, conflicts_with = "self_test")]
    pub import_data: Option<PathBuf>,
    #[arg(long, conflicts_with = "self_test")]
    pub log: Option<PathBuf>,
    #[arg(long, requires = "self_test_report")]
    pub self_test: bool,
    #[arg(long, requires = "self_test")]
    pub self_test_report: Option<PathBuf>,
    #[cfg(feature = "ui-smoke")]
    #[arg(long, conflicts_with = "self_test", requires = "data_dir")]
    pub ui_fixture: Option<PathBuf>,
    #[cfg(feature = "ui-smoke")]
    #[arg(long, conflicts_with = "self_test", requires = "data_dir")]
    pub ui_screenshot: Option<PathBuf>,
    #[cfg(feature = "ui-smoke")]
    #[arg(long, requires = "ui_screenshot")]
    pub ui_tab: Option<crate::ui::Tab>,
    #[cfg(feature = "ui-smoke")]
    #[arg(long, requires = "ui_screenshot")]
    pub ui_small: bool,
    #[cfg(feature = "ui-smoke")]
    #[arg(long,requires_all=["ui_screenshot","ui_fixture"])]
    pub ui_window_picker: Option<crate::ui::WindowPickerFixture>,
    #[cfg(feature = "ui-smoke")]
    #[arg(long,requires_all=["ui_screenshot","ui_fixture"])]
    pub ui_buff_capture: Option<crate::ui::BuffCaptureFixture>,
    #[cfg(feature = "ui-smoke")]
    #[arg(long, requires = "ui_screenshot")]
    pub ui_sizes: bool,
    #[cfg(feature = "ui-smoke")]
    #[arg(long, requires = "ui_screenshot")]
    pub ui_actions: Option<crate::ui::ActionsPage>,
    #[cfg(feature = "ui-smoke")]
    #[arg(long, requires = "ui_screenshot")]
    pub ui_settings: Option<crate::ui::SettingsPage>,
    #[cfg(feature = "ui-smoke")]
    #[arg(long, requires = "ui_screenshot")]
    pub ui_safety: Option<crate::ui::SafetyPage>,
    #[cfg(feature = "ui-smoke")]
    #[arg(long,requires_all=["ui_screenshot","ui_fixture"])]
    pub ui_safety_capture: bool,
    #[cfg(feature = "ui-smoke")]
    #[arg(long,requires_all=["ui_screenshot","ui_fixture"])]
    pub ui_run_state: Option<crate::ui::RunFixture>,
}
