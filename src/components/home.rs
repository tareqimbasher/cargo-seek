pub mod scope_dropdown;
pub mod search_results;
pub mod sort_dropdown;

use super::Component;

use std::sync::Arc;
use std::{fs, io::Write, iter::Cycle, process::Command};

use chrono::Utc;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use enum_iterator::{all, reverse_all, Sequence};
use ratatui::style::Color;
use ratatui::{
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::{Style, Styled, Stylize},
    text::{Line, Text},
    widgets::{
        block::{Position, Title},
        Block, Borders, List, ListItem, Padding, Paragraph, Wrap,
    },
    Frame,
};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::Mutex;
use tui_input::{backend::crossterm::EventHandler, Input};

use crate::cargo::cargo_env::CargoEnv;
use crate::cargo::metadata::Crate;
use crate::components::button::{Button, State, BLUE, GRAY, ORANGE, PURPLE};
use crate::components::home::scope_dropdown::{Scope, ScopeDropdown};
use crate::components::home::search_results::SearchResults;
use crate::components::home::sort_dropdown::SortDropdown;
use crate::components::status_bar::{StatusDuration, StatusLevel};
use crate::errors::AppResult;
use crate::services::crate_search_manager::{CrateSearchManager, SearchOptions};
use crate::tui::Tui;
use crate::util::Util;
use crate::{
    action::{Action, CargoAction, SearchAction},
    app::Mode,
    config::Config,
};

#[derive(Default, PartialEq, Clone, Debug, Eq, Sequence, Serialize, Deserialize)]
pub enum Focusable {
    #[default]
    Search,
    Sort,
    Scope,
    Results,
    AddButton,
    InstallButton,
    ReadmeButton,
    DocsButton,
}

impl Focusable {
    pub fn next(&self) -> Focusable {
        let mut variants: Cycle<_> = all::<Focusable>().cycle();
        variants.find(|v| v == self);
        variants.next().unwrap()
    }

    pub fn prev(&self) -> Focusable {
        let mut variants: Cycle<_> = reverse_all::<Focusable>().cycle();
        variants.find(|v| v == self);
        variants.next().unwrap()
    }
}

pub struct Home {
    cargo_env: Arc<Mutex<CargoEnv>>,
    input: Input,
    sort_dropdown: SortDropdown,
    scope_dropdown: ScopeDropdown,
    show_usage: bool,
    focused: Focusable,
    crate_search_manager: CrateSearchManager,
    is_searching: bool,
    search_results: Option<SearchResults>,
    spinner_state: throbber_widgets_tui::ThrobberState,
    action_tx: UnboundedSender<Action>,
    config: Config,
    scope: Scope,
}

impl Home {
    pub fn new(
        cargo_env: Arc<Mutex<CargoEnv>>,
        action_tx: UnboundedSender<Action>,
    ) -> AppResult<Self> {
        Ok(Self {
            cargo_env,
            input: Input::default(),
            sort_dropdown: SortDropdown::new(),
            scope_dropdown: ScopeDropdown::new(),
            show_usage: true,
            focused: Focusable::default(),
            search_results: None,
            crate_search_manager: CrateSearchManager::new(action_tx.clone())?,
            is_searching: false,
            spinner_state: throbber_widgets_tui::ThrobberState::default(),
            action_tx,
            config: Config::default(),
            scope: Scope::default(),
        })
    }

    fn reset(&mut self) -> AppResult<()> {
        self.input.reset();
        self.search_results = None;
        self.action_tx
            .send(Action::UpdateStatus(StatusLevel::Info, "Ready".into()))?;
        Ok(())
    }

    pub fn go_to_page(&self, page: usize, query: String) -> AppResult<()> {
        if let Some(results) = &self.search_results {
            let requested_page = if page >= results.page_count() {
                results.page_count()
            } else {
                page
            };

            if requested_page == results.current_page() {
                return Ok(());
            }

            self.action_tx.send(Action::Search(SearchAction::Search(
                query,
                self.sort_dropdown.get_selected(),
                requested_page,
                Some(format!("Loading page {}", requested_page)),
            )))?;
        }

        Ok(())
    }

