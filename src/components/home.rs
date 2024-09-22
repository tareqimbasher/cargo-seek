use super::Component;
use crate::app::Mode;
use crate::{action::Action, config::Config, http_client};
use chrono::{DateTime, Utc};
use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::layout::Alignment;
use ratatui::text::{Line, Text};
use ratatui::widgets::block::Title;
use ratatui::widgets::Wrap;
use ratatui::{
    layout::{Constraint, Layout, Rect},
    style::{Modifier, Style, Stylize},
    widgets::{Block, Borders, List, ListItem, ListState, Padding, Paragraph},
    Frame,
};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;

#[derive(Default, PartialEq)]
enum Focus {
    #[default]
    Search,
    Results,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Meta {
    #[serde(default)]
    page: u32,
    next_page: Option<String>,
    prev_page: Option<String>,
    total: u32,
}

#[derive(Debug, Default, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchResults {
    crates: Vec<SearchItem>,
    meta: Meta,
    #[serde(default)]
    state: ListState,
}

impl SearchResults {
    fn total_items(&self) -> u32 {
        self.meta.total
    }

    fn current_page_len(&self) -> usize {
        self.crates.len()
    }

    fn current_page(&self) -> u32 {
        self.meta.page
    }

    fn pages(&self) -> u32 {
        self.meta.total.div_ceil(100)
    }

    // fn has_next_page(&self) -> bool {
    //     let so_far = self.meta.page * 100;
    //     so_far + 100 <= self.meta.total
    // }
    //
    // fn has_prev_page(&self) -> bool {
    //     self.meta.page > 1
    // }

    // fn go_to_next_page(&self, query: String, command_tx: UnboundedSender<Action>) {
    //     if self.has_next_page() {
    //         command_tx
    //             .send(Action::Search(query, self.meta.page + 1))
    //             .unwrap()
    //     }
    // }

    // fn go_to_prev_page(&self, query: String, command_tx: UnboundedSender<Action>) {
    //     if self.has_prev_page() {
    //         command_tx
    //             .send(Action::Search(query, self.meta.page - 1))
    //             .unwrap()
    //     }
    // }

    fn go_back_pages(&self, pages: u32, query: String, command_tx: UnboundedSender<Action>) {
        let requested_page = if pages >= self.meta.page {
            1
        } else {
            self.meta.page - pages
        };

        if requested_page == self.current_page() {
            return;
        }

        command_tx
            .send(Action::Search(query, requested_page))
            .unwrap()
    }

    fn go_to_page(&self, page: u32, query: String, command_tx: UnboundedSender<Action>) {
        let requested_page = if page >= self.pages() {
            self.pages()
        } else {
            page
        };

        if requested_page == self.current_page() {
            return;
        }

        command_tx
            .send(Action::Search(query, requested_page))
            .unwrap()
    }

    fn go_next_pages(&self, pages: u32, query: String, command_tx: UnboundedSender<Action>) {
        let mut requested_page = self.meta.page + pages;

        if requested_page > self.pages() {
            requested_page = self.pages();
        }

        if requested_page == self.current_page() {
            return;
        }

        command_tx
            .send(Action::Search(query, requested_page))
            .unwrap()
    }

    fn select_next(&mut self) {
        self.state.select_next();
    }

    fn select_previous(&mut self) {
        self.state.select_previous();
    }

    fn select_first(&mut self) {
        self.state.select_first();
    }

    fn select_last(&mut self) {
        self.state.select_last();
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SearchItem {
    pub exact_match: bool,
    pub name: String,
    pub newest_version: String,
    pub max_version: String,
    pub max_stable_version: Option<String>,
    pub description: String,
    pub downloads: i64,
    pub recent_downloads: i64,
    pub homepage: Option<String>,
    pub documentation: Option<String>,
    pub repository: Option<String>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
}

#[derive(Default)]
pub struct Home {
    input: Input,
    focused: Focus,
    searching: bool,
    search_results: SearchResults,
    spinner_state: throbber_widgets_tui::ThrobberState,
    command_tx: Option<UnboundedSender<Action>>,
    config: Config,
}

impl Home {
    pub fn new() -> Self {
        Self::default()
    }

    fn render_left(&mut self, frame: &mut Frame, area: Rect) {
        let [search, results] =
            Layout::vertical([Constraint::Length(3), Constraint::Min(5)]).areas(area);

        self.render_search(frame, search);
        self.render_results(frame, results);
    }

    fn render_search(&mut self, frame: &mut Frame, area: Rect) {
        let spinner_len = if self.searching { 3 } else { 0 };

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
                        Focus::Search => self.config.styles[&Mode::Home]["focus"],
                        _ => Style::default(),
                    }),
            );
        frame.render_widget(input, search);

