use ratatui::Frame;
use ratatui::layout::{Constraint, Flex, Layout, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::text::Line;
use ratatui::widgets::{Block, Clear};

/// The result of routing a key to an open popup.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum KeyOutcome<T> {
    /// The key was handled; the popup stays open.
    Pending,
    /// The popup was dismissed without a result.
    Cancelled,
    /// The user confirmed; carries the produced value.
    Submitted(T),
}

impl<T> KeyOutcome<T> {
    /// Transforms the submitted value, leaving `Pending`/`Cancelled` untouched. Lets an owner wrap a
    /// widget's raw result into a richer one (e.g. a selected variant into an `Action`).
    pub fn map<U>(self, f: impl FnOnce(T) -> U) -> KeyOutcome<U> {
        match self {
            KeyOutcome::Pending => KeyOutcome::Pending,
            KeyOutcome::Cancelled => KeyOutcome::Cancelled,
            KeyOutcome::Submitted(value) => KeyOutcome::Submitted(f(value)),
        }
    }
}

/// A modal frame shared by components that are displayed as a popup.
pub struct Popup<'a> {
    width: u16,
    height: u16,
    title: Option<Line<'a>>,
    footer: Option<Line<'a>>,
    border_style: Style,
}

impl<'a> Popup<'a> {
    pub fn new(width: u16, height: u16) -> Self {
        Self {
            width,
            height,
            title: None,
            footer: None,
            border_style: Style::default(),
        }
    }

    /// A title centered along the top border.
    pub fn title(mut self, title: impl Into<Line<'a>>) -> Self {
        self.title = Some(title.into());
        self
    }

    /// A hint centered along the bottom border.
    pub fn footer(mut self, footer: impl Into<Line<'a>>) -> Self {
        self.footer = Some(footer.into());
        self
    }

    pub fn border_style(mut self, style: Style) -> Self {
        self.border_style = style;
        self
    }

    /// Dims the background, clears a margin around the popup, draws the block, and returns the
    /// inner content area for the caller to render into.
    pub fn render(self, frame: &mut Frame, area: Rect) -> Rect {
        let popup = center(area, self.width, self.height);
        let clear = center(
            area,
            self.width.saturating_add(4).min(area.width),
            self.height.saturating_add(2).min(area.height),
        );

        let mut block = Block::bordered().border_style(self.border_style);
        if let Some(title) = self.title {
            block = block.title(title.centered());
        }
        if let Some(footer) = self.footer {
            block = block.title_bottom(footer.centered());
        }

        let inner = block.inner(popup);

        frame
            .buffer_mut()
            .set_style(area, Style::new().add_modifier(Modifier::DIM));
        frame.render_widget(Clear, clear);
        frame.render_widget(block, popup);
        inner
    }
}

fn center(area: Rect, width: u16, height: u16) -> Rect {
    let [area] = Layout::horizontal([Constraint::Length(width)])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::vertical([Constraint::Length(height)])
        .flex(Flex::Center)
        .areas(area);
    area
}
