use async_trait::async_trait;
use crossterm::event::{KeyCode, KeyEvent};
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::prelude::Stylize;
use ratatui::widgets::block::Title;
use ratatui::widgets::{Block, Clear, List, ListItem, ListState};
use ratatui::Frame;
use std::fmt::Display;
use strum::IntoEnumIterator;

use crate::action::Action;
use crate::app::Mode;
use crate::components::Component;
use crate::config::Config;
use crate::errors::AppResult;

pub struct Dropdown<T> {
    header: String,
    config: Config,
    is_focused: bool,
    state: ListState,
    on_enter: Box<dyn Fn(&T) + Send + Sync>,
}

impl<T: IntoEnumIterator + Default + Clone> Dropdown<T> {
    pub fn new(
        header: String,
        selected_ix: usize,
        on_enter: Box<dyn Fn(&T) + Send + Sync>,
    ) -> Self {
        Dropdown {
            header,
            config: Config::default(),
            is_focused: false,
            state: ListState::default().with_selected(Some(selected_ix)),
            on_enter,
        }
    }

    pub fn set_is_focused(&mut self, focused: bool) {
        self.is_focused = focused;
    }

    pub fn get_selected(&self) -> T {
        if let Some(ix) = self.state.selected() {
            if let Some(value) = T::iter().nth(ix) {
                return value.clone();
            }
        }
        T::default()
    }
}

#[async_trait]
impl<T: IntoEnumIterator + Default + Display + Clone + 'static> Component for Dropdown<T> {
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
                let selected_value = self.get_selected();
                self.on_enter.as_ref()(&selected_value);
            }
            _ => {}
        }

        Ok(None)
    }

    fn draw(&mut self, _: &Mode, frame: &mut Frame, area: Rect) -> AppResult<()> {
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

        let [_, dropdown_wrapper_rect] =
            Layout::vertical([Constraint::Length(4), Constraint::Length(8)]).areas(main);

        let [_, dropdown_rect, _] = Layout::horizontal([
            Constraint::Min(0),
            Constraint::Length(35),
            Constraint::Min(0),
        ])
        .areas(dropdown_wrapper_rect);

        let dropdown = Block::bordered()
            .title(Title::from(format!(" {0}: ", self.header)).alignment(Alignment::Center))
            .border_style(self.config.styles[&Mode::App]["accent"]);

        frame.render_widget(Clear, dropdown_wrapper_rect);
        frame.render_widget(&dropdown, dropdown_rect);

        let list = List::new(T::iter().map(|x| ListItem::new(x.to_string())))
            .highlight_style(self.config.styles[&Mode::App]["accent"].bold())
            .highlight_symbol("▶ ");

        frame.render_stateful_widget(list, dropdown.inner(dropdown_rect), &mut self.state);

        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
