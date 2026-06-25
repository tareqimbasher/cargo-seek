use crossterm::event::KeyEvent;
use ratatui::layout::{Constraint, Layout, Rect};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, info};

use crate::action::Action;
use crate::cargo;
use crate::cargo::{CargoCommand, CargoEnv, CargoEvent};
use crate::components::Component;
use crate::components::app_id::AppId;
use crate::components::fps::FpsCounter;
use crate::components::home::Home;
use crate::components::status_bar::{StatusBar, StatusCommand, StatusLevel};
use crate::config::Config;
use crate::errors::AppResult;
use crate::tui::{Event, Tui};

pub struct App {
    cargo_env: Arc<RwLock<CargoEnv>>,
    mode: Mode,
    config: Config,
    components: Vec<Box<dyn Component>>,
    tick_rate: f64,
    frame_rate: f64,
    should_quit: bool,
    should_suspend: bool,
    last_tick_key_events: Vec<KeyEvent>,
    action_tx: mpsc::UnboundedSender<Action>,
    action_rx: mpsc::UnboundedReceiver<Action>,
}

#[derive(Default, Debug, Copy, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
pub enum Mode {
    App,
    #[default]
    Home,
}

impl App {
    pub fn new(
        tick_rate: f64,
        frame_rate: f64,
        show_counter: bool,
        project_dir: Option<PathBuf>,
        initial_search_term: Option<String>,
    ) -> AppResult<Self> {
        let (action_tx, action_rx) = mpsc::unbounded_channel();

        let cargo_env = Arc::new(RwLock::new(CargoEnv::new(project_dir)));

        let mut components: Vec<Box<dyn Component>> = vec![
            Box::new(Home::new(
                initial_search_term,
                cargo_env.clone(),
                action_tx.clone(),
            )?),
            Box::new(StatusBar::new(action_tx.clone())),
            Box::new(AppId::new()), // Should be after other components so it gets drawn on top of them
        ];

        if show_counter {
            components.push(Box::new(FpsCounter::default()));
        }

        Ok(Self {
            cargo_env,
            mode: Mode::Home,
            config: Config::new()?,
            components,
            tick_rate,
            frame_rate,
            should_quit: false,
            should_suspend: false,
            last_tick_key_events: Vec::new(),
            action_tx,
            action_rx,
        })
    }

    pub async fn run(&mut self) -> AppResult<()> {
        // Start by reading the current cargo environment
        self.cargo_env.write().await.read().ok();

        let mut tui = Tui::new()?
            // .mouse(true)
            // .paste(true)
            .tick_rate(self.tick_rate)
            .frame_rate(self.frame_rate);
        tui.enter()?;

        for component in self.components.iter_mut() {
            component.register_config_handler(self.config.clone())?;
            component.init(&mut tui)?;
        }

        let action_tx = self.action_tx.clone();
        loop {
            self.handle_events(&mut tui).await?;
            self.handle_actions(&mut tui).await?;
            if self.should_suspend {
                tui.suspend()?;
                action_tx.send(Action::Resume)?;
                action_tx.send(Action::ClearScreen)?;
                // tui.mouse(true);
                tui.enter()?;
            } else if self.should_quit {
                tui.stop()?;
                break;
            }
        }
        tui.exit()?;
        Ok(())
    }

    async fn handle_events(&mut self, tui: &mut Tui) -> AppResult<()> {
        let Some(event) = tui.next_event().await else {
            return Ok(());
        };
        let action_tx = self.action_tx.clone();
        match event {
            Event::Quit => action_tx.send(Action::Quit)?,
            Event::Tick => action_tx.send(Action::Tick)?,
            Event::Render => action_tx.send(Action::Render)?,
            Event::Resize(x, y) => action_tx.send(Action::Resize { w: x, h: y })?,
            Event::Key(key) => self.handle_key_event(key)?,
            _ => {}
        }
        for component in self.components.iter_mut() {
            if let Some(action) = component.handle_events(Some(event.clone()))? {
                action_tx.send(action)?;
            }
        }
        Ok(())
    }

    fn handle_key_event(&mut self, key: KeyEvent) -> AppResult<()> {
        let action_tx = self.action_tx.clone();

        for mode in [&self.mode, &Mode::App] {
            if let Some(keymap) = self.config.keybindings.get(mode) {
                if let Some(action) = keymap.get(&vec![key]) {
                    info!("Got action: {action:?}");
                    action_tx.send(action.clone())?;
                    return Ok(());
                }

                // If the key was not handled as a single key action,
                // then consider it for multi-key combinations.
                self.last_tick_key_events.push(key);

                // Check for multi-key combinations
                if let Some(action) = keymap.get(&self.last_tick_key_events) {
                    info!("Got action: {action:?}");
                    action_tx.send(action.clone())?;
                    return Ok(());
                }
            }
        }

        Ok(())
    }

