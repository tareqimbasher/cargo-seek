use enum_iterator::{all, reverse_all, Sequence};
use serde::{Deserialize, Serialize};
use std::iter::Cycle;

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

pub fn is_results_or_details_focused(focused: &Focusable) -> bool {
    *focused == Focusable::Results
        || *focused == Focusable::DocsButton
        || *focused == Focusable::ReadmeButton
        || *focused == Focusable::CratesIoButton
        || *focused == Focusable::LibRsButton
}
