use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::widgets::{List, ListItem, ListState};
use std::fmt::Display;
use std::marker::PhantomData;
use strum::IntoEnumIterator;

use crate::components::ux::{KeyOutcome, Popup};
use crate::config::Config;

/// A modal dropdown, rendered as a popup, listing every variant of `T`, with one highlighted
/// as the selection.
pub struct Dropdown<T> {
    config: Config,
    header: String,
    state: ListState,
    marker: PhantomData<T>,
}

impl<T: IntoEnumIterator + Display + PartialEq> Dropdown<T> {
    /// Builds a dropdown over `T`'s variants with `selected` pre-highlighted.
    pub fn new(config: Config, header: String, selected: T) -> Self {
        let selected_ix = T::iter()
            .position(|variant| variant == selected)
            .unwrap_or(0);
        Self {
            config,
            header,
            state: ListState::default().with_selected(Some(selected_ix)),
            marker: PhantomData,
        }
    }

    /// The currently highlighted variant.
    fn selected(&self) -> T {
        self.state
            .selected()
            .and_then(|ix| T::iter().nth(ix))
            .or_else(|| T::iter().next())
            .expect("a dropdown is never built over a variant-less enum")
    }

    fn select_next(&mut self) {
        let count = T::iter().count();
        if count == 0 {
            return;
        }
        let next = self.state.selected().map_or(0, |i| (i + 1).min(count - 1));
        self.state.select(Some(next));
    }

    fn select_previous(&mut self) {
        let prev = self.state.selected().map_or(0, |i| i.saturating_sub(1));
        self.state.select(Some(prev));
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> KeyOutcome<T> {
        match key.code {
            KeyCode::Esc => return KeyOutcome::Cancelled,
            KeyCode::Enter => return KeyOutcome::Submitted(self.selected()),
            KeyCode::Up => self.select_previous(),
            KeyCode::Down => self.select_next(),
            _ => {}
        }
        KeyOutcome::Pending
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let count = T::iter().count() as u16;

        let inner = Popup::new(35, count + 2)
            .title(format!(" {}: ", self.header))
            .footer(" Enter confirm · Esc cancel ")
            .border_style(self.config.theme.accent)
            .render(frame, area);

        let list = List::new(T::iter().map(|variant| ListItem::new(variant.to_string())))
            .highlight_style(self.config.theme.accent.bold())
            .highlight_symbol("▶ ");

        frame.render_stateful_widget(list, inner, &mut self.state);
    }
}