    async fn handle_actions(&mut self, tui: &mut Tui) -> AppResult<()> {
        while let Ok(action) = self.action_rx.try_recv() {
            if !matches!(action, Action::Tick) && !matches!(action, Action::Render) {
                debug!("{action:?}");
            }

            let action_clone = action.clone();

            match action {
                Action::Tick => {
                    self.last_tick_key_events.drain(..);
                }
                Action::Quit => self.should_quit = true,
                Action::Suspend => self.should_suspend = true,
                Action::Resume => self.should_suspend = false,
                Action::ClearScreen => tui.terminal.clear()?,
                Action::Resize { w, h } => self.handle_resize(tui, w, h)?,
                Action::Render => self.render(tui)?,
                Action::Cargo(cargo_action) => self.handle_cargo_actions(tui, cargo_action).await?,
                _ => {}
            }

            for component in self.components.iter_mut() {
                if let Some(sub_action) = component.update(action_clone.clone(), tui).await? {
                    self.action_tx.send(sub_action)?
                };
            }
        }
        Ok(())
    }

    fn handle_resize(&mut self, tui: &mut Tui, w: u16, h: u16) -> AppResult<()> {
        tui.resize(Rect::new(0, 0, w, h))?;
        self.render(tui)?;
        Ok(())
    }

    async fn handle_cargo_actions(&mut self, tui: &mut Tui, action: CargoCommand) -> AppResult<()> {
        match action {
            CargoCommand::Add { name, version } => {
                let progress = format!("Adding {name} v{version}");
                let success = format!("Added {name} v{version}");
                let failure = format!("Failed to add {name}");
                self.run_cargo_action(tui, true, progress, success, failure, move || {
                    cargo::add(&name, Some(version), true)
                })
                .await?;
            }
            CargoCommand::Remove(name) => {
                let progress = format!("Removing {name}");
                let success = format!("Removed {name}");
                let failure = format!("Failed to remove {name}");
                self.run_cargo_action(tui, false, progress, success, failure, move || {
                    cargo::remove(name, false)
                })
                .await?;
            }
            CargoCommand::Install { name, version } => {
                let progress = format!("Installing {name} v{version}");
                let success = format!("Installed {name} v{version}");
                let failure = format!("Failed to install {name}");
                self.run_cargo_action(tui, true, progress, success, failure, move || {
                    cargo::install(name, Some(version), true)
                })
                .await?;
            }
            CargoCommand::Uninstall(name) => {
                let progress = format!("Uninstalling {name}");
                let success = format!("Uninstalled {name}");
                let failure = format!("Failed to uninstall {name}");
                self.run_cargo_action(tui, false, progress, success, failure, move || {
                    cargo::uninstall(name, false)
                })
                .await?;
            }
            CargoCommand::Refresh => {
                self.cargo_env.write().await.read()?;
                self.action_tx
                    .send(Action::CargoEvent(CargoEvent::Refreshed))
                    .ok();
            }
        }

        Ok(())
    }

    /// Runs a cargo command on a background task, reporting progress/success/failure to the status
    /// bar and refreshing the cargo environment on success. Interactive commands (`add`/`install`)
    /// stream their output to the real terminal, so the TUI is exited for the duration and
    /// re-entered afterwards.
    async fn run_cargo_action<F>(
        &mut self,
        tui: &mut Tui,
        interactive: bool,
        progress: String,
        success: String,
        failure: String,
        op: F,
    ) -> AppResult<()>
    where
        F: FnOnce() -> AppResult<()> + Send + 'static,
    {
        self.action_tx
            .send(Action::Status(StatusCommand::UpdateStatus(
                StatusLevel::Info,
                progress,
            )))?;

        if interactive {
            tui.exit()?;
        }

        let tx = self.action_tx.clone();
        tokio::spawn(async move {
            match op() {
                Ok(_) => {
                    tx.send(Action::Status(StatusCommand::UpdateStatus(
                        StatusLevel::Info,
                        success,
                    )))
                    .ok();
                    tx.send(Action::Cargo(CargoCommand::Refresh)).ok();
                }
                Err(_) => {
                    tx.send(Action::Status(StatusCommand::UpdateStatus(
                        StatusLevel::Error,
                        failure,
                    )))
                    .ok();
                }
            }
        })
        .await?;

        if interactive {
            tui.enter()?;
            tui.terminal.clear()?;
        }

        Ok(())
    }

    fn render(&mut self, tui: &mut Tui) -> AppResult<()> {
        tui.draw(|frame| {
            let [main_content_area, status_bar_area] =
                Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(frame.area());

            for component in self.components.iter_mut() {
                let mut area = main_content_area;

                if component.as_any().downcast_ref::<StatusBar>().is_some() {
                    area = status_bar_area;
                }

                if let Err(err) = component.draw(&self.mode, frame, area) {
                    let _ = self
                        .action_tx
                        .send(Action::Error(format!("Failed to draw: {err:?}")));
                }
            }
        })?;
        Ok(())
    }
}
