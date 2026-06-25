use reqwest::Url;
use std::sync::Arc;

use crate::action::Action;
use crate::cargo::CargoEvent;
use crate::components::home::focusable::Focusable;
use crate::components::home::{Home, HomeCommand};
use crate::components::status_bar::{StatusCommand, StatusDuration, StatusLevel};
use crate::errors::AppResult;
use crate::search::{
    CrateSearchManager, DEFAULT_PER_PAGE, SearchCommand, SearchEvent, SearchOptions, SearchResults,
};
use crate::tui::Tui;

pub async fn handle_action(
    home: &mut Home,
    action: &Action,
    tui: &mut Tui,
) -> AppResult<Option<Action>> {
    let _ = tui;
    match action {
        Action::Tick => {
            if home.is_searching {
                home.spinner_state.calc_next();
            }
        }

        Action::Home(command) => match command {
            HomeCommand::Focus(focusable) => {
                let focusable = focusable.clone();
                home.sort_dropdown
                    .set_is_focused(focusable == Focusable::Sort);
                home.scope_dropdown
                    .set_is_focused(focusable == Focusable::Scope);
                home.focused = focusable;
            }
            HomeCommand::FocusNext => {
                let has_search_results = home.search_results.is_some();
                let show_help = home.show_help;

                if show_help {
                    let next = match home.focused {
                        Focusable::Help => Focusable::Search,
                        Focusable::Search if has_search_results => Focusable::Results,
                        Focusable::Results => Focusable::Help,
                        _ => Focusable::Help,
                    };
                    return Ok(Some(Action::Home(HomeCommand::Focus(next))));
                } else {
                    let mut next = home.focused.next();
                    // Tab focus cycle should skip these elements
                    while next == Focusable::Help
                        || next == Focusable::Sort
                        || next == Focusable::Scope
                    {
                        next = next.next();
                    }
                    return Ok(Some(Action::Home(HomeCommand::Focus(next))));
                }
            }
            HomeCommand::FocusPrevious => {
                let has_search_results = home.search_results.is_some();
                let show_help = home.show_help;

                if show_help {
                    let prev = match home.focused {
                        Focusable::Help if has_search_results => Focusable::Results,
                        Focusable::Search => Focusable::Help,
                        Focusable::Results => Focusable::Search,
                        _ => Focusable::Search,
                    };
                    return Ok(Some(Action::Home(HomeCommand::Focus(prev))));
                } else {
                    let mut prev = home.focused.prev();
                    // Tab focus cycle should skip these elements
                    while prev == Focusable::Help
                        || prev == Focusable::Sort
                        || prev == Focusable::Scope
                    {
                        prev = prev.prev();
                    }

                    if !home.show_help && prev == Focusable::Help {
                        prev = prev.prev();
                    }

                    return Ok(Some(Action::Home(HomeCommand::Focus(prev))));
                }
            }
            HomeCommand::ToggleHelp => {
                let was_showing = home.show_help;
                home.show_help = !home.show_help;
                home.vertical_help_scroll = 0;
                return if was_showing {
                    Ok(Some(Action::Home(HomeCommand::Focus(Focusable::Search))))
                } else {
                    Ok(Some(Action::Home(HomeCommand::Focus(Focusable::Help))))
                };
            }
            HomeCommand::OpenReadme => {
                // TODO setting if open in browser or cli
                if let Some(url) = home
                    .search_results
                    .as_ref()
                    .and_then(|results| results.selected())
                    .and_then(|cr| cr.repository.as_ref())
                    .and_then(|docs| Url::parse(docs).ok())
                {
                    open::that(url.to_string())?;
                }
            }
            HomeCommand::RenderReadme(_) => {
                // TODO: optionally render the README in-terminal (glow/mdcat) instead of
                // opening it in the browser; fall back to the browser if neither exists.
            }
            HomeCommand::OpenDocs => {
                if let Some(url) = home
                    .search_results
                    .as_ref()
                    .and_then(|results| results.selected())
                    .and_then(|cr| cr.documentation.as_ref())
                    .and_then(|docs| Url::parse(docs).ok())
                {
                    open::that(url.to_string())?;
                }
            }
            HomeCommand::OpenCratesIo => {
                if let Some(url) = home
                    .search_results
                    .as_ref()
                    .and_then(|results| results.selected())
                    .and_then(|cr| {
                        Url::parse(format!("https://crates.io/crates/{}", cr.id).as_str()).ok()
                    })
                {
                    open::that(url.to_string())?;
                }
            }
            HomeCommand::OpenLibRs => {
                if let Some(url) = home
                    .search_results
                    .as_ref()
                    .and_then(|results| results.selected())
                    .and_then(|cr| {
                        Url::parse(format!("https://lib.rs/crates/{}", cr.id).as_str()).ok()
                    })
                {
                    open::that(url.to_string())?;
                }
            }
        },

        Action::Search(command) => return handle_search_command(home, command),

        Action::SearchEvent(event) => return handle_search_event(home, event),

        Action::CargoEvent(event) => match event {
            CargoEvent::Refreshed => {
                // Re-annotate the visible results when the cargo environment changes.
                if let Some(search_results) = &mut home.search_results {
                    let cargo_env = home.cargo_env.read().await;
                    search_results.update_results(&cargo_env);
                }
            }
        },
        _ => {}
    }
    Ok(None)
}

