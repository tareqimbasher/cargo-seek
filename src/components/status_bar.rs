use std::cmp::PartialEq;

use ratatui::layout::{Alignment, Constraint, Layout, Rect};
use ratatui::style::{Styled, Stylize};
use ratatui::text::{Line, Text};
use ratatui::widgets::Paragraph;
use ratatui::Frame;
use serde::{Deserialize, Serialize};
use strum::Display;
use tokio::sync::mpsc::UnboundedSender;

use crate::action::Action;
use crate::app::Mode;
use crate::components::status_bar::StatusLevel::Info;
use crate::config::Config;
use crate::errors::AppResult;

#[derive(Debug, Clone, PartialEq, Eq, Display, Serialize, Deserialize)]
pub enum StatusLevel {
    Info,
    Progress,
    Success,
    Error,
}

#[derive(Debug, Clone, PartialEq, Eq)]
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
    duration: StatusDuration,
    message: String,
}

pub struct StatusBar {
    status: Option<StatusMessage>,
    last_annoying: Option<StatusMessage>,
    config: Config,
    action_tx: UnboundedSender<Action>,
}

impl StatusBar {
    pub fn new(action_tx: UnboundedSender<Action>) -> Self {
        StatusBar {
            status: None,
            last_annoying: None,
            config: Config::default(),
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
            duration: duration.clone(),
            message: text.clone(),
        };

        if duration == StatusDuration::Annoying {
            self.last_annoying = Some(message.clone());
        } else if text.is_empty() && self.last_annoying.is_some() {
            self.status = self.last_annoying.clone();
        } else {
            self.status = Some(message);
            let sleep: Option<u64> = match duration {
                StatusDuration::None => Some(0),
                StatusDuration::Short => Some(3),
                StatusDuration::Long => Some(10),
                StatusDuration::Seconds(s) => Some(s),
                _ => None,
            };

            if let Some(sleep) = sleep {
                let tx = self.action_tx.clone();
                tokio::spawn(async move {
                    tokio::time::sleep(tokio::time::Duration::from_secs(sleep)).await;
                    tx.send(Action::UpdateStatus(Info, "Ready".into())).unwrap();
                });
            }
        }
    }

    pub fn info<S: Into<String>>(&mut self, status: S) {
        self.set_status(status, StatusLevel::Info, StatusDuration::Long);
    }

    pub fn progress<S: Into<String>>(&mut self, status: S) {
        self.set_status(status, StatusLevel::Progress, StatusDuration::Sticky);
    }

    pub fn success<S: Into<String>>(&mut self, status: S) {
        self.set_status(status, StatusLevel::Success, StatusDuration::Long);
    }

    pub fn error<S: Into<String>>(&mut self, status: S) {
        self.set_status(status, StatusLevel::Error, StatusDuration::Long);
    }

    pub fn register_config_handler(&mut self, config: Config) -> AppResult<()> {
        self.config = config;
        Ok(())
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) -> AppResult<()> {
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

        let accent = self.config.styles[&Mode::Home]["accent"];
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
}
