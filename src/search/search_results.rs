use ratatui::widgets::ListState;
use serde::Deserialize;

use crate::search::Crate;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SearchResults {
    pub crates: Vec<Crate>,
    pub total_count: usize,
    pub list_state: ListState,
    current_page: usize,
}

impl SearchResults {
    pub fn new(page: usize) -> Self {
        SearchResults {
            crates: Vec::default(),
            total_count: 0,
            current_page: page,
            list_state: ListState::default(),
        }
    }

    pub fn page_count(&self) -> usize {
        self.total_count.div_ceil(100)
    }

    pub fn current_page(&self) -> usize {
        self.current_page
    }

    pub fn current_page_count(&self) -> usize {
        self.crates.len()
    }

    pub fn has_next_page(&self) -> bool {
        let so_far = self.current_page * 100;
        so_far < self.total_count
    }

    pub fn has_prev_page(&self) -> bool {
        self.current_page > 1
    }

    pub fn selected_index(&self) -> Option<usize> {
        if let Some(index) = self.list_state.selected() {
            if index == usize::MAX {
                // Index can be usize::MAX to denote last item
                return Some(self.crates.len() - 1);
            }
            return Some(index);
        }
        None
    }

    pub fn selected(&self) -> Option<&Crate> {
        if let Some(ix) = self.selected_index()
            && let Some(item) = self.crates.get(ix)
        {
            return Some(item);
        }
        None
    }

    pub fn select_index(&mut self, index: Option<usize>) -> Option<&Crate> {
        self.list_state.select(index);
        self.selected()
    }

    pub fn select_next(&mut self) -> Option<&Crate> {
        self.list_state.select_next();
        self.selected()
    }

    pub fn select_previous(&mut self) -> Option<&Crate> {
        self.list_state.select_previous();
        self.selected()
    }

    pub fn select_first(&mut self) -> Option<&Crate> {
        self.list_state.select_first();
        self.selected()
    }

    pub fn select_last(&mut self) -> Option<&Crate> {
        self.list_state.select_last();
        self.selected()
    }
}
