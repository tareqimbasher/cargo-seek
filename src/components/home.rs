pub mod types;

use super::Component;

use chrono::Utc;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use enum_iterator::{all, reverse_all, Sequence};
use ratatui::style::Styled;
use ratatui::{
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Text},
    widgets::{block::Title, Block, Borders, List, ListItem, Padding, Paragraph, Wrap},
    Frame,
};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use std::{fs, io::Write, iter::Cycle, process::Command};
use tokio::sync::mpsc::UnboundedSender;
use tui_input::{backend::crossterm::EventHandler, Input};

use crate::components::button::{Button, State, BLUE, GRAY, ORANGE, PURPLE};
use crate::components::home::types::{Crate, SearchResults};
use crate::errors::AppResult;
use crate::tui::Tui;
use crate::util::Util;
use crate::{
    action::{Action, SearchAction},
    app::Mode,
    config::Config,
    http_client,
};

#[derive(Default, PartialEq, Clone, Debug, Eq, Sequence, Serialize, Deserialize)]
pub enum Focusable {
    #[default]
    Search,
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

#[derive(Default)]
pub struct Home {
    input: Input,
    show_usage: bool,
    focused: Focusable,
    search_results: Option<SearchResults>,
    spinner_state: throbber_widgets_tui::ThrobberState,
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
}

impl Home {
    pub fn new() -> Self {
        Self {
            show_usage: true,
            ..Default::default()
        }
    }

    // fn send_action(&self, action: Action) -> AppResult<()> {
    //     if let Some(ref sender) = self.command_tx {
    //         sender.send(action)?;
    //         Ok(())
    //     } else {
    //         Err(AppError::CommandChannelNotInitialized(
    //             std::any::type_name::<Self>().into(),
    //         ))
    //     }
    // }

    fn reset(&mut self) -> AppResult<()> {
        self.input.reset();
        self.search_results = None;
        Ok(())
    }

    fn render_left(&mut self, frame: &mut Frame, area: Rect) -> AppResult<()> {
        let [search, results] =
            Layout::vertical([Constraint::Length(3), Constraint::Min(5)]).areas(area);

        self.render_search(frame, search)?;
        self.render_results(frame, results)?;

        Ok(())
    }

