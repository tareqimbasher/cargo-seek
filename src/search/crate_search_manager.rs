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
        // Cancel previous search
        if let Some(cancel_search_tx) = self.cancel_search_tx.take() {
            let _ = cancel_search_tx.send(());
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
            // Pages are 1-indexed
            let page = options.page.unwrap_or(1).max(1);
            let per_page = options.per_page.unwrap_or(DEFAULT_PER_PAGE);
            let mut still_needed = per_page;
            let mut search_results = SearchResults::new(page, per_page);

            // The read guard must not be held across the network call below.
            {
                let cargo_env = cargo_env.read().await;

                // Search crates added to the current project
                if options.scope.includes(Scope::Project)
                    && let Some(project) = &cargo_env.project
                {
                    let mut results = Self::search_project(&term, project);
                    search_results.total_count += results.len();
                    results = results
                        .into_iter()
                        .skip((page - 1) * per_page)
                        .take(still_needed)
                        .collect();
                    Self::extend_results(
                        &mut search_results,
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
                    let mut results = Self::search_binaries(&term, &cargo_env);
                    search_results.total_count += results.len();
                    results = results
                        .into_iter()
                        .skip((page - 1) * per_page)
                        .take(still_needed)
                        .collect();
                    Self::extend_results(
                        &mut search_results,
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
            if options.scope.includes(Scope::Online) {
                let registry = Self::search_registry(
                    crates_io_client,
                    &term,
                    still_needed,
                    page,
                    options.sort,
                );
                let outcome = tokio::select! {
                    biased;
                    _ = &mut cancel_search_rx => return,
                    outcome = registry => outcome,
                };
                match outcome {
                    Ok((mut results, count)) => {
                        Self::extend_results(
                            &mut search_results,
                            &mut results,
                            per_page,
                            &mut still_needed,
                        );
                        search_results.total_count += count;
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

            // Fresh guard, held only for the synchronous annotation and not across an await.
            {
                let cargo_env = cargo_env.read().await;
                search_results.update_results(&cargo_env);
            }

            tx.send(Action::SearchEvent(SearchEvent::Completed(search_results)))
                .ok();
        });
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
        search_results: &mut SearchResults,
        new_results: &mut Vec<Crate>,
        per_page: usize,
        still_needed: &mut usize,
    ) {
        if *still_needed >= new_results.len() {
            search_results.crates.append(new_results);
        } else if *still_needed > 0 {
            search_results
                .crates
                .extend(new_results.drain(..*still_needed));
        }
        *still_needed = per_page.saturating_sub(search_results.crates.len());
    }

    pub fn load_crate_metadata(&mut self, name: &str) -> AppResult<()> {
        if let Some(cancel_hydrate_tx) = self.cancel_hydrate_tx.take() {
            let _ = cancel_hydrate_tx.send(());
        }

        let (cancel_hydrate_tx, mut cancel_hydrate_rx) = oneshot::channel();
        self.cancel_hydrate_tx = Some(cancel_hydrate_tx);
        let tx = self.action_tx.clone();
        let crates_io_client = self.crates_io_client.clone();
        let name = name.to_owned();

        tokio::spawn(async move {
            // Debounce, then run the request both racing cancellation so reselecting quickly
            // drops the pending work instead of running it to completion.
            tokio::select! {
                biased;
                _ = &mut cancel_hydrate_rx => return,
                _ = tokio::time::sleep(Duration::from_millis(700)) => {}
            }

            let response = tokio::select! {
                biased;
                _ = &mut cancel_hydrate_rx => return,
                response = crates_io_client.get_crate(&name) => response,
            };

            match response {
                Ok(response) => {
                    tx.send(Action::SearchEvent(SearchEvent::MetadataLoaded(Box::new(
                        response,
                    ))))
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
        let mut results = SearchResults::new(1, DEFAULT_PER_PAGE);
        let mut new = vec![cr("a", false), cr("b", false)];
        let mut still_needed = 5;
        CrateSearchManager::extend_results(&mut results, &mut new, 5, &mut still_needed);
        assert_eq!(results.crates.len(), 2);
        assert_eq!(still_needed, 3);
        assert!(new.is_empty());
    }

    #[test]
    fn extend_takes_only_whats_still_needed() {
        let mut results = SearchResults::new(1, DEFAULT_PER_PAGE);
        let mut new = vec![cr("a", false), cr("b", false), cr("c", false)];
        let mut still_needed = 2;
        CrateSearchManager::extend_results(&mut results, &mut new, 5, &mut still_needed);
        assert_eq!(results.crates.len(), 2);
        assert_eq!(still_needed, 3);
        assert_eq!(new.len(), 1); // "c" is left behind
    }

    #[test]
    fn extend_adds_nothing_when_already_full() {
        let mut results = SearchResults::new(1, DEFAULT_PER_PAGE);
        results.crates = vec![cr("x", false)];
        let mut new = vec![cr("a", false)];
        let mut still_needed = 0;
        CrateSearchManager::extend_results(&mut results, &mut new, 1, &mut still_needed);
        assert_eq!(results.crates.len(), 1);
        assert_eq!(still_needed, 0);
        assert_eq!(new.len(), 1); // untouched
    }
}
