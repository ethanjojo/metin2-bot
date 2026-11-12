//! Verified update handoff and the temporary, independent progress window.

mod process;
mod progress;
mod security;
mod session;
mod window;

pub use progress::{InstallPhase, InstallProgress};
pub use session::PreparedUpdate;
pub use window::run;
#[cfg(feature = "ui-smoke")]
pub use window::status_preview;
