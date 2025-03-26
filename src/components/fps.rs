use std::time::Instant;
use async_trait::async_trait;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Style, Stylize},
    text::Span,
    widgets::Paragraph,
    Frame,
};

use super::Component;

use crate::action::Action;
use crate::app::Mode;
use crate::errors::AppResult;
use crate::tui::Tui;

#[derive(Debug, Clone, PartialEq)]
pub struct FpsCounter {
    last_tick_update: Instant,
    tick_count: u32,
    ticks_per_second: f64,

    last_frame_update: Instant,
    frame_count: u32,
    frames_per_second: f64,
}

impl Default for FpsCounter {
    fn default() -> Self {
        Self::new()
    }
}

impl FpsCounter {
    pub fn new() -> Self {
        Self {
            last_tick_update: Instant::now(),
            tick_count: 0,
            ticks_per_second: 0.0,
            last_frame_update: Instant::now(),
            frame_count: 0,
            frames_per_second: 0.0,
        }
    }

    fn app_tick(&mut self) -> AppResult<()> {
        self.tick_count += 1;
        let now = Instant::now();
        let elapsed = (now - self.last_tick_update).as_secs_f64();
        if elapsed >= 1.0 {
            self.ticks_per_second = self.tick_count as f64 / elapsed;
            self.last_tick_update = now;
            self.tick_count = 0;
        }
        Ok(())
    }

    fn render_tick(&mut self) -> AppResult<()> {
        self.frame_count += 1;
        let now = Instant::now();
        let elapsed = (now - self.last_frame_update).as_secs_f64();
        if elapsed >= 1.0 {
            self.frames_per_second = self.frame_count as f64 / elapsed;
            self.last_frame_update = now;
            self.frame_count = 0;
        }
        Ok(())
    }
}

#[async_trait]
impl Component for FpsCounter {
    async fn update(&mut self, action: Action, tui: &mut Tui) -> AppResult<Option<Action>> {
        let _ = tui; // to appease clippy

        match action {
            Action::Tick => self.app_tick()?,
            Action::Render => self.render_tick()?,
            _ => {}
        };
        Ok(None)
    }

    fn draw(&mut self, _: &Mode, frame: &mut Frame, area: Rect) -> AppResult<()> {
        let [top, _] = Layout::vertical([Constraint::Length(1), Constraint::Min(0)]).areas(area);
        let message = format!(
            "{:.2} ticks/sec, {:.2} FPS",
            self.ticks_per_second, self.frames_per_second
        );
        let span = Span::styled(message, Style::new().dim());
        let paragraph = Paragraph::new(span).right_aligned();
        frame.render_widget(paragraph, top);
        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
