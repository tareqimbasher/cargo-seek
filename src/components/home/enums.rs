use enum_iterator::{all, reverse_all, Sequence};
use serde::{Deserialize, Serialize};
use std::iter::Cycle;

use crate::search::Sort;

#[derive(Default, PartialEq, Clone, Debug, Eq, Sequence, Serialize, Deserialize)]
pub enum Focusable {
    Usage,
    #[default]
    Search,
    Sort,
    Scope,
    Results,
    DocsButton,
    ReadmeButton,
    CratesIoButton,
    LibRsButton,
}

impl Focusable {
    pub fn next(&self) -> Focusable {
        let mut variants: Cycle<_> = all::<Focusable>().cycle();
        variants.find(|v| v == self);
        variants.next().unwrap()
    }

    pub fn prev(&self) -> Focusable {
        let mut variants: Cycle<_> = reverse_all::<Focusable>().cycle();
        variants.find(|v| v == self);
        variants.next().unwrap()
    }
}

impl std::fmt::Display for Sort {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let output = match self {
            Sort::Relevance => "Relevance",
            Sort::Name => "Name",
            Sort::Downloads => "Downloads",
            Sort::RecentDownloads => "Recent Downloads",
            Sort::RecentlyUpdated => "Recently Updated",
            Sort::NewlyAdded => "Newly Added",
        };
        write!(f, "{}", output)
    }
}

pub fn is_results_or_details_focused(focused: &Focusable) -> bool {
    *focused == Focusable::Results
        || *focused == Focusable::DocsButton
        || *focused == Focusable::ReadmeButton
        || *focused == Focusable::CratesIoButton
        || *focused == Focusable::LibRsButton
}
