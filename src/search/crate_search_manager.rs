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

            if cancel_search_rx.try_recv().is_ok() {
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

            if cancel_search_rx.try_recv().is_ok() {
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

            if cancel_search_rx.try_recv().is_ok() {
                return;
            }

            Self::update_results(&mut search_results, &cargo_env);

            tx.send(Action::Search(SearchAction::Render(search_results)))
                .ok();
        })
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
                    project_version: None,
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
            tx.send(Action::CrateDataLoaded(Box::new(data)))?;

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

#[cfg(test)]
mod tests {
    use std::collections::HashMap;

    struct Crate {
        pub id: String,
        pub is_initialized: bool,
    }

    fn deduplicate(crates: Vec<Crate>) -> Vec<Crate> {
        let mut map = HashMap::<String, Crate>::new();

        for cr in crates {
            let existing = map.get(&cr.id);
            if let Some(existing) = existing {
                if existing.is_initialized {
                    continue;
                }
            }
            map.insert(cr.id.clone(), cr);
        }

        map.into_values().collect()
    }

    #[test]
    fn test_deduplicate() {
        let crates = vec![
            Crate {
                id: "1".into(),
                is_initialized: true,
            },
            Crate {
                id: "2".into(),
                is_initialized: true,
            },
            Crate {
                id: "3".into(),
                is_initialized: false,
            },
            Crate {
                id: "3".into(),
                is_initialized: false,
            },
            Crate {
                id: "2".into(),
                is_initialized: false,
            },
            Crate {
                id: "2".into(),
                is_initialized: false,
            },
            Crate {
                id: "1".into(),
                is_initialized: false,
            },
            Crate {
                id: "2".into(),
                is_initialized: false,
            },
            Crate {
                id: "4".into(),
                is_initialized: true,
            },
            Crate {
                id: "4".into(),
                is_initialized: true,
            },
        ];

        let mut crates = deduplicate(crates);
        crates.sort_by(|a, b| a.id.cmp(&b.id));

        assert_eq!(crates.len(), 4);
        assert_eq!(crates[0].id, "1".to_string());
        assert_eq!(crates[0].is_initialized, true);
        assert_eq!(crates[1].id, "2".to_string());
        assert_eq!(crates[1].is_initialized, true);
        assert_eq!(crates[2].id, "3".to_string());
        assert_eq!(crates[2].is_initialized, false);
        assert_eq!(crates[3].id, "4".to_string());
        assert_eq!(crates[3].is_initialized, true);
    }
}
