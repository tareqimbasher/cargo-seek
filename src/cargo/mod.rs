//! Wraps the `cargo` CLI and the current cargo environment.
//!
//! `CargoEnv` holds the parsed current project and the installed binaries. This module shells out
//! to the relevant `cargo` subcommands and defines the command/event actions the app runs.

mod api;
mod cargo_env;
mod error;
mod project;

use serde::Deserialize;
use strum::Display;

pub use api::*;
pub use cargo_env::CargoEnv;
pub use error::CargoError;
pub use project::*;

/// A cargo command to execute.
#[derive(Debug, Clone, Display, Deserialize)]
pub enum CargoCommand {
    Add {
        name: String,
        version: String,
        /// Features to enable. Empty means none beyond the defaults.
        features: Vec<String>,
        /// Pass `--no-default-features` (set when the user unchecks a default feature).
        no_default_features: bool,
    },
    Remove(String),
    // Update(String),
    // UpdateAll,
    Install {
        name: String,
        version: String,
        /// Features to enable. Empty means none beyond the defaults.
        features: Vec<String>,
        /// Pass `--no-default-features` (set when the user unchecked a default feature).
        no_default_features: bool,
    },
    Uninstall(String),
    /// Re-read the cargo environment.
    Refresh,
}

/// A cargo-environment event.
#[derive(Debug, Clone, Display)]
pub enum CargoEvent {
    /// The cargo environment finished refreshing.
    Refreshed,
}
