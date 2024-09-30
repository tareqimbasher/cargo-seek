use serde::{Deserialize, Serialize};
use strum::Display;

use crate::components::home::search_sort_dropdown::Sort;
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

    ToggleUsage,
    Focus(Focusable),
    FocusNext,
    FocusPrevious,

    Search(SearchAction),

    OpenReadme,
    RenderReadme(String),
    OpenDocs,
}

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum SearchAction {
    Clear,
    Search(String, Sort, usize),
    SortBy(Sort),
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
