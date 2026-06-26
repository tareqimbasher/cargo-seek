use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::prelude::Stylize;
use ratatui::text::Line;
use ratatui::widgets::block::{Position, Title};
use ratatui::widgets::{Block, Clear, List, ListItem, ListState};

use crate::action::Action;
use crate::cargo::CargoCommand;
use crate::config::Config;

/// Which cargo action the picker dispatches once features are chosen.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FeatureIntent {
    Add,
    Install,
}

/// What happened when a key was routed to an open picker, interpreted by the owning component.
///
/// The picker handles its own navigation internally and reports only the lifecycle transition, so
/// the owner just maps the outcome to an action and closes the modal.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KeyOutcome {
    /// The key was handled; the picker stays open.
    Pending,
    /// The picker was dismissed without a selection.
    Cancelled,
    /// The user confirmed; read the result via [`FeatureSelector::confirm`].
    Submitted,
}

struct FeatureItem {
    name: String,
    is_default: bool,
    checked: bool,
}

/// A modal checklist for choosing which crate features to enable before `cargo add`/`install`.
///
/// Built on demand for the selected crate with the default features pre-checked. The owning
/// component routes keys to it while it is open and draws it as a centered overlay; on confirm it
/// produces the corresponding [`CargoCommand`].
pub struct FeatureSelector {
    config: Config,
    crate_name: String,
    version: String,
    intent: FeatureIntent,
    items: Vec<FeatureItem>,
    state: ListState,
}

impl FeatureSelector {
    pub fn new(
        config: Config,
        crate_name: String,
        version: String,
        intent: FeatureIntent,
        features: &[String],
        default_features: &[String],
    ) -> Self {
        let items: Vec<FeatureItem> = features
            .iter()
            .map(|name| {
                let is_default = default_features.iter().any(|d| d == name);
                FeatureItem {
                    name: name.clone(),
                    is_default,
                    // Defaults start checked so confirming straight away matches a plain add.
                    checked: is_default,
                }
            })
            .collect();

        let selected = (!items.is_empty()).then_some(0);
        Self {
            config,
            crate_name,
            version,
            intent,
            items,
            state: ListState::default().with_selected(selected),
        }
    }

