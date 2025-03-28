mod action_handler;
mod draw;
mod focusable;
mod key_handler;

use super::Component;

use async_trait::async_trait;
use crossterm::event::KeyEvent;
use ratatui::{layout::Rect, Frame};
use std::sync::Arc;
use tokio::sync::mpsc::UnboundedSender;
use tokio::sync::RwLock;
use tui_input::Input;

use crate::cargo::CargoEnv;
use crate::components::home::action_handler::handle_action;
use crate::components::home::draw::render;
use crate::components::home::key_handler::handle_key;
use crate::components::status_bar::StatusLevel;
use crate::components::ux::Dropdown;
use crate::errors::AppResult;
use crate::search::{CrateSearchManager, Scope, SearchResults, Sort};
use crate::tui::Tui;
use crate::{
    action::{Action, SearchAction},
    app::Mode,
    config::Config,
};
pub use focusable::Focusable;

pub struct Home {
    cargo_env: Arc<RwLock<CargoEnv>>,
    input: Input,
    scope_dropdown: Dropdown<Scope>,
    sort_dropdown: Dropdown<Sort>,
    show_usage: bool,
    focused: Focusable,
    crate_search_manager: CrateSearchManager,
    is_searching: bool,
    search_results: Option<SearchResults>,
    spinner_state: throbber_widgets_tui::ThrobberState,
    action_tx: UnboundedSender<Action>,
    config: Config,
    scope: Scope,
    pub vertical_usage_scroll: usize,
}

impl Home {
    pub fn new(
        cargo_env: Arc<RwLock<CargoEnv>>,
        action_tx: UnboundedSender<Action>,
    ) -> AppResult<Self> {
        let tx = action_tx.clone();
        let tx2 = action_tx.clone();

        Ok(Self {
            cargo_env,
            input: Input::default(),
            scope_dropdown: Dropdown::new(
                "Search in".into(),
                1,
                Box::new(move |selected: &Scope| {
                    tx.send(Action::Search(SearchAction::Scope(selected.clone())))
                        .unwrap();
                }),
            ),
            sort_dropdown: Dropdown::new(
                "Sort by".into(),
                0,
                Box::new(move |selected: &Sort| {
                    tx2.send(Action::Search(SearchAction::SortBy(selected.clone())))
                        .unwrap();
                }),
            ),
            show_usage: true,
            focused: Focusable::default(),
            search_results: None,
            crate_search_manager: CrateSearchManager::new(action_tx.clone())?,
            is_searching: false,
            spinner_state: throbber_widgets_tui::ThrobberState::default(),
            action_tx,
            config: Config::default(),
            scope: Scope::default(),
            vertical_usage_scroll: 0,
        })
    }

    fn reset(&mut self) -> AppResult<()> {
        self.input.reset();
        self.search_results = None;
        self.action_tx
            .send(Action::UpdateStatus(StatusLevel::Info, "ready".into()))?;
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

#[cfg(test)]
mod tests {
    use super::*;
    use crate::action::Action;
    use crate::search::SearchResults;
    use pretty_assertions::assert_eq;
    use tokio::sync::mpsc;

    fn get_home() -> Home {
        let (action_tx, _) = mpsc::unbounded_channel();
        Home::new(Arc::new(RwLock::new(CargoEnv::new(None))), action_tx).unwrap()
    }

    fn get_home_and_tui() -> (Home, Tui) {
        let (action_tx, _) = mpsc::unbounded_channel();
        (
            Home::new(Arc::new(RwLock::new(CargoEnv::new(None))), action_tx).unwrap(),
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
            match home.update(ac.clone().unwrap(), tui).await {
                Ok(action) => {
                    ac = action;
                }
                Err(err) => {
                    panic!("{}", err)
                }
            }
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
    async fn test_search_clear_action() {
        let (mut home, _) = get_home_and_tui();

        assert_eq!(true, home.input.value().is_empty());

        // simulate search
        home.search_results = Some(SearchResults::new(1));

        //execute_update_with_home(&mut home, &mut tui, Action::Search(SearchAction::Clear)).await;

        //assert_eq!(home.search_results, None);
    }
}
