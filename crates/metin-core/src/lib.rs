//! Platform-independent, deterministic MetinBot domain logic.
mod buffs;
mod controller;
mod geometry;
mod profile;
mod session;
mod shortcut;
pub mod validation;
pub use buffs::*;
pub use controller::*;
pub use geometry::Region;
pub use profile::*;
pub use session::*;
pub use shortcut::{Key, Shortcut};

pub type Result<T> = std::result::Result<T, ValidationError>;

#[derive(Debug, Clone, PartialEq, thiserror::Error)]
#[error("{0}")]
pub struct ValidationError(pub String);

pub(crate) fn require(valid: bool, message: &str) -> Result<()> {
    if valid {
        Ok(())
    } else {
        Err(ValidationError(message.into()))
    }
}
