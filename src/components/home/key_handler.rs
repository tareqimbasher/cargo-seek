use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tui_input::backend::crossterm::EventHandler;

use crate::action::Action;
use crate::cargo::CargoAction;
use crate::components::Component;
use crate::components::home::focusable::is_results_or_details_focused;
use crate::components::home::{Focusable, Home, HomeAction, SearchAction};
use crate::errors::AppResult;
use crate::search::Crate;

pub fn handle_key(home: &mut Home, key: KeyEvent) -> AppResult<Option<Action>> {
    if let Some(action) = handle_global_shortcuts(home, key)? {
        return Ok(Some(action));
    }

    match home.focused {
        Focusable::Search => handle_search_focus(home, key),
        Focusable::Results => handle_results_focus(home, key),
        Focusable::Sort => handle_sort_focus(home, key),
        Focusable::Scope => handle_scope_focus(home, key),
        _ => Ok(None),
    }
}

fn handle_global_shortcuts(home: &mut Home, key: KeyEvent) -> AppResult<Option<Action>> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    if let Some(_) = get_focused_crate(home)
        && ctrl
        && key.code == KeyCode::Char('d')
    {
        return Ok(Some(Action::Home(HomeAction::OpenDocs)));
    }

    match key.code {
        KeyCode::Char('h') if ctrl && home.search_results.is_some() => {
            return Ok(Some(Action::Home(HomeAction::ToggleUsage)));
        }
        KeyCode::Esc => {
            return if home.focused == Focusable::Search {
                Ok(Some(Action::Home(HomeAction::Search(SearchAction::Clear))))
            } else {
                Ok(Some(Action::Home(HomeAction::Focus(Focusable::Search))))
            };
        }
        KeyCode::Char('s') if ctrl => {
            return Ok(Some(Action::Home(HomeAction::Focus(
                if home.focused == Focusable::Sort {
                    Focusable::Search
                } else {
                    Focusable::Sort
                },
            ))));
        }
        KeyCode::Char('a') if ctrl => {
            return Ok(Some(Action::Home(HomeAction::Focus(
                if home.focused == Focusable::Scope {
                    Focusable::Search
                } else {
                    Focusable::Scope
                },
            ))));
        }
        KeyCode::Char('/') => {
            return Ok(Some(Action::Home(HomeAction::Focus(Focusable::Search))));
        }
        KeyCode::BackTab => {
            return Ok(Some(Action::Home(HomeAction::FocusPrevious)));
        }
        KeyCode::Tab => {
            return Ok(Some(Action::Home(HomeAction::FocusNext)));
        }
        KeyCode::Enter => match home.focused {
            Focusable::Search => {
                return Ok(Some(Action::Home(HomeAction::Search(
                    SearchAction::Search {
                        term: home.input.value().to_string(),
                        page: 1,
                        hide_usage: true,
                        status: None,
                    },
                ))));
            }
            Focusable::Results => {}
            Focusable::DocsButton => {
                return Ok(Some(Action::Home(HomeAction::OpenDocs)));
            }
            Focusable::RepositoryButton => {
                return Ok(Some(Action::Home(HomeAction::OpenReadme)));
            }
            Focusable::CratesIoButton => {
                return Ok(Some(Action::Home(HomeAction::OpenCratesIo)));
            }
            Focusable::LibRsButton => {
                return Ok(Some(Action::Home(HomeAction::OpenLibRs)));
            }
            _ => {}
        },
        KeyCode::Up => match home.focused {
            Focusable::CratesIoButton => {
                return Ok(Some(Action::Home(HomeAction::Focus(Focusable::DocsButton))));
            }
            Focusable::LibRsButton => {
                return Ok(Some(Action::Home(HomeAction::Focus(
                    Focusable::RepositoryButton,
                ))));
            }
            Focusable::Usage => {
                if home.vertical_usage_scroll > 0 {
                    home.vertical_usage_scroll -= 1;
                }
            }
            _ => {}
        },
        KeyCode::Down => match home.focused {
            Focusable::DocsButton => {
                return Ok(Some(Action::Home(HomeAction::Focus(
                    Focusable::CratesIoButton,
                ))));
            }
            Focusable::RepositoryButton => {
                return Ok(Some(Action::Home(HomeAction::Focus(
                    Focusable::LibRsButton,
                ))));
            }
            Focusable::Usage => {
                if home.vertical_usage_scroll < 21 {
                    home.vertical_usage_scroll += 1;
                }
            }
            _ => {}
        },
        KeyCode::Left => {
            if ctrl && home.left_column_width_percent >= 10 {
                home.left_column_width_percent -= 10;
                return Ok(None);
            }
            match home.focused {
                Focusable::RepositoryButton => {
                    return Ok(Some(Action::Home(HomeAction::Focus(Focusable::DocsButton))));
                }
                Focusable::LibRsButton => {
                    return Ok(Some(Action::Home(HomeAction::Focus(
                        Focusable::CratesIoButton,
                    ))));
                }
                _ => {}
            }
        }
        KeyCode::Right => {
            if ctrl && home.left_column_width_percent <= 90 {
                home.left_column_width_percent += 10;
                return Ok(None);
            }
            match home.focused {
                Focusable::DocsButton => {
                    return Ok(Some(Action::Home(HomeAction::Focus(
                        Focusable::RepositoryButton,
                    ))));
                }
                Focusable::CratesIoButton => {
                    return Ok(Some(Action::Home(HomeAction::Focus(
                        Focusable::LibRsButton,
                    ))));
                }
                _ => {}
            }
        }
        KeyCode::Char('a') => {
            if is_results_or_details_focused(&home.focused)
                && let Some(search_results) = home.search_results.as_ref()
                && let Some(selected) = search_results.selected()
            {
                return Ok(Some(Action::Cargo(CargoAction::Add {
                    name: selected.name.clone(),
                    version: selected.version.clone(),
                })));
            }
        }
        KeyCode::Char('r') => {
            if let Some(selected) = get_focused_crate(home) {
                return Ok(Some(Action::Cargo(CargoAction::Remove(
                    selected.name.clone(),
                ))));
            }
        }
        KeyCode::Char('i') => {
            if let Some(selected) = get_focused_crate(home) {
                return Ok(Some(Action::Cargo(CargoAction::Install {
                    name: selected.name.clone(),
                    version: selected.version.clone(),
                })));
            }
        }
        KeyCode::Char('u') => {
            if let Some(selected) = get_focused_crate(home) {
                return Ok(Some(Action::Cargo(CargoAction::Uninstall(
                    selected.name.clone(),
                ))));
            }
        }
        _ => {}
    }

    Ok(None)
}

