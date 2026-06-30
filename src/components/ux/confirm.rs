use crossterm::event::{KeyCode, KeyEvent};
use ratatui::Frame;
use ratatui::layout::{Alignment, Constraint, Flex, Layout, Rect};
use ratatui::widgets::{Paragraph, Wrap};

use crate::components::ux::{Button, KeyOutcome, Popup, State};
use crate::config::Config;

/// A modal yes/cancel prompt rendered as a popup.
pub struct Confirm {
    config: Config,
    message: String,
    /// `1` is the affirmative button, `0` the cancel button.
    selected: usize,
}

impl Confirm {
    /// Builds a prompt showing `message`. When `default_cancel` is set, Cancel starts selected.
    pub fn new(config: Config, message: &str, default_cancel: bool) -> Self {
        Confirm {
            config,
            message: message.to_owned(),
            selected: if default_cancel { 0 } else { 1 },
        }
    }

    pub fn handle_key(&mut self, key: KeyEvent) -> KeyOutcome<()> {
        match key.code {
            KeyCode::Esc => return KeyOutcome::Cancelled,
            KeyCode::Enter => {
                return if self.selected == 1 {
                    KeyOutcome::Submitted(())
                } else {
                    KeyOutcome::Cancelled
                };
            }
            KeyCode::Right => self.toggle(),
            KeyCode::Left => self.toggle(),
            _ => {}
        }
        KeyOutcome::Pending
    }

    fn toggle(&mut self) {
        self.selected = if self.selected == 0 { 1 } else { 0 };
    }

    pub fn draw(&mut self, frame: &mut Frame, area: Rect) {
        let width = 60.min(area.width);
        let inner_width = usize::from(width.saturating_sub(2).max(1));
        let lines = self
            .message
            .chars()
            .count()
            .div_ceil(inner_width)
            .clamp(1, 10);
        let message_lines = u16::try_from(lines).unwrap_or(10);
        // border (2) + the centered message with a blank row above and below (2)
        // + the button row (1) + a trailing gap (1).
        let height = (message_lines + 6).min(area.height);

        let inner = Popup::new(width, height)
            .title(" Confirm ")
            .footer(" ← → select · Enter confirm · Esc cancel ")
            .border_style(self.config.theme.accent)
            .render(frame, area);

        let [message_area, button_row, _] = Layout::vertical([
            Constraint::Min(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(inner);

        let [message_area] = Layout::vertical([Constraint::Length(message_lines)])
            .flex(Flex::Center)
            .areas(message_area);
        frame.render_widget(
            Paragraph::new(self.message.as_str())
                .wrap(Wrap { trim: true })
                .alignment(Alignment::Center),
            message_area,
        );

        let [_, cancel_area, _, confirm_area, _] = Layout::horizontal([
            Constraint::Length(4),
            Constraint::Fill(1),
            Constraint::Length(4),
            Constraint::Fill(1),
            Constraint::Length(4),
        ])
        .areas(button_row);

        frame.render_widget(
            Button::new("Cancel")
                .theme(super::button::GRAY)
                .state(if self.selected == 0 {
                    State::Selected
                } else {
                    State::Normal
                }),
            cancel_area,
        );
        frame.render_widget(
            Button::new("Yes")
                .theme(super::button::RED)
                .state(if self.selected == 1 {
                    State::Selected
                } else {
                    State::Normal
                }),
            confirm_area,
        );
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crossterm::event::{KeyEvent, KeyModifiers};
    use pretty_assertions::assert_eq;

    fn confirm(default_cancel: bool) -> Confirm {
        Confirm::new(Config::default(), "remove tokio v1.0.0?", default_cancel)
    }

    fn press(dialog: &mut Confirm, code: KeyCode) -> KeyOutcome<()> {
        dialog.handle_key(KeyEvent::new(code, KeyModifiers::empty()))
    }

    #[test]
    fn default_cancel_makes_a_reflexive_enter_abort() {
        // Opening the prompt and hitting Enter must not run the action.
        let mut dialog = confirm(true);
        assert_eq!(press(&mut dialog, KeyCode::Enter), KeyOutcome::Cancelled);
    }

    #[test]
    fn enter_confirms_when_the_affirmative_is_selected() {
        let mut dialog = confirm(false);
        assert_eq!(
            press(&mut dialog, KeyCode::Enter),
            KeyOutcome::Submitted(())
        );
    }

    #[test]
    fn left_and_right_both_toggle_to_the_other_button() {
        let mut via_right = confirm(true);
        press(&mut via_right, KeyCode::Right);
        assert_eq!(
            press(&mut via_right, KeyCode::Enter),
            KeyOutcome::Submitted(())
        );

        let mut via_left = confirm(true);
        press(&mut via_left, KeyCode::Left);
        assert_eq!(
            press(&mut via_left, KeyCode::Enter),
            KeyOutcome::Submitted(())
        );
    }

    #[test]
    fn toggling_twice_returns_to_the_starting_button() {
        let mut dialog = confirm(true);
        press(&mut dialog, KeyCode::Right);
        press(&mut dialog, KeyCode::Right);
        assert_eq!(press(&mut dialog, KeyCode::Enter), KeyOutcome::Cancelled);
    }

    #[test]
    fn esc_cancels_even_with_the_affirmative_selected() {
        let mut dialog = confirm(false);
        assert_eq!(press(&mut dialog, KeyCode::Esc), KeyOutcome::Cancelled);
    }

    #[test]
    fn an_unhandled_key_is_pending_and_keeps_the_selection() {
        let mut dialog = confirm(false);
        assert_eq!(press(&mut dialog, KeyCode::Char('x')), KeyOutcome::Pending);
        // Selection is untouched, so Enter still confirms.
        assert_eq!(
            press(&mut dialog, KeyCode::Enter),
            KeyOutcome::Submitted(())
        );
    }
}
