use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tui_input::backend::crossterm::EventHandler;

use crate::action::Action;
use crate::cargo::CargoCommand;
use crate::components::home::cargo_request::CargoIntent;
use crate::components::home::overlay::Overlay;
use crate::components::home::{Focusable, Home, HomeCommand};
use crate::components::ux::{Confirm, Dropdown, KeyOutcome};
use crate::errors::AppResult;
use crate::search::SearchCommand;

pub fn handle_key(home: &mut Home, key: KeyEvent) -> AppResult<Option<Action>> {
    if home.overlay.is_some() {
        return handle_overlay_key(home, key);
    }

    if let Some(action) = handle_global_shortcuts(home, key)? {
        return Ok(Some(action));
    }

    let is_details_focused = home.is_details_focused();

    match home.focused {
        Focusable::Search => handle_search_focus(home, key),
        Focusable::Results if !is_details_focused => handle_results_focus(home, key),
        _ => {
            if is_details_focused {
                return handle_details_focus(home, key);
            }
            Ok(None)
        }
    }
}

fn handle_global_shortcuts(home: &mut Home, key: KeyEvent) -> AppResult<Option<Action>> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

    if home.get_focused_crate().is_some() && ctrl && key.code == KeyCode::Char('d') {
        return Ok(Some(Action::Home(HomeCommand::OpenDocs)));
    }

    match key.code {
        KeyCode::Char('h') if ctrl && home.search_results.is_some() => {
            return Ok(Some(Action::Home(HomeCommand::ToggleHelp)));
        }
        KeyCode::Esc => {
            return if home.focused == Focusable::Search {
                Ok(Some(Action::Search(SearchCommand::Clear)))
            } else {
                Ok(Some(Action::Home(HomeCommand::Focus(Focusable::Search))))
            };
        }
        KeyCode::Char('s') if ctrl => {
            open_sort_overlay(home);
            return Ok(None);
        }
        KeyCode::Char('a') if ctrl => {
            open_scope_overlay(home);
            return Ok(None);
        }
        KeyCode::Char('/') => {
            return Ok(Some(Action::Home(HomeCommand::Focus(Focusable::Search))));
        }
        KeyCode::BackTab => {
            return Ok(Some(Action::Home(HomeCommand::FocusPrevious)));
        }
        KeyCode::Tab => {
            return Ok(Some(Action::Home(HomeCommand::FocusNext)));
        }
        KeyCode::Enter => match home.focused {
            Focusable::Search => {
                return Ok(Some(Action::Search(SearchCommand::Run {
                    term: home.input.value().to_string(),
                    page: 1,
                    hide_help: true,
                    status: None,
                })));
            }
            Focusable::Results => {}
            Focusable::DocsButton => {
                return Ok(Some(Action::Home(HomeCommand::OpenDocs)));
            }
            Focusable::RepositoryButton => {
                return Ok(Some(Action::Home(HomeCommand::OpenReadme)));
            }
            Focusable::CratesIoButton => {
                return Ok(Some(Action::Home(HomeCommand::OpenCratesIo)));
            }
            Focusable::LibRsButton => {
                return Ok(Some(Action::Home(HomeCommand::OpenLibRs)));
            }
            _ => {}
        },
        KeyCode::Up if home.focused == Focusable::Help => {
            if home.vertical_help_scroll > 0 {
                home.vertical_help_scroll -= 1;
            }
        }
        KeyCode::Down if home.focused == Focusable::Help => {
            if home.vertical_help_scroll < home.max_help_scroll {
                home.vertical_help_scroll += 1;
            }
        }
        KeyCode::Left if ctrl && home.left_column_width_percent >= 10 => {
            home.left_column_width_percent -= 10;
            return Ok(None);
        }
        KeyCode::Right if ctrl && home.left_column_width_percent <= 90 => {
            home.left_column_width_percent += 10;
            return Ok(None);
        }
        KeyCode::Char('a') => {
            if home.get_focused_crate().is_some() {
                return Ok(Some(Action::Home(HomeCommand::BeginCargoRequest(
                    CargoIntent::Add,
                ))));
            }
        }
        KeyCode::Char('r') => {
            if let Some(selected) = home.get_focused_crate() {
                home.overlay = Some(Overlay::Confirm(
                    Confirm::new(
                        home.config.clone(),
                        format!(
                            "Are you sure you want to remove {} v{}?",
                            selected.name, selected.version
                        )
                        .as_str(),
                        true,
                    ),
                    Action::Cargo(CargoCommand::Remove(selected.name.clone())),
                ));
            }
        }
        KeyCode::Char('i') => {
            if home.get_focused_crate().is_some() {
                return Ok(Some(Action::Home(HomeCommand::BeginCargoRequest(
                    CargoIntent::Install,
                ))));
            }
        }
        KeyCode::Char('u') => {
            if let Some(selected) = home.get_focused_crate() {
                home.overlay = Some(Overlay::Confirm(
                    Confirm::new(
                        home.config.clone(),
                        format!(
                            "Are you sure you want to uninstall {} v{}?",
                            selected.name, selected.version
                        )
                        .as_str(),
                        true,
                    ),
                    Action::Cargo(CargoCommand::Uninstall(selected.name.clone())),
                ));
            }
        }
        _ => {}
    }

    Ok(None)
}

