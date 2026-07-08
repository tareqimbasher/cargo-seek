use async_trait::async_trait;
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Styled, Stylize};
use ratatui::text::{Line, Text};
use ratatui::widgets::Paragraph;
use serde::Deserialize;
use std::time::{Duration, Instant};
use strum::Display;

use crate::action::Action;
use crate::app::Mode;
use crate::components::{Component, Placement};
use crate::config::Config;
use crate::errors::AppResult;
use crate::tui::Tui;

#[derive(Debug, Clone, Display, Deserialize)]
pub enum StatusCommand {
    ResetStatus,
    UpdateStatus(StatusLevel, String),
    UpdateStatusWithDuration(StatusLevel, StatusDuration, String),
}

#[derive(Debug, Clone, PartialEq, Eq, Display, Deserialize)]
pub enum StatusLevel {
    Info,
    Progress,
    Success,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq, Display, Deserialize)]
pub enum StatusDuration {
    /// Is not rendered to screen (ex: when clearing status)
    None,
    Short,
    Long,
    Seconds(u64),
    /// Stays on screen till next status update
    Sticky,
}

#[derive(Debug, Clone)]
struct StatusMessage {
    level: StatusLevel,
    message: String,
}

/// The statusbar component.
pub struct StatusBar {
    status: Option<StatusMessage>,
    clear_after: Option<Instant>,
    config: Config,
}

impl StatusBar {
    pub fn new() -> Self {
        StatusBar {
            status: None,
            clear_after: None,
            config: Config::default(),
        }
    }

    fn set_status<S: Into<String>>(
        &mut self,
        status: S,
        level: StatusLevel,
        duration: StatusDuration,
    ) {
        self.status = Some(StatusMessage {
            level,
            message: status.into(),
        });

        let now = Instant::now();
        self.clear_after = match duration {
            StatusDuration::None => Some(now),
            StatusDuration::Short => Some(now + Duration::from_secs(3)),
            StatusDuration::Long => Some(now + Duration::from_secs(10)),
            StatusDuration::Seconds(s) => Some(now + Duration::from_secs(s)),
            StatusDuration::Sticky => None,
        };
    }

    /// Sets the idle, sticky "Ready" message shown when nothing else is happening.
    fn ready(&mut self) {
        self.set_status("Ready", StatusLevel::Info, StatusDuration::Sticky);
    }

    /// Reverts a timed status to "Ready" once its deadline has passed. Called on every `Tick`.
    fn clear_if_expired(&mut self) {
        if self
            .clear_after
            .is_some_and(|deadline| Instant::now() >= deadline)
        {
            self.ready();
        }
    }

    fn info<S: Into<String>>(&mut self, status: S) {
        self.set_status(status, StatusLevel::Info, StatusDuration::Long);
    }

    fn info_with_duration<S: Into<String>>(&mut self, duration: StatusDuration, status: S) {
        self.set_status(status, StatusLevel::Info, duration);
    }

    fn progress<S: Into<String>>(&mut self, status: S) {
        self.set_status(status, StatusLevel::Progress, StatusDuration::Sticky);
    }

    fn progress_with_duration<S: Into<String>>(&mut self, duration: StatusDuration, status: S) {
        self.set_status(status, StatusLevel::Progress, duration);
    }

    fn success<S: Into<String>>(&mut self, status: S) {
        self.set_status(status, StatusLevel::Success, StatusDuration::Long);
    }

    fn success_with_duration<S: Into<String>>(&mut self, duration: StatusDuration, status: S) {
        self.set_status(status, StatusLevel::Success, duration);
    }

    fn error<S: Into<String>>(&mut self, status: S) {
        self.set_status(status, StatusLevel::Error, StatusDuration::Long);
    }

    fn error_with_duration<S: Into<String>>(&mut self, duration: StatusDuration, status: S) {
        self.set_status(status, StatusLevel::Error, duration);
    }
}

#[async_trait]
impl Component for StatusBar {
    fn register_config_handler(&mut self, config: Config) -> AppResult<()> {
        self.config = config;
        Ok(())
    }

    fn init(&mut self, tui: &mut Tui) -> AppResult<()> {
        let _ = tui; // to appease clippy
        self.ready();
        Ok(())
    }

    async fn update(&mut self, action: &Action, tui: &mut Tui) -> AppResult<Option<Action>> {
        let _ = tui;
        match action {
            Action::Tick => self.clear_if_expired(),
            Action::Status(StatusCommand::UpdateStatus(level, message)) => match level {
                StatusLevel::Info => self.info(message.as_str()),
                StatusLevel::Progress => self.progress(message.as_str()),
                StatusLevel::Success => self.success(message.as_str()),
                StatusLevel::Error => self.error(message.as_str()),
            },
            Action::Status(StatusCommand::UpdateStatusWithDuration(level, duration, message)) => {
                let duration = duration.clone();
                let message = message.as_str();
                match level {
                    StatusLevel::Info => self.info_with_duration(duration, message),
                    StatusLevel::Progress => self.progress_with_duration(duration, message),
                    StatusLevel::Success => self.success_with_duration(duration, message),
                    StatusLevel::Error => self.error_with_duration(duration, message),
                }
            }
            Action::Status(StatusCommand::ResetStatus) => self.ready(),
            _ => {}
        };

        Ok(None)
    }

    fn draw(&mut self, _: &Mode, frame: &mut Frame, area: Rect) -> AppResult<()> {
        let accent = self.config.theme.accent;
        let text = vec![
            "/: ".set_style(accent),
            "search".into(),
            "  ".into(),
            "ctrl+h: ".set_style(accent),
            "help".into(),
        ];
        let text_length = text.iter().map(|x| x.content.len()).sum::<usize>();

        let [left, right] =
            Layout::horizontal([Constraint::Min(1), Constraint::Length(text_length as u16)])
                .areas(area);

        if let Some(status) = &self.status {
            let icon = match status.level {
                StatusLevel::Info => "ℹ️".cyan(),
                StatusLevel::Progress => "⏳".yellow(),
                StatusLevel::Success => "✅".green(),
                StatusLevel::Error => "❌".red(),
            };

            let text = Text::from(Line::from(vec![
                icon,
                " ".into(),
                status.message.as_str().into(),
            ]));
            frame.render_widget(Paragraph::new(text), left);
        }

        frame.render_widget(
            Paragraph::new(Text::from(Line::from(text))).alignment(Alignment::Right),
            right,
        );

        Ok(())
    }

    fn placement(&self) -> Placement {
        Placement::StatusBar
    }
}
