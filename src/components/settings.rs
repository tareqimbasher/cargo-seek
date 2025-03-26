use async_trait::async_trait;
use crossterm::event::KeyEvent;
use ratatui::layout::{Constraint, Layout, Rect};
use ratatui::text::Line;
use ratatui::widgets::Paragraph;
use ratatui::Frame;

use crate::action::Action;
use crate::app::Mode;
use crate::components::Component;
use crate::config::Config;
use crate::errors::AppResult;
use crate::tui::Tui;

pub struct Settings {
    config: Config,
}

impl Settings {
    pub fn new() -> Self {
        Settings {
            config: Config::default(),
        }
    }
}

#[async_trait]
impl Component for Settings {
    fn register_config_handler(&mut self, config: Config) -> AppResult<()> {
        self.config = config;
        Ok(())
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> AppResult<Option<Action>> {
        let _ = key;
        Ok(None)
    }

    async fn update(&mut self, action: Action, tui: &mut Tui) -> AppResult<Option<Action>> {
        let _ = action;
        let _ = tui;

        Ok(None)
    }

    fn draw(&mut self, mode: &Mode, frame: &mut Frame, area: Rect) -> AppResult<()> {
        if *mode != Mode::Settings {
            return Ok(());
        }

        let [left, _] =
            Layout::horizontal([Constraint::Length(30), Constraint::Fill(1)]).areas(area);

        frame.render_widget(Paragraph::new(Line::from("Theme")), left);

        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
