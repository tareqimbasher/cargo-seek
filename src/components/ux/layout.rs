use ratatui::layout::{Constraint, Flex, Layout, Rect};

/// Centers a sub-area within `area`, sized by the given horizontal and vertical constraints.
pub fn center(area: Rect, horizontal: Constraint, vertical: Constraint) -> Rect {
    let [area] = Layout::horizontal([horizontal])
        .flex(Flex::Center)
        .areas(area);
    let [area] = Layout::vertical([vertical]).flex(Flex::Center).areas(area);
    area
}
