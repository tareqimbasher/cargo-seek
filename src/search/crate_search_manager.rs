use crates_io_api::{AsyncClient, CratesQuery};
use reqwest::{Client, header};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::{RwLock, oneshot};
use tracing::error;

use crate::action::Action;
use crate::cargo::{CargoEnv, Project};
use crate::errors::AppResult;
use crate::search::{
    Crate, DEFAULT_PER_PAGE, Scope, SearchEvent, SearchOptions, SearchResults, Sort,
};

pub struct CrateSearchManager {
    crates_io_client: Arc<AsyncClient>,
    action_tx: UnboundedSender<Action>,
    cancel_search_tx: Option<oneshot::Sender<()>>,
    cancel_hydrate_tx: Option<oneshot::Sender<()>>,
}

impl CrateSearchManager {
    pub fn new(action_tx: UnboundedSender<Action>) -> AppResult<Self> {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::USER_AGENT,
            header::HeaderValue::from_str("cargo-seek (github:tareqimbasher/cargo-seek)")?,
        );

        let client = AsyncClient::with_http_client(
            Client::builder()
                .default_headers(headers)
                .timeout(Duration::from_secs(10))
                .build()?,
            Duration::from_millis(1100),
        );

        Ok(CrateSearchManager {
            crates_io_client: Arc::new(client),
            action_tx,
            cancel_search_tx: None,
            cancel_hydrate_tx: None,
        })
    }

    pub fn search(&mut self, options: SearchOptions, cargo_env: Arc<RwLock<CargoEnv>>) {
        // Cancel any in-flight search or hydrate operation.
        if let Some(cancel_search_tx) = self.cancel_search_tx.take() {
            let _ = cancel_search_tx.send(());
        }

        if let Some(cancel_hydrate_tx) = self.cancel_hydrate_tx.take() {
            let _ = cancel_hydrate_tx.send(());
        }

        let (cancel_search_tx, mut cancel_search_rx) = oneshot::channel();
        self.cancel_search_tx = Some(cancel_search_tx);
        let tx = self.action_tx.clone();
        let crates_io_client = self.crates_io_client.clone();

        tokio::spawn(async move {
            if cancel_search_rx.try_recv().is_ok() {
                return;
            }

            let term = options.term.unwrap_or_default().to_lowercase();
            let per_page = options.per_page.unwrap_or(DEFAULT_PER_PAGE);
            let max_reachable_page = max_reachable_registry_page(&options.sort, &term, per_page);
            let current_page = options.page.unwrap_or(1).clamp(1, max_reachable_page);
            let mut current_page_results: Vec<Crate> = Vec::new();
            let mut total_result_count = 0;
            let mut still_needed = per_page;

            // The read guard must not be held across the network call below.
            {
                let cargo_env = cargo_env.read().await;

                // Search crates added to the current project
                if options.scope.includes(Scope::Project)
                    && let Some(project) = &cargo_env.project
                {
                    let mut results = search_project(&term, project);
                    total_result_count += results.len();
                    results = results
                        .into_iter()
                        .skip((current_page - 1) * per_page)
                        .collect();
                    extend_results(
                        &mut current_page_results,
                        &mut results,
                        per_page,
                        &mut still_needed,
                    );
                }

                if cancel_search_rx.try_recv().is_ok() {
                    return;
                }

                // Search globally installed binaries
                if options.scope.includes(Scope::Installed) {
                    let mut results = search_binaries(&term, &cargo_env);
                    total_result_count += results.len();
                    results = results
                        .into_iter()
                        .skip((current_page - 1) * per_page)
                        .collect();
                    extend_results(
                        &mut current_page_results,
                        &mut results,
                        per_page,
                        &mut still_needed,
                    );
                }
            }

            if cancel_search_rx.try_recv().is_ok() {
                return;
            }

            // Search the online registry
            if options.scope.includes(Scope::Online) && still_needed > 0 {
                let registry = search_registry(
                    crates_io_client,
                    &term,
                    still_needed,
                    current_page,
                    options.sort,
                );
                let outcome = tokio::select! {
                    biased;
                    _ = &mut cancel_search_rx => return,
                    outcome = registry => outcome,
                };
                match outcome {
                    Ok((mut results, count)) => {
                        extend_results(
                            &mut current_page_results,
                            &mut results,
                            per_page,
                            &mut still_needed,
                        );
                        total_result_count += count;
                    }
                    Err(err) => {
                        let _ =
                            tx.send(Action::SearchEvent(SearchEvent::Failed(format!("{err:#}"))));
                        return;
                    }
                }
            }

            if cancel_search_rx.try_recv().is_ok() {
                return;
            }

            let mut search_results = SearchResults::new(
                current_page_results,
                total_result_count,
                current_page,
                per_page,
                max_reachable_page,
            );

            // Fresh guard, held only for the synchronous update and not across an await.
            {
                let cargo_env = cargo_env.read().await;
                search_results.update_results(&cargo_env);
            }

            tx.send(Action::SearchEvent(SearchEvent::Completed(search_results)))
                .ok();
        });
    }

    /// Starts the fetching of metadata for the named crate and then fires
    /// [`SearchEvent::MetadataLoaded`] or [`SearchEvent::MetadataFailed`].
    ///
    /// When `debounce` is [`true`] the load waits for a short period before fetching so that rapid
    /// consecutive calls coalesce into a single request. When `debounce` is [`false`], metadata is
    /// fetched immediately.
    ///
    /// Any previous in-flight load is canceled first. Both the wait and the request race
    /// cancellation, so re-calling this function drops the pending work instead of running it to
    /// completion.
    pub fn start_metadata_load(&mut self, name: &str, debounce: bool) -> AppResult<()> {
        if let Some(cancel_hydrate_tx) = self.cancel_hydrate_tx.take() {
            let _ = cancel_hydrate_tx.send(());
        }

        let (cancel_hydrate_tx, mut cancel_hydrate_rx) = oneshot::channel();
        self.cancel_hydrate_tx = Some(cancel_hydrate_tx);
        let tx = self.action_tx.clone();
        let crates_io_client = self.crates_io_client.clone();
        let name = name.to_owned();

        tokio::spawn(async move {
            if debounce {
                tokio::select! {
                    biased;
                    _ = &mut cancel_hydrate_rx => return,
                    _ = tokio::time::sleep(Duration::from_millis(700)) => {}
                }
            }

            let response = tokio::select! {
                biased;
                _ = &mut cancel_hydrate_rx => return,
                response = crates_io_client.get_crate(&name) => response,
            };

            match response {
                Ok(response) => {
                    tx.send(Action::SearchEvent(SearchEvent::MetadataLoaded {
                        response: Box::new(response),
                    }))
                    .ok();
                }
                Err(err) => {
                    error!("failed to load metadata for `{name}`: {err:#}");
                    tx.send(Action::SearchEvent(SearchEvent::MetadataFailed {
                        name,
                        message: format!("{err}"),
                    }))
                    .ok();
                }
            }
        });

        Ok(())
    }
}

