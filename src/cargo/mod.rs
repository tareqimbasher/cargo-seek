mod api;
mod cargo_env;
mod project;

use serde::Deserialize;
use strum_macros::Display;

pub use api::*;
pub use cargo_env::CargoEnv;
pub use project::*;

#[derive(Debug, Clone, Display, Deserialize)]
pub enum CargoAction {
    Add { name: String, version: String },
    Remove(String),
    // Update(String),
    // UpdateAll,
    Install { name: String, version: String },
    Uninstall(String),

    RefreshCargoEnv,
    CargoEnvRefreshed,
    CrateMetadataLoaded(Box<crates_io_api::CrateResponse>),
}
