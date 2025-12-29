use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use tui_input::backend::crossterm::EventHandler;

use crate::action::Action;
use crate::cargo::CargoAction;
use crate::components::Component;
use crate::components::home::{Focusable, Home, HomeAction, SearchAction};
use crate::errors::AppResult;

pub fn handle_key(home: &mut Home, key: KeyEvent) -> AppResult<Option<Action>> {
    if let Some(action) = handle_global_shortcuts(home, key)? {
        return Ok(Some(action));
    }

    let is_details_focused = home.is_details_focused();

    match home.focused {
        Focusable::Search => handle_search_focus(home, key),
        Focusable::Results if !is_details_focused => handle_results_focus(home, key),
        Focusable::Sort => handle_sort_focus(home, key),
        Focusable::Scope => handle_scope_focus(home, key),
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

    if let Some(_) = home.get_focused_crate()
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
        KeyCode::Up if home.focused == Focusable::Usage => {
            if home.vertical_usage_scroll > 0 {
                home.vertical_usage_scroll -= 1;
            }
        }
        KeyCode::Down if home.focused == Focusable::Usage => {
            if home.vertical_usage_scroll < 21 {
                home.vertical_usage_scroll += 1;
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
            if home.is_results_or_details_focused()
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
            if let Some(selected) = home.get_focused_crate() {
                return Ok(Some(Action::Cargo(CargoAction::Remove(
                    selected.name.clone(),
                ))));
            }
        }
        KeyCode::Char('i') => {
            if let Some(selected) = home.get_focused_crate() {
                return Ok(Some(Action::Cargo(CargoAction::Install {
                    name: selected.name.clone(),
                    version: selected.version.clone(),
                })));
            }
        }
        KeyCode::Char('u') => {
            if let Some(selected) = home.get_focused_crate() {
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
            KeyCode::Left if !ctrl && results.has_prev_page() => {
                let pages = 1;
                return Ok(Some(Action::Home(HomeAction::Search(
                    SearchAction::NavPagesBack(pages),
                ))));
            }
            KeyCode::Right if !ctrl && results.has_next_page() => {
                let pages = 1;
                return Ok(Some(Action::Home(HomeAction::Search(
                    SearchAction::NavPagesForward(pages),
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
        return Ok(Some(Action::Home(HomeAction::Focus(focusable))));
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
