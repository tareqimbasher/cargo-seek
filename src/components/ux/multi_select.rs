use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::Rect;
use ratatui::text::{Line, Span};
use ratatui::widgets::{List, ListItem, ListState};

use crate::components::ux::{KeyOutcome, Popup};
use crate::config::Config;

/// One row of a [`MultiSelect`].
pub struct MultiSelectItem<T> {
    pub value: T,
    pub label: Line<'static>,
    pub checked: bool,
}

impl<T> MultiSelectItem<T> {
    pub fn new(value: T, label: impl Into<Line<'static>>, checked: bool) -> Self {
        Self {
            value,
            label: label.into(),
            checked,
        }
    }
}

/// A modal list of toggleable items rendered as a popup.
pub struct MultiSelect<T> {
    config: Config,
    title: String,
    items: Vec<MultiSelectItem<T>>,
    state: ListState,
}

impl<T: Clone> MultiSelect<T> {
    pub fn new(config: Config, title: String, items: Vec<MultiSelectItem<T>>) -> Self {
        let selected = (!items.is_empty()).then_some(0);
        Self {
            config,
            title,
            items,
            state: ListState::default().with_selected(selected),
        }
    }

    /// The values of the currently checked items, in display order.
    pub fn checked(&self) -> Vec<T> {
        self.items
            .iter()
            .filter(|item| item.checked)
            .map(|item| item.value.clone())
            .collect()
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> KeyOutcome<Vec<T>> {
        match key.code {
            KeyCode::Esc => return KeyOutcome::Cancelled,
            KeyCode::Enter => return KeyOutcome::Submitted(self.checked()),
            KeyCode::Up => self.select_previous(),
            KeyCode::Down => self.select_next(),
            KeyCode::Char(' ') => self.toggle_selected(),
            _ => {}
        }
        KeyOutcome::Pending
    }

    fn select_next(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let next = self
            .state
            .selected()
            .map_or(0, |i| (i + 1).min(self.items.len() - 1));
        self.state.select(Some(next));
    }

    fn select_previous(&mut self) {
        if self.items.is_empty() {
            return;
        }
        let prev = self.state.selected().map_or(0, |i| i.saturating_sub(1));
        self.state.select(Some(prev));
    }

    fn toggle_selected(&mut self) {
        if let Some(index) = self.state.selected()
            && let Some(item) = self.items.get_mut(index)
        {
            item.checked = !item.checked;
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        // Cap the popup to the available area; the list scrolls via its state when it overflows.
        let inner_height = (self.items.len() as u16).clamp(1, area.height.saturating_sub(4).max(1));

        let list_items: Vec<ListItem> = self
            .items
            .iter()
            .map(|item| {
                let checkbox = if item.checked { "[x] " } else { "[ ] " };
                let mut spans = vec![Span::from(checkbox)];
                spans.extend(item.label.spans.iter().cloned());
                ListItem::new(Line::from(spans))
            })
            .collect();

        let list = List::new(list_items)
            .highlight_style(self.config.theme.accent.bold())
            .highlight_symbol("▶ ");

        let inner = Popup::new(54.min(area.width), inner_height + 2)
            .title(self.title.as_str())
            .footer(" Space toggle · Enter confirm · Esc cancel ")
            .border_style(self.config.theme.accent)
            .render(frame, area);

        frame.render_stateful_widget(list, inner, &mut self.state);
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, KeyModifiers};
    use pretty_assertions::assert_eq;

    fn multi_select(items: &[(&str, bool)]) -> MultiSelect<String> {
        let items = items
            .iter()
            .map(|(name, checked)| {
                MultiSelectItem::new(name.to_string(), name.to_string(), *checked)
            })
            .collect();
        MultiSelect::new(Config::default(), " demo ".into(), items)
    }

    fn press(ms: &mut MultiSelect<String>, code: KeyCode) -> KeyOutcome<Vec<String>> {
        ms.handle_key(KeyEvent::new(code, KeyModifiers::empty()))
    }

    #[test]
    fn checked_reports_initially_checked_items_in_order() {
        let ms = multi_select(&[("a", true), ("b", false), ("c", true)]);
        assert_eq!(ms.checked(), vec!["a".to_string(), "c".to_string()]);
    }

    #[test]
    fn space_toggles_the_selected_item() {
        let mut ms = multi_select(&[("a", false), ("b", false)]);
        press(&mut ms, KeyCode::Down); // move to "b"
        press(&mut ms, KeyCode::Char(' '));
        assert_eq!(ms.checked(), vec!["b".to_string()]);
    }

    #[test]
    fn selection_clamps_at_both_ends() {
        let mut ms = multi_select(&[("a", false), ("b", false)]);
        // Walking up from the top stays put; checking confirms which row is selected.
        press(&mut ms, KeyCode::Up);
        press(&mut ms, KeyCode::Char(' '));
        assert_eq!(ms.checked(), vec!["a".to_string()]);

        // Walking past the bottom stays on the last row.
        let mut ms = multi_select(&[("a", false), ("b", false)]);
        press(&mut ms, KeyCode::Down);
        press(&mut ms, KeyCode::Down);
        press(&mut ms, KeyCode::Char(' '));
        assert_eq!(ms.checked(), vec!["b".to_string()]);
    }

    #[test]
    fn enter_submits_and_esc_cancels() {
        let mut ms = multi_select(&[("a", true)]);
        assert_eq!(
            press(&mut ms, KeyCode::Enter),
            KeyOutcome::Submitted(vec!["a".to_string()])
        );
        assert_eq!(press(&mut ms, KeyCode::Esc), KeyOutcome::Cancelled);
    }
}