fn search_project(term: &str, project: &Project) -> Vec<Crate> {
    let mut results: Vec<Crate> = Vec::new();

    for package in &project.packages {
        for dep in &package.dependencies {
            let name_lower = dep.name.to_lowercase();
            if name_lower.contains(term) {
                let mut cr = Crate::from_dependency(dep);
                cr.exact_match = name_lower == term;
                results.push(cr);
            }
        }
    }

    results
}

fn search_binaries(term: &str, cargo_env: &CargoEnv) -> Vec<Crate> {
    let mut results: Vec<Crate> = Vec::new();

    for bin in &cargo_env.installed_binaries {
        let name_lower = bin.name.to_lowercase();
        if name_lower.contains(term) {
            let mut cr = Crate::from_binary(bin);
            cr.exact_match = name_lower == term;
            results.push(cr);
        }
    }

    results
}

async fn search_registry(
    crates_io_client: Arc<AsyncClient>,
    term: &str,
    per_page: usize,
    page: usize,
    sort: Sort,
) -> AppResult<(Vec<Crate>, usize)> {
    let sort = match sort {
        Sort::Relevance => crates_io_api::Sort::Relevance,
        Sort::Name => crates_io_api::Sort::Alphabetical,
        Sort::Downloads => crates_io_api::Sort::Downloads,
        Sort::RecentDownloads => crates_io_api::Sort::RecentDownloads,
        Sort::RecentlyUpdated => crates_io_api::Sort::RecentUpdates,
        Sort::NewlyAdded => crates_io_api::Sort::NewlyAdded,
    };

    let result = crates_io_client
        .crates(
            CratesQuery::builder()
                .search(term)
                .sort(sort)
                .page_size(per_page as u64)
                .page(page as u64)
                .build(),
        )
        .await?;

    let results = result
        .crates
        .into_iter()
        .map(Crate::from_crates_io)
        .collect();
    Ok((results, result.meta.total as usize))
}

