use super::Component;
use crate::app::Mode;
use crate::{action::Action, config::Config, http_client};
use chrono::{DateTime, Utc};
use color_eyre::Result;
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use lazy_static::lazy_static;
use num_format::{Locale, ToFormattedString};
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
use std::str::FromStr;
use sys_locale::get_locale;
use tokio::sync::mpsc::UnboundedSender;
use tui_input::backend::crossterm::EventHandler;
use tui_input::Input;

lazy_static! {
    // TODO maybe remove 1 or both libs?
    static ref LOCALE_STR: String = get_locale().unwrap_or(String::from("en-US"));
    static ref LOCALE: Locale = Locale::from_str(LOCALE_STR.as_str()).unwrap_or(Locale::en);
}

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

    fn has_next_page(&self) -> bool {
        let so_far = self.meta.page * 100;
        so_far + 100 <= self.meta.total
    }

    fn has_prev_page(&self) -> bool {
        self.meta.page > 1
    }

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

    fn get_selected(&self) -> Option<&SearchItem> {
        if let Some(ix) = self.state.selected() {
            if let Some(item) = self.crates.get(ix) {
                return Some(item);
            }
        }

        None
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
    pub id: String,
    pub name: String,
    pub description: Option<String>,
    pub homepage: Option<String>,
    pub documentation: Option<String>,
    pub repository: Option<String>,
    pub max_version: String,
    pub max_stable_version: Option<String>,
    pub downloads: u64,
    pub recent_downloads: Option<u64>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    pub exact_match: bool,
}

impl SearchItem {
    fn version(&self) -> &str {
        match &self.max_stable_version {
            Some(v) => v,
            None => self.max_version.as_str(),
        }
    }
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
    hide_usage: bool,
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
                let version = i.version();

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
        let selected = self.search_results.get_selected();

