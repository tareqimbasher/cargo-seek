use reqwest::Url;
use std::sync::Arc;
use std::{fs, io::Write, process::Command};

use crate::action::{Action, SearchAction};
use crate::components::home::focusable::Focusable;
use crate::components::home::Home;
use crate::components::status_bar::{StatusDuration, StatusLevel};
use crate::errors::AppResult;
use crate::search::{CrateSearchManager, SearchOptions, SearchResults};
use crate::tui::Tui;

pub async fn handle_action(
    home: &mut Home,
    action: Action,
    tui: &mut Tui,
) -> AppResult<Option<Action>> {
    match action {
        Action::Tick => {
            // add any logic here that should run on every tick
            if home.is_searching {
                home.spinner_state.calc_next();
            }
        }
        Action::Render => {
            // add any logic here that should run on every render
        }
        Action::Focus(focusable) => {
            home.sort_dropdown
                .set_is_focused(focusable == Focusable::Sort);
            home.scope_dropdown
                .set_is_focused(focusable == Focusable::Scope);
            home.focused = focusable;
        }
        Action::FocusNext => {
            let has_search_results = home.search_results.is_some();
            let show_usage = home.show_usage;

            if !has_search_results || show_usage {
                return if home.focused == Focusable::Usage {
                    Ok(Some(Action::Focus(Focusable::Search)))
                } else {
                    Ok(Some(Action::Focus(Focusable::Usage)))
                };
            }

            let mut next = home.focused.next();
            while next == Focusable::Usage || next == Focusable::Sort || next == Focusable::Scope {
                next = next.next();
            }

            return Ok(Some(Action::Focus(next)));
        }
        Action::FocusPrevious => {
            let has_search_results = home.search_results.is_some();
            let show_usage = home.show_usage;

            if !has_search_results || show_usage {
                return if home.focused == Focusable::Usage {
                    Ok(Some(Action::Focus(Focusable::Search)))
                } else {
                    Ok(Some(Action::Focus(Focusable::Usage)))
                };
            }

            let mut prev = home.focused.prev();
            while prev == Focusable::Usage || prev == Focusable::Sort || prev == Focusable::Scope {
                prev = prev.prev();
            }

            if !home.show_usage && prev == Focusable::Usage {
                prev = prev.prev();
            }

            return Ok(Some(Action::Focus(prev)));
        }
        Action::ToggleUsage => {
            let was_showing = home.show_usage;
            home.show_usage = !home.show_usage;
            home.vertical_usage_scroll = 0;
            return if was_showing {
                Ok(Some(Action::Focus(Focusable::Search)))
            } else {
                Ok(Some(Action::Focus(Focusable::Usage)))
            };
        }
        Action::Search(action) => match action {
            SearchAction::Clear => home.reset()?,
            SearchAction::Search(term, page, status) => {
                let tx = home.action_tx.clone();

                let scope = home.scope_dropdown.get_selected();
                let sort = home.sort_dropdown.get_selected();

                let status = status.unwrap_or("Searching".into());
                tx.send(Action::UpdateStatus(
                    StatusLevel::Progress,
                    status.to_string(),
                ))?;

                home.is_searching = true;
                home.crate_search_manager.search(
                    SearchOptions {
                        term: Some(term),
                        scope,
                        sort,
                        page: Some(page),
                        per_page: Some(100),
                    },
                    Arc::clone(&home.cargo_env),
                );

                return Ok(None);
            }
            SearchAction::Error(err) => {
                home.is_searching = false;
                home.action_tx
                    .send(Action::UpdateStatus(StatusLevel::Error, err))
                    .ok();
            }
            SearchAction::SortBy(sort) => {
                home.action_tx.send(Action::Focus(Focusable::Search))?;

                if home.search_results.is_none() {
                    return Ok(None);
                }

                let status = format!("Sorting by: {}", sort);
                return Ok(Some(Action::Search(SearchAction::Search(
                    home.input.value().into(),
                    1,
                    Some(status),
                ))));
            }
            SearchAction::Scope(scope) => {
                home.action_tx.send(Action::Focus(Focusable::Search))?;

                if home.search_results.is_none() {
                    return Ok(None);
                }

                let status = format!("Scoped to: {}", scope);
                return Ok(Some(Action::Search(SearchAction::Search(
                    home.input.value().into(),
                    1,
                    Some(status),
                ))));
            }
            SearchAction::Render(mut results) => {
                home.is_searching = false;

                let results_len = results.current_page_count();

                let exact_match_ix = results.crates.iter().position(|c| c.exact_match);
                if exact_match_ix.is_some() {
                    results.select_index(exact_match_ix);
                    home.action_tx.send(Action::Focus(Focusable::Results))?;
                } else if results_len > 0 {
                    results.select_index(Some(0));
                }
                check_needs_hydrate(&mut results, &mut home.crate_search_manager);

                home.search_results = Some(results);
                home.show_usage = false;

                home.action_tx.send(Action::UpdateStatusWithDuration(
                    StatusLevel::Success,
                    StatusDuration::Short,
                    if results_len > 0 {
                        format!("Loaded {results_len} results")
                    } else {
                        "No results".to_string()
                    },
                ))?;
            }
            SearchAction::NavPagesForward(pages) => {
                home.go_pages_forward(pages, home.input.value().to_string())?;
            }
            SearchAction::NavPagesBack(pages) => {
                home.go_pages_back(pages, home.input.value().to_string())?;
            }
            SearchAction::NavFirstPage => {
                home.go_to_page(1, home.input.value().to_string())?;
            }
            SearchAction::NavLastPage => {
                home.go_to_page(usize::MAX, home.input.value().to_string())?;
            }
            _ => {
                if let Some(results) = home.search_results.as_mut() {
                    match action {
                        SearchAction::SelectIndex(index) => {
                            results.select_index(index);
                            check_needs_hydrate(results, &mut home.crate_search_manager);
                        }
                        SearchAction::SelectNext => {
                            results.select_next();
                            check_needs_hydrate(results, &mut home.crate_search_manager);
                        }
                        SearchAction::SelectPrev => {
                            results.select_previous();
                            check_needs_hydrate(results, &mut home.crate_search_manager);
                        }
                        SearchAction::SelectFirst => {
                            results.select_first();
                            check_needs_hydrate(results, &mut home.crate_search_manager);
                        }
                        SearchAction::SelectLast => {
                            results.select_last();
                            check_needs_hydrate(results, &mut home.crate_search_manager);
                        }
                        _ => {}
                    }
                }
            }
        },
        Action::CargoEnvRefreshed => {
            if let Some(search_results) = &mut home.search_results {
                let cargo_env = home.cargo_env.read().await;
                CrateSearchManager::update_results(search_results, &cargo_env);
            }
        }
        Action::CrateMetadataLoaded(data) => {
            if let Some(results) = home.search_results.as_mut() {
                if let Some(index) = results.selected_index() {
                    let cr = &mut results.crates[index];
                    if cr.id == data.id {
                        CrateSearchManager::hydrate(data, cr);
                    }
                }
            }
        }
        Action::OpenReadme => {
            // TODO setting if open in browser or cli
            if let Some(url) = home
                .search_results
                .as_ref()
                .and_then(|results| results.selected())
                .and_then(|krate| krate.repository.as_ref())
                .and_then(|docs| Url::parse(docs).ok())
            {
                open::that(url.to_string())?;
            }

            // if let Some(url) = self
            //     .search_results
            //     .as_ref()
            //     .and_then(|results| results.get_selected())
            //     .and_then(|krate| krate.repository.as_ref())
            //     .and_then(|docs| Url::parse(docs).ok())
            // {
            //     let tx = home.action_tx.clone();
            //     tokio::spawn(async move {
            //         if let Some(markdown) = http_client::INSTANCE
            //             .get_repo_readme(url.to_string())
            //             .await
            //             .unwrap()
            //         {
            //             tx.send(Action::RenderReadme(markdown)).unwrap();
            //         }
            //     });
            // }
        }
        Action::RenderReadme(markdown) => {
            // TODO if this fails, open in browser

            let mut temp_file = tempfile::NamedTempFile::new()?;
            write!(temp_file, "{}", markdown)?;
            let original_path = temp_file.path().to_path_buf();

            if let Some(parent) = original_path.parent() {
                let new_path = parent.join("rseek_readme_tmp.md");
                fs::rename(&original_path, &new_path)?;

                tui.exit()?;

                // TODO Check if glow doesn't exist use mdcat for example. And if neither exists, open url
                // TODO Windows: dunce
                let mut glow = Command::new("glow").arg("-p").arg(&new_path).spawn()?;

                let _ = glow.wait()?;

                if new_path.exists() {
                    fs::remove_file(new_path).ok();
                }

                tui.enter()?;
                tui.terminal.clear()?;
            } else {
                fs::remove_file(original_path).ok();
            }
        }
        Action::OpenDocs => {
            if let Some(url) = home
                .search_results
                .as_ref()
                .and_then(|results| results.selected())
                .and_then(|krate| krate.documentation.as_ref())
                .and_then(|docs| Url::parse(docs).ok())
            {
                open::that(url.to_string())?;
            }
        }
        Action::OpenCratesIo => {
            if let Some(url) = home
                .search_results
                .as_ref()
                .and_then(|results| results.selected())
                .and_then(|krate| {
                    Url::parse(format!("https://crates.io/crates/{}", krate.id).as_str()).ok()
                })
            {
                open::that(url.to_string())?;
            }
        }
        Action::OpenLibRs => {
            if let Some(url) = home
                .search_results
                .as_ref()
                .and_then(|results| results.selected())
                .and_then(|krate| {
                    Url::parse(format!("https://lib.rs/crates/{}", krate.id).as_str()).ok()
                })
            {
                open::that(url.to_string())?;
            }
        }
        _ => {}
    }
    Ok(None)
}

fn check_needs_hydrate(results: &mut SearchResults, crate_search_manager: &mut CrateSearchManager) {
    if let Some(cr) = results.selected() {
        if !cr.is_metadata_loaded() {
            crate_search_manager.get_crate_data(&cr.name).ok();
        }
    }
}
