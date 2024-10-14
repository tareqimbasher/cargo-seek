use chrono::{DateTime, Utc};
use ratatui::widgets::ListState;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Crate {
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub documentation: Option<String>,
    pub repository: Option<String>,
    pub max_version: String,
    pub max_stable_version: Option<String>,
    pub downloads: u64,
    pub recent_downloads: Option<u64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub exact_match: bool,
    #[serde(default)]
    pub is_local: bool,
    #[serde(default)]
    pub is_installed: bool,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResults {
    pub crates: Vec<Crate>,
    pub meta: Meta,
    #[serde(default)]
    state: ListState,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Meta {
    #[serde(default)]
    pub current_page: usize,
    #[serde(rename = "total")]
    pub total_count: usize,
}

impl Crate {
    pub fn version(&self) -> &str {
        match &self.max_stable_version {
            Some(v) => v,
            None => self.max_version.as_str(),
        }
    }
}

impl SearchResults {
    pub fn set_current_page(&mut self, page: usize) {
        self.meta.current_page = page;
    }

    pub fn total_count(&self) -> usize {
        self.meta.total_count
    }

    pub fn page_count(&self) -> usize {
        self.meta.total_count.div_ceil(100)
    }

    pub fn current_page(&self) -> usize {
        self.meta.current_page
    }

    pub fn current_page_count(&self) -> usize {
        self.crates.len()
    }

    pub fn has_next_page(&self) -> bool {
        let so_far = self.meta.current_page * 100;
        so_far + 100 <= self.meta.total_count
    }

    pub fn has_prev_page(&self) -> bool {
        self.meta.current_page > 1
    }

    pub fn get_selected_index(&self) -> Option<usize> {
        self.state.selected()
    }

    pub fn get_selected(&self) -> Option<&Crate> {
        if let Some(ix) = self.get_selected_index() {
            if let Some(item) = self.crates.get(ix) {
                return Some(item);
            }
        }

        None
    }

    pub fn select_index(&mut self, index: Option<usize>) -> Option<&Crate> {
        self.state.select(index);
        self.get_selected()
    }

    pub fn select_next(&mut self) -> Option<&Crate> {
        self.state.select_next();
        self.get_selected()
    }

    pub fn select_previous(&mut self) -> Option<&Crate> {
        self.state.select_previous();
        self.get_selected()
    }

    pub fn select_first(&mut self) -> Option<&Crate> {
        self.state.select_first();
        self.get_selected()
    }

    pub fn select_last(&mut self) -> Option<&Crate> {
        self.state.select_last();
        self.get_selected()
    }

    pub fn list_state(&mut self) -> &mut ListState {
        &mut self.state
    }
}
