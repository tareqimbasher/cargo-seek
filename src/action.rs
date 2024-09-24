use crate::components::home::{Focusable, SearchResults};
use serde::{Deserialize, Serialize};
use strum::Display;

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
    RenderReadme(String),

    Search(SearchAction),
}

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum SearchAction {
    Search(String, u32),
    Render(SearchResults, u32),
    Clear,
    NavNextPage(u32),
    NavPrevPage(u32),
    NavFirstPage,
    NavLastPage,
    Select(Option<usize>),
    SelectNext,
    SelectPrev,
    SelectFirst,
    SelectLast,
}
