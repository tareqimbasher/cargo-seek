use serde::Deserialize;
use strum::Display;

use crate::search::{Scope, SearchResults, Sort};

/// A search instruction: run/clear a search, change sort/scope, paginate, or move the selection.
#[derive(Debug, Clone, PartialEq, Eq, Display, Deserialize)]
pub enum SearchCommand {
    Clear,
    Run {
        term: String,
        page: usize,
        hide_help: bool,
        status: Option<String>,
    },
    SortBy(Sort),
    Scope(Scope),
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

/// The result of search-related work performed off the UI thread.
#[derive(Debug, Clone, Display, Deserialize)]
pub enum SearchEvent {
    /// A search finished and produced these results.
    Completed(SearchResults),
    /// A search failed with this message.
    Failed(String),
    /// Full crate metadata finished loading (lazy hydration of the selected crate).
    MetadataLoaded(Box<crates_io_api::CrateResponse>),
    /// Lazy hydration of the named crate's metadata failed with this message.
    MetadataFailed { name: String, message: String },
}
