use enum_iterator::Sequence;
use serde::{Deserialize, Serialize};
use strum_macros::{Display, EnumIter};

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

#[derive(Debug, Default)]
pub struct SearchOptions {
    pub term: Option<String>,
    pub page: Option<usize>,
    pub per_page: Option<usize>,
    pub sort: Sort,
    pub scope: Scope,
}
