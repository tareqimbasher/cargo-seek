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