fn handle_search_focus(home: &mut Home, key: KeyEvent) -> AppResult<Option<Action>> {
    match key.code {
        KeyCode::Down => {
            if home.search_results.is_some() {
                return Ok(Some(Action::Home(HomeAction::Focus(Focusable::Results))));
            }
        }
        _ => {
            // Send to input box
            home.input.handle_event(&crossterm::event::Event::Key(key));
        }
    }
    Ok(None)
}

fn handle_results_focus(home: &mut Home, key: KeyEvent) -> AppResult<Option<Action>> {
    if let Some(results) = &home.search_results {
        if results.crates.is_empty() {
            return Ok(None);
        }

        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

        match key.code {
            // List navigation
            KeyCode::Up => {
                if let Some(selected_ix) = results.selected_index()
                    && selected_ix == 0
                {
                    return Ok(Some(Action::Home(HomeAction::Focus(Focusable::Search))));
                }

                return Ok(Some(Action::Home(HomeAction::Search(
                    SearchAction::SelectPrev,
                ))));
            }
            KeyCode::Down => {
                return Ok(Some(Action::Home(HomeAction::Search(
                    SearchAction::SelectNext,
                ))));
            }
            KeyCode::Home if !ctrl => {
                return Ok(Some(Action::Home(HomeAction::Search(
                    SearchAction::SelectFirst,
                ))));
            }
            KeyCode::End if !ctrl => {
                return Ok(Some(Action::Home(HomeAction::Search(
                    SearchAction::SelectLast,
                ))));
            }
            // Page navigation
            KeyCode::Right if results.has_next_page() => {
                let pages = 1;
                return Ok(Some(Action::Home(HomeAction::Search(
                    SearchAction::NavPagesForward(pages),
                ))));
            }
            KeyCode::Left if results.has_prev_page() => {
                let pages = 1;
                return Ok(Some(Action::Home(HomeAction::Search(
                    SearchAction::NavPagesBack(pages),
                ))));
            }
            KeyCode::Home if ctrl => {
                return Ok(Some(Action::Home(HomeAction::Search(
                    SearchAction::NavFirstPage,
                ))));
            }
            KeyCode::End if ctrl => {
                return Ok(Some(Action::Home(HomeAction::Search(
                    SearchAction::NavLastPage,
                ))));
            }
            _ => {}
        }
    }

    Ok(None)
}

fn handle_sort_focus(home: &mut Home, key: KeyEvent) -> AppResult<Option<Action>> {
    if let Some(action) = home.sort_dropdown.handle_key_event(key)? {
        Ok(Some(action))
    } else {
        Ok(None)
    }
}

fn handle_scope_focus(home: &mut Home, key: KeyEvent) -> AppResult<Option<Action>> {
    if let Some(action) = home.scope_dropdown.handle_key_event(key)? {
        Ok(Some(action))
    } else {
        Ok(None)
    }
}

fn get_focused_crate(home: &mut Home) -> Option<&Crate> {
    if is_results_or_details_focused(&home.focused)
        && let Some(search_results) = home.search_results.as_ref()
        && let Some(selected) = search_results.selected()
    {
        Some(selected)
    } else {
        None
    }
}
