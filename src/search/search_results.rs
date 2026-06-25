use ratatui::widgets::ListState;
use serde::Deserialize;

use crate::search::Crate;

/// Number of results requested per page.
pub const DEFAULT_PER_PAGE: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
pub struct SearchResults {
    pub crates: Vec<Crate>,
    pub total_count: usize,
    pub list_state: ListState,
    current_page: usize,
    per_page: usize,
}

impl SearchResults {
    pub fn new(page: usize, per_page: usize) -> Self {
        SearchResults {
            crates: Vec::default(),
            total_count: 0,
            current_page: page,
            per_page,
            list_state: ListState::default(),
        }
    }

    pub fn page_count(&self) -> usize {
        debug_assert!(self.per_page > 0, "per_page must be non-zero");
        self.total_count.div_ceil(self.per_page)
    }

    pub fn current_page(&self) -> usize {
        self.current_page
    }

    /// Resolves a navigation request to the page that should be loaded, or `None` when there is
    /// nothing to do (no results, or the request is already the current page).
    ///
    /// Pages are 1-indexed; `requested` is clamped into `1..=page_count()`.
    pub fn resolve_page(&self, requested: usize) -> Option<usize> {
        let last_page = self.page_count();
        if last_page == 0 {
            return None;
        }
        let target = requested.clamp(1, last_page);
        (target != self.current_page).then_some(target)
    }

    pub fn current_page_len(&self) -> usize {
        self.crates.len()
    }

    /// Number of results on all pages before the current one.
    pub fn items_before_current_page(&self) -> usize {
        self.current_page.saturating_sub(1) * self.per_page
    }

    pub fn has_next_page(&self) -> bool {
        self.current_page * self.per_page < self.total_count
    }

    pub fn has_prev_page(&self) -> bool {
        self.current_page > 1
    }

    /// The selected index, always clamped into range (or `None` when there is no selection or no
    /// results). The selection can briefly fall out of range — `ListState` over-increments past
    /// the last item, and results may be replaced/deduplicated underneath it — so every read
    /// clamps rather than trusting the raw value.
    pub fn selected_index(&self) -> Option<usize> {
        let selected = self.list_state.selected()?;
        if self.crates.is_empty() {
            None
        } else {
            Some(selected.min(self.crates.len() - 1))
        }
    }

    pub fn selected(&self) -> Option<&Crate> {
        self.crates.get(self.selected_index()?)
    }

    pub fn select_index(&mut self, index: Option<usize>) -> Option<&Crate> {
        let index = match index {
            Some(i) if !self.crates.is_empty() => Some(i.min(self.crates.len() - 1)),
            _ => None,
        };
        self.list_state.select(index);
        self.selected()
    }

    pub fn select_next(&mut self) -> Option<&Crate> {
        let next = self.selected_index().map_or(0, |i| i + 1);
        self.select_index(Some(next))
    }

    pub fn select_previous(&mut self) -> Option<&Crate> {
        let prev = self.selected_index().map_or(0, |i| i.saturating_sub(1));
        self.select_index(Some(prev))
    }

    pub fn select_first(&mut self) -> Option<&Crate> {
        self.select_index(Some(0))
    }

    pub fn select_last(&mut self) -> Option<&Crate> {
        let last = self.crates.len().saturating_sub(1);
        self.select_index(Some(last))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::Crate;
    use pretty_assertions::assert_eq;

    fn results_with(total_count: usize, current_page: usize, crates: usize) -> SearchResults {
        let mut r = SearchResults::new(current_page, DEFAULT_PER_PAGE);
        r.total_count = total_count;
        r.crates = (0..crates)
            .map(|i| Crate {
                id: i.to_string(),
                ..Default::default()
            })
            .collect();
        r
    }

    #[test]
    fn page_count_rounds_up() {
        assert_eq!(SearchResults::new(1, DEFAULT_PER_PAGE).page_count(), 0);
        assert_eq!(results_with(1, 1, 0).page_count(), 1);
        assert_eq!(results_with(100, 1, 0).page_count(), 1);
        assert_eq!(results_with(101, 1, 0).page_count(), 2);
        assert_eq!(results_with(250, 1, 0).page_count(), 3);
    }

    #[test]
    fn resolve_page_is_none_when_there_are_no_results() {
        // A 0-result search has page_count() == 0; navigation must not emit page 0, which would
        // underflow the search task's `(page - 1)` skip math.
        let empty = SearchResults::new(1, DEFAULT_PER_PAGE);
        assert_eq!(empty.resolve_page(0), None);
        assert_eq!(empty.resolve_page(1), None);
        assert_eq!(empty.resolve_page(5), None);
    }

    #[test]
    fn resolve_page_clamps_into_one_to_page_count() {
        // 250 results -> 3 pages; currently on page 2.
        let on_page_2 = results_with(250, 2, 0);
        assert_eq!(on_page_2.resolve_page(0), Some(1)); // clamps up to the first page
        assert_eq!(on_page_2.resolve_page(1), Some(1));
        assert_eq!(on_page_2.resolve_page(9), Some(3)); // clamps down to the last page
        assert_eq!(on_page_2.resolve_page(3), Some(3));
        assert_eq!(on_page_2.resolve_page(2), None); // already on the requested page
    }

    #[test]
    fn has_next_page_until_total_is_consumed() {
        // 250 results at 100/page: pages 1 and 2 have a next page, page 3 does not.
        assert!(results_with(250, 1, 0).has_next_page());
        assert!(results_with(250, 2, 0).has_next_page());
        assert!(!results_with(250, 3, 0).has_next_page());
    }

    #[test]
    fn has_prev_page_after_the_first() {
        assert!(!results_with(250, 1, 0).has_prev_page());
        assert!(results_with(250, 2, 0).has_prev_page());
    }

    #[test]
    fn items_before_current_page_counts_prior_pages() {
        assert_eq!(results_with(250, 1, 0).items_before_current_page(), 0);
        assert_eq!(results_with(250, 3, 0).items_before_current_page(), 200);
    }

    #[test]
    fn selected_index_clamps_out_of_range_to_last() {
        let mut r = results_with(3, 1, 3);
        // ListState can hold an out-of-range value (over-increment, or usize::MAX from
        // select_last); selected_index() clamps it to the last item.
        r.list_state.select(Some(usize::MAX));
        assert_eq!(r.selected_index(), Some(2));
        r.list_state.select(Some(99));
        assert_eq!(r.selected_index(), Some(2));
    }

    #[test]
    fn selected_index_is_none_when_empty() {
        let mut r = results_with(0, 1, 0);
        r.list_state.select(Some(0));
        assert_eq!(r.selected_index(), None);
        assert_eq!(r.selected(), None);
    }

    #[test]
    fn selected_index_passes_through_normal_values() {
        let mut r = results_with(3, 1, 3);
        r.list_state.select(Some(1));
        assert_eq!(r.selected_index(), Some(1));
        r.list_state.select(None);
        assert_eq!(r.selected_index(), None);
    }

    #[test]
    fn select_next_stops_at_the_last_item() {
        let mut r = results_with(3, 1, 3);
        r.select_last();
        assert_eq!(r.selected_index(), Some(2));
        // Pressing down again must not over-scroll past the end (previously blanked selection).
        r.select_next();
        assert_eq!(r.selected_index(), Some(2));
        assert!(r.selected().is_some());
    }

    #[test]
    fn select_previous_stops_at_the_first_item() {
        let mut r = results_with(3, 1, 3);
        r.select_first();
        r.select_previous();
        assert_eq!(r.selected_index(), Some(0));
    }
}
