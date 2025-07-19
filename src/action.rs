use crate::components::{Focusable, StatusDuration, StatusLevel};
use crate::search::{Scope, SearchResults, Sort};
use serde::Deserialize;
use strum::Display;

#[derive(Debug, Clone, Display, Deserialize)]
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

    Search(SearchAction),
    Cargo(CargoAction),

    RefreshCargoEnv,
    CargoEnvRefreshed,
    CrateMetadataLoaded(Box<crates_io_api::Crate>),

    OpenDocs,
    OpenReadme,
    RenderReadme(String),
    OpenCratesIo,
    OpenLibRs,

    ToggleSettings,
    Settings(SettingsAction),
}

#[derive(Debug, Clone, PartialEq, Eq, Display, Deserialize)]
pub enum SearchAction {
    Clear,
    Search(String, usize, bool, Option<String>),
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

#[derive(Debug, Clone, PartialEq, Eq, Display, Deserialize)]
pub enum CargoAction {
    Add(String, String),
    Remove(String),
    // Update(String),
    // UpdateAll,
    Install(String, String),
    Uninstall(String),
}

#[derive(Debug, Clone, PartialEq, Eq, Display, Deserialize)]
pub enum SettingsAction {
    Save,
}
