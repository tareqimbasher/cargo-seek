use serde::{Deserialize, Serialize};
use strum::Display;

use crate::components::home::enums::Focusable;
use crate::components::status_bar::{StatusDuration, StatusLevel};
use crate::search::{Scope, SearchResults, Sort};

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum Action {
    Tick,
    Render,
    Resize(u16, u16),
    Suspend,
    Resume,
    Quit,
    ClearScreen,
    Error(String),
    Help,

    ToggleUsage,
    Focus(Focusable),
    FocusNext,
    FocusPrevious,

    UpdateStatus(StatusLevel, String),
    UpdateStatusWithDuration(StatusLevel, StatusDuration, String),

    RefreshCargoEnv,
    CargoEnvRefreshed,

    Search(SearchAction),
    Cargo(CargoAction),

    OpenDocs,
    OpenReadme,
    RenderReadme(String),
    OpenCratesIo,
    OpenLibRs,

    ToggleSettings,
    Settings(SettingsAction),
}

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum SearchAction {
    Clear,
    Search(String, Sort, usize, Option<String>),
    Error(String),
    SortBy(Sort),
    Scope(Scope),
    Render(SearchResults),

    NavPagesForward(usize),
    NavPagesBack(usize),
    NavFirstPage,
    NavLastPage,
    SelectIndex(Option<usize>),
    SelectNext,
    SelectPrev,
    SelectFirst,
    SelectLast,
}

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum CargoAction {
    Add(String, String),
    Remove(String),
    // Update(String),
    // UpdateAll,
    Install(String, String),
    Uninstall(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum SettingsAction {
    Save,
}