fn extend_results(
    results: &mut Vec<Crate>,
    new_results: &mut Vec<Crate>,
    per_page: usize,
    still_needed: &mut usize,
) {
    if *still_needed >= new_results.len() {
        results.append(new_results);
    } else if *still_needed > 0 {
        results.extend(new_results.drain(..*still_needed));
    }
    *still_needed = per_page.saturating_sub(results.len());
}

/// The highest page number crates.io will serve for a query.
///
/// The registry refuses deep pagination: any query is limited to the first 20,000 results, and
/// sorting by relevance with a non-empty query tightens that to the first 1,000.
/// Past those bounds the API returns `400`, so page navigation and the reported page count are
/// clamped to this rather than to the (potentially far larger) result total.
///
/// These figures are crates.io's published pagination policy, not something we can read back at
/// runtime (the client discards the 400 body). If deep-pagination requests start failing again,
/// check whether the limits moved at <https://crates.io/data-access> and update the constants below.
fn max_reachable_registry_page(sort: &Sort, term: &str, per_page: usize) -> usize {
    const MAX_RESULTS: usize = 20_000;
    const MAX_RELEVANCE_RESULTS: usize = 1_000;

    let cap = if matches!(sort, Sort::Relevance) && !term.trim().is_empty() {
        MAX_RELEVANCE_RESULTS
    } else {
        MAX_RESULTS
    };

    // At least page 1 is always reachable, even if a caller picks a per_page larger than the cap.
    (cap / per_page.max(1)).max(1)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cr(id: &str, metadata_loaded: bool) -> Crate {
        Crate {
            id: id.to_string(),
            name: id.to_string(),
            metadata_loaded,
            ..Default::default()
        }
    }

    #[test]
    fn extend_appends_all_when_there_is_room() {
        let mut crates: Vec<Crate> = Vec::new();
        let mut new = vec![cr("a", false), cr("b", false)];
        let mut still_needed = 5;
        extend_results(&mut crates, &mut new, 5, &mut still_needed);
        assert_eq!(crates.len(), 2);
        assert_eq!(still_needed, 3);
        assert!(new.is_empty());
    }

    #[test]
    fn extend_takes_only_whats_still_needed() {
        let mut crates: Vec<Crate> = Vec::new();
        let mut new = vec![cr("a", false), cr("b", false), cr("c", false)];
        let mut still_needed = 2;
        extend_results(&mut crates, &mut new, 5, &mut still_needed);
        assert_eq!(crates.len(), 2);
        assert_eq!(still_needed, 3);
        assert_eq!(new.len(), 1); // "c" is left behind
    }

    #[test]
    fn extend_adds_nothing_when_already_full() {
        let mut crates = vec![cr("x", false)];
        let mut new = vec![cr("a", false)];
        let mut still_needed = 0;
        extend_results(&mut crates, &mut new, 1, &mut still_needed);
        assert_eq!(crates.len(), 1);
        assert_eq!(still_needed, 0);
        assert_eq!(new.len(), 1); // untouched
    }

    #[test]
    fn max_registry_page_relevance_with_query_caps_at_1000_results() {
        // Relevance + a real query: crates.io serves only the first 1,000 results (10 pages).
        assert_eq!(
            max_reachable_registry_page(&Sort::Relevance, "tokio", 100),
            10
        );
    }

    #[test]
    fn max_registry_page_empty_relevance_query_uses_the_deep_cap() {
        // An empty query has nothing to rank, so the 1,000 relevance cap does not apply, only
        // the 20,000 deep-pagination cap (200 pages) does.
        assert_eq!(max_reachable_registry_page(&Sort::Relevance, "", 100), 200);
        assert_eq!(
            max_reachable_registry_page(&Sort::Relevance, "   ", 100),
            200
        );
    }

    #[test]
    fn max_registry_page_non_relevance_sorts_use_the_deep_cap() {
        // The 1,000-result cap is unique to relevance; every other sort pages to the 20,000 deep
        // cap even with a query. So this asserts the whole non-relevance sort set.
        for sort in [
            Sort::Name,
            Sort::Downloads,
            Sort::RecentDownloads,
            Sort::RecentlyUpdated,
            Sort::NewlyAdded,
        ] {
            assert_eq!(
                max_reachable_registry_page(&sort, "serde", 100),
                200,
                "sort {sort:?}"
            );
        }
    }

    #[test]
    fn max_registry_page_is_never_below_one() {
        // Page 1 must stay reachable even if per_page exceeds the cap.
        assert_eq!(
            max_reachable_registry_page(&Sort::Relevance, "tokio", 100_000),
            1
        );
    }
}
