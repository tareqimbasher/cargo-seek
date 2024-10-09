use ratatui::{
    layout::{Constraint, Layout, Rect},
    text::Span,
    widgets::Paragraph,
    Frame,
};

use super::Component;

use crate::app::Mode;
use crate::config::Config;
use crate::errors::AppResult;

pub struct AppId {
    id: String,
    config: Config,
}

impl AppId {
    pub fn new() -> Self {
        Self {
            id: format!(" 📦 seekr v{} ", env!("CARGO_PKG_VERSION")),
            config: Config::default(),
        }
    }
}

impl Component for AppId {
    fn register_config_handler(&mut self, config: crate::config::Config) -> AppResult<()> {
        self.config = config;
        Ok(())
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> AppResult<()> {
        let [left, _] = Layout::horizontal([Constraint::Min(0), Constraint::Length(1)]).areas(area);
        let span = Span::styled(&self.id, self.config.styles[&Mode::Home]["title"]);
        let paragraph = Paragraph::new(span).right_aligned();
        frame.render_widget(paragraph, left);
        Ok(())
    }
}
