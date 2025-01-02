#![allow(dead_code)]

use ratatui::buffer::Buffer;
use ratatui::layout::Rect;
use ratatui::prelude::Modifier;
use ratatui::style::{Color, Style};
use ratatui::text::Line;
use ratatui::widgets::Widget;

/// A custom widget that renders a button with a label, theme and state.
#[derive(Debug, Clone)]
pub struct Button<'a> {
    label: Line<'a>,
    theme: Theme,
    state: State,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum State {
    Normal,
    Selected,
    Active,
}

#[derive(Debug, Clone, Copy)]
pub struct Theme {
    text: Color,
    background: Color,
    highlight: Color,
    shadow: Color,
}

pub const WHITE: Theme = Theme {
    text: Color::Rgb(0, 0, 0),
    background: Color::Rgb(224, 224, 224),
    highlight: Color::Rgb(255, 255, 255),
    shadow: Color::Rgb(160, 160, 160),
};

pub const BLACK: Theme = Theme {
    text: Color::Rgb(255, 255, 255),
    background: Color::Rgb(48, 48, 48),
    highlight: Color::Rgb(64, 64, 64),
    shadow: Color::Rgb(32, 32, 32),
};

pub const GRAY: Theme = Theme {
    text: Color::Rgb(255, 255, 255),
    background: Color::Rgb(96, 96, 96),
    highlight: Color::Rgb(144, 144, 144),
    shadow: Color::Rgb(48, 48, 48),
};

pub const RED: Theme = Theme {
    text: Color::Rgb(255, 255, 255),
    background: Color::Rgb(144, 48, 48),
    highlight: Color::Rgb(192, 64, 64),
    shadow: Color::Rgb(96, 32, 32),
};

pub const GREEN: Theme = Theme {
    text: Color::Rgb(255, 255, 255),
    background: Color::Rgb(48, 144, 48),
    highlight: Color::Rgb(64, 192, 64),
    shadow: Color::Rgb(32, 96, 32),
};

pub const BLUE: Theme = Theme {
    text: Color::Rgb(255, 255, 255),
    background: Color::Rgb(48, 72, 144),
    highlight: Color::Rgb(64, 96, 192),
    shadow: Color::Rgb(32, 48, 96),
};

pub const ORANGE: Theme = Theme {
    text: Color::Rgb(255, 255, 255),
    background: Color::Rgb(144, 96, 48),
    highlight: Color::Rgb(192, 128, 64),
    shadow: Color::Rgb(96, 64, 32),
};

pub const YELLOW: Theme = Theme {
    text: Color::Rgb(255, 255, 255),
    background: Color::Rgb(144, 144, 48),
    highlight: Color::Rgb(192, 192, 64),
    shadow: Color::Rgb(96, 96, 32),
};

pub const CYAN: Theme = Theme {
    text: Color::Rgb(255, 255, 255),
    background: Color::Rgb(48, 144, 144),
    highlight: Color::Rgb(64, 192, 192),
    shadow: Color::Rgb(32, 96, 96),
};

pub const PURPLE: Theme = Theme {
    text: Color::Rgb(255, 255, 255),
    background: Color::Rgb(96, 48, 144),
    highlight: Color::Rgb(128, 64, 192),
    shadow: Color::Rgb(64, 32, 96),
};

/// A button with a label that can be themed.
impl<'a> Button<'a> {
    pub fn new<T: Into<Line<'a>>>(label: T) -> Self {
        Button {
            label: label.into(),
            theme: BLUE,
            state: State::Normal,
        }
    }

    pub const fn theme(mut self, theme: Theme) -> Self {
        self.theme = theme;
        self
    }

    pub const fn state(mut self, state: State) -> Self {
        self.state = state;
        self
    }
}

impl<'a> Widget for Button<'a> {
    #[allow(clippy::cast_possible_truncation)]
    fn render(self, area: Rect, buf: &mut Buffer) {
        let (background, text, shadow, highlight) = self.colors();
        let mut modifier = Modifier::BOLD;

        if self.state == State::Selected {
            modifier |= Modifier::UNDERLINED;
        }

        buf.set_style(
            area,
            Style::new().bg(background).fg(text).add_modifier(modifier),
        );

        // render top line if there's enough space
        if area.height > 2 {
            buf.set_string(
                area.x,
                area.y,
                "▔".repeat(area.width as usize),
                Style::new().fg(highlight).bg(background),
            );
        }
        // render bottom line if there's enough space
        if area.height > 1 {
            buf.set_string(
                area.x,
                area.y + area.height - 1,
                "▁".repeat(area.width as usize),
                Style::new().fg(shadow).bg(background),
            );
        }
        // render label centered
        buf.set_line(
            area.x + (area.width.saturating_sub(self.label.width() as u16)) / 2,
            area.y + (area.height.saturating_sub(1)) / 2,
            &self.label,
            area.width,
        );
    }
}

impl Button<'_> {
    const fn colors(&self) -> (Color, Color, Color, Color) {
        let theme = self.theme;
        match self.state {
            State::Normal => (theme.background, theme.text, theme.shadow, theme.highlight),
            State::Selected => (theme.highlight, theme.text, theme.shadow, theme.highlight),
            State::Active => (theme.background, theme.text, theme.highlight, theme.shadow),
        }
    }
}