        match self.focused {
            Focus::Search => {
                // Make the cursor visible and ask ratatui to put it at the specified coordinates after rendering
                frame.set_cursor_position((
                    // Put cursor past the end of the input text
                    search.x + (self.input.visual_cursor().max(scroll) - scroll) as u16 + 1,
                    // Move one line down, from the border to the input line
                    search.y + 1,
                ))
            }
            Focus::Results =>
                // Hide the cursor. `Frame` does this by default, so we don't need to do anything here
                {}
        }

        if self.searching {
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
    }

    fn render_results(&mut self, frame: &mut Frame, area: Rect) {
        let results = &self.search_results;

        let correction = match results.state.selected() {
            Some(_) => 4,
            None => 2,
        };

        let list_items: Vec<ListItem> = results
            .crates
            .iter()
            .map(|i| {
                let name = i.name.as_str();
                let version = i.newest_version.as_str();

                let space_between = area.width as usize - (name.len() + version.len()) - correction;
                let line = format!("{}{}{}", name, " ".repeat(space_between), version);

                ListItem::new(line)
            })
            .collect();

        let selected = match results.state.selected() {
            None => 0,
            Some(s) => {
                if s == usize::MAX {
                    results.current_page_len()
                } else if s == usize::MIN {
                    1
                } else if s > results.current_page_len() - 1 {
                    // ListState select_next() increments selected even after last item is selected
                    s
                } else {
                    s + 1
                }
            }
        };

        let list = List::new(list_items)
            .block(
                Block::default()
                    .title(format!(" {}/{} ", selected, results.current_page_len()))
                    .title(
                        Title::from(format!(
                            " Page {} of {} ({} items) ",
                            results.current_page(),
                            results.pages(),
                            results.total_items()
                        ))
                        .alignment(Alignment::Right),
                    )
                    .borders(Borders::ALL)
                    .border_style(match self.focused {
                        Focus::Results => self.config.styles[&Mode::Home]["focus"],
                        _ => Style::default(),
                    }),
            )
            .highlight_style(self.config.styles[&Mode::Home]["focus"].add_modifier(Modifier::BOLD))
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, area, &mut self.search_results.state);
    }

    fn render_right(&mut self, frame: &mut Frame, area: Rect) {
        let mut selected: Option<&SearchItem> = None;
        let mut crate_details: Option<Paragraph> = None;

        if let Some(ix) = self.search_results.state.selected() {
            if let Some(item) = self.search_results.crates.get(ix) {
                selected = Some(item);

                let text = Text::from(vec![
                    Line::from(vec![
                        item.name.as_str().green(),
                        " ".into(),
                        item.newest_version.as_str().yellow().bold(),
                    ]),
                    Line::from(vec![
                        "Description: ".into(),
                        item.description.as_str().blue(),
                    ]),
                    Line::from(vec![
                        "Downloads: ".into(),
                        item.downloads.to_string().green(),
                    ]),
                ]);

                crate_details = Some(
                    Paragraph::new(text)
                        .wrap(Wrap { trim: true })
                        .scroll((1, 0)),
                );
            }
        }

        let right_block = Block::default()
            .title(if let Some(item) = selected {
                format!(" {} ", item.name)
            } else {
                " Usage ".to_string()
            })
            .title_style(self.config.styles[&Mode::Home]["title"])
            .padding(Padding::uniform(1))
            .borders(Borders::ALL);

        frame.render_widget(&right_block, area);

        match crate_details {
            None => {}
            Some(p) => {
                frame.render_widget(p, right_block.inner(area));
            }
        }
    }
}

impl Component for Home {
    fn register_action_handler(&mut self, tx: UnboundedSender<Action>) -> Result<()> {
        self.command_tx = Some(tx);
        Ok(())
    }

