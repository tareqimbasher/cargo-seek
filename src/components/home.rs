use std::{fs, io::Write, process::Command};

use super::Component;

use chrono::{DateTime, Utc};
use crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::Styled;
use ratatui::{
    layout::{Alignment, Constraint, Flex, Layout, Rect},
    style::{Style, Stylize},
    text::{Line, Text},
    widgets::{block::Title, Block, Borders, List, ListItem, ListState, Padding, Paragraph, Wrap},
    Frame,
};
use serde::{Deserialize, Serialize};
use tokio::sync::mpsc::UnboundedSender;
use tui_input::{backend::crossterm::EventHandler, Input};

use crate::components::button::{Button, BLUE, GRAY, ORANGE, PURPLE};
use crate::errors::{AppError, AppResult};
use crate::tui::Tui;
use crate::util::Util;
use crate::{
    action::{Action, SearchAction},
    app::Mode,
    config::Config,
    http_client,
};

#[derive(Default, PartialEq, Clone, Debug, Eq, Serialize, Deserialize)]
pub enum Focusable {
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
    crates: Vec<Crate>,
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

    fn go_prev_pages(
        &self,
        pages: u32,
        query: String,
        command_tx: UnboundedSender<Action>,
    ) -> AppResult<()> {
        let requested_page = if pages >= self.meta.page {
            1
        } else {
            self.meta.page - pages
        };

        if requested_page == self.current_page() {
            return Ok(());
        }

        command_tx.send(Action::Search(SearchAction::Search(query, requested_page)))?;

        Ok(())
    }

    fn go_to_page(
        &self,
        page: u32,
        query: String,
        command_tx: UnboundedSender<Action>,
    ) -> AppResult<()> {
        let requested_page = if page >= self.pages() {
            self.pages()
        } else {
            page
        };

        if requested_page == self.current_page() {
            return Ok(());
        }

        command_tx.send(Action::Search(SearchAction::Search(query, requested_page)))?;

        Ok(())
    }

    fn go_next_pages(
        &self,
        pages: u32,
        query: String,
        command_tx: UnboundedSender<Action>,
    ) -> AppResult<()> {
        let mut requested_page = self.meta.page + pages;

        if requested_page > self.pages() {
            requested_page = self.pages();
        }

        if requested_page == self.current_page() {
            return Ok(());
        }

        command_tx.send(Action::Search(SearchAction::Search(query, requested_page)))?;

        Ok(())
    }

    fn get_selected(&self) -> Option<&Crate> {
        if let Some(ix) = self.state.selected() {
            if let Some(item) = self.crates.get(ix) {
                return Some(item);
            }
        }

        None
    }

    fn select(&mut self, index: Option<usize>) -> Option<&Crate> {
        self.state.select(index);
        self.get_selected()
    }

    fn select_next(&mut self) -> Option<&Crate> {
        self.state.select_next();
        self.get_selected()
    }

    fn select_previous(&mut self) -> Option<&Crate> {
        self.state.select_previous();
        self.get_selected()
    }

    fn select_first(&mut self) -> Option<&Crate> {
        self.state.select_first();
        self.get_selected()
    }