    /// Routes a key while the picker is the active modal. It consumes every key, so the caller must
    /// not fall through to other handlers.
    pub fn handle_key(&mut self, key: KeyEvent) -> KeyOutcome {
        match key.code {
            KeyCode::Esc => return KeyOutcome::Cancelled,
            KeyCode::Enter => return KeyOutcome::Submitted,
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

    /// Builds the cargo command for the current selection.
    ///
    /// When every default feature is still checked, cargo enables them implicitly, so only the
    /// extra (non-default) selections are passed. If the user unchecked any default, the defaults
    /// are turned off (`--no-default-features`) and the full kept set is passed explicitly.
    pub fn confirm(&self) -> Action {
        let no_default_features = self.items.iter().any(|i| i.is_default && !i.checked);

        let features: Vec<String> = self
            .items
            .iter()
            .filter(|i| i.checked && (no_default_features || !i.is_default))
            .map(|i| i.name.clone())
            .collect();

        match self.intent {
            FeatureIntent::Add => Action::Cargo(CargoCommand::Add {
                name: self.crate_name.clone(),
                version: self.version.clone(),
                features,
                no_default_features,
            }),
            FeatureIntent::Install => Action::Cargo(CargoCommand::Install {
                name: self.crate_name.clone(),
                version: self.version.clone(),
                features,
                no_default_features,
            }),
        }
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let verb = match self.intent {
            FeatureIntent::Add => "Add",
            FeatureIntent::Install => "Install",
        };

        // Cap the popup to the available area; the list scrolls via its state when it overflows.
        let inner_height = (self.items.len() as u16).clamp(1, area.height.saturating_sub(4).max(1));
        let popup = center(area, 54.min(area.width), inner_height + 2);

        let block = Block::bordered()
            .title(
                Title::from(format!(" {verb} {} — features ", self.crate_name))
                    .alignment(Alignment::Center),
            )
            .title(
                Title::from(" Space toggle · Enter confirm · Esc cancel ")
                    .position(Position::Bottom)
                    .alignment(Alignment::Center),
            )
            .border_style(self.config.theme.accent);

        let items: Vec<ListItem> = self
            .items
            .iter()
            .map(|item| {
                let checkbox = if item.checked { "[x] " } else { "[ ] " };
                let name = if item.is_default {
                    item.name.clone().bold()
                } else {
                    item.name.clone().into()
                };
                ListItem::new(Line::from(vec![checkbox.into(), name]))
            })
            .collect();

        let list = List::new(items)
            .highlight_style(self.config.theme.accent.bold())
            .highlight_symbol("▶ ");

        frame.render_widget(Clear, popup);
        let inner = block.inner(popup);
        frame.render_widget(&block, popup);
        frame.render_stateful_widget(list, inner, &mut self.state);
    }
}

fn center(area: Rect, width: u16, height: u16) -> Rect {
    let [_, row, _] = Layout::vertical([
        Constraint::Min(0),
        Constraint::Length(height),
        Constraint::Min(0),
    ])
    .areas(area);

    let [_, cell, _] = Layout::horizontal([
        Constraint::Min(0),
        Constraint::Length(width),
        Constraint::Min(0),
    ])
    .areas(row);

    cell
}

#[cfg(test)]
mod tests {
    use super::{FeatureIntent, FeatureSelector};
    use crate::action::Action;
    use crate::cargo::CargoCommand;
    use crate::config::Config;
    use pretty_assertions::assert_eq;

    fn selector(features: &[&str], defaults: &[&str]) -> FeatureSelector {
        let features: Vec<String> = features.iter().map(|s| s.to_string()).collect();
        let defaults: Vec<String> = defaults.iter().map(|s| s.to_string()).collect();
        FeatureSelector::new(
            Config::default(),
            "demo".into(),
            "1.0.0".into(),
            FeatureIntent::Add,
            &features,
            &defaults,
        )
    }

    /// Toggles the checkbox at `index` by walking the selection there from the top.
    fn toggle(selector: &mut FeatureSelector, index: usize) {
        selector.state.select(Some(0));
        for _ in 0..index {
            selector.select_next();
        }
        selector.toggle_selected();
    }

    fn add_args(action: Action) -> (Vec<String>, bool) {
        match action {
            Action::Cargo(CargoCommand::Add {
                features,
                no_default_features,
                ..
            }) => (features, no_default_features),
            other => panic!("expected an Add command, got {other:?}"),
        }
    }

    #[test]
    fn keeping_defaults_passes_no_extra_features() {
        let sel = selector(&["derive", "std", "env"], &["derive", "std"]);
        let (features, no_default_features) = add_args(sel.confirm());

        // Defaults stay enabled implicitly, so nothing is passed and they aren't disabled.
        assert!(features.is_empty());
        assert!(!no_default_features);
    }

    #[test]
    fn enabling_a_non_default_passes_only_that_feature() {
        let mut sel = selector(&["derive", "std", "env"], &["derive", "std"]);
        toggle(&mut sel, 2); // enable "env"
        let (features, no_default_features) = add_args(sel.confirm());

        assert_eq!(features, vec!["env".to_string()]);
        assert!(!no_default_features);
    }

    #[test]
    fn unchecking_a_default_disables_defaults_and_passes_the_kept_set() {
        let mut sel = selector(&["derive", "std", "env"], &["derive", "std"]);
        toggle(&mut sel, 0); // disable the "derive" default
        let (features, no_default_features) = add_args(sel.confirm());

        // With a default turned off, defaults are disabled and the surviving selection is explicit.
        assert!(no_default_features);
        assert_eq!(features, vec!["std".to_string()]);
    }

    #[test]
    fn crate_without_defaults_passes_the_selection_as_is() {
        let mut sel = selector(&["a", "b"], &[]);
        toggle(&mut sel, 0); // enable "a"
        let (features, no_default_features) = add_args(sel.confirm());

        assert_eq!(features, vec!["a".to_string()]);
        assert!(!no_default_features);
    }
}