/// Routes a key to the active overlay and applies its outcome.
fn handle_overlay_key(home: &mut Home, key: KeyEvent) -> AppResult<Option<Action>> {
    let outcome = match home.overlay.as_mut() {
        Some(overlay) => overlay.handle_key(key),
        None => return Ok(None),
    };

    match outcome {
        KeyOutcome::Pending => Ok(None),
        KeyOutcome::Cancelled => {
            home.overlay = None;
            Ok(None)
        }
        KeyOutcome::Submitted(action) => {
            home.overlay = None;
            Ok(Some(action))
        }
    }
}

/// Opens the sort dropdown, initialized to the current sort.
fn open_sort_overlay(home: &mut Home) {
    home.overlay = Some(Overlay::Sort(Dropdown::new(
        home.config.clone(),
        "Sort by".into(),
        home.sort.clone(),
    )));
}

/// Opens the scope dropdown, initialized to the current scope.
fn open_scope_overlay(home: &mut Home) {
    home.overlay = Some(Overlay::Scope(Dropdown::new(
        home.config.clone(),
        "Search in".into(),
        home.scope.clone(),
    )));
}

fn handle_search_focus(home: &mut Home, key: KeyEvent) -> AppResult<Option<Action>> {
    match key.code {
        KeyCode::Down => {
            if home.search_results.is_some() {
                return Ok(Some(Action::Home(HomeCommand::Focus(Focusable::Results))));
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
                    return Ok(Some(Action::Home(HomeCommand::Focus(Focusable::Search))));
                }

                return Ok(Some(Action::Search(SearchCommand::SelectPrev)));
            }
            KeyCode::Down => {
                return Ok(Some(Action::Search(SearchCommand::SelectNext)));
            }
            KeyCode::Home if !ctrl => {
                return Ok(Some(Action::Search(SearchCommand::SelectFirst)));
            }
            KeyCode::End if !ctrl => {
                return Ok(Some(Action::Search(SearchCommand::SelectLast)));
            }
            // Page navigation
            KeyCode::Left if !ctrl && results.has_prev_page() => {
                return Ok(Some(Action::Search(SearchCommand::NavPagesBack(1))));
            }
            KeyCode::Right if !ctrl && results.has_next_page() => {
                return Ok(Some(Action::Search(SearchCommand::NavPagesForward(1))));
            }
            KeyCode::Home if ctrl => {
                return Ok(Some(Action::Search(SearchCommand::NavFirstPage)));
            }
            KeyCode::End if ctrl => {
                return Ok(Some(Action::Search(SearchCommand::NavLastPage)));
            }
            _ => {}
        }
    }

    Ok(None)
}

fn handle_details_focus(home: &mut Home, key: KeyEvent) -> AppResult<Option<Action>> {
    let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);
    if ctrl {
        return Ok(None);
    }

    let focused = &home.focused;

    let next = match key.code {
        KeyCode::Up => buttons_move_vertical(home, focused, -1),
        KeyCode::Down => buttons_move_vertical(home, focused, 1),
        KeyCode::Left => buttons_move_horizontal(home, focused, -1),
        KeyCode::Right => buttons_move_horizontal(home, focused, 1),
        _ => None,
    };

    if let Some(focusable) = next {
        return Ok(Some(Action::Home(HomeCommand::Focus(focusable))));
    }

    Ok(None)
}

// Used for focus positioning for buttons in the details pane/box
fn button_rows(home: &Home) -> Vec<Vec<Focusable>> {
    let top = [Focusable::DocsButton, Focusable::RepositoryButton]
        .into_iter()
        .filter(|f| home.should_show_button(f))
        .collect();

    let bottom = [Focusable::CratesIoButton, Focusable::LibRsButton]
        .into_iter()
        .filter(|f| home.should_show_button(f))
        .collect();

    vec![top, bottom]
}

fn buttons_find_pos(rows: &[Vec<Focusable>], f: &Focusable) -> Option<(usize, usize)> {
    for (row_idx, row) in rows.iter().enumerate() {
        if let Some(col_idx) = row.iter().position(|x| x == f) {
            return Some((row_idx, col_idx));
        }
    }
    None
}

fn buttons_move_horizontal(home: &Home, current: &Focusable, dir: i32) -> Option<Focusable> {
    let rows = button_rows(home);
    let (row_idx, col_idx) = buttons_find_pos(&rows, current)?;

    let row = rows.get(row_idx)?;
    if row.is_empty() {
        return None;
    }

    let len = row.len() as i32;
    let new_idx = (col_idx as i32 + dir).rem_euclid(len) as usize;
    row.get(new_idx).cloned()
}

fn buttons_move_vertical(home: &Home, current: &Focusable, dir: i32) -> Option<Focusable> {
    let rows = button_rows(home);
    let (row_idx, col_idx) = buttons_find_pos(&rows, current)?;

    let new_row_idx_i = row_idx as i32 + dir;
    if new_row_idx_i < 0 || new_row_idx_i >= rows.len() as i32 {
        // No row above/below
        return None;
    }

    let target_row = &rows[new_row_idx_i as usize];

    if target_row.is_empty() {
        return None;
    }

    // If no button could be found at index, fall back to the last button in that row
    target_row
        .get(col_idx)
        .cloned()
        .or_else(|| target_row.last().cloned())
}
