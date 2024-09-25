use chrono::{DateTime, Utc};
use ratatui::widgets::ListState;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;

use crate::action::{Action, SearchAction};
use crate::errors::AppResult;

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
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResults {
    pub crates: Vec<Crate>,
    meta: Meta,
    #[serde(default)]
    state: ListState,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Meta {
    #[serde(default)]
    pub current_page: u32,
    #[serde(rename = "total")]
    pub total_count: u32,
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
    pub fn set_current_page(&mut self, page: u32) {
        self.meta.current_page = page;
    }

    pub fn total_count(&self) -> u32 {
        self.meta.total_count
    }

    pub fn page_count(&self) -> u32 {
        self.meta.total_count.div_ceil(100)
    }

    pub fn current_page(&self) -> u32 {
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

    pub fn go_to_page(
        &self,
        page: u32,
        query: String,
        command_tx: UnboundedSender<Action>,
    ) -> AppResult<()> {
        let requested_page = if page >= self.page_count() {
            self.page_count()
        } else {
            page
        };

        if requested_page == self.current_page() {
            return Ok(());
        }

        command_tx.send(Action::Search(SearchAction::Search(query, requested_page)))?;

        Ok(())
    }

    pub fn go_prev_pages(
        &self,
        pages: u32,
        query: String,
        command_tx: UnboundedSender<Action>,
    ) -> AppResult<()> {
        let requested_page = if pages >= self.meta.current_page {
            1
        } else {
            self.meta.current_page - pages
        };

        if requested_page == self.current_page() {
            return Ok(());
        }

        command_tx.send(Action::Search(SearchAction::Search(query, requested_page)))?;

        Ok(())
    }

    pub fn go_next_pages(
        &self,
        pages: u32,
        query: String,
        command_tx: UnboundedSender<Action>,
    ) -> AppResult<()> {
        let mut requested_page = self.meta.current_page + pages;

        if requested_page > self.page_count() {
            requested_page = self.page_count();
        }

        if requested_page == self.current_page() {
            return Ok(());
        }

        command_tx.send(Action::Search(SearchAction::Search(query, requested_page)))?;

        Ok(())
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