    fn select_last(&mut self) -> Option<&Crate> {
        self.state.select_last();
        self.get_selected()
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct Crate {
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

impl Crate {
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
    show_usage: bool,
    focused: Focusable,
    searching: bool,
    search_results: SearchResults,
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

    fn send_action(&self, action: Action) -> AppResult<()> {
        if let Some(ref sender) = self.command_tx {
            sender.send(action)?;
            Ok(())
        } else {
            Err(AppError::CommandChannelNotInitialized(
                std::any::type_name::<Self>().into(),
            ))
        }
    }

    fn reset(&mut self) -> AppResult<()> {
        self.input.reset();
        self.search_results = SearchResults::default();

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
                        Focusable::Search => self.config.styles[&Mode::Home]["accent"],
                        _ => Style::default(),
                    }),
            );
        frame.render_widget(input, search);

        match self.focused {
            Focusable::Search => {
                // Make the cursor visible and ask ratatui to put it at the specified coordinates after rendering
                frame.set_cursor_position((
                    // Put cursor past the end of the input text
                    search.x + (self.input.visual_cursor().max(scroll) - scroll) as u16 + 1,
                    // Move one line down, from the border to the input line
                    search.y + 1,
                ))
            }
            Focusable::Results =>
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

        Ok(())
    }

    fn render_results(&mut self, frame: &mut Frame, area: Rect) -> AppResult<()> {
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
                        Focusable::Results => self.config.styles[&Mode::Home]["accent"],
                        _ => Style::default(),
                    }),
            )
            .highlight_style(self.config.styles[&Mode::Home]["accent"].bold())
            .highlight_symbol("> ");
        frame.render_stateful_widget(list, area, &mut self.search_results.state);

        Ok(())
    }

    fn render_right(&mut self, frame: &mut Frame, area: Rect) -> AppResult<()> {
        if self.show_usage {
            self.render_usage(frame, area)?;
        } else if let Some(krate) = self.search_results.get_selected() {
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
            .borders(Borders::ALL);

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
        frame.render_widget(Button::new("Add").theme(BLUE), button1_area);
        frame.render_widget(Button::new("Install").theme(PURPLE), button2_area);

        // Buttons row 2
        let [property_area, button1_area, _, button2_area] =
            buttons_row_layout.areas(buttons_row2_area);

        frame.render_widget(Text::from("Links:").set_style(prop_style), property_area);

        let mut button_areas = vec![button1_area, button2_area];

        if krate.repository.is_some() {
            frame.render_widget(Button::new("README").theme(GRAY), button_areas.remove(0));
        }

        if krate.documentation.is_some() {
            frame.render_widget(Button::new("Docs").theme(ORANGE), button_areas.remove(0));
        }

        Ok(())
    }

    fn render_no_results(&self, frame: &mut Frame, area: Rect) -> AppResult<()> {
        let text = Text::raw("0 crates found");
        let centered = self.center(
            area,
            Constraint::Length(text.width() as u16),
            Constraint::Length(1),
        )?;
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
        let has_results = !self.search_results.crates.is_empty();
        let ctrl = key.modifiers.contains(KeyModifiers::CONTROL);

        match key.code {
            KeyCode::Enter if self.focused == Focusable::Search => {
                return Ok(Some(Action::Search(SearchAction::Search(
                    self.input.value().to_string(),
                    1,
                ))));
            }
            KeyCode::Esc => {
                if self.focused == Focusable::Search {
                    return Ok(Some(Action::Search(SearchAction::Clear)));
                } else if self.focused == Focusable::Results {
                    return Ok(Some(Action::Focus(Focusable::Search)));
                }
            }
            KeyCode::Char('h') if ctrl => {
                return Ok(Some(Action::ToggleUsage));
            }
            // Page navigation
            KeyCode::Right
                if self.search_results.has_next_page() && self.focused == Focusable::Results =>
            {
                let pages = match ctrl {
                    true => 10,
                    false => 1,
                };
                return Ok(Some(Action::Search(SearchAction::NavNextPage(pages))));
            }
            KeyCode::Left
                if self.search_results.has_prev_page() && self.focused == Focusable::Results =>
            {
                let pages = match ctrl {
                    true => 10,
                    false => 1,
                };
                return Ok(Some(Action::Search(SearchAction::NavNextPage(pages))));
            }
            KeyCode::Home if ctrl && has_results => {
                self.send_action(Action::Focus(Focusable::Results))?;
                return Ok(Some(Action::Search(SearchAction::NavFirstPage)));
            }
            KeyCode::End if ctrl && has_results => {
                self.send_action(Action::Focus(Focusable::Results))?;
                return Ok(Some(Action::Search(SearchAction::NavLastPage)));
            }
            // List navigation
            KeyCode::Down if has_results => {
                self.send_action(Action::Focus(Focusable::Results))?;
                return Ok(Some(Action::Search(SearchAction::SelectNext)));
            }
            KeyCode::Up if has_results => {
                self.send_action(Action::Focus(Focusable::Results))?;
                return Ok(Some(Action::Search(SearchAction::SelectPrev)));
            }
            KeyCode::Home if has_results && self.focused == Focusable::Results => {
                return Ok(Some(Action::Search(SearchAction::SelectFirst)));
            }
            KeyCode::End if has_results && self.focused == Focusable::Results => {
                return Ok(Some(Action::Search(SearchAction::SelectLast)));
            }
            KeyCode::BackTab => {
                return Ok(Some(Action::FocusPrevious));
            }
            KeyCode::Tab => {
                return Ok(Some(Action::FocusNext));
            }
            // Send to input box
            _ => {
                if self.focused == Focusable::Search {
                    self.input.handle_event(&crossterm::event::Event::Key(key));
                }
            }
        }
        Ok(None)
    }

    fn update(&mut self, action: Action, tui: &mut Tui) -> AppResult<Option<Action>> {
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
            Action::Focus(focusable) => {
                self.focused = focusable;
            }
            Action::FocusNext | Action::FocusPrevious => match self.focused {
                Focusable::Search if !self.search_results.crates.is_empty() => {
                    if self.search_results.state.selected().is_none() {
                        return Ok(Some(Action::Search(SearchAction::SelectNext)));
                    }

                    return Ok(Some(Action::Focus(Focusable::Results)));
                }
                Focusable::Results => {
                    return Ok(Some(Action::Focus(Focusable::Search)));
                }
                _ => {}
            },
            Action::ToggleUsage => {
                self.show_usage = !self.show_usage;
            }
            Action::RenderReadme(markdown) => {
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
            Action::Search(action) => {
                match action {
                    SearchAction::Search(term, page) => {
                        if let Some(tx) = self.command_tx.clone() {
                            self.searching = true;

                            tokio::spawn(async move {
                                let result = http_client::INSTANCE.search(term, page).await;

                                tx.send(Action::Search(SearchAction::Render(result, page)))
                                    .unwrap();
                            });
                        }
                    }
                    SearchAction::Render(results, page) => {
                        let exact_match_ix = results.crates.iter().position(|c| c.exact_match);
                        let changed_pages = page != self.search_results.meta.page;

                        self.show_usage = false;
                        self.searching = false;
                        self.search_results = results;
                        self.search_results.meta.page = page;

                        if exact_match_ix.is_some() {
                            return Ok(Some(Action::Search(SearchAction::Select(exact_match_ix))));
                        } else if changed_pages && self.search_results.current_page_len() > 0 {
                            return Ok(Some(Action::Search(SearchAction::Select(Some(0)))));
                        }
                    }
                    SearchAction::Clear => self.reset()?,
                    SearchAction::NavNextPage(pages) => {
                        self.search_results.go_next_pages(
                            pages,
                            self.input.value().to_string(),
                            self.command_tx.clone().unwrap(),
                        )?;
                    }
                    SearchAction::NavPrevPage(pages) => {
                        self.search_results.go_prev_pages(
                            pages,
                            self.input.value().to_string(),
                            self.command_tx.clone().unwrap(),
                        )?;
                    }
                    SearchAction::NavFirstPage => {
                        self.search_results.go_to_page(
                            1,
                            self.input.value().to_string(),
                            self.command_tx.clone().unwrap(),
                        )?;
                    }
                    SearchAction::NavLastPage => {
                        self.search_results.go_to_page(
                            self.search_results.pages(),
                            self.input.value().to_string(),
                            self.command_tx.clone().unwrap(),
                        )?;
                    }
                    SearchAction::Select(index) => {
                        if let Some(selected) = self.search_results.select(index) {
                            if let Some(repository) = &selected.repository {
                                if !repository.is_empty() {
                                    // let repository = repository.clone();
                                    // let tx = self.command_tx.clone();
                                    //
                                    // tokio::spawn(async move {
                                    //     if let Some(markdown) =
                                    //         http_client::INSTANCE.get_repo_readme(repository).await
                                    //     {
                                    //         tx.unwrap()
                                    //             .send(Action::RenderReadme(markdown))
                                    //             .unwrap();
                                    //     }
                                    // });
                                }
                            }
                        }
                    }
                    SearchAction::SelectNext => {
                        self.search_results.select_next();
                    }
                    SearchAction::SelectPrev => {
                        self.search_results.select_previous();
                    }
                    SearchAction::SelectFirst => {
                        self.search_results.select_first();
                    }
                    SearchAction::SelectLast => {
                        self.search_results.select_last();
                    }
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
