use std::cmp::PartialEq;
use async_trait::async_trait;
use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Styled, Stylize};
use ratatui::text::{Line, Text};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use serde::{Deserialize, Serialize};
use strum::Display;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::oneshot;

use crate::action::Action;
use crate::app::Mode;
use crate::components::status_bar::StatusLevel::Info;
use crate::components::Component;
use crate::config::Config;
use crate::errors::AppResult;
use crate::tui::Tui;

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum StatusLevel {
    Info,
    Progress,
    Success,
    Error,
}

#[allow(dead_code)]
#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum StatusDuration {
    /// Is not rendered to screen (ex: when clearing status)
    None,
    Short,
    Long,
    Seconds(u64),
    /// Stays on screen till next status update
    Sticky,
    /// Stays on screen till next status update. Unless next status update is also "Annoying",
    /// this status will appear after next status duration elapses.
    Annoying,
}

#[derive(Debug, Clone)]
struct StatusMessage {
    level: StatusLevel,
    message: String,
}

pub struct StatusBar {
    status: Option<StatusMessage>,
    last_annoying: Option<StatusMessage>,
    config: Config,
    cancel_tx: Option<oneshot::Sender<()>>,
    action_tx: UnboundedSender<Action>,
}

impl StatusBar {
    pub fn new(action_tx: UnboundedSender<Action>) -> Self {
        StatusBar {
            status: None,
            last_annoying: None,
            config: Config::default(),
            cancel_tx: None,
            action_tx,
        }
    }

    pub fn set_status<S: Into<String>>(
        &mut self,
        status: S,
        level: StatusLevel,
        duration: StatusDuration,
    ) {
        let text = status.into();
        let message = StatusMessage {
            level,
            message: text.clone(),
        };

        if duration == StatusDuration::Annoying {
            self.last_annoying = Some(message.clone());
        } else if text.is_empty() && self.last_annoying.is_some() {
            self.status = self.last_annoying.clone();
        } else {
            self.status = Some(message);

            // Cancel any clear task currently pending
            if let Some(cancel_tx) = self.cancel_tx.take() {
                let _ = cancel_tx.send(());
                self.cancel_tx = None;
            }

            let sleep_seconds: Option<u64> = match duration {
                StatusDuration::None => Some(0),
                StatusDuration::Short => Some(3),
                StatusDuration::Long => Some(10),
                StatusDuration::Seconds(s) => Some(s),
                _ => None,
            };

            if let Some(sleep_seconds) = sleep_seconds {
                let (cancel_tx, mut cancel_rx) = oneshot::channel();
                self.cancel_tx = Some(cancel_tx);

                let tx = self.action_tx.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(tokio::time::Duration::from_secs(sleep_seconds)).await;
                    if cancel_rx.try_recv().is_ok() {
                        return;
                    }
                    tx.send(Action::UpdateStatus(Info, "ready".into())).unwrap();
                });
            }
        }
    }

    pub fn info<S: Into<String>>(&mut self, status: S) {
        self.set_status(status, StatusLevel::Info, StatusDuration::Long);
    }

    pub fn info_with_duration<S: Into<String>>(&mut self, duration: StatusDuration, status: S) {
        self.set_status(status, StatusLevel::Info, duration);
    }

    pub fn progress<S: Into<String>>(&mut self, status: S) {
        self.set_status(status, StatusLevel::Progress, StatusDuration::Sticky);
    }

    pub fn progress_with_duration<S: Into<String>>(&mut self, duration: StatusDuration, status: S) {
        self.set_status(status, StatusLevel::Progress, duration);
    }

    pub fn success<S: Into<String>>(&mut self, status: S) {
        self.set_status(status, StatusLevel::Success, StatusDuration::Long);
    }

    pub fn success_with_duration<S: Into<String>>(&mut self, duration: StatusDuration, status: S) {
        self.set_status(status, StatusLevel::Success, duration);
    }

    pub fn error<S: Into<String>>(&mut self, status: S) {
        self.set_status(status, StatusLevel::Error, StatusDuration::Long);
    }

    pub fn error_with_duration<S: Into<String>>(&mut self, duration: StatusDuration, status: S) {
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
        self.info("Ready");
        Ok(())
    }

    async fn update(&mut self, action: Action, tui: &mut Tui) -> AppResult<Option<Action>> {
        let _ = tui;
        match action {
            Action::UpdateStatus(level, message) => match level {
                StatusLevel::Info => self.info(message),
                StatusLevel::Progress => self.progress(message),
                StatusLevel::Success => self.success(message),
                StatusLevel::Error => self.error(message),
            },
            Action::UpdateStatusWithDuration(level, duration, message) => match level {
                StatusLevel::Info => self.info_with_duration(duration, message),
                StatusLevel::Progress => self.progress_with_duration(duration, message),
                StatusLevel::Success => self.success_with_duration(duration, message),
                StatusLevel::Error => self.error_with_duration(duration, message),
            },
            _ => {}
        };

        Ok(None)
    }

    fn draw(&mut self, _: &Mode, frame: &mut Frame, area: Rect) -> AppResult<()> {
        let [left, right] =
            Layout::horizontal([Constraint::Percentage(50), Constraint::Percentage(50)])
                .areas(area);

        if let Some(status) = &self.status {
            let icon = match status.level {
                StatusLevel::Info => "ℹ️".cyan(),
                StatusLevel::Progress => "...".yellow(),
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

        let accent = self.config.styles[&Mode::App]["accent"];
        frame.render_widget(
            Paragraph::new(Text::from(Line::from(vec![
                "/: ".set_style(accent),
                "search".into(),
                " ".into(),
                "a: ".set_style(accent),
                "add".into(),
                " ".into(),
                "r: ".set_style(accent),
                "remove".into(),
                " ".into(),
                "i: ".set_style(accent),
                "install".into(),
                " ".into(),
                "u: ".set_style(accent),
                "uninstall".into(),
                " ".into(),
                "Ctrl+h: ".set_style(accent),
                "help".into(),
            ])))
            .alignment(Alignment::Right),
            right,
        );

        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
