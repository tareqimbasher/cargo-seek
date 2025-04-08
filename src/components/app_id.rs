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

/// A component that renders the name and version of the app.
pub struct AppId {
    id: String,
    config: Config,
}

impl AppId {
    pub fn new() -> Self {
        Self {
            id: format!(" 📦 cargo-seek v{} ", env!("CARGO_PKG_VERSION")),
            config: Config::default(),
        }
    }
}

impl Component for AppId {
    fn register_config_handler(&mut self, config: Config) -> AppResult<()> {
        self.config = config;
        Ok(())
    }

    fn draw(&mut self, _: &Mode, frame: &mut Frame, area: Rect) -> AppResult<()> {
        let [left, _] = Layout::horizontal([Constraint::Min(0), Constraint::Length(1)]).areas(area);
        let span = Span::styled(&self.id, self.config.styles[&Mode::App]["title"]);
        let paragraph = Paragraph::new(span).right_aligned();
        frame.render_widget(paragraph, left);
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
