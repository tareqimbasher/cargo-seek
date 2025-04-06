use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

#[derive(Default, PartialEq, Clone, Debug, Eq, EnumIter, Serialize, Deserialize)]
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
        let mut variants = Focusable::iter();
        variants.find(|v| v == self);
        variants.next().unwrap()
    }

    pub fn prev(&self) -> Focusable {
        let mut variants = Focusable::iter();
        variants.find(|v| v == self);
        variants.next_back().unwrap()
    }
}

pub fn is_results_or_details_focused(focused: &Focusable) -> bool {
    *focused == Focusable::Results
        || *focused == Focusable::DocsButton
        || *focused == Focusable::ReadmeButton
        || *focused == Focusable::CratesIoButton
        || *focused == Focusable::LibRsButton
}
