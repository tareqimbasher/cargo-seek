use crates_io_api::{AsyncClient, CratesQuery};
use reqwest::{header, Client};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::{oneshot, RwLock};
use tokio::task::JoinHandle;

use crate::action::{Action, SearchAction};
use crate::cargo::{CargoEnv, Project};
use crate::errors::{AppError, AppResult};
use crate::search::{Crate, Scope, SearchOptions, SearchResults, Sort};

pub struct CrateSearchManager {
    crates_io_client: Arc<AsyncClient>,
    cancel_tx: Option<oneshot::Sender<()>>,
    action_tx: UnboundedSender<Action>,
}

impl CrateSearchManager {
    pub fn new(action_tx: UnboundedSender<Action>) -> AppResult<Self> {
        let mut headers = header::HeaderMap::new();
        headers.insert(
            header::USER_AGENT,
            header::HeaderValue::from_str("seekr (github:tareqimbasher/seekr)")?,
        );

        let client = AsyncClient::with_http_client(
            Client::builder()
                .default_headers(headers)
                .timeout(Duration::from_secs(10))
                .build()?,
            Duration::from_millis(1000),
        );

        Ok(CrateSearchManager {
            crates_io_client: Arc::new(client),
            cancel_tx: None,
            action_tx,
        })
    }

    pub fn search(
        &mut self,
        options: SearchOptions,
        cargo_env: Arc<RwLock<CargoEnv>>,
    ) -> JoinHandle<()> {
        if let Some(cancel_tx) = self.cancel_tx.take() {
            let _ = cancel_tx.send(());
        }

        let (cancel_tx, mut cancel_rx) = oneshot::channel();
        self.cancel_tx = Some(cancel_tx);
        let tx = self.action_tx.clone();

        let crates_io_client = self.crates_io_client.clone();

        tokio::spawn(async move {
            if cancel_rx.try_recv().is_ok() {
                return;
            }

            let cargo_env = cargo_env.read().await;
            let term = options.term.unwrap_or("".to_string()).to_lowercase();
            let page = options.page.unwrap_or(1);
            let per_page = options.per_page.unwrap_or(100);

            let mut search_results = SearchResults::new(page);

            // Crates added to the current project
            if options.scope == Scope::All || options.scope == Scope::Project {
                if let Some(project) = &cargo_env.project {
                    let mut results = Self::search_project(&term, project);
                    search_results.total_count += results.len();

                    let mut still_needed = per_page - search_results.crates.len();
                    if still_needed > results.len() {
                        still_needed = results.len();
                    }
                    search_results.crates.extend(results.drain(..still_needed));
                }
            }

            if cancel_rx.try_recv().is_ok() {
                return;
            }

            // Globally installed cargo binaries
            if options.scope == Scope::All || options.scope == Scope::Installed {
                let mut results = Self::search_binaries(&term, &cargo_env);
                search_results.total_count += results.len();

                let mut still_needed = per_page - search_results.crates.len();
                if still_needed > results.len() {
                    still_needed = results.len();
                }
                search_results.crates.extend(results.drain(..still_needed));
            }

            if cancel_rx.try_recv().is_ok() {
                return;
            }

            // Crates in registry
            if options.scope == Scope::All || options.scope == Scope::Online {
                let result =
                    Self::search_registry(crates_io_client, &term, per_page, page, options.sort)
                        .await;

                match result {
                    Ok(r) => {
                        search_results.total_count += r.1;
                        let mut results = r.0;
                        let mut still_needed = per_page - search_results.crates.len();
                        if still_needed > results.len() {
                            still_needed = results.len();
                        }
                        search_results.crates.extend(results.drain(..still_needed));
                    }
                    Err(err) => {
                        tx.send(Action::Search(SearchAction::Error(format!("{:#}", err))))
                            .ok();
                    }
                }
            }

            if cancel_rx.try_recv().is_ok() {
                return;
            }

            // Back-fill is_local and is_installed for search results that don't have it
            Self::update_results(&mut search_results, &cargo_env);

            tx.send(Action::Search(SearchAction::Render(search_results)))
                .ok();
        })
    }

    pub fn update_results(search_results: &mut SearchResults, cargo_env: &CargoEnv) {
        for cr in &mut search_results.crates {
            if let Some(proj) = &cargo_env.project {
                cr.local_version = proj.get_local_version(&cr.name);
            }

            cr.installed_version = cargo_env.get_installed_version(&cr.name);
        }
    }

    fn search_binaries(term: &str, cargo_env: &CargoEnv) -> Vec<Crate> {
        let mut results: Vec<Crate> = Vec::new();

        for package in &cargo_env.installed {
            let name_lower = package.name.to_lowercase();
            if name_lower.contains(term) {
                results.push(Crate {
                    id: package.name.clone(),
                    name: package.name.clone(),
                    description: None,
                    homepage: None,
                    documentation: None,
                    repository: None,
                    version: package.version.clone(),
                    max_version: None,
                    max_stable_version: None,
                    downloads: None,
                    recent_downloads: None,
                    created_at: None,
                    updated_at: None,
                    exact_match: name_lower == term,
                    local_version: None,
                    installed_version: Some(package.version.clone()),
                });
            }
        }

        results
        //.sort_by(|a, b| a.id.cmp(&b.id));
    }

    fn search_project(term: &str, project: &Project) -> Vec<Crate> {
        let mut results: Vec<Crate> = Vec::new();

        for package in &project.packages {
            for dep in &package.dependencies {
                let name_lower = dep.name.to_lowercase();
                if name_lower.contains(term) {
                    results.push(Crate {
                        id: dep.name.clone(),
                        name: dep.name.clone(),
                        description: None,
                        homepage: None,
                        documentation: None,
                        repository: None,
                        version: dep.req.clone(),
                        max_version: None,
                        max_stable_version: None,
                        downloads: None,
                        recent_downloads: None,
                        created_at: None,
                        updated_at: None,
                        exact_match: name_lower == term,
                        local_version: Some(dep.req.clone()),
                        installed_version: None,
                    });
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
            .await;

        match result {
            Ok(sr) => {
                let results = sr
                    .crates
                    .into_iter()
                    .map(|c| Crate {
                        id: c.id,
                        name: c.name,
                        description: c.description,
                        homepage: c.homepage,
                        documentation: c.documentation,
                        repository: c.repository,
                        version: c
                            .max_stable_version
                            .clone()
                            .unwrap_or(c.max_version.clone()),
                        max_version: Some(c.max_version),
                        max_stable_version: c.max_stable_version,
                        downloads: Some(c.downloads),
                        recent_downloads: c.recent_downloads,
                        created_at: Some(c.created_at),
                        updated_at: Some(c.updated_at),
                        exact_match: c.exact_match.unwrap_or(false),
                        local_version: None,
                        installed_version: None,
                    })
                    .collect();
                Ok((results, sr.meta.total as usize))
            }
            Err(err) => Err(AppError::Unknown(format!("{:#}", err))),
        }
    }
}
