pub mod action_handler;
pub mod draw;
pub mod focusable;
pub mod key_handler;

use super::{Component, StatusAction};

use async_trait::async_trait;
use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};
use serde::Deserialize;
use std::sync::Arc;
use strum_macros::Display;
use tokio::sync::RwLock;
use tokio::sync::mpsc::UnboundedSender;
use tui_input::Input;

use crate::cargo::CargoEnv;
use crate::components::home::focusable::Focusable;
use crate::components::home::{
    action_handler::handle_action, draw::render, key_handler::handle_key,
};
use crate::components::status_bar::StatusLevel;
use crate::components::ux::Dropdown;
use crate::errors::AppResult;
use crate::search::{Crate, CrateSearchManager, Scope, SearchResults, Sort};
use crate::tui::Tui;
use crate::{action::Action, app::Mode, config::Config};

#[derive(Debug, Clone, PartialEq, Eq, Display, Deserialize)]
pub enum HomeAction {
    Focus(Focusable),
    FocusNext,
    FocusPrevious,
    ToggleUsage,

    Search(SearchAction),

    OpenDocs,
    OpenReadme,
    RenderReadme(String),
    OpenCratesIo,
    OpenLibRs,
}

#[derive(Debug, Clone, PartialEq, Eq, Display, Deserialize)]
pub enum SearchAction {
    Clear,
    Search {
        term: String,
        page: usize,
        hide_usage: bool,
        status: Option<String>,
    },
    Error(String),
    SortBy(Sort),
    Scope(Scope),
    Render(SearchResults),

    NavPagesForward(usize),
    NavPagesBack(usize),
    NavFirstPage,
    NavLastPage,
    SelectIndex(Option<usize>),
    SelectNext,
    SelectPrev,
    SelectFirst,
    SelectLast,
}

/// The home (main) component.
pub struct Home {
    config: Config,
    cargo_env: Arc<RwLock<CargoEnv>>,
    crate_search_manager: CrateSearchManager,
    left_column_width_percent: u16,
    show_usage: bool,
    focused: Focusable,
    input: Input,
    scope_dropdown: Dropdown<Scope>,
    sort_dropdown: Dropdown<Sort>,
    is_searching: bool,
    search_results: Option<SearchResults>,
    spinner_state: throbber_widgets_tui::ThrobberState,
    action_tx: UnboundedSender<Action>,
    vertical_usage_scroll: usize,
}

impl Home {
    pub fn new(
        initial_search_term: Option<String>,
        cargo_env: Arc<RwLock<CargoEnv>>,
        action_tx: UnboundedSender<Action>,
    ) -> AppResult<Self> {
        let tx = action_tx.clone();
        let tx2 = action_tx.clone();

        let input = Input::default().with_value(initial_search_term.unwrap_or_default());

        Ok(Self {
            cargo_env,
            left_column_width_percent: 40,
            show_usage: true,
            focused: Focusable::default(),
            input,
            scope_dropdown: Dropdown::new(
                "Search in".into(),
                Scope::default() as usize,
                Box::new(move |selected: &Scope| {
                    tx.send(Action::Home(HomeAction::Search(SearchAction::Scope(
                        selected.clone(),
                    ))))
                    .unwrap();
                }),
            ),
            sort_dropdown: Dropdown::new(
                "Sort by".into(),
                Sort::default() as usize,
                Box::new(move |selected: &Sort| {
                    tx2.send(Action::Home(HomeAction::Search(SearchAction::SortBy(
                        selected.clone(),
                    ))))
                    .unwrap();
                }),
            ),
            search_results: None,
            crate_search_manager: CrateSearchManager::new(action_tx.clone())?,
            is_searching: false,
            spinner_state: throbber_widgets_tui::ThrobberState::default(),
            action_tx,
            config: Config::default(),
            vertical_usage_scroll: 0,
        })
    }

    fn reset(&mut self) -> AppResult<()> {
        self.input.reset();
        self.search_results = None;
        self.action_tx
            .send(Action::Status(StatusAction::UpdateStatus(
                StatusLevel::Info,
                "Ready".into(),
            )))?;
        Ok(())
    }

