use serde::{Deserialize, Serialize};
use strum::Display;

use crate::components::home::types::SearchResults;
use crate::components::home::Focusable;

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

    Focus(Focusable),
    FocusNext,
    FocusPrevious,
    ToggleUsage,

    Search(SearchAction),

    OpenReadme,
    RenderReadme(String),
    OpenDocs,
}

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum SearchAction {
    Search(String, u32),
    Render(SearchResults),
    Clear,

    NavNextPage(u32),
    NavPrevPage(u32),
    NavFirstPage,
    NavLastPage,
    SelectIndex(Option<usize>),
    SelectNext,
    SelectPrev,
    SelectFirst,
    SelectLast,
}