    pub fn go_pages_back(&self, pages: usize, query: String) -> AppResult<()> {
        if let Some(results) = &self.search_results {
            let requested_page = if pages >= results.current_page() {
                1
            } else {
                results.current_page() - pages
            };

            if requested_page == results.current_page() {
                return Ok(());
            }

            self.action_tx.send(Action::Search(SearchAction::Search(
                query,
                self.sort_dropdown.get_selected(),
                requested_page,
                Some(format!("Loading page {}", requested_page)),
            )))?;
        }

        Ok(())
    }

    pub fn go_pages_forward(&self, pages: usize, query: String) -> AppResult<()> {
        if let Some(results) = &self.search_results {
            let mut requested_page = results.current_page() + pages;

            if requested_page > results.page_count() {
                requested_page = results.page_count();
            }

            if requested_page == results.current_page() {
                return Ok(());
            }

            self.action_tx.send(Action::Search(SearchAction::Search(
                query,
                self.sort_dropdown.get_selected(),
                requested_page,
                Some(format!("Loading page {}", requested_page)),
            )))?;
        }

        Ok(())
    }

    fn render_left(&mut self, frame: &mut Frame, area: Rect) -> AppResult<()> {
        let [search, results] =
            Layout::vertical([Constraint::Length(3), Constraint::Min(5)]).areas(area);

        self.render_search(frame, search)?;
        self.render_results(frame, results)?;
        self.sort_dropdown.draw(frame, area)?;
        self.scope_dropdown.draw(frame, area)?;

        Ok(())
    }

    fn render_search(&mut self, frame: &mut Frame, area: Rect) -> AppResult<()> {
        let spinner_len = if self.is_searching { 3 } else { 0 };

        let [search, spinner] =
            Layout::horizontal([Constraint::Min(1), Constraint::Length(spinner_len)]).areas(area);

        // The width of the input area, removing 2 for the width of the border on each side
        let scroll_width = if search.width < 2 {
            0
        } else {
            search.width - 2
        };
        let scroll = self.input.visual_scroll(scroll_width as usize);
        let input = Paragraph::new(self.input.value())
            .scroll((0, scroll as u16))
            .block(
                Block::default()
                    .title(" Search ")
                    .borders(Borders::ALL)
                    .border_style(match self.focused {
                        Focusable::Search => self.config.styles[&Mode::Home]["accent_active"],
                        _ => Style::default(),
                    }),
            );
        frame.render_widget(input, search);

        if self.focused == Focusable::Search {
            // Make the cursor visible and ask ratatui to put it at the specified coordinates after rendering
            frame.set_cursor_position((
                // Put cursor past the end of the input text
                search.x + (self.input.visual_cursor().max(scroll) - scroll) as u16 + 1,
                // Move one line down, from the border to the input line
                search.y + 1,
            ))
        }

        if self.is_searching {
            let throbber_border = Block::default().padding(Padding::uniform(1));
            frame.render_widget(&throbber_border, spinner);

            let throbber = throbber_widgets_tui::Throbber::default()
                .style(self.config.styles[&Mode::Home]["throbber"])
                .throbber_set(throbber_widgets_tui::BRAILLE_EIGHT)
                .use_type(throbber_widgets_tui::WhichUse::Spin);

            frame.render_stateful_widget(
                throbber,
                throbber_border.inner(spinner),
                &mut self.spinner_state,
            );
        }

        Ok(())
    }