    fn register_config_handler(&mut self, config: Config) -> Result<()> {
        self.config = config;
        Ok(())
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> Result<Option<Action>> {
        match key.code {
            KeyCode::Enter => {
                return Ok(Action::Search(self.input.value().to_string(), 1).into());
            }
            KeyCode::Esc => {
                if self.focused == Focus::Search {
                    self.input.reset();
                    self.search_results = SearchResults::default();
                } else if self.focused == Focus::Results {
                    self.focused = Focus::Search;
                }
            }
            // Page navigation
            KeyCode::Right
                if !self.search_results.crates.is_empty() && self.focused == Focus::Results =>
            {
                let pages = match key.modifiers.contains(KeyModifiers::CONTROL) {
                    true => 10,
                    false => 1,
                };

                self.search_results.go_next_pages(
                    pages,
                    self.input.value().to_string(),
                    self.command_tx.clone().unwrap(),
                );
            }
            KeyCode::Left
                if !self.search_results.crates.is_empty() && self.focused == Focus::Results =>
            {
                self.focused = Focus::Results;

                let pages = match key.modifiers.contains(KeyModifiers::CONTROL) {
                    true => 10,
                    false => 1,
                };

                self.search_results.go_back_pages(
                    pages,
                    self.input.value().to_string(),
                    self.command_tx.clone().unwrap(),
                );
            }
            KeyCode::Home
                if !self.search_results.crates.is_empty()
                    && key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.focused = Focus::Results;
                self.search_results.go_to_page(
                    1,
                    self.input.value().to_string(),
                    self.command_tx.clone().unwrap(),
                );
            }
            KeyCode::End
                if !self.search_results.crates.is_empty()
                    && key.modifiers.contains(KeyModifiers::CONTROL) =>
            {
                self.focused = Focus::Results;
                self.search_results.go_to_page(
                    self.search_results.pages(),
                    self.input.value().to_string(),
                    self.command_tx.clone().unwrap(),
                );
            }
            // List navigation
            KeyCode::Down if !self.search_results.crates.is_empty() => {
                self.focused = Focus::Results;
                self.search_results.select_next();
            }
            KeyCode::Up if !self.search_results.crates.is_empty() => {
                self.focused = Focus::Results;
                self.search_results.select_previous();
            }
            KeyCode::Home if !self.search_results.crates.is_empty() => {
                self.focused = Focus::Results;
                self.search_results.select_first();
            }
            KeyCode::End if !self.search_results.crates.is_empty() => {
                self.focused = Focus::Results;
                self.search_results.select_last();
            }
            KeyCode::BackTab => {
                return Ok(Action::FocusPrevious.into());
            }
            KeyCode::Tab => {
                return Ok(Action::FocusNext.into());
            }
            _ => match self.focused {
                Focus::Search => {
                    self.input.handle_event(&crossterm::event::Event::Key(key));
                }
                Focus::Results => {}
            },
        }
        Ok(None)
    }

    fn update(&mut self, action: Action) -> Result<Option<Action>> {
        match action {
            Action::Tick => {
                // add any logic here that should run on every tick
                if self.searching {
                    self.spinner_state.calc_next();
                }
            }
            Action::Render => {
                // add any logic here that should run on every render
            }
            Action::FocusNext | Action::FocusPrevious => match self.focused {
                Focus::Search if !self.search_results.crates.is_empty() => {
                    self.focused = Focus::Results;

                    if self.search_results.state.selected().is_none() {
                        self.search_results.state.select_next()
                    }
                }
                Focus::Results => {
                    self.focused = Focus::Search;
                }
                _ => {}
            },
            Action::Search(query, page) => {
                self.searching = true;
                let tx = self.command_tx.clone();

                tokio::spawn(async move {
                    let url = format!(
                        "https://crates.io/api/v1/crates?q={}&per_page=100&page={}",
                        query, page
                    );

                    let json = http_client::CLIENT
                        .get(&url)
                        .send()
                        .await
                        .unwrap()
                        .text()
                        .await
                        .unwrap();

                    // TODO if deserialization fails, log it
                    let response = serde_json::from_str::<SearchResults>(&json).unwrap_or_default();

                    tx.unwrap()
                        .send(Action::RenderSearchResults(response, page))
                        .unwrap();
                });
            }
            Action::RenderSearchResults(results, page) => {
                let exact_match_ix = results.crates.iter().position(|c| c.exact_match);

                self.searching = false;
                self.search_results = results;
                self.search_results.meta.page = page;

                if exact_match_ix.is_some() {
                    self.focused = Focus::Results;
                    self.search_results.state.select(exact_match_ix);
                }
            }
            _ => {}
        }
        Ok(None)
    }

    fn draw(&mut self, frame: &mut Frame, area: Rect) -> Result<()> {
        let [left, right] =
            Layout::horizontal([Constraint::Percentage(40), Constraint::Percentage(60)])
                .areas(area);

        self.render_left(frame, left);
        self.render_right(frame, right);

        Ok(())
    }
}
