use crates_io_api::{AsyncClient, CratesQuery};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;

use crate::action::{Action, SearchAction};
use crate::cargo::cargo_env::CargoEnv;
use crate::cargo::metadata::Crate;
use crate::cargo::project::Project;
use crate::components::home::scope_dropdown::Scope;
use crate::components::home::search_results::SearchResults;
use crate::components::home::sort_dropdown::Sort;
use crate::errors::{AppError, AppResult};

#[derive(Debug, Default)]
pub struct SearchOptions {
    pub term: Option<String>,
    pub page: Option<usize>,
    pub per_page: Option<usize>,
    pub sort: Sort,
    pub scope: Scope,
}

pub struct CrateSearchManager {
    crates_io_client: Arc<AsyncClient>,
    cancel_tx: Option<oneshot::Sender<()>>,
    action_tx: UnboundedSender<Action>,
}

impl CrateSearchManager {
    pub fn new(action_tx: UnboundedSender<Action>) -> AppResult<Self> {
        Ok(CrateSearchManager {
            crates_io_client: Arc::new(AsyncClient::new(
                "seekr (github:tareqimbasher/seekr)",
                std::time::Duration::from_millis(1000),
            )?),
            cancel_tx: None,
            action_tx,
        })
    }

    pub fn search(
        &mut self,
        options: SearchOptions,
        cargo_env: Arc<Mutex<CargoEnv>>,
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

            let cargo_env = cargo_env.lock().await;
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
            // todo optimize
            for cr in &mut search_results.crates {
                if !cr.is_local {
                    if let Some(proj) = &cargo_env.project {
                        cr.is_local = proj.contains_package(&cr.name);
                    }
                }

                if !cr.is_installed {
                    cr.is_installed = cargo_env.is_installed(&cr.name);
                }
            }

            tx.send(Action::Search(SearchAction::Render(search_results)))
                .ok();
        })
    }

    fn search_binaries(term: &str, cargo_env: &CargoEnv) -> Vec<Crate> {
        let mut results: Vec<Crate> = Vec::new();

        for package in &cargo_env.installed {
            if package.name.to_lowercase().contains(&term) {
                results.push(Crate {
                    id: package.name.clone(),
                    name: package.name.clone(),
                    description: None,
                    homepage: None,
                    documentation: None,
                    repository: None,
                    max_version: package.version.clone(),
                    max_stable_version: None,
                    downloads: None,
                    recent_downloads: None,
                    created_at: None,
                    updated_at: None,
                    exact_match: package.name.to_lowercase() == term,
                    is_local: false,
                    is_installed: true,
                });
            }
        }

        results
        //.sort_by(|a, b| a.id.cmp(&b.id));
    }

    fn search_project(term: &str, project: &Project) -> Vec<Crate> {
        let mut results: Vec<Crate> = Vec::new();

        for package in project.packages.iter() {
            for dep in package.dependencies.iter() {
                if dep.name.to_lowercase().contains(&term) {
                    results.push(Crate {
                        id: dep.name.clone(),
                        name: dep.name.clone(),
                        description: None,
                        homepage: None,
                        documentation: None,
                        repository: None,
                        max_version: dep.req.clone(),
                        max_stable_version: None,
                        downloads: None,
                        recent_downloads: None,
                        created_at: None,
                        updated_at: None,
                        exact_match: dep.name.to_lowercase() == term,
                        is_local: true,
                        is_installed: false,
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
                let results = &mut sr
                    .crates
                    .iter()
                    .map(|c| Crate {
                        id: c.id.clone(),
                        name: c.name.to_string(),
                        description: c.description.clone(),
                        homepage: c.homepage.clone(),
                        documentation: c.documentation.clone(),
                        repository: c.repository.clone(),
                        max_version: c.max_version.to_string(),
                        max_stable_version: c.max_stable_version.clone(),
                        downloads: Some(c.downloads),
                        recent_downloads: c.recent_downloads,
                        created_at: Some(c.created_at),
                        updated_at: Some(c.updated_at),
                        exact_match: c.exact_match.unwrap_or(false),
                        is_local: false,
                        is_installed: false,
                    })
                    .collect::<Vec<_>>();
                Ok((results.to_vec(), sr.meta.total as usize))
            }
            Err(err) => Err(AppError::Unknown(format!("{:#}", err))),
        }
    }
}