    fn render_results(&mut self, frame: &mut Frame, area: Rect) -> AppResult<()> {
        let block = Block::default()
            .borders(Borders::ALL)
            .border_style(match self.focused {
                Focusable::Results => self.config.styles[&Mode::Home]["accent_active"],
                _ => Style::default(),
            })
            .title(
                Title::from(
                    format!(" ▼ {} ", self.sort_dropdown.get_selected()).set_style(
                        if self.focused == Focusable::Sort {
                            self.config.styles[&Mode::Home]["title"]
                        } else {
                            Style::default()
                        },
                    ),
                )
                .alignment(Alignment::Right),
            )
            .title(
                Title::from(
                    format!(" ▼ {} ", self.scope_dropdown.get_selected()).set_style(
                        if self.focused == Focusable::Scope {
                            self.config.styles[&Mode::Home]["title"]
                        } else {
                            Style::default()
                        },
                    ),
                )
                .alignment(Alignment::Right),
            );

        if let Some(results) = self.search_results.as_mut() {
            let correction = match results.get_selected_index() {
                Some(_) => 4,
                None => 2,
            };

            let list_items: Vec<ListItem> = results
                .crates
                .iter()
                .map(|i| {
                    let tag = if i.is_local {
                        "[local]"
                    } else if i.is_installed {
                        "[installed]"
                    } else {
                        ""
                    };

                    let name = i.name.to_string();
                    let version = i.version();

                    let mut white_space = area.width as i32 - name.len() as i32 - 20 - correction;
                    if !tag.is_empty() {
                        white_space -= tag.len() as i32;
                    }

                    if white_space < 0 {
                        white_space = 1;
                    }

                    let line = format!(
                        "{}{}{}{:>20}",
                        name,
                        " ".repeat(white_space as usize),
                        tag,
                        version
                    );

                    ListItem::new(if i.is_local {
                        line.set_style(Style::default().fg(Color::LightCyan))
                    } else if i.is_installed {
                        line.set_style(Style::default().fg(Color::LightMagenta))
                    } else {
                        line.into()
                    })
                })
                .collect();

            let items_in_prev_pages = match results.current_page() {
                1 => 0,
                p => {
                    if p < 1 {
                        0
                    } else {
                        (p - 1) * 100
                    }
                }
            };

            let selected_item_num = match results.get_selected_index() {
                None => 0,
                Some(ix) => {
                    if ix == usize::MAX {
                        results.current_page_count()
                    } else if ix == usize::MIN {
                        1
                    } else if ix > results.current_page_count() - 1 {
                        // ListState select_next() increments selected even after last item is selected
                        ix
                    } else {
                        ix + 1
                    }
                }
            };

            let selected_item_num_in_total = items_in_prev_pages + selected_item_num;

            let list = List::new(list_items)
                .block(
                    block
                        .title(Title::from(format!(
                            " {}/{} ",
                            selected_item_num_in_total, results.total_count
                        )))
                        .title(
                            Title::from(format!(
                                " Page {}/{} ",
                                results.current_page(),
                                results.page_count(),
                            ))
                            .position(Position::Bottom)
                            .alignment(Alignment::Right),
                        ),
                )
                .highlight_style(self.config.styles[&Mode::Home]["accent"].bold())
                .highlight_symbol("> ");

            frame.render_stateful_widget(list, area, results.list_state());
        } else {
            frame.render_widget(block, area);
        }

        Ok(())
    }

    fn render_right(&mut self, frame: &mut Frame, area: Rect) -> AppResult<()> {
        if self.show_usage || self.search_results.is_none() {
            self.render_usage(frame, area)?;
            return Ok(());
        } else if let Some(krate) = self.search_results.as_ref().unwrap().get_selected() {
            self.render_crate_details(krate, frame, area)?;
        } else {
            self.render_no_results(frame, area)?;
        }

        Ok(())
    }

