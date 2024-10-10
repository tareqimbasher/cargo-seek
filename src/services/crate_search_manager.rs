use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::oneshot;
use tokio::task::JoinHandle;

use crate::action::{Action, SearchAction};
use crate::components::home::scope_dropdown::Scope;
use crate::components::home::sort_dropdown::Sort;
use crate::components::home::types::{Crate, Meta, SearchResults};
use crate::components::status_bar::StatusLevel;
use crate::http_client;
use crate::project::Project;

#[derive(Debug, Default)]
pub struct SearchOptions {
    pub term: Option<String>,
    pub page: Option<usize>,
    pub sort: Sort,
    pub scope: Scope,
}

pub struct CrateSearchManager {
    current_task: Option<JoinHandle<()>>,
    cancel_tx: Option<oneshot::Sender<()>>,
    action_tx: UnboundedSender<Action>,
}

impl CrateSearchManager {
    pub fn new(action_tx: UnboundedSender<Action>) -> Self {
        CrateSearchManager {
            current_task: None,
            cancel_tx: None,
            action_tx,
        }
    }

    pub fn search(&mut self, options: SearchOptions, project: &Option<Project>) {
        if let Some(cancel_tx) = self.cancel_tx.take() {
            let _ = cancel_tx.send(());
        }

        let (cancel_tx, mut cancel_rx) = oneshot::channel();
        self.cancel_tx = Some(cancel_tx);
        let tx = self.action_tx.clone();

        let proj = project.clone();

        self.current_task = Some(tokio::spawn(async move {
            if cancel_rx.try_recv().is_ok() {
                return;
            }

            let term = options.term.unwrap_or("".to_string()).to_lowercase();

            let mut search_results = SearchResults::default();

            if options.scope == Scope::All || options.scope == Scope::Project {
                let dependencies: Option<Vec<Crate>> = proj.map(|p| {
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
                                })
                            } else {
                                None
                            }
                        })
                        .collect()
                });

                if let Some(dependencies) = dependencies {
                    search_results.crates = dependencies;
                    search_results.meta = Meta::default();
                    search_results.meta.total_count = search_results.crates.len();
                    search_results.meta.current_page = 1;
                }
            }

            if cancel_rx.try_recv().is_ok() {
                return;
            }

            if options.scope == Scope::All || options.scope == Scope::Online {
                let result = http_client::INSTANCE
                    .search(term, options.sort, 100, options.page.unwrap_or(1))
                    .await;

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
