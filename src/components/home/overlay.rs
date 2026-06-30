use crossterm::event::KeyEvent;
use ratatui::Frame;
use ratatui::layout::Rect;

use crate::action::Action;
use crate::components::home::feature_selector::FeatureSelector;
use crate::components::ux::{Confirm, Dropdown, KeyOutcome};
use crate::search::{Scope, SearchCommand, Sort};

/// The one popup that can be open at a time over [`Home`](super::Home).
pub enum Overlay {
    Sort(Dropdown<Sort>),
    Scope(Dropdown<Scope>),
    Features(FeatureSelector),
    Confirm(Confirm, Action),
}

impl Overlay {
    /// Routes a key to the active widget.
    pub fn handle_key(&mut self, key: KeyEvent) -> KeyOutcome<Action> {
        match self {
            Overlay::Sort(dropdown) => dropdown
                .handle_key(key)
                .map(|sort| Action::Search(SearchCommand::SortBy(sort))),
            Overlay::Scope(dropdown) => dropdown
                .handle_key(key)
                .map(|scope| Action::Search(SearchCommand::Scope(scope))),
            Overlay::Features(features) => features.handle_key(key),
            Overlay::Confirm(affirm, action) => affirm.handle_key(key).map(|()| action.clone()),
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        match self {
            Overlay::Sort(dropdown) => dropdown.draw(frame, area),
            Overlay::Scope(dropdown) => dropdown.draw(frame, area),
            Overlay::Features(features) => features.draw(frame, area),
            Overlay::Confirm(confirm, _) => confirm.draw(frame, area),
        }
    }
}