    fn render_usage(&self, frame: &mut Frame, area: Rect) -> AppResult<()> {
        let prop_style = self.config.styles[&Mode::Home]["accent"].bold();
        let pad = 25;

        let text = Text::from(vec![
            Line::from(vec!["SEARCH".bold()]),
            Line::from(vec![
                format!("{:<pad$}", "ENTER:").set_style(prop_style),
                "Search".bold(),
            ]),
            Line::from(vec![
                format!("{:<pad$}", "Ctrl + s:").set_style(prop_style),
                "Sort".bold(),
            ]),
            Line::from(vec![
                format!("{:<pad$}", "Ctrl + f:").set_style(prop_style),
                "Filter".bold(),
            ]),
            Line::default(),
            Line::from(vec!["NAVIGATION".bold()]),
            Line::from(vec![
                format!("{:<pad$}", "TAB:").set_style(prop_style),
                "Switch between boxes".bold(),
            ]),
            Line::from(vec![
                format!("{:<pad$}", "ESC:").set_style(prop_style),
                "Go back to search; clear results".bold(),
            ]),
            Line::from(vec![
                format!("{:<pad$}", "Ctrl + h:").set_style(prop_style),
                "Toggle this usage screen".bold(),
            ]),
            Line::from(vec![
                format!("{:<pad$}", "Ctrl + z:").set_style(prop_style),
                "Suspend".bold(),
            ]),
            Line::from(vec![
                format!("{:<pad$}", "Ctrl + c:").set_style(prop_style),
                "Quit".bold(),
            ]),
            Line::default(),
            Line::from(vec!["LIST".bold()]),
            Line::from(vec![
                format!("{:<pad$}", "Up/Down:").set_style(prop_style),
                "Scroll in crate list".bold(),
            ]),
            Line::from(vec![
                format!("{:<pad$}", "Home/End:").set_style(prop_style),
                "Go to first/last crate in list".bold(),
            ]),
            Line::from(vec![
                format!("{:<pad$}", "a:").set_style(prop_style),
                "Add".bold(),
                " (WIP)".gray(),
            ]),
            Line::from(vec![
                format!("{:<pad$}", "r:").set_style(prop_style),
                "Remove".bold(),
                " (WIP)".gray(),
            ]),
            Line::from(vec![
                format!("{:<pad$}", "i:").set_style(prop_style),
                "Install".bold(),
                " (WIP)".gray(),
            ]),
            Line::from(vec![
                format!("{:<pad$}", "u:").set_style(prop_style),
                "Uninstall".bold(),
                " (WIP)".gray(),
            ]),
            Line::from(vec![
                format!("{:<pad$}", "Ctrl + o:").set_style(prop_style),
                "Open docs URL".bold(),
                " (WIP)".gray(),
            ]),
            Line::default(),
            Line::from(vec!["PAGING".bold()]),
            Line::from(vec![
                format!("{:<pad$}", "Left/Right:").set_style(prop_style),
                "Go backward/forward a page".bold(),
            ]),
            Line::from(vec![
                format!("{:<pad$}", "Ctrl + Left/Right:").set_style(prop_style),
                "Go backward/forward 10 pages".bold(),
            ]),
            Line::from(vec![
                format!("{:<pad$}", "Ctrl + Home/End:").set_style(prop_style),
                "Go to first/last page".bold(),
            ]),
        ]);

        let block = Block::default()
            .title(" 📖 Usage ")
            .title_style(self.config.styles[&Mode::Home]["title"])
            .padding(Padding::uniform(1))
            .borders(Borders::ALL);

        frame.render_widget(&block, area);

        frame.render_widget(
            Paragraph::new(text)
                .wrap(Wrap { trim: false })
                .scroll((0, 0)),
            block.inner(area),
        );

        Ok(())
    }