    fn render_search(&mut self, frame: &mut Frame, area: Rect) -> AppResult<()> {
        let spinner_len = if http_client::INSTANCE.is_working() {
            3
        } else {
            0
        };

        let [search, spinner] =
            Layout::horizontal([Constraint::Min(1), Constraint::Length(spinner_len)]).areas(area);

        // The width of the input area, removing 2 for the width of the border on each side
        let scroll_width = search.width - 2;
        let scroll = self.input.visual_scroll(scroll_width as usize);
        let input = Paragraph::new(self.input.value())
            .scroll((0, scroll as u16))
            .block(
                Block::default()
                    .title(" Search ")
                    .borders(Borders::ALL)
                    .border_style(match self.focused {
                        Focusable::Search => self.config.styles[&Mode::Home]["accent"],
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

        if http_client::INSTANCE.is_working() {
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
                Focusable::Results => self.config.styles[&Mode::Home]["accent"],
                _ => Style::default(),
            });

        if let Some(results) = self.search_results.as_mut() {
            let correction = match results.get_selected_index() {
                Some(_) => 4,
                None => 2,
            };

            let list_items: Vec<ListItem> = results
                .crates
                .iter()
                .map(|i| {
                    let name = i.name.as_str();
                    let version = i.version();

                    let space_between =
                        area.width as usize - (name.len() + version.len()) - correction;
                    let line = format!("{}{}{}", name, " ".repeat(space_between), version);

                    ListItem::new(line)
                })
                .collect();

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

            let list = List::new(list_items)
                .block(
                    block
                        .title(Title::from(format!(
                            " {}/{} ",
                            selected_item_num,
                            results.current_page_count()
                        )))
                        .title(
                            Title::from(format!(
                                " Page {} of {} ({} items) ",
                                results.current_page(),
                                results.page_count(),
                                results.total_count()
                            ))
                            .alignment(Alignment::Right),
                        ),
                )
                .highlight_style(self.config.styles[&Mode::Home]["accent"].bold())
                .highlight_symbol("> ");

            frame.render_stateful_widget(list, area, results.list_state());
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

        let text = Text::from(vec![
            Line::from(vec![
                format!("{:<25}", "ENTER:").set_style(prop_style),
                "Search".bold(),
            ]),
            Line::from(vec![
                format!("{:<25}", "a:").set_style(prop_style),
                "Add".bold(),
                " (WIP)".gray(),
            ]),
            Line::from(vec![
                format!("{:<25}", "i:").set_style(prop_style),
                "Install".bold(),
                " (WIP)".gray(),
            ]),
            Line::from(vec![
                format!("{:<25}", "Ctrl + o:").set_style(prop_style),
                "Open docs URL".bold(),
                " (WIP)".gray(),
            ]),
            Line::default(),
            Line::from(vec!["NAVIGATION".bold()]),
            Line::from(vec![
                format!("{:<25}", "TAB:").set_style(prop_style),
                "Switch between boxes".bold(),
            ]),
            Line::from(vec![
                format!("{:<25}", "ESC:").set_style(prop_style),
                "Go back to search; clear results".bold(),
            ]),
            Line::from(vec![
                format!("{:<25}", "Ctrl + h:").set_style(prop_style),
                "Toggle this usage screen".bold(),
            ]),
            Line::from(vec![
                format!("{:<25}", "Ctrl + z:").set_style(prop_style),
                "Suspend".bold(),
            ]),
            Line::from(vec![
                format!("{:<25}", "Ctrl + c:").set_style(prop_style),
                "Quit".bold(),
            ]),
            Line::default(),
            Line::from(vec!["LIST".bold()]),
            Line::from(vec![
                format!("{:<25}", "Up/Down:").set_style(prop_style),
                "Scroll in crate list".bold(),
            ]),
            Line::from(vec![
                format!("{:<25}", "Home/End:").set_style(prop_style),
                "Go to first/last crate in list".bold(),
            ]),
            Line::default(),
            Line::from(vec!["PAGING".bold()]),
            Line::from(vec![
                format!("{:<25}", "Left/Right:").set_style(prop_style),
                "Go backward/forward a page".bold(),
            ]),
            Line::from(vec![
                format!("{:<25}", "Ctrl + Left/Right:").set_style(prop_style),
                "Go backward/forward 10 pages".bold(),
            ]),
            Line::from(vec![
                format!("{:<25}", "Ctrl + Home/End:").set_style(prop_style),
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
                .wrap(Wrap { trim: true })
                .scroll((0, 0)),
            block.inner(area),
        );

        Ok(())
    }

    fn render_crate_details(&self, krate: &Crate, frame: &mut Frame, area: Rect) -> AppResult<()> {
        let main_block = Block::default()
            .title(format!(" 🧐 {} ", krate.name))
            .title_style(self.config.styles[&Mode::Home]["title"])
            .padding(Padding::uniform(1))
            .borders(Borders::ALL)
            .border_style(match self.focused {
                Focusable::AddButton
                | Focusable::InstallButton
                | Focusable::ReadmeButton
                | Focusable::DocsButton => self.config.styles[&Mode::Home]["accent"],
                _ => Style::default(),
            });

        let left_column_width = 25;
        let updated_relative = Util::get_relative_time(krate.updated_at, Utc::now());

        let prop_style = self.config.styles[&Mode::Home]["accent"].bold();

        let text = Text::from(vec![
            Line::from(vec![
                format!("{:<left_column_width$}", "Description:").set_style(prop_style),
                krate.description.clone().unwrap_or_default().bold(),
            ]),
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
                format!("{:<25}", "Documentation:").set_style(prop_style),
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
                Util::format_number(krate.recent_downloads.unwrap_or_default()).into(),
            ]),
            Line::from(vec![
                format!("{:<left_column_width$}", "Created:").set_style(prop_style),
                krate
                    .created_at
                    .format("%d/%m/%Y %H:%M:%S (UTC)")
                    .to_string()
                    .into(),
            ]),
            Line::from(vec![
                format!("{:<left_column_width$}", "Updated:").set_style(prop_style),
                format!(
                    "{} ({})",
                    krate.updated_at.format("%d/%m/%Y %H:%M:%S (UTC)"),
                    updated_relative
                )
                .into(),
            ]),
        ]);

        let details_paragraph_lines = text.lines.len();
        let details_paragraph = Paragraph::new(text);

        frame.render_widget(&main_block, area);

        let [details_area, _, buttons_row1_area, _, buttons_row2_area] = Layout::vertical([
            Constraint::Length(details_paragraph_lines as u16),
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
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> AppResult<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> AppResult<()> {
        self.config = config;
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
            KeyCode::BackTab => {
                return Ok(Some(Action::FocusPrevious));
            }
            KeyCode::Tab => {
                return Ok(Some(Action::FocusNext));
            }
            KeyCode::Enter => match self.focused {
                Focusable::Search => {
                    return Ok(Some(Action::Search(SearchAction::Search(
                        self.input.value().to_string(),
                        1,
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

            return Ok(None);
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
                        return Ok(Some(Action::Search(SearchAction::NavNextPage(pages))));
                    }
                    KeyCode::Left if results.has_prev_page() => {
                        let pages = match ctrl {
                            true => 10,
                            false => 1,
                        };
                        return Ok(Some(Action::Search(SearchAction::NavNextPage(pages))));
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

            return Ok(None);
        }

        Ok(None)
    }

    fn update(&mut self, action: Action, tui: &mut Tui) -> AppResult<Option<Action>> {
        match action {
            Action::Tick => {
                // add any logic here that should run on every tick
                if http_client::INSTANCE.is_working() {
                    self.spinner_state.calc_next();
                }
            }
            Action::Render => {
                // add any logic here that should run on every render
            }
            Action::Focus(focusable) => {
                self.focused = focusable;
            }
            Action::FocusNext => {
                if let Some(results) = &self.search_results {
                    if results.get_selected_index().is_some() {
                        return Ok(Some(Action::Focus(self.focused.next())));
                    }
                }
                return Ok(Some(Action::Focus(Focusable::Search)));
            }
            Action::FocusPrevious => return Ok(Some(Action::Focus(self.focused.prev()))),
            Action::ToggleUsage => {
                self.show_usage = !self.show_usage;
            }
            Action::Search(action) => match action {
                SearchAction::Search(term, page) => {
                    if let Some(tx) = self.command_tx.clone() {
                        tokio::spawn(async move {
                            if let Ok(results) = http_client::INSTANCE.search(term, 100, page).await
                            {
                                tx.send(Action::Search(SearchAction::Render(results)))
                                    .unwrap();
                            }
                        });
                    }
                }
                SearchAction::Render(mut results) => {
                    let exact_match_ix = results.crates.iter().position(|c| c.exact_match);

                    if exact_match_ix.is_some() {
                        results.select_index(exact_match_ix);
                    } else if results.current_page_count() > 0 {
                        results.select_index(Some(0));
                    }

                    self.search_results = Some(results);
                    self.show_usage = false;
                }
                SearchAction::Clear => self.reset()?,
                _ => {
                    if let Some(results) = self.search_results.as_mut() {
                        match action {
                            SearchAction::NavNextPage(pages) => {
                                results.go_next_pages(
                                    pages,
                                    self.input.value().to_string(),
                                    self.command_tx.clone().unwrap(),
                                )?;
                            }
                            SearchAction::NavPrevPage(pages) => {
                                results.go_prev_pages(
                                    pages,
                                    self.input.value().to_string(),
                                    self.command_tx.clone().unwrap(),
                                )?;
                            }
                            SearchAction::NavFirstPage => {
                                results.go_to_page(
                                    1,
                                    self.input.value().to_string(),
                                    self.command_tx.clone().unwrap(),
                                )?;
                            }
                            SearchAction::NavLastPage => {
                                results.go_to_page(
                                    results.page_count(),
                                    self.input.value().to_string(),
                                    self.command_tx.clone().unwrap(),
                                )?;
                            }
                            SearchAction::SelectIndex(index) => {
                                if let Some(selected) = results.select_index(index) {
                                    if selected.repository.is_none() {
                                        return Ok(None);
                                    }

                                    let repository = selected.repository.as_ref().unwrap();

                                    if repository.is_empty() {
                                        return Ok(None);
                                    }
                                }
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
            Action::OpenReadme => {
                // TODO setting if open in browser or cli
                // if let Some(url) = self
                //     .search_results
                //     .as_ref()
                //     .and_then(|results| results.get_selected())
                //     .and_then(|krate| krate.repository.as_ref())
                //     .and_then(|docs| Url::parse(docs).ok())
                // {
                //     open::that(url.to_string())?;
                // }

                if let Some(url) = self
                    .search_results
                    .as_ref()
                    .and_then(|results| results.get_selected())
                    .and_then(|krate| krate.repository.as_ref())
                    .and_then(|docs| Url::parse(docs).ok())
                {
                    let tx = self.command_tx.clone();

                    tokio::spawn(async move {
                        if let Some(markdown) = http_client::INSTANCE
                            .get_repo_readme(url.to_string())
                            .await
                            .unwrap()
                        {
                            tx.unwrap().send(Action::RenderReadme(markdown)).unwrap();
                        }
                    });
                }
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
}
