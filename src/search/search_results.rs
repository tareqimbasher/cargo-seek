use crates_io_api::CrateResponse;
use indexmap::IndexMap;
use ratatui::widgets::ListState;

use crate::cargo::CargoEnv;
use crate::search::Crate;

/// The default number of results requested per page.
pub const DEFAULT_PER_PAGE: usize = 100;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SearchResults {
    /// Current page crate results.
    pub crates: Vec<Crate>,
    /// Total results matching the query across all pages.
    total_count: usize,
    /// The current active results page. 1-indexed.
    current_page: usize,
    /// The max number of results included in a single results page.
    per_page: usize,
    /// Highest page the backing source will actually serve. crates.io caps deep pagination, so
    /// `total_count` can far exceed what is reachable; page navigation clamps to this instead.
    max_page: usize,
    pub list_state: ListState,
}

impl SearchResults {
    pub fn new(
        crates: Vec<Crate>,
        total_count: usize,
        current_page: usize,
        per_page: usize,
        max_page: usize,
    ) -> Self {
        SearchResults {
            crates,
            total_count,
            current_page,
            per_page,
            max_page,
            list_state: ListState::default(),
        }
    }

    pub fn page_count(&self) -> usize {
        debug_assert!(self.per_page > 0, "per_page must be non-zero");
        self.total_count.div_ceil(self.per_page).min(self.max_page)
    }

    pub fn current_page(&self) -> usize {
        self.current_page
    }

    /// Total results matching the query across all pages.
    pub fn total_count(&self) -> usize {
        self.total_count
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
        self.current_page < self.page_count()
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

    /// Hydrates the selected crate from a metadata response.
    pub fn hydrate_selected(&mut self, response: &CrateResponse) {
        if let Some(index) = self.selected_index()
            && self.crates[index].name == response.crate_data.name
        {
            self.crates[index].hydrate(response);
        }
    }

    /// Deduplicates the results, then annotates each with its project/installed version from the
    /// cargo environment.
    pub fn update_results(&mut self, cargo_env: &CargoEnv) {
        self.deduplicate();

        for cr in &mut self.crates {
            if let Some(proj) = &cargo_env.project {
                cr.project_version = proj.get_local_version(&cr.name);
                cr.project_dep_kind = proj.get_local_dep_kind(&cr.name);
                cr.project_features = proj
                    .get_local_features(&cr.name)
                    .map(|features| features.into_iter().map(String::from).collect());
                cr.project_default_features = proj.get_local_default_features(&cr.name);
            }
            cr.installed_version = cargo_env.get_installed_version(&cr.name);
        }
    }

    /// Collapses crates sharing an id, keeping an already-hydrated copy over a stub.
    fn deduplicate(&mut self) {
        let mut map = IndexMap::<String, Crate>::new();

        for cr in self.crates.drain(0..) {
            if map.get(&cr.id).is_some_and(|v| v.is_metadata_loaded()) {
                continue;
            }
            map.insert(cr.id.clone(), cr);
        }

        self.crates = map.into_values().collect();
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::search::Crate;
    use pretty_assertions::assert_eq;

    fn results_with(total_count: usize, current_page: usize, crates: usize) -> SearchResults {
        let crates = (0..crates)
            .map(|i| Crate {
                id: i.to_string(),
                ..Default::default()
            })
            .collect();
        SearchResults::new(
            crates,
            total_count,
            current_page,
            DEFAULT_PER_PAGE,
            usize::MAX,
        )
    }

    /// A results set with an explicit registry page cap and no crates, for pagination-cap tests.
    fn results_capped(total_count: usize, current_page: usize, max_page: usize) -> SearchResults {
        SearchResults::new(
            Vec::new(),
            total_count,
            current_page,
            DEFAULT_PER_PAGE,
            max_page,
        )
    }

    #[test]
    fn page_count_rounds_up() {
        assert_eq!(results_with(0, 1, 0).page_count(), 0);
        assert_eq!(results_with(1, 1, 0).page_count(), 1);
        assert_eq!(results_with(100, 1, 0).page_count(), 1);
        assert_eq!(results_with(101, 1, 0).page_count(), 2);
        assert_eq!(results_with(250, 1, 0).page_count(), 3);
    }

    #[test]
    fn page_count_is_clamped_to_max_page() {
        // crates.io reports a huge total but only serves the first 200 pages; the count the UI
        // shows and navigates by must be the reachable one, not total/per_page.
        assert_eq!(results_capped(295_273, 1, 200).page_count(), 200);

        // The cap never invents pages when the true count is smaller.
        assert_eq!(results_capped(250, 1, 200).page_count(), 3);
    }

    #[test]
    fn resolve_page_is_none_when_there_are_no_results() {
        // A 0-result search has page_count() == 0; navigation must not emit page 0, which would
        // underflow the search task's `(page - 1)` skip math.
        let empty = results_with(0, 1, 0);
        assert_eq!(empty.resolve_page(0), None);
        assert_eq!(empty.resolve_page(1), None);
        assert_eq!(empty.resolve_page(5), None);
    }

    #[test]
    fn resolve_page_clamps_to_max_page() {
        // 295k results would be 2953 pages, but only 200 are reachable. Any jump past the cap
        // must resolve to the last reachable page.
        let r = results_capped(295_273, 1, 200);
        assert_eq!(r.resolve_page(2953), Some(200));
        assert_eq!(r.resolve_page(201), Some(200));
        assert_eq!(r.resolve_page(200), Some(200));
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
    fn has_next_page_stops_at_max_page() {
        // The last reachable page must not advertise a next page even though total_count
        // says there is more.
        assert!(!results_capped(295_273, 200, 200).has_next_page());
        assert!(results_capped(295_273, 199, 200).has_next_page());
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

    fn cr(id: &str, metadata_loaded: bool) -> Crate {
        Crate {
            id: id.to_string(),
            name: id.to_string(),
            metadata_loaded,
            ..Default::default()
        }
    }

    #[test]
    fn deduplicate_prefers_the_hydrated_copy() {
        let crates = vec![cr("a", false), cr("a", true), cr("b", false)];
        let mut results = SearchResults::new(crates, 0, 1, DEFAULT_PER_PAGE, usize::MAX);
        results.deduplicate();
        assert_eq!(results.crates.len(), 2);
        let a = results.crates.iter().find(|c| c.id == "a").unwrap();
        assert!(a.is_metadata_loaded());
    }

    #[test]
    fn deduplicate_keeps_the_already_hydrated_entry() {
        // Hydrated copy first, then an unhydrated duplicate: keep the hydrated one.
        let crates = vec![cr("a", true), cr("a", false)];
        let mut results = SearchResults::new(crates, 0, 1, DEFAULT_PER_PAGE, usize::MAX);
        results.deduplicate();
        assert_eq!(results.crates.len(), 1);
        assert!(results.crates[0].is_metadata_loaded());
    }
}
