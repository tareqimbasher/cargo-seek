use crates_io_api::{AsyncClient, CratesQuery};
use indexmap::IndexMap;
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

    pub fn search(
        &mut self,
        options: SearchOptions,
        cargo_env: Arc<RwLock<CargoEnv>>,
    ) -> JoinHandle<()> {
        // Cancel previous search
        if let Some(cancel_search_tx) = self.cancel_search_tx.take() {
            let _ = cancel_search_tx.send(());
        }

        let (cancel_search_tx, mut cancel_search_rx) = oneshot::channel();
        self.cancel_search_tx = Some(cancel_search_tx);
        let tx = self.action_tx.clone();
        let crates_io_client = self.crates_io_client.clone();

        tokio::spawn(async move {
            let mut cancelled = || cancel_search_rx.try_recv().is_ok();

            if cancelled() {
                return;
            }

            let cargo_env = cargo_env.read().await;

            let term = options.term.unwrap_or_default().to_lowercase();
            let search_all = options.scope == Scope::All;
            let page = options.page.unwrap_or(1);
            let per_page = options.per_page.unwrap_or(100);
            let mut still_needed = per_page;
            let mut search_results = SearchResults::new(page);

            // Search crates added to the current project
            if search_all || options.scope == Scope::Project {
                if let Some(project) = &cargo_env.project {
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
            }

            if cancelled() {
                return;
            }

            // Search globally installed binaries
            if search_all || options.scope == Scope::Installed {
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

            if cancelled() {
                return;
            }

            // Search the online registry
            if search_all || options.scope == Scope::Online {
                match Self::search_registry(
                    crates_io_client,
                    &term,
                    still_needed,
                    page,
                    options.sort,
                )
                .await
                {
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
                        let _ = tx.send(Action::Search(SearchAction::Error(format!("{err:#}"))));
                        return;
                    }
                }
            }

            if cancelled() {
                return;
            }

            Self::update_results(&mut search_results, &cargo_env);

            tx.send(Action::Search(SearchAction::Render(search_results)))
                .ok();
        })
    }

    fn search_binaries(term: &str, cargo_env: &CargoEnv) -> Vec<Crate> {
        let mut results: Vec<Crate> = Vec::new();

        for bin in &cargo_env.installed_binaries {
            let name_lower = bin.name.to_lowercase();
            if name_lower.contains(term) {
                results.push(Crate {
                    id: bin.name.clone(),
                    name: bin.name.clone(),
                    description: None,
                    homepage: None,
                    documentation: None,
                    repository: None,
                    version: bin.version.clone(),
                    max_version: None,
                    max_stable_version: None,
                    downloads: None,
                    recent_downloads: None,
                    created_at: None,
                    updated_at: None,
                    exact_match: name_lower == term,
                    project_version: None,
                    installed_version: Some(bin.version.clone()),
                });
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
                        project_version: Some(dep.req.clone()),
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
            .await?;

        let results = result
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
                project_version: None,
                installed_version: None,
            })
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

    pub fn get_crate_data(&mut self, name: &str) -> AppResult<()> {
        if let Some(cancel_hydrate_tx) = self.cancel_hydrate_tx.take() {
            let _ = cancel_hydrate_tx.send(());
        }

        let (cancel_hydrate_tx, mut cancel_hydrate_rx) = oneshot::channel();
        self.cancel_hydrate_tx = Some(cancel_hydrate_tx);
        let tx = self.action_tx.clone();
        let crates_io_client = self.crates_io_client.clone();
        let name = name.to_owned();

        tokio::spawn(async move {
            tokio::time::sleep(Duration::from_millis(700)).await;

            if cancel_hydrate_rx.try_recv().is_ok() {
                return Ok(());
            }

            let response = crates_io_client.get_crate(&name).await?;
            let data = response.crate_data;
            tx.send(Action::CrateMetadataLoaded(Box::new(data)))?;

            Ok::<_, AppError>(())
        });

        Ok(())
    }

    pub fn update_results(search_results: &mut SearchResults, cargo_env: &CargoEnv) {
        Self::deduplicate(search_results);

        // Calculate project_version and installed_version
        for cr in &mut search_results.crates {
            if let Some(proj) = &cargo_env.project {
                cr.project_version = proj.get_local_version(&cr.name);
            }

            cr.installed_version = cargo_env.get_installed_version(&cr.name);
        }
    }

    fn deduplicate(search_results: &mut SearchResults) {
        let mut map = IndexMap::<String, Crate>::new();

        for cr in search_results.crates.drain(0..) {
            if map.get(&cr.id).is_some_and(|v| v.is_metadata_loaded()) {
                continue;
            }
            map.insert(cr.id.clone(), cr);
        }

        search_results.crates = map.into_values().collect();
    }

    pub fn hydrate(data: Box<crates_io_api::Crate>, cr: &mut Crate) {
        cr.name = data.name;
        cr.description = data.description;
        cr.homepage = data.homepage;
        cr.documentation = data.documentation;
        cr.repository = data.repository;
        cr.version = data
            .max_stable_version
            .clone()
            .unwrap_or(data.max_version.clone());
        cr.max_version = Some(data.max_version);
        cr.max_stable_version = data.max_stable_version;
        cr.downloads = Some(data.downloads);
        cr.recent_downloads = data.recent_downloads;
        cr.created_at = Some(data.created_at);
        cr.updated_at = Some(data.updated_at);
        cr.exact_match = data.exact_match.unwrap_or_default();
    }
}
