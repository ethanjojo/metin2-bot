use clap::Parser;
use std::path::PathBuf;
#[derive(Debug, Clone, Copy, clap::ValueEnum)]
pub enum InputDriverCommand {
    Status,
    Install,
    Preview,
    Remove,
}
impl InputDriverCommand {
    pub fn operation(self) -> u32 {
        match self {
            Self::Status => 0,
            Self::Install => 1,
            Self::Preview => 2,
            Self::Remove => 3,
        }
    }
    pub fn name(self) -> &'static str {
        match self {
            Self::Status => "status",
            Self::Install => "install",
            Self::Preview => "preview",
            Self::Remove => "remove",
        }
    }
}
#[derive(Debug, Parser)]
#[command(
    name = crate::PRODUCT_NAME,
    bin_name = "companion",
    version=crate::VERSION,
    about = "Local color and target-panel assistant"
    ,group(clap::ArgGroup::new("migration").args(["finish_upgrade", "migrate_data"]))
)]
pub struct Cli {
    #[arg(long, requires_all = ["migration_report", "previous_inventory"], conflicts_with_all = ["data_dir", "input_driver", "self_test", "import_data", "log"])]
    pub finish_upgrade: Option<PathBuf>,
    #[arg(long, requires = "finish_upgrade", conflicts_with = "migrate_data")]
    pub previous_inventory: Option<PathBuf>,
    #[arg(long, requires = "migration_report", conflicts_with_all = ["input_driver", "self_test", "import_data", "log"])]
    pub migrate_data: bool,
    #[arg(long, requires = "migration")]
    pub migration_report: Option<PathBuf>,
    #[arg(long, value_enum, requires = "input_driver_report", conflicts_with_all = ["data_dir", "import_data", "log", "self_test"])]
    pub input_driver: Option<InputDriverCommand>,
    #[arg(long, requires = "input_driver")]
    pub input_driver_report: Option<PathBuf>,
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
    #[arg(long, requires = "ui_screenshot")]
    pub ui_icon_sheet: bool,
    #[cfg(feature = "ui-smoke")]
    #[arg(long, requires = "ui_screenshot")]
    pub ui_profile_menu: bool,
    #[cfg(feature = "ui-smoke")]
    #[arg(long,requires_all=["ui_screenshot","ui_fixture"])]
    pub ui_window_picker: Option<crate::ui::WindowPickerFixture>,
    #[cfg(feature = "ui-smoke")]
    #[arg(long,requires_all=["ui_screenshot","ui_fixture"])]
    pub ui_buff_capture: Option<crate::ui::BuffCaptureFixture>,
    #[cfg(feature = "ui-smoke")]
    #[arg(long,requires_all=["ui_screenshot","ui_fixture"],conflicts_with="ui_buff_capture")]
    pub ui_buff_detail: bool,
    #[cfg(feature = "ui-smoke")]
    #[arg(long, requires = "ui_buff_detail")]
    pub ui_buff_icon_picker: bool,
    #[cfg(feature = "ui-smoke")]
    #[arg(long,requires_all=["ui_screenshot","ui_fixture"])]
    pub ui_size_review: bool,
    #[cfg(feature = "ui-smoke")]
    #[arg(long, requires = "ui_screenshot")]
    pub ui_sizes: bool,
    #[cfg(feature = "ui-smoke")]
    #[arg(long, num_args = 1, requires = "ui_screenshot")]
    pub ui_overlay_controls: Option<bool>,
    #[cfg(feature = "ui-smoke")]
    #[arg(long, requires = "ui_screenshot")]
    pub ui_actions: Option<crate::ui::ActionsPage>,
    #[cfg(feature = "ui-smoke")]
    #[arg(long, requires = "ui_screenshot")]
    pub ui_settings: Option<crate::ui::SettingsPage>,
    #[cfg(feature = "ui-smoke")]
    #[arg(long, requires = "ui_screenshot", value_parser=["idle","available","downloading","ready","failed"])]
    pub ui_updates: Option<String>,
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
