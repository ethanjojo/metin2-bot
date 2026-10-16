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
}
