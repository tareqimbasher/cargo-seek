use crates_io_api::{AsyncClient, CratesQuery};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::{oneshot, Mutex};
use tokio::task::JoinHandle;

use crate::action::{Action, SearchAction};
use crate::cargo::cargo_env::CargoEnv;
use crate::components::home::scope_dropdown::Scope;
use crate::components::home::sort_dropdown::Sort;
use crate::components::home::types::{Crate, Meta, SearchResults};
use crate::components::status_bar::StatusLevel;
use crate::errors::AppResult;
use crate::http_client;

#[derive(Debug, Default)]
pub struct SearchOptions {
    pub term: Option<String>,
    pub page: Option<usize>,
    pub sort: Sort,
    pub scope: Scope,
}

pub struct CrateSearchManager {
    crates_io_client: Arc<AsyncClient>,
    current_task: Option<JoinHandle<()>>,
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
            current_task: None,
            cancel_tx: None,
            action_tx,
        })
    }

    pub fn search(&mut self, options: SearchOptions, cargo_env: Arc<Mutex<CargoEnv>>) {
        if let Some(cancel_tx) = self.cancel_tx.take() {
            let _ = cancel_tx.send(());
        }

        let (cancel_tx, mut cancel_rx) = oneshot::channel();
        self.cancel_tx = Some(cancel_tx);
        let tx = self.action_tx.clone();

        let crates_io_client = self.crates_io_client.clone();

        self.current_task = Some(tokio::spawn(async move {
            if cancel_rx.try_recv().is_ok() {
                return;
            }

            let cargo_env = cargo_env.lock().await;

            let term = options.term.unwrap_or("".to_string()).to_lowercase();

            let mut search_results = SearchResults::default();

            if options.scope == Scope::All || options.scope == Scope::Installed {
                for package in &cargo_env.installed {
                    if !package.name.to_lowercase().contains(&term) {
                        continue;
                    }

                    search_results.crates.push(Crate {
                        id: package.name.clone(),
                        name: package.name.clone(),
                        description: None,
                        homepage: None,
                        documentation: None,
                        repository: None,
                        max_version: package.version.clone(),
                        max_stable_version: None,
                        downloads: 0,
                        recent_downloads: None,
                        created_at: Default::default(),
                        updated_at: Default::default(),
                        exact_match: package.name.to_lowercase() == term,
                        is_local: false,
                        is_installed: true,
                    })
                }
            }

            if options.scope == Scope::All || options.scope == Scope::Project {
                let dependencies: Option<Vec<Crate>> = cargo_env.project.as_ref().map(|p| {
                    p.packages
                        .iter()
                        .flat_map(|pp| pp.dependencies.clone()) // Collect all dependencies into a single iterator
                        .filter_map(|dependency| {
                            if dependency.name.to_lowercase().contains(&term) {
                                Some(Crate {
                                    id: dependency.name.clone(),
                                    name: dependency.name.clone(),
                                    description: None,
                                    homepage: None,
                                    documentation: None,
                                    repository: None,
                                    max_version: dependency.req.clone(),
                                    max_stable_version: None,
                                    downloads: 0,
                                    recent_downloads: None,
                                    created_at: Default::default(),
                                    updated_at: Default::default(),
                                    exact_match: dependency.name.to_lowercase() == term,
                                    is_local: true,
                                    is_installed: false,
                                })
                            } else {
                                None
                            }
                        })
                        .collect()
                });

                if let Some(mut dependencies) = dependencies {
                    search_results.crates.append(&mut dependencies);
                }
            }

            search_results.meta = Meta::default();
            search_results.meta.total_count = search_results.crates.len();
            search_results.meta.current_page = 1;

            if cancel_rx.try_recv().is_ok() {
                return;
            }

            if options.scope == Scope::All || options.scope == Scope::Online {
                let result = http_client::INSTANCE
                    .search(term.clone(), options.sort, 100, options.page.unwrap_or(1))
                    .await;

                // let two = crates_io_client
                //     .crates(
                //         CratesQuery::builder()
                //             .search(term)
                //             .sort(crates_io_api::Sort::Relevance)
                //             .page_size(100)
                //             .page(options.page.unwrap_or(1) as u64)
                //             .build(),
                //     )
                //     .await?;

                if cancel_rx.try_recv().is_ok() {
                    return;
                }

                match result {
                    Ok(mut sr) => {
                        search_results.crates.append(&mut sr.crates);
                        search_results.meta = sr.meta;
                    }
                    Err(err) => {
                        tx.send(Action::UpdateStatus(
                            StatusLevel::Error,
                            format!("{:#}", err),
                        ))
                        .ok();
                    }
                };
            }

            // todo optimize

            for cr in &mut search_results.crates {
                if !cr.is_local {
                    if let Some(proj) = &cargo_env.project {
                        cr.is_local = proj.contains_package(&cr.name);
                    }
                }

                if !cr.is_installed {
                    // cr.is_installed = globally_installed_package_names.contains(&cr.name);
                }
            }

            tx.send(Action::Search(SearchAction::Render(search_results)))
                .ok();
        }));
    }

    #[allow(dead_code)]
    pub async fn wait_for_task_completion(&mut self) {
        if let Some(task) = self.current_task.take() {
            let _ = task.await;
        }
    }
}
