//! The home (main) view — most of the app, split by concern (input, actions, rendering, focus)
//! into submodules.

pub mod action_handler;
pub mod draw;
pub mod feature_selector;
pub mod focusable;
pub mod key_handler;
pub mod overlay;

use super::{Component, StatusCommand};

use async_trait::async_trait;
use crossterm::event::KeyEvent;
use ratatui::{Frame, layout::Rect};
use serde::Deserialize;
use std::sync::Arc;
use strum::Display;
use tokio::sync::RwLock;
use tokio::sync::mpsc::UnboundedSender;
use tui_input::Input;

use crate::cargo::CargoEnv;
use crate::components::home::feature_selector::{FeatureIntent, FeatureSelector};
use crate::components::home::focusable::Focusable;
use crate::components::home::overlay::Overlay;
use crate::components::home::{
    action_handler::handle_action, draw::render, key_handler::handle_key,
};
use crate::components::status_bar::StatusLevel;
use crate::errors::AppResult;
use crate::search::{Crate, CrateSearchManager, Scope, SearchCommand, SearchResults, Sort};
use crate::tui::Tui;
use crate::{action::Action, app::Mode, config::Config};

#[derive(Debug, Clone, PartialEq, Eq, Display, Deserialize)]
pub enum HomeCommand {
    Focus(Focusable),
    FocusNext,
    FocusPrevious,
    ToggleHelp,

    OpenDocs,
    OpenReadme,
    RenderReadme(String),
    OpenCratesIo,
    OpenLibRs,
}

/// The home (main) component.
pub struct Home {
    config: Config,
    cargo_env: Arc<RwLock<CargoEnv>>,
    crate_search_manager: CrateSearchManager,
    left_column_width_percent: u16,
    show_help: bool,
    focused: Focusable,
    input: Input,
    sort: Sort,
    scope: Scope,
    overlay: Option<Overlay>,
    is_searching: bool,
    search_results: Option<SearchResults>,
    spinner_state: throbber_widgets_tui::ThrobberState,
    action_tx: UnboundedSender<Action>,
    vertical_help_scroll: usize,
    max_help_scroll: usize,
}

impl Home {
    pub fn new(
        initial_search_term: Option<String>,
        cargo_env: Arc<RwLock<CargoEnv>>,
        action_tx: UnboundedSender<Action>,
    ) -> AppResult<Self> {
        let input = Input::default().with_value(initial_search_term.unwrap_or_default());

        Ok(Self {
            cargo_env,
            left_column_width_percent: 40,
            show_help: true,
            focused: Focusable::default(),
            input,
            sort: Sort::default(),
            scope: Scope::default(),
            overlay: None,
            search_results: None,
            crate_search_manager: CrateSearchManager::new(action_tx.clone())?,
            is_searching: false,
            spinner_state: throbber_widgets_tui::ThrobberState::default(),
            action_tx,
            config: Config::default(),
            vertical_help_scroll: 0,
            max_help_scroll: 0,
        })
    }

    fn reset(&mut self) -> AppResult<()> {
        self.input.reset();
        self.search_results = None;
        self.action_tx
            .send(Action::Status(StatusCommand::UpdateStatus(
                StatusLevel::Info,
                "Ready".into(),
            )))?;
        Ok(())
    }

    pub fn go_to_page(&self, page: usize, query: &str) -> AppResult<()> {
        if let Some(results) = &self.search_results
            && let Some(requested_page) = results.resolve_page(page)
        {
            self.action_tx.send(Action::Search(SearchCommand::Run {
                term: query.to_string(),
                page: requested_page,
                hide_help: false,
                status: Some(format!("Loading page {requested_page}")),
            }))?;
        }

        Ok(())
    }

    pub fn go_to_last_page(&self, query: &str) -> AppResult<()> {
        if let Some(results) = &self.search_results {
            self.go_to_page(results.page_count(), query)?;
        }
        Ok(())
    }

    pub fn go_pages_back(&self, pages: usize, query: &str) -> AppResult<()> {
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

    pub fn go_pages_forward(&self, pages: usize, query: &str) -> AppResult<()> {
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

    /// Builds a feature picker for the focused crate, or `None` when there is nothing to pick.
    fn feature_picker_for(&self, intent: FeatureIntent) -> Option<FeatureSelector> {
        let cr = self.get_focused_crate()?;
        let features = cr.features.as_deref()?;
        if features.is_empty() {
            return None;
        }

        Some(FeatureSelector::new(
            self.config.clone(),
            cr.name.clone(),
            cr.version.clone(),
            intent,
            features,
            &cr.default_features,
        ))
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
        self.config = config;
        Ok(())
    }

    fn init(&mut self, tui: &mut Tui) -> AppResult<()> {
        let _ = tui;

        let initial_search_term = self.input.value();
        if !initial_search_term.is_empty() {
            self.action_tx
                .send(Action::Search(SearchCommand::Run {
                    term: initial_search_term.to_string(),
                    page: 1,
                    hide_help: true,
                    status: None,
                }))
                .ok();
        }

        Ok(())
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> AppResult<Option<Action>> {
        handle_key(self, key)
    }

    async fn update(&mut self, action: &Action, tui: &mut Tui) -> AppResult<Option<Action>> {
        handle_action(self, action, tui).await
    }

    fn draw(&mut self, mode: &Mode, frame: &mut Frame, area: Rect) -> AppResult<()> {
        if *mode != Mode::Home {
            return Ok(());
        }
        render(self, frame, area)
    }
}
