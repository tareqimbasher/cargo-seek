use crate::action::{Action, SearchAction};
use crate::app::Mode;
use crate::components::Component;
use crate::config::Config;
use crate::errors::AppResult;
use crossterm::event::{KeyCode, KeyEvent};
use enum_iterator::{all, Sequence};
use ratatui::{
    layout::{Alignment, Constraint, Layout, Rect},
    style::Stylize,
    widgets::{block::Title, Block, Clear, List, ListItem, ListState},
    Frame,
};
use serde::{Deserialize, Serialize};
use strum::Display;

#[derive(Debug, Default, Display, Clone, PartialEq, Eq, Sequence, Serialize, Deserialize)]
pub enum Scope {
    All,
    #[default]
    Online,
    Project,
    Installed,
}

pub struct ScopeDropdown {
    config: Config,
    is_focused: bool,
    state: ListState,
}

impl ScopeDropdown {
    pub fn new() -> Self {
        ScopeDropdown {
            config: Config::default(),
            is_focused: false,
            state: ListState::default().with_selected(Some(1)),
        }
    }

    pub fn set_is_focused(&mut self, focused: bool) {
        self.is_focused = focused;
    }

    pub fn get_selected(&self) -> Scope {
        if let Some(ix) = self.state.selected() {
            if let Some(value) = all::<Scope>().nth(ix) {
                return value;
            }
        }
        Scope::default()
    }
}

impl Component for ScopeDropdown {
    fn register_config_handler(&mut self, config: Config) -> AppResult<()> {
        self.config = config;
        Ok(())
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> AppResult<Option<Action>> {
        if !self.is_focused {
            return Ok(None);
        }

        match key.code {
            KeyCode::Up => {
                self.state.select_previous();
            }
            KeyCode::Down => {
                self.state.select_next();
            }
            KeyCode::Enter => {
                return Ok(Some(Action::Search(SearchAction::Scope(
                    self.get_selected(),
                ))));
            }
            _ => {}
        }

        Ok(None)
    }

    fn draw(&mut self, mode: &Mode, frame: &mut Frame, area: Rect) -> AppResult<()> {
        if !self.is_focused {
            return Ok(());
        }

        if self.state.selected().is_none() {
            self.state.select_first();
        }

        let [_, main, _] = Layout::horizontal([
            Constraint::Length(1),
            Constraint::Min(0),
            Constraint::Length(1),
        ])
        .areas(area);

        let [_, sort_by_dropdown_wrapper] =
            Layout::vertical([Constraint::Length(4), Constraint::Length(8)]).areas(main);

        let [_, sort_by_dropdown, _] = Layout::horizontal([
            Constraint::Min(0),
            Constraint::Length(35),
            Constraint::Min(0),
        ])
        .areas(sort_by_dropdown_wrapper);

        let dropdown = Block::bordered()
            .title(Title::from(" Search in: ").alignment(Alignment::Center))
            .border_style(self.config.styles[&Mode::App]["accent"]);

        frame.render_widget(Clear, sort_by_dropdown_wrapper);
        frame.render_widget(&dropdown, sort_by_dropdown);

        let list = List::new(vec![
            ListItem::new("All"),
            ListItem::new("Online"),
            ListItem::new("Project"),
            ListItem::new("Installed"),
        ])
        .highlight_style(self.config.styles[&Mode::App]["accent"].bold())
        .highlight_symbol("> ");

        frame.render_stateful_widget(list, dropdown.inner(sort_by_dropdown), &mut self.state);

        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
