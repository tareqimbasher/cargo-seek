use serde::{Deserialize, Serialize};
use strum::IntoEnumIterator;
use strum_macros::EnumIter;

#[derive(Default, PartialEq, Clone, Debug, Eq, EnumIter, Serialize, Deserialize)]
pub enum Focusable {
    Usage,
    #[default]
    Search,
    Sort,
    Scope,
    Results,
    DocsButton,
    RepositoryButton,
    CratesIoButton,
    LibRsButton,
}

impl Focusable {
    pub fn next(&self) -> Focusable {
        let mut variants = Focusable::iter();
        variants.find(|v| v == self);
        if let Some(next) = variants.next() {
            next
        } else {
            variants.get(0).unwrap()
        }
    }

    pub fn prev(&self) -> Focusable {
        let variants: Vec<_> = Focusable::iter().collect();
        let pos = variants
            .iter()
            .position(|v| *v == *self)
            .expect("self should be in the list of variants");
        variants[(pos + variants.len() - 1) % variants.len()].clone()
    }
}
