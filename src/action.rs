use serde::{Deserialize, Serialize};
use strum::Display;

use crate::components::home::scope_dropdown::Scope;
use crate::components::home::sort_dropdown::Sort;
use crate::components::home::types::SearchResults;
use crate::components::home::Focusable;
use crate::components::status_bar::{StatusDuration, StatusLevel};
use crate::project::Project;

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

    ReadProject,
    HandleReadProjectCompleted(Project),

    Search(SearchAction),
    Cargo(CargoAction),

    OpenReadme,
    RenderReadme(String),
    OpenDocs,
}

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum SearchAction {
    Clear,
    Search(String, Sort, usize, Option<String>),
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