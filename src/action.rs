use serde::Deserialize;
use strum::Display;

use crate::cargo::CargoAction;
use crate::components::home::HomeAction;
use crate::components::status_bar::StatusAction;

#[derive(Debug, Clone, Display, Deserialize)]
pub enum Action {
    Tick,
    Render,
    Resize { w: u16, h: u16 },
    Suspend,
    Resume,
    Quit,
    ClearScreen,
    Error(String),
    Help,

    Home(HomeAction),
    Status(StatusAction),
    Cargo(CargoAction),

    ToggleSettings,
}
