use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tui_input::backend::crossterm::EventHandler;

use crate::action::{Action, CargoAction, SearchAction};
use crate::components::home::focusable::{is_results_or_details_focused, Focusable};
use crate::components::home::Home;
use crate::components::Component;
use crate::errors::AppResult;

pub fn handle_key(home: &mut Home, key: KeyEvent) -> AppResult<Option<Action>> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    // Try match key combos that should be handled regardless what is focused
    match key.code {
        KeyCode::Char('h') if ctrl && home.search_results.is_some() => {
            return Ok(Some(Action::ToggleUsage));
        }
        KeyCode::Esc => {
            return if home.focused == Focusable::Search {
                Ok(Some(Action::Search(SearchAction::Clear)))
            } else {
                Ok(Some(Action::Focus(Focusable::Search)))
            }
        }
        KeyCode::Char('s') if ctrl => {
            return Ok(Some(Action::Focus(if home.focused == Focusable::Sort {
                Focusable::Search
            } else {
                Focusable::Sort
            })));
        }
        KeyCode::Char('a') if ctrl => {
            return Ok(Some(Action::Focus(if home.focused == Focusable::Scope {
                Focusable::Search
            } else {
                Focusable::Scope
            })));
        }
        KeyCode::Char('/') => {
            return Ok(Some(Action::Focus(Focusable::Search)));
        }
        KeyCode::BackTab => {
            return Ok(Some(Action::FocusPrevious));
        }
        KeyCode::Tab => {
            return Ok(Some(Action::FocusNext));
        }
        KeyCode::Enter => match home.focused {
            Focusable::Search => {
                return Ok(Some(Action::Search(SearchAction::Search(
                    home.input.value().to_string(),
                    1,
                    true,
                    None,
                ))));
            }
            Focusable::Results => {}
            Focusable::DocsButton => {
                return Ok(Some(Action::OpenDocs));
            }
            Focusable::RepositoryButton => {
                return Ok(Some(Action::OpenReadme));
            }
            Focusable::CratesIoButton => {
                return Ok(Some(Action::OpenCratesIo));
            }
            Focusable::LibRsButton => {
                return Ok(Some(Action::OpenLibRs));
            }
            _ => {}
        },
        KeyCode::Up => match home.focused {
            Focusable::CratesIoButton => {
                return Ok(Some(Action::Focus(Focusable::DocsButton)));
            }
            Focusable::LibRsButton => {
                return Ok(Some(Action::Focus(Focusable::RepositoryButton)));
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
                return Ok(Some(Action::Focus(Focusable::CratesIoButton)));
            }
            Focusable::RepositoryButton => {
                return Ok(Some(Action::Focus(Focusable::LibRsButton)));
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
            } else {
                match home.focused {
                    Focusable::RepositoryButton => {
                        return Ok(Some(Action::Focus(Focusable::DocsButton)));
                    }
                    Focusable::LibRsButton => {
                        return Ok(Some(Action::Focus(Focusable::CratesIoButton)));
                    }
                    _ => {}
                }
            }
        }
        KeyCode::Right => {
            if ctrl && home.left_column_width_percent <= 90 {
                home.left_column_width_percent += 10;
                return Ok(None);
            } else {
                match home.focused {
                    Focusable::DocsButton => {
                        return Ok(Some(Action::Focus(Focusable::RepositoryButton)));
                    }
                    Focusable::CratesIoButton => {
                        return Ok(Some(Action::Focus(Focusable::LibRsButton)));
                    }
                    _ => {}
                }
            }
        }
        KeyCode::Char('a') => {
            if is_results_or_details_focused(&home.focused) {
                if let Some(search_results) = home.search_results.as_ref() {
                    if let Some(selected) = search_results.selected() {
                        return Ok(Some(Action::Cargo(CargoAction::Add(
                            selected.name.clone(),
                            selected.version.clone(),
                        ))));
                    }
                }
            }
        }
        KeyCode::Char('r') => {
            if is_results_or_details_focused(&home.focused) {
                if let Some(search_results) = home.search_results.as_ref() {
                    if let Some(selected) = search_results.selected() {
                        return Ok(Some(Action::Cargo(CargoAction::Remove(
                            selected.name.clone(),
                        ))));
                    }
                }
            }
        }
        KeyCode::Char('i') => {
            if is_results_or_details_focused(&home.focused) {
                if let Some(search_results) = home.search_results.as_ref() {
                    if let Some(selected) = search_results.selected() {
                        return Ok(Some(Action::Cargo(CargoAction::Install(
                            selected.name.clone(),
                            selected.version.clone(),
                        ))));
                    }
                }
            }
        }
        KeyCode::Char('u') => {
            if is_results_or_details_focused(&home.focused) {
                if let Some(search_results) = home.search_results.as_ref() {
                    if let Some(selected) = search_results.selected() {
                        return Ok(Some(Action::Cargo(CargoAction::Uninstall(
                            selected.name.clone(),
                        ))));
                    }
                }
            }
        }
        _ => {}
    }

    if home.focused == Focusable::Search {
        match key.code {
            KeyCode::Down => {
                if home.search_results.is_some() {
                    return Ok(Some(Action::Focus(Focusable::Results)));
                }
            }
            _ => {
                // Send to input box
                home.input.handle_event(&crossterm::event::Event::Key(key));
            }
        }
    }

    if home.focused == Focusable::Results {
        if let Some(results) = &home.search_results {
            if results.crates.is_empty() {
                return Ok(None);
            }

            match key.code {
                // List navigation
                KeyCode::Up => {
                    if let Some(selected_ix) = results.selected_index() {
                        if selected_ix == 0 {
                            return Ok(Some(Action::Focus(Focusable::Search)));
                        }
                    }

                    return Ok(Some(Action::Search(SearchAction::SelectPrev)));
                }
                KeyCode::Down => {
                    return Ok(Some(Action::Search(SearchAction::SelectNext)));
                }
                KeyCode::Home if !ctrl => {
                    return Ok(Some(Action::Search(SearchAction::SelectFirst)));
                }
                KeyCode::End if !ctrl => {
                    return Ok(Some(Action::Search(SearchAction::SelectLast)));
                }
                // Page navigation
                KeyCode::Right if results.has_next_page() => {
                    let pages = 1;
                    return Ok(Some(Action::Search(SearchAction::NavPagesForward(pages))));
                }
                KeyCode::Left if results.has_prev_page() => {
                    let pages = 1;
                    return Ok(Some(Action::Search(SearchAction::NavPagesBack(pages))));
                }
                KeyCode::Home if ctrl => {
                    return Ok(Some(Action::Search(SearchAction::NavFirstPage)));
                }
                KeyCode::End if ctrl => {
                    return Ok(Some(Action::Search(SearchAction::NavLastPage)));
                }
                _ => {}
            }
        }
    }

    if is_results_or_details_focused(&home.focused) && ctrl && key.code == KeyCode::Char('d') {
        return Ok(Some(Action::OpenDocs));
    }

    if home.focused == Focusable::Sort {
        if let Some(action) = home.sort_dropdown.handle_key_event(key)? {
            return Ok(Some(action));
        }
    }

    if home.focused == Focusable::Scope {
        if let Some(action) = home.scope_dropdown.handle_key_event(key)? {
            return Ok(Some(action));
        }
    }

    Ok(None)
}
