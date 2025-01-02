use enum_iterator::{all, reverse_all, Sequence};
use serde::{Deserialize, Serialize};
use std::iter::Cycle;
use strum::Display;
use strum_macros::EnumIter;

#[derive(Default, PartialEq, Clone, Debug, Eq, Sequence, Serialize, Deserialize)]
pub enum Focusable {
    #[default]
    Search,
    Sort,
    Scope,
    Results,
    AddButton,
    InstallButton,
    ReadmeButton,
    DocsButton,
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

#[derive(
    Debug, Default, Display, Clone, EnumIter, PartialEq, Eq, Sequence, Serialize, Deserialize,
)]
pub enum Scope {
    All,
    #[default]
    Online,
    Project,
    Installed,
}

#[derive(Debug, Default, Clone, EnumIter, PartialEq, Eq, Sequence, Serialize, Deserialize)]
pub enum Sort {
    #[default]
    Relevance,
    Name,
    Downloads,
    RecentDownloads,
    RecentlyUpdated,
    NewlyAdded,
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