fn handle_search_command(home: &mut Home, command: &SearchCommand) -> AppResult<Option<Action>> {
    match command {
        SearchCommand::Clear => home.reset()?,
        SearchCommand::Run {
            term,
            page,
            hide_help,
            status,
        } => {
            let tx = home.action_tx.clone();

            let scope = home.scope_dropdown.get_selected();
            let sort = home.sort_dropdown.get_selected();

            let status = status.clone().unwrap_or_else(|| "Searching".into());
            tx.send(Action::Status(StatusCommand::UpdateStatus(
                StatusLevel::Progress,
                status,
            )))?;

            home.is_searching = true;
            if *hide_help {
                home.show_help = false;
            }

            home.crate_search_manager.search(
                SearchOptions {
                    term: Some(term.clone()),
                    scope,
                    sort,
                    page: Some(*page),
                    per_page: Some(DEFAULT_PER_PAGE),
                },
                Arc::clone(&home.cargo_env),
            );

            return Ok(None);
        }
        SearchCommand::SortBy(sort) => {
            home.action_tx
                .send(Action::Home(HomeCommand::Focus(Focusable::Search)))?;

            if home.search_results.is_none() {
                return Ok(None);
            }

            let status = format!("Sorting by: {sort}");
            return Ok(Some(Action::Search(SearchCommand::Run {
                term: home.input.value().into(),
                page: 1,
                hide_help: false,
                status: Some(status),
            })));
        }
        SearchCommand::Scope(scope) => {
            home.action_tx
                .send(Action::Home(HomeCommand::Focus(Focusable::Search)))?;

            if home.search_results.is_none() {
                return Ok(None);
            }

            let status = format!("Scoped to: {scope}");
            return Ok(Some(Action::Search(SearchCommand::Run {
                term: home.input.value().into(),
                page: 1,
                hide_help: false,
                status: Some(status),
            })));
        }
        SearchCommand::NavPagesForward(pages) => {
            home.go_pages_forward(*pages, home.input.value())?;
        }
        SearchCommand::NavPagesBack(pages) => {
            home.go_pages_back(*pages, home.input.value())?;
        }
        SearchCommand::NavFirstPage => {
            home.go_to_page(1, home.input.value())?;
        }
        SearchCommand::NavLastPage => {
            home.go_to_last_page(home.input.value())?;
        }
        _ => {
            if let Some(results) = home.search_results.as_mut() {
                match command {
                    SearchCommand::SelectIndex(index) => {
                        results.select_index(*index);
                        load_metadata_if_needed(results, &mut home.crate_search_manager);
                    }
                    SearchCommand::SelectNext => {
                        results.select_next();
                        load_metadata_if_needed(results, &mut home.crate_search_manager);
                    }
                    SearchCommand::SelectPrev => {
                        results.select_previous();
                        load_metadata_if_needed(results, &mut home.crate_search_manager);
                    }
                    SearchCommand::SelectFirst => {
                        results.select_first();
                        load_metadata_if_needed(results, &mut home.crate_search_manager);
                    }
                    SearchCommand::SelectLast => {
                        results.select_last();
                        load_metadata_if_needed(results, &mut home.crate_search_manager);
                    }
                    _ => {}
                }
            }
        }
    }
    Ok(None)
}

fn handle_search_event(home: &mut Home, event: &SearchEvent) -> AppResult<Option<Action>> {
    match event {
        SearchEvent::Completed(results) => {
            let mut results = results.clone();
            home.is_searching = false;

            let results_len = results.current_page_len();

            let exact_match_ix = results.crates.iter().position(|c| c.exact_match);
            if exact_match_ix.is_some() {
                results.select_index(exact_match_ix);
                home.action_tx
                    .send(Action::Home(HomeCommand::Focus(Focusable::Results)))?;
            } else if results_len > 0 {
                results.select_index(Some(0));
            }
            load_metadata_if_needed(&mut results, &mut home.crate_search_manager);

            home.search_results = Some(results);

            home.action_tx
                .send(Action::Status(StatusCommand::UpdateStatusWithDuration(
                    StatusLevel::Success,
                    StatusDuration::Short,
                    if results_len > 0 {
                        format!("Loaded {results_len} results")
                    } else {
                        "No results".to_string()
                    },
                )))?;
        }
        SearchEvent::Failed(err) => {
            home.is_searching = false;
            home.action_tx
                .send(Action::Status(StatusCommand::UpdateStatus(
                    StatusLevel::Error,
                    err.clone(),
                )))
                .ok();
        }
        SearchEvent::MetadataLoaded(data) => {
            if let Some(results) = home.search_results.as_mut()
                && let Some(index) = results.selected_index()
            {
                let cr = &mut results.crates[index];
                if cr.id == data.crate_data.id {
                    cr.hydrate(data.clone());
                }
            }
        }
        SearchEvent::MetadataFailed { name, message } => {
            home.action_tx
                .send(Action::Status(StatusCommand::UpdateStatusWithDuration(
                    StatusLevel::Error,
                    StatusDuration::Short,
                    format!("Couldn't load details for {name}: {message}"),
                )))
                .ok();
        }
    }
    Ok(None)
}

fn load_metadata_if_needed(
    results: &mut SearchResults,
    crate_search_manager: &mut CrateSearchManager,
) {
    if let Some(cr) = results.selected()
        && !cr.is_metadata_loaded()
    {
        crate_search_manager.load_crate_metadata(&cr.name).ok();
    }
}
