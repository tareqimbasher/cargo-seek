//! The `Action` enum — the single message type that flows over the app's channel.
//!
//! State changes propagate only by sending `Action`s; a handler may return a follow-up `Action`
//! that the loop re-queues. Variants wrap the per-area command/event enums.

use serde::Deserialize;
use strum::Display;

use crate::cargo::{CargoCommand, CargoEvent};
use crate::components::home::HomeCommand;
use crate::components::status_bar::StatusCommand;
use crate::search::{SearchCommand, SearchEvent};

#[derive(Debug, Clone, Display, Deserialize)]
pub enum Action {
    // Lifecycle
    Tick,
    Render,
    Resize { w: u16, h: u16 },
    Suspend,
    Resume,
    Quit,
    ClearScreen,
    Error(String),

    // Commands
    ToggleSettings,
    Home(HomeCommand),
    Search(SearchCommand),
    Cargo(CargoCommand),
    Status(StatusCommand),

    // Events
    SearchEvent(SearchEvent),
    CargoEvent(CargoEvent),
}
