//! Crate search across scopes (online + local project/installed), with cancellable in-flight
//! searches and lazy metadata hydration.

mod action;
mod cargo_crate;
mod crate_search_manager;
mod search_options;
mod search_results;

pub use action::*;
pub use cargo_crate::*;
pub use crate_search_manager::*;
pub use search_options::*;
pub use search_results::*;
