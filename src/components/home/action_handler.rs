use reqwest::Url;
use std::sync::Arc;

use crate::action::Action;
use crate::cargo::CargoEvent;
use crate::components::home::cargo_request::{
    FeatureStep, PendingCargoRequest, decide_feature_step,
};
use crate::components::home::focusable::Focusable;
use crate::components::home::overlay::Overlay;
use crate::components::home::{Home, HomeCommand};
use crate::components::status_bar::{StatusCommand, StatusDuration, StatusLevel};
use crate::errors::AppResult;
use crate::search::{DEFAULT_PER_PAGE, SearchCommand, SearchEvent, SearchOptions};
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
                home.focused = *focusable;
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
                    // Help isn't a Tab stop when it's hidden.
                    while next == Focusable::Help {
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
                    // Help isn't a Tab stop when it's hidden.
                    while prev == Focusable::Help {
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
            HomeCommand::BeginCargoRequest(intent) => {
                let step = decide_feature_step(home.get_focused_crate(), &home.config, *intent);
                if let Some(step) = step {
                    apply_feature_step(home, step)?;
                }
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

            let scope = home.scope.clone();
            let sort = home.sort.clone();

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
            home.sort = sort.clone();
            home.action_tx
                .send(Action::Home(HomeCommand::Focus(Focusable::Search)))?;

            if home.search_results.is_some() {
                home.action_tx.send(Action::Search(SearchCommand::Run {
                    term: home.input.value().into(),
                    page: 1,
                    hide_help: false,
                    status: Some(format!("Sorting by: {sort}")),
                }))?;
            }
        }
        SearchCommand::Scope(scope) => {
            home.scope = scope.clone();
            home.action_tx
                .send(Action::Home(HomeCommand::Focus(Focusable::Search)))?;

            if home.search_results.is_some() {
                home.action_tx.send(Action::Search(SearchCommand::Run {
                    term: home.input.value().into(),
                    page: 1,
                    hide_help: false,
                    status: Some(format!("Scoped to: {scope}")),
                }))?;
            }
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
        SearchCommand::SelectIndex(index) => {
            if let Some(results) = home.search_results.as_mut() {
                results.select_index(*index);
            }
            home.on_selection_changed();
        }
        SearchCommand::SelectNext => {
            if let Some(results) = home.search_results.as_mut() {
                results.select_next();
            }
            home.on_selection_changed();
        }
        SearchCommand::SelectPrev => {
            if let Some(results) = home.search_results.as_mut() {
                results.select_previous();
            }
            home.on_selection_changed();
        }
        SearchCommand::SelectFirst => {
            if let Some(results) = home.search_results.as_mut() {
                results.select_first();
            }
            home.on_selection_changed();
        }
        SearchCommand::SelectLast => {
            if let Some(results) = home.search_results.as_mut() {
                results.select_last();
            }
            home.on_selection_changed();
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

            home.search_results = Some(results);
            home.on_selection_changed();

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
        SearchEvent::MetadataLoaded { response } => {
            if let Some(results) = home.search_results.as_mut() {
                results.hydrate_selected(response);
            }

            // Resolve a deferred request only when this load is for the crate it was waiting on.
            let awaited = home
                .pending_cargo_request
                .as_ref()
                .is_some_and(|pending| pending.crate_name == response.crate_data.name);
            if awaited {
                let intent = home
                    .pending_cargo_request
                    .take()
                    .expect("pending feature present per `awaited`")
                    .intent;

                home.action_tx
                    .send(Action::Status(StatusCommand::ResetStatus))
                    .ok();

                // Drop the request if an overlay opened while it loaded (e.g. a sort/scope dropdown);
                // popping the picker over it would replace something the user is interacting with.
                let step = if home.overlay.is_none() {
                    decide_feature_step(home.get_focused_crate(), &home.config, intent)
                } else {
                    None
                };
                if let Some(step) = step {
                    apply_feature_step(home, step)?;
                }
            }
        }
        SearchEvent::MetadataFailed { name, message } => {
            // If we were waiting on this crate's features, drop the request and say so.
            // Otherwise, it was a passive prefetch, so report it as a details-loading failure.
            let waiting_on_features = home
                .pending_cargo_request
                .as_ref()
                .is_some_and(|pending| pending.crate_name == *name);
            let status = if waiting_on_features {
                home.pending_cargo_request = None;
                format!("Couldn't load features for {name}")
            } else {
                format!("Couldn't load details for {name}: {message}")
            };
            home.action_tx
                .send(Action::Status(StatusCommand::UpdateStatusWithDuration(
                    StatusLevel::Error,
                    StatusDuration::Short,
                    status,
                )))
                .ok();
        }
    }
    Ok(None)
}

/// Acts on a [`FeatureStep`].
fn apply_feature_step(home: &mut Home, step: FeatureStep) -> AppResult<()> {
    match step {
        FeatureStep::Pick(selector) => {
            home.overlay = Some(Overlay::Features(*selector));
        }
        FeatureStep::Run(action) => {
            home.action_tx.send(action)?;
        }
        FeatureStep::AwaitMetadata { intent, name } => {
            home.pending_cargo_request = Some(PendingCargoRequest {
                intent,
                crate_name: name.clone(),
            });
            home.crate_search_manager
                .start_metadata_load(&name, false)
                .ok();
            home.action_tx
                .send(Action::Status(StatusCommand::UpdateStatus(
                    StatusLevel::Progress,
                    format!("Loading features for {name}…"),
                )))?;
        }
    }
    Ok(())
}
