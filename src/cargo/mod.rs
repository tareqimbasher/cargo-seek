mod api;
mod cargo_env;
mod project;

use serde::Deserialize;
use strum_macros::Display;

pub use api::*;
pub use cargo_env::CargoEnv;
pub use project::*;

/// A cargo command to execute (handled by the app event loop).
#[derive(Debug, Clone, Display, Deserialize)]
pub enum CargoCommand {
    Add {
        name: String,
        version: String,
    },
    Remove(String),
    // Update(String),
    // UpdateAll,
    Install {
        name: String,
        version: String,
    },
    Uninstall(String),
    /// Re-read the cargo environment (installed binaries + current project).
    Refresh,
}

/// A cargo-environment event (handled by the components that display it).
#[derive(Debug, Clone, Display, Deserialize)]
pub enum CargoEvent {
    /// The cargo environment finished refreshing.
    Refreshed,
}