        if self.hide_usage && selected.is_some() {
            let item = selected.unwrap();

            let updated_diff = Utc::now().signed_duration_since(item.updated_at);
            let updated_relative = if updated_diff.num_days() > 1 {
                format!("{} days ago", updated_diff.num_days())
            } else if updated_diff.num_hours() > 1 {
                format!("{} hours ago", updated_diff.num_hours())
            } else if updated_diff.num_seconds() > 1 {
                format!("{} minutes ago", updated_diff.num_minutes())
            } else {
                format!("{} seconds ago", updated_diff.num_seconds())
            };

            // TODO Why isn't it taking it from lazy static?
            let locale = Locale::from_str(LOCALE_STR.as_str()).unwrap_or(Locale::en);

            let text = Text::from(vec![
                Line::from(vec![
                    format!("{:<25}", "Description:").yellow().bold(),
                    item.description.clone().unwrap_or_default().bold(),
                ]),
                Line::default(),
                Line::from(vec![
                    format!("{:<25}", "Version:").yellow().bold(),
                    item.version().into(),
                ]),
                Line::from(vec![
                    format!("{:<25}", "Latest Version:").yellow().bold(),
                    item.max_version.to_string().into(),
                ]),
                Line::from(vec![
                    format!("{:<25}", "Home Page:").yellow().bold(),
                    item.homepage.clone().unwrap_or_default().into(),
                ]),
                Line::from(vec![
                    format!("{:<25}", "Documentation:").yellow().bold(),
                    item.documentation.clone().unwrap_or_default().into(),
                ]),
                Line::from(vec![
                    format!("{:<25}", "Repository:").yellow().bold(),
                    item.repository.clone().unwrap_or_default().into(),
                ]),
                Line::from(vec![
                    format!("{:<25}", "crates.io Page:").yellow().bold(),
                    format!("https://crates.io/crates/{}", item.id).into(),
                ]),
                Line::from(vec![
                    format!("{:<25}", "Downloads:").yellow().bold(),
                    item.downloads.to_formatted_string(&locale).into(),
                ]),
                Line::from(vec![
                    format!("{:<25}", "Recent Downloads:").yellow().bold(),
                    item.recent_downloads
                        .unwrap_or_default()
                        .to_formatted_string(&locale)
                        .into(),
                ]),
                Line::from(vec![
                    format!("{:<25}", "Created:").yellow().bold(),
                    item.created_at
                        .format("%d/%m/%Y %H:%M:%S (UTC)")
                        .to_string()
                        .into(),
                ]),
                Line::from(vec![
                    format!("{:<25}", "Updated:").yellow().bold(),
                    format!(
                        "{} ({})",
                        item.updated_at.format("%d/%m/%Y %H:%M:%S (UTC)"),
                        updated_relative
                    )
                    .into(),
                ]),
            ]);

            let right_block = Block::default()
                .title(format!(" 🧐 {} ", item.name))
                .title_style(self.config.styles[&Mode::Home]["title"])
                .padding(Padding::uniform(1))
                .borders(Borders::ALL);

            frame.render_widget(&right_block, area);

            frame.render_widget(
                Paragraph::new(text)
                    .wrap(Wrap { trim: true })
                    .scroll((0, 0)),
                right_block.inner(area),
            );
        } else {
            let text = Text::from(vec![
                Line::from(vec![
                    format!("{:<25}", "ENTER:").yellow().bold(),
                    "Search".bold(),
                ]),
                Line::from(vec![
                    format!("{:<25}", "a:").yellow().bold(),
                    "Add".bold(),
                    " (WIP)".gray()
                ]),
                Line::from(vec![
                    format!("{:<25}", "i:").yellow().bold(),
                    "Install".bold(),
                    " (WIP)".gray()
                ]),
                Line::from(vec![
                    format!("{:<25}", "Ctrl + o:").yellow().bold(),
                    "Open docs URL".bold(),
                    " (WIP)".gray()
                ]),
                Line::default(),
                Line::from(vec!["NAVIGATION".bold()]),
                Line::from(vec![
                    format!("{:<25}", "TAB:").yellow().bold(),
                    "Switch between boxes".bold(),
                ]),
                Line::from(vec![
                    format!("{:<25}", "ESC:").yellow().bold(),
                    "Go back to search; clear results".bold(),
                ]),
                Line::from(vec![
                    format!("{:<25}", "Ctrl + h:").yellow().bold(),
                    "Toggle this usage screen".bold(),
                ]),
                Line::from(vec![
                    format!("{:<25}", "Ctrl + z:").yellow().bold(),
                    "Suspend".bold(),
                ]),
                Line::from(vec![
                    format!("{:<25}", "Ctrl + c:").yellow().bold(),
                    "Quit".bold(),
                ]),
                Line::default(),
                Line::from(vec!["LIST".bold()]),
                Line::from(vec![
                    format!("{:<25}", "Up/Down:").yellow().bold(),
                    "Scroll in crate list".bold(),
                ]),
                Line::from(vec![
                    format!("{:<25}", "Home/End:").yellow().bold(),
                    "Go to first/last crate in list".bold(),
                ]),
                Line::default(),
                Line::from(vec!["PAGING".bold()]),
                Line::from(vec![
                    format!("{:<25}", "Left/Right:").yellow().bold(),
                    "Go backward/forward a page".bold(),
                ]),
                Line::from(vec![
                    format!("{:<25}", "Ctrl + Left/Right:").yellow().bold(),
                    "Go backward/forward 10 pages".bold(),
                ]),
                Line::from(vec![
                    format!("{:<25}", "Ctrl + Home/End:").yellow().bold(),
                    "Go to first/last page".bold(),
                ]),
            ]);

            let right_block = Block::default()
                .title(" 📖 Usage ")
                .title_style(self.config.styles[&Mode::Home]["title"])
                .padding(Padding::uniform(1))
                .borders(Borders::ALL);

            frame.render_widget(&right_block, area);

            frame.render_widget(
                Paragraph::new(text)
                    .wrap(Wrap { trim: true })
                    .scroll((0, 0)),
                right_block.inner(area),
            );
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
        let has_results = !self.search_results.crates.is_empty();
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

        match key.code {
            KeyCode::Enter if self.focused == Focus::Search => {
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
            KeyCode::Char('h') if ctrl => {
                self.hide_usage = !self.hide_usage;
            }
            // Page navigation
            KeyCode::Right
                if self.search_results.has_next_page() && self.focused == Focus::Results =>
            {
                let pages = match ctrl {
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
                if self.search_results.has_prev_page() && self.focused == Focus::Results =>
            {
                let pages = match ctrl {
                    true => 10,
                    false => 1,
                };

                self.search_results.go_back_pages(
                    pages,
                    self.input.value().to_string(),
                    self.command_tx.clone().unwrap(),
                );
            }
            KeyCode::Home if ctrl && has_results => {
                self.focused = Focus::Results;
                self.search_results.go_to_page(
                    1,
                    self.input.value().to_string(),
                    self.command_tx.clone().unwrap(),
                );
            }
            KeyCode::End if ctrl && has_results => {
                self.focused = Focus::Results;
                self.search_results.go_to_page(
                    self.search_results.pages(),
                    self.input.value().to_string(),
                    self.command_tx.clone().unwrap(),
                );
            }
            // List navigation
            KeyCode::Down if has_results => {
                self.focused = Focus::Results;
                self.search_results.select_next();
            }
            KeyCode::Up if has_results => {
                self.focused = Focus::Results;
                self.search_results.select_previous();
            }
            KeyCode::Home if has_results && self.focused == Focus::Results => {
                self.search_results.select_first();
            }
            KeyCode::End if has_results && self.focused == Focus::Results => {
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
                // TODO this isn't that great
                if self.searching {
                    std::thread::sleep(std::time::Duration::from_secs(1));
                }

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
                let changed_pages = page != self.search_results.meta.page;

                self.hide_usage = true;
                self.searching = false;
                self.search_results = results;
                self.search_results.meta.page = page;

                if exact_match_ix.is_some() {
                    self.focused = Focus::Results;
                    self.search_results.state.select(exact_match_ix);
                } else if changed_pages && self.search_results.current_page_len() > 0 {
                    self.search_results.state.select(Some(0));
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
