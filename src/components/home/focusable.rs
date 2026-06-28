use serde::{Deserialize, Serialize};
use strum::{EnumCount, FromRepr};

#[derive(
    Default, PartialEq, Clone, Copy, Debug, Eq, EnumCount, FromRepr, Serialize, Deserialize,
)]
#[repr(usize)]
pub enum Focusable {
    Help,
    #[default]
    Search,
    Results,
    DocsButton,
    RepositoryButton,
    CratesIoButton,
    LibRsButton,
}

impl Focusable {
    /// The next focus target in Tab order, wrapping past the last back to the first.
    pub fn next(&self) -> Focusable {
        Self::from_repr((*self as usize + 1) % Self::COUNT).expect("modulo COUNT stays in range")
    }

    /// The previous focus target in Tab order, wrapping past the first back to the last.
    pub fn prev(&self) -> Focusable {
        Self::from_repr((*self as usize + Self::COUNT - 1) % Self::COUNT)
            .expect("modulo COUNT stays in range")
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use pretty_assertions::assert_eq;

    #[test]
    fn next_advances_and_wraps_past_the_last() {
        assert_eq!(Focusable::Help.next(), Focusable::Search);
        assert_eq!(Focusable::Search.next(), Focusable::Results);
        assert_eq!(Focusable::LibRsButton.next(), Focusable::Help);
    }

    #[test]
    fn prev_retreats_and_wraps_past_the_first() {
        assert_eq!(Focusable::Results.prev(), Focusable::Search);
        assert_eq!(Focusable::Search.prev(), Focusable::Help);
        assert_eq!(Focusable::Help.prev(), Focusable::LibRsButton);
    }
}