    fn render_crate_details(&self, krate: &Crate, frame: &mut Frame, area: Rect) -> AppResult<()> {
        let details_focused = self.focused == Focusable::AddButton
            || self.focused == Focusable::InstallButton
            || self.focused == Focusable::ReadmeButton
            || self.focused == Focusable::DocsButton;

        let main_block = Block::default()
            .title(format!(" 🧐 {} ", krate.name))
            .title_style(self.config.styles[&Mode::Home]["title"])
            .padding(Padding::uniform(1))
            .borders(Borders::ALL)
            .border_style(if details_focused {
                self.config.styles[&Mode::Home]["accent_active"]
            } else {
                Style::default()
            });

        let left_column_width = 25;

        let prop_style = self.config.styles[&Mode::Home][if details_focused {
            "accent_active"
        } else {
            "accent"
        }]
        .bold();

        let text = Text::from(vec![
            Line::from(vec![
                format!("{:<left_column_width$}", "Version:").set_style(prop_style),
                krate.version().into(),
            ]),
            Line::from(vec![
                format!("{:<left_column_width$}", "Latest Version:").set_style(prop_style),
                krate.max_version.to_string().into(),
            ]),
            Line::from(vec![
                format!("{:<left_column_width$}", "Home Page:").set_style(prop_style),
                krate.homepage.clone().unwrap_or_default().into(),
            ]),
            Line::from(vec![
                format!("{:<left_column_width$}", "Documentation:").set_style(prop_style),
                krate.documentation.clone().unwrap_or_default().into(),
            ]),
            Line::from(vec![
                format!("{:<left_column_width$}", "Repository:").set_style(prop_style),
                krate.repository.clone().unwrap_or_default().into(),
            ]),
            Line::from(vec![
                format!("{:<left_column_width$}", "crates.io Page:").set_style(prop_style),
                format!("https://crates.io/crates/{}", krate.id).into(),
            ]),
            Line::from(vec![
                format!("{:<left_column_width$}", "Downloads:").set_style(prop_style),
                Util::format_number(krate.downloads).into(),
            ]),
            Line::from(vec![
                format!("{:<left_column_width$}", "Recent Downloads:").set_style(prop_style),
                Util::format_number(krate.recent_downloads).into(),
            ]),
            Line::from(vec![
                format!("{:<left_column_width$}", "Created:").set_style(prop_style),
                match krate.created_at.as_ref() {
                    None => "".into(),
                    Some(v) => v.format("%d/%m/%Y %H:%M:%S (UTC)").to_string().into(),
                },
            ]),
            Line::from(vec![
                format!("{:<left_column_width$}", "Updated:").set_style(prop_style),
                match krate.created_at.as_ref() {
                    None => "".into(),
                    Some(v) => {
                        let updated_relative = match krate.updated_at {
                            None => "".into(),
                            Some(v) => Util::get_relative_time(v, Utc::now()),
                        };

                        format!(
                            "{} ({})",
                            v.format("%d/%m/%Y %H:%M:%S (UTC)"),
                            updated_relative
                        )
                        .into()
                    }
                },
            ]),
            Line::from(vec![
                format!("{:<left_column_width$}", "Description:").set_style(prop_style),
                krate.description.clone().unwrap_or_default().bold(),
            ]),
        ]);

        let details_paragraph_lines = text.lines.len();
        let details_paragraph = Paragraph::new(text).wrap(Wrap { trim: false });

        frame.render_widget(&main_block, area);

        let [details_area, _, buttons_row1_area, _, buttons_row2_area] = Layout::vertical([
            Constraint::Length((details_paragraph_lines + 1) as u16),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
            Constraint::Length(1),
        ])
        .areas(main_block.inner(area));

        frame.render_widget(details_paragraph, details_area);

        let buttons_row_layout = Layout::horizontal([
            Constraint::Length(left_column_width as u16),
            Constraint::Length(10),
            Constraint::Length(1),
            Constraint::Length(10),
        ]);

        // Buttons row 1
        let [property_area, button1_area, _, button2_area] =
            buttons_row_layout.areas(buttons_row1_area);

        frame.render_widget(Text::from("Cargo:").set_style(prop_style), property_area);
        frame.render_widget(
            Button::new("Add")
                .theme(BLUE)
                .state(match self.focused == Focusable::AddButton {
                    true => State::Selected,
                    _ => State::Normal,
                }),
            button1_area,
        );
        frame.render_widget(
            Button::new("Install").theme(PURPLE).state(
                match self.focused == Focusable::InstallButton {
                    true => State::Selected,
                    _ => State::Normal,
                },
            ),
            button2_area,
        );

        // Buttons row 2
        let [property_area, button1_area, _, button2_area] =
            buttons_row_layout.areas(buttons_row2_area);

        frame.render_widget(Text::from("Links:").set_style(prop_style), property_area);

        let mut button_areas = vec![button1_area, button2_area];

        if krate.repository.is_some() {
            frame.render_widget(
                Button::new("README").theme(GRAY).state(
                    match self.focused == Focusable::ReadmeButton {
                        true => State::Selected,
                        _ => State::Normal,
                    },
                ),
                button_areas.remove(0),
            );
        }

        if krate.documentation.is_some() {
            frame.render_widget(
                Button::new("Docs").theme(ORANGE).state(
                    match self.focused == Focusable::DocsButton {
                        true => State::Selected,
                        _ => State::Normal,
                    },
                ),
                button_areas.remove(0),
            );
        }

        Ok(())
    }

    fn render_no_results(&self, frame: &mut Frame, area: Rect) -> AppResult<()> {
        let main_block = Block::default()
            .title(" No results ")
            .title_style(self.config.styles[&Mode::Home]["title"])
            .padding(Padding::uniform(1))
            .borders(Borders::ALL);

        let text = Text::raw("0 crates found");
        let centered = self.center(
            main_block.inner(area),
            Constraint::Length(text.width() as u16),
            Constraint::Length(1),
        )?;

        frame.render_widget(main_block, area);
        frame.render_widget(text, centered);

        Ok(())
    }

    fn center(&self, area: Rect, horizontal: Constraint, vertical: Constraint) -> AppResult<Rect> {
        let [area] = Layout::horizontal([horizontal])
            .flex(Flex::Center)
            .areas(area);
        let [area] = Layout::vertical([vertical]).flex(Flex::Center).areas(area);
        Ok(area)
    }
}