    pub fn go_to_page(&self, page: usize, query: String) -> AppResult<()> {
        if let Some(results) = &self.search_results {
            let requested_page = if page >= results.page_count() {
                results.page_count()
            } else {
                page
            };

            if requested_page != results.current_page() {
                self.action_tx
                    .send(Action::Home(HomeAction::Search(SearchAction::Search {
                        term: query,
                        page: requested_page,
                        hide_usage: false,
                        status: Some(format!("Loading page {requested_page}")),
                    })))?;
            }
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

            self.go_to_page(requested_page, query)?
        }

        Ok(())
    }

    pub fn go_pages_forward(&self, pages: usize, query: String) -> AppResult<()> {
        if let Some(results) = &self.search_results {
            let mut requested_page = results.current_page() + pages;

            if requested_page > results.page_count() {
                requested_page = results.page_count();
            }

            self.go_to_page(requested_page, query)?
        }

        Ok(())
    }

    pub fn is_details_focused(&self) -> bool {
        self.focused == Focusable::DocsButton
            || self.focused == Focusable::RepositoryButton
            || self.focused == Focusable::CratesIoButton
            || self.focused == Focusable::LibRsButton
    }

    pub fn is_results_or_details_focused(&self) -> bool {
        self.focused == Focusable::Results || self.is_details_focused()
    }

    fn get_focused_crate(&self) -> Option<&Crate> {
        if self.is_results_or_details_focused()
            && let Some(search_results) = self.search_results.as_ref()
            && let Some(selected) = search_results.selected()
        {
            Some(selected)
        } else {
            None
        }
    }

    fn should_show_docs_button(&self) -> bool {
        if let Some(search_results) = self.search_results.as_ref()
            && let Some(selected) = search_results.selected()
            && selected.documentation.is_some()
        {
            return true;
        }
        false
    }

    fn should_show_repo_button(&self) -> bool {
        if let Some(search_results) = self.search_results.as_ref()
            && let Some(selected) = search_results.selected()
            && selected.repository.is_some()
        {
            return true;
        }
        false
    }

    fn should_show_cratesio_button(&self) -> bool {
        if let Some(search_results) = self.search_results.as_ref()
            && search_results.selected().is_some()
        {
            return true;
        }
        false
    }

    fn should_show_librs_button(&self) -> bool {
        if let Some(search_results) = self.search_results.as_ref()
            && search_results.selected().is_some()
        {
            return true;
        }

        false
    }

    fn should_show_button(&self, f: &Focusable) -> bool {
        match f {
            Focusable::DocsButton => self.should_show_docs_button(),
            Focusable::RepositoryButton => self.should_show_repo_button(),
            Focusable::CratesIoButton => self.should_show_cratesio_button(),
            Focusable::LibRsButton => self.should_show_librs_button(),
            _ => false,
        }
    }
}

#[async_trait]
impl Component for Home {
    fn register_config_handler(&mut self, config: Config) -> AppResult<()> {
        self.sort_dropdown.register_config_handler(config.clone())?;
        self.scope_dropdown
            .register_config_handler(config.clone())?;
        self.config = config;
        Ok(())
    }

    fn init(&mut self, tui: &mut Tui) -> AppResult<()> {
        let _ = tui;

        let initial_search_term = self.input.value();
        if !initial_search_term.is_empty() {
            self.action_tx
                .send(Action::Home(HomeAction::Search(SearchAction::Search {
                    term: initial_search_term.to_string(),
                    page: 1,
                    hide_usage: true,
                    status: None,
                })))
                .ok();
        }

        Ok(())
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> AppResult<Option<Action>> {
        handle_key(self, key)
    }

    async fn update(&mut self, action: Action, tui: &mut Tui) -> AppResult<Option<Action>> {
        handle_action(self, action, tui).await
    }

    fn draw(&mut self, mode: &Mode, frame: &mut Frame, area: Rect) -> AppResult<()> {
        if *mode != Mode::Home {
            return Ok(());
        }
        render(self, frame, area)
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }
}
