use serde::{Deserialize, Serialize};
use strum::Display;

use crate::components::home::enums::{Focusable, Scope, Sort};
use crate::components::status_bar::{StatusDuration, StatusLevel};
use crate::services::crate_search_manager::SearchResults;

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

    Search(SearchAction),
    Cargo(CargoAction),

    OpenReadme,
    RenderReadme(String),
    OpenDocs,

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
    Update(String),
    UpdateAll,
}

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum SettingsAction {
    Save,
}