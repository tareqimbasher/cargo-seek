use reqwest::Url;
use std::sync::Arc;
use std::{fs, io::Write, process::Command};

use crate::action::{Action, CargoAction, SearchAction};
use crate::components::home::enums::Focusable;
use crate::components::home::Home;
use crate::components::status_bar::{StatusDuration, StatusLevel};
use crate::errors::AppResult;
use crate::search::SearchOptions;
use crate::tui::Tui;

pub fn handle_action(home: &mut Home, action: Action, tui: &mut Tui) -> AppResult<Option<Action>> {
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
            let mut next = home.focused.next();
            while next == Focusable::Sort || next == Focusable::Scope {
                next = next.next();
            }

            return Ok(Some(Action::Focus(next)));
        }
        Action::FocusPrevious => {
            let mut prev = home.focused.prev();
            while prev == Focusable::Sort || prev == Focusable::Scope {
                prev = prev.prev();
            }

            return Ok(Some(Action::Focus(prev)));
        }
        Action::ToggleUsage => {
            home.show_usage = !home.show_usage;
        }
        Action::Search(action) => match action {
            SearchAction::Clear => home.reset()?,
            SearchAction::Search(term, sort, page, status) => {
                let tx = home.action_tx.clone();

                let status = status.unwrap_or("Searching".into());
                tx.send(Action::UpdateStatus(
                    StatusLevel::Progress,
                    status.to_string(),
                ))?;

                home.is_searching = true;
                home.crate_search_manager.search(
                    SearchOptions {
                        term: Some(term),
                        sort,
                        page: Some(page),
                        per_page: Some(100),
                        scope: home.scope.clone(),
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
                    sort,
                    1,
                    Some(status),
                ))));
            }
            SearchAction::Scope(scope) => {
                home.action_tx.send(Action::Focus(Focusable::Search))?;

                home.scope = scope;

                if home.search_results.is_none() {
                    return Ok(None);
                }

                return Ok(Some(Action::Search(SearchAction::Search(
                    home.input.value().into(),
                    home.sort_dropdown.get_selected(),
                    1,
                    Some(format!("Scoped to: {}", home.scope)),
                ))));
            }
            SearchAction::Render(mut results) => {
                home.is_searching = false;

                let results_len = results.current_page_count();

                let exact_match_ix = results.crates.iter().position(|c| c.exact_match);

                if exact_match_ix.is_some() {
                    results.select_index(exact_match_ix);
                } else if results_len > 0 {
                    results.select_index(Some(0));
                }

                home.search_results = Some(results);
                home.show_usage = false;

                if results_len > 0 {
                    home.action_tx.send(Action::UpdateStatusWithDuration(
                        StatusLevel::Success,
                        StatusDuration::Short,
                        format!("Loaded {results_len} results"),
                    ))?;
                }
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
                        }
                        SearchAction::SelectNext => {
                            results.select_next();
                        }
                        SearchAction::SelectPrev => {
                            results.select_previous();
                        }
                        SearchAction::SelectFirst => {
                            results.select_first();
                        }
                        SearchAction::SelectLast => {
                            results.select_last();
                        }
                        _ => {}
                    }
                }
            }
        },
        Action::Cargo(action) => return match action {
            CargoAction::Add(crate_name, version) => {
                let _ = crate_name;
                let _ = version;
                Ok(Some(Action::RefreshCargoEnv))
            }
            CargoAction::Remove(crate_name) => {
                let _ = crate_name;
                Ok(Some(Action::RefreshCargoEnv))
            }
            CargoAction::Update(crate_name) => {
                let _ = crate_name;
                Ok(Some(Action::RefreshCargoEnv))
            }
            CargoAction::UpdateAll => {
                Ok(Some(Action::RefreshCargoEnv))
            }
        },
        Action::OpenReadme => {
            // TODO setting if open in browser or cli
            if let Some(url) = home
                .search_results
                .as_ref()
                .and_then(|results| results.get_selected())
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
                .and_then(|results| results.get_selected())
                .and_then(|krate| krate.documentation.as_ref())
                .and_then(|docs| Url::parse(docs).ok())
            {
                open::that(url.to_string())?;
            }
        }
        _ => {}
    }
    Ok(None)
}