impl Component for Home {
    fn register_config_handler(&mut self, config: Config) -> AppResult<()> {
        self.sort_dropdown.register_config_handler(config.clone())?;
        self.scope_dropdown
            .register_config_handler(config.clone())?;
        self.config = config;
        Ok(())
    }

    fn init(&mut self, tui: &mut Tui) -> AppResult<()> {
        let _ = tui; // to appease clippy
        Ok(())
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> AppResult<Option<Action>> {
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

        // Try match key combos that should be handled regardless what is focused
        match key.code {
            KeyCode::Char('h') if ctrl => {
                return Ok(Some(Action::ToggleUsage));
            }
            KeyCode::Esc => {
                return if self.focused == Focusable::Search {
                    Ok(Some(Action::Search(SearchAction::Clear)))
                } else {
                    Ok(Some(Action::Focus(Focusable::Search)))
                }
            }
            KeyCode::Char('s') if ctrl => {
                return Ok(Some(Action::Focus(if self.focused == Focusable::Sort {
                    Focusable::Search
                } else {
                    Focusable::Sort
                })));
            }
            KeyCode::Char('f') if ctrl => {
                return Ok(Some(Action::Focus(if self.focused == Focusable::Scope {
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
                if self.search_results.is_some() {
                    return Ok(Some(Action::FocusNext));
                }
            }
            KeyCode::Enter => match self.focused {
                Focusable::Search => {
                    return Ok(Some(Action::Search(SearchAction::Search(
                        self.input.value().to_string(),
                        self.sort_dropdown.get_selected(),
                        1,
                        None,
                    ))));
                }
                Focusable::Results => {}
                Focusable::AddButton => {}
                Focusable::InstallButton => {}
                Focusable::ReadmeButton => {
                    return Ok(Some(Action::OpenReadme));
                }
                Focusable::DocsButton => {
                    return Ok(Some(Action::OpenDocs));
                }
                _ => {}
            },
            KeyCode::Up => match self.focused {
                Focusable::ReadmeButton => {
                    return Ok(Some(Action::Focus(Focusable::AddButton)));
                }
                Focusable::DocsButton => {
                    return Ok(Some(Action::Focus(Focusable::InstallButton)));
                }
                _ => {}
            },
            KeyCode::Down => match self.focused {
                Focusable::AddButton => {
                    return Ok(Some(Action::Focus(Focusable::ReadmeButton)));
                }
                Focusable::InstallButton => {
                    return Ok(Some(Action::Focus(Focusable::DocsButton)));
                }
                _ => {}
            },
            KeyCode::Left => match self.focused {
                Focusable::InstallButton => {
                    return Ok(Some(Action::Focus(Focusable::AddButton)));
                }
                Focusable::DocsButton => {
                    return Ok(Some(Action::Focus(Focusable::ReadmeButton)));
                }
                _ => {}
            },
            KeyCode::Right => match self.focused {
                Focusable::AddButton => {
                    return Ok(Some(Action::Focus(Focusable::InstallButton)));
                }
                Focusable::ReadmeButton => {
                    return Ok(Some(Action::Focus(Focusable::DocsButton)));
                }
                _ => {}
            },
            _ => {}
        }

        if self.focused == Focusable::Search {
            match key.code {
                KeyCode::Down => {
                    return Ok(Some(Action::Focus(Focusable::Results)));
                }
                _ => {
                    // Send to input box
                    self.input.handle_event(&crossterm::event::Event::Key(key));
                }
            }
        }

        if self.focused == Focusable::Results {
            if let Some(results) = &self.search_results {
                if results.crates.is_empty() {
                    return Ok(None);
                }

                match key.code {
                    // List navigation
                    KeyCode::Up => {
                        if let Some(selected_ix) = results.get_selected_index() {
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
                        let pages = match ctrl {
                            true => 10,
                            false => 1,
                        };
                        return Ok(Some(Action::Search(SearchAction::NavPagesForward(pages))));
                    }
                    KeyCode::Left if results.has_prev_page() => {
                        let pages = match ctrl {
                            true => 10,
                            false => 1,
                        };
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

        if self.focused == Focusable::Sort {
            if let Some(action) = self.sort_dropdown.handle_key_event(key)? {
                return Ok(Some(action));
            }
        }

        if self.focused == Focusable::Scope {
            if let Some(action) = self.scope_dropdown.handle_key_event(key)? {
                return Ok(Some(action));
            }
        }

        Ok(None)
    }

    fn update(&mut self, action: Action, tui: &mut Tui) -> AppResult<Option<Action>> {
        match action {
            Action::Tick => {
                // add any logic here that should run on every tick
                if self.is_searching {
                    self.spinner_state.calc_next();
                }
            }
            Action::Render => {
                // add any logic here that should run on every render
            }
            Action::Focus(focusable) => {
                self.sort_dropdown
                    .set_is_focused(focusable == Focusable::Sort);
                self.scope_dropdown
                    .set_is_focused(focusable == Focusable::Scope);
                self.focused = focusable;
            }
            Action::FocusNext => {
                let mut next = self.focused.next();
                while next == Focusable::Sort || next == Focusable::Scope {
                    next = next.next();
                }

                return Ok(Some(Action::Focus(next)));
            }
            Action::FocusPrevious => {
                let mut prev = self.focused.prev();
                while prev == Focusable::Sort || prev == Focusable::Scope {
                    prev = prev.prev();
                }

                return Ok(Some(Action::Focus(prev)));
            }
            Action::ToggleUsage => {
                self.show_usage = !self.show_usage;
            }
            Action::Search(action) => match action {
                SearchAction::Clear => self.reset()?,
                SearchAction::Search(term, sort, page, status) => {
                    let tx = self.action_tx.clone();

                    let status = status.unwrap_or("Searching".into());
                    tx.send(Action::UpdateStatus(
                        StatusLevel::Progress,
                        status.to_string(),
                    ))?;

                    self.is_searching = true;
                    self.crate_search_manager.search(
                        SearchOptions {
                            term: Some(term),
                            sort,
                            page: Some(page),
                            per_page: Some(100),
                            scope: self.scope.clone(),
                        },
                        Arc::clone(&self.cargo_env),
                    );

                    return Ok(None);
                }
                SearchAction::Error(err) => {
                    self.is_searching = false;
                    self.action_tx
                        .send(Action::UpdateStatus(StatusLevel::Error, err))
                        .ok();
                }
                SearchAction::SortBy(sort) => {
                    self.action_tx.send(Action::Focus(Focusable::Search))?;

                    if self.search_results.is_none() {
                        return Ok(None);
                    }

                    let status = format!("Sorting by: {}", sort);
                    return Ok(Some(Action::Search(SearchAction::Search(
                        self.input.value().into(),
                        sort,
                        1,
                        Some(status),
                    ))));
                }
                SearchAction::Scope(scope) => {
                    self.action_tx.send(Action::Focus(Focusable::Search))?;

                    self.scope = scope;

                    if self.search_results.is_none() {
                        return Ok(None);
                    }

                    return Ok(Some(Action::Search(SearchAction::Search(
                        self.input.value().into(),
                        self.sort_dropdown.get_selected(),
                        1,
                        Some(format!("Scoped to: {}", self.scope)),
                    ))));
                }
                SearchAction::Render(mut results) => {
                    self.is_searching = false;

                    let results_len = results.current_page_count();

                    let exact_match_ix = results.crates.iter().position(|c| c.exact_match);

                    if exact_match_ix.is_some() {
                        results.select_index(exact_match_ix);
                    } else if results_len > 0 {
                        results.select_index(Some(0));
                    }

                    self.search_results = Some(results);
                    self.show_usage = false;

                    if results_len > 0 {
                        self.action_tx.send(Action::UpdateStatusWithDuration(
                            StatusLevel::Success,
                            StatusDuration::Short,
                            format!("Loaded {results_len} results"),
                        ))?;
                    }
                }
                SearchAction::NavPagesForward(pages) => {
                    self.go_pages_forward(pages, self.input.value().to_string())?;
                }
                SearchAction::NavPagesBack(pages) => {
                    self.go_pages_back(pages, self.input.value().to_string())?;
                }
                SearchAction::NavFirstPage => {
                    self.go_to_page(1, self.input.value().to_string())?;
                }
                SearchAction::NavLastPage => {
                    self.go_to_page(usize::MAX, self.input.value().to_string())?;
                }
                _ => {
                    if let Some(results) = self.search_results.as_mut() {
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
            Action::Cargo(action) => match action {
                CargoAction::Add(crate_name, version) => {
                    let _ = crate_name;
                    let _ = version;
                    return Ok(Some(Action::RefreshCargoEnv));
                }
                CargoAction::Remove(crate_name) => {
                    let _ = crate_name;
                    return Ok(Some(Action::RefreshCargoEnv));
                }
                CargoAction::Update(crate_name) => {
                    let _ = crate_name;
                    return Ok(Some(Action::RefreshCargoEnv));
                }
                CargoAction::UpdateAll => {
                    return Ok(Some(Action::RefreshCargoEnv));
                }
            },
            Action::OpenReadme => {
                // TODO setting if open in browser or cli
                if let Some(url) = self
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
                //     let tx = self.action_tx.clone();
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
                if let Some(url) = self
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

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> AppResult<()> {
        let [left, right] =
            Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)])
                .areas(area);

        self.render_left(frame, left)?;
        self.render_right(frame, right)?;

        Ok(())
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Action;
    use pretty_assertions::assert_eq;
    use tokio::sync::mpsc;

    fn get_home() -> Home {
        let (action_tx, _) = mpsc::unbounded_channel();
        Home::new(Arc::new(Mutex::new(CargoEnv::new(None))), action_tx).unwrap()
    }

    fn get_home_and_tui() -> (Home, Tui) {
        let (action_tx, _) = mpsc::unbounded_channel();
        (
            Home::new(Arc::new(Mutex::new(CargoEnv::new(None))), action_tx).unwrap(),
            Tui::new().unwrap(),
        )
    }

    async fn execute_update(action: Action) -> (Home, Tui) {
        let mut home = get_home();
        let mut tui = Tui::new().unwrap();

        execute_update_with_home(&mut home, &mut tui, action).await;
        (home, tui)
    }

    async fn execute_update_with_home(home: &mut Home, tui: &mut Tui, action: Action) {
        let mut ac: Option<Action> = Some(action);

        while ac.is_some() {
            ac = home.update(ac.clone().unwrap(), tui).unwrap();
        }
    }

    #[tokio::test]
    async fn test_usage_shown_at_start() {
        let home = get_home();
        assert_eq!(home.show_usage, true);
    }

    #[tokio::test]
    async fn test_toggle_usage() {
        let (mut home, mut tui) = execute_update(Action::ToggleUsage).await;

        assert_eq!(home.show_usage, false);

        execute_update_with_home(&mut home, &mut tui, Action::ToggleUsage).await;

        assert_eq!(home.show_usage, true);
    }

    #[test]
    fn test_default_focus_is_search() {
        let home = get_home();
        assert_eq!(home.focused, Focusable::Search);
    }

    #[tokio::test]
    async fn test_focus_action() {
        let (home, _) = execute_update(Action::Focus(Focusable::Results)).await;
        assert_eq!(home.focused, Focusable::Results);
    }

    #[tokio::test]
    async fn test_focus_next_action() {
        let (home, _) = execute_update(Action::FocusNext).await;
        assert_eq!(home.focused, Focusable::Sort);
    }

    #[tokio::test]
    async fn test_focus_next_action_when_last_is_focused() {
        let (mut home, mut tui) = execute_update(Action::Focus(Focusable::DocsButton)).await;

        execute_update_with_home(&mut home, &mut tui, Action::FocusNext).await;

        assert_eq!(home.focused, Focusable::Search);
    }

    #[tokio::test]
    async fn test_focus_previous_action() {
        let (mut home, mut tui) = execute_update(Action::Focus(Focusable::DocsButton)).await;

        execute_update_with_home(&mut home, &mut tui, Action::FocusPrevious).await;

        assert_eq!(home.focused, Focusable::ReadmeButton);
    }

    #[tokio::test]
    async fn test_focus_previous_action_when_first_is_focused() {
        let (mut home, mut tui) = execute_update(Action::Focus(Focusable::Search)).await;

        execute_update_with_home(&mut home, &mut tui, Action::FocusPrevious).await;

        assert_eq!(home.focused, Focusable::DocsButton);
    }

    #[tokio::test]
    async fn test_search_clear_action() {
        let (mut home, mut tui) = get_home_and_tui();

        assert_eq!(true, home.input.value().is_empty());

        // simulate search
        home.search_results = Some(SearchResults::new(1));

        execute_update_with_home(&mut home, &mut tui, Action::Search(SearchAction::Clear)).await;

        assert_eq!(home.search_results, None);
    }
}
