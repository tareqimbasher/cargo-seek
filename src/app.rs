//! The application event loop.
//!
//! `App` owns the components, the `Action` channel, and the shared cargo environment. Each
//! iteration translates terminal events into `Action`s, dispatches them, and renders.

use crossterm::event::KeyEvent;
use ratatui::layout::{Constraint, Layout, Rect};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use tokio::sync::{RwLock, mpsc};
use tracing::{debug, error, info};

use crate::action::Action;
use crate::cargo;
use crate::cargo::{CargoCommand, CargoEnv, CargoError, CargoEvent, OutputMode};
use crate::components::app_id::AppId;
use crate::components::fps::FpsCounter;
use crate::components::home::Home;
use crate::components::status_bar::{StatusBar, StatusCommand, StatusLevel};
use crate::components::{Component, Placement};
use crate::config::Config;
use crate::errors::AppResult;
use crate::tui::{Event, Tui};

pub struct App {
    cargo_env: Arc<RwLock<CargoEnv>>,
    cargo_busy: Arc<AtomicBool>,
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
            cargo_busy: Arc::new(AtomicBool::new(false)),
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
        self.cargo_env.write().await.refresh_blocking();

        let mut tui = Tui::new()?
            // .mouse(true)
            // .paste(true)
            .tick_rate(self.tick_rate)
            .frame_rate(self.frame_rate);
        tui.enter()?;

        // Restore the terminal on every exit path, including a mid-loop error — otherwise a
        // failure would leave it in raw mode. `Tui`'s `Drop` is only a last-resort backstop.
        let result = self.run_loop(&mut tui).await;
        let restored = tui.exit();
        result.and(restored)
    }

    /// The main event/render loop: set up components, then run until `should_quit` or the first
    /// error. The terminal is restored by `run`, not here.
    async fn run_loop(&mut self, tui: &mut Tui) -> AppResult<()> {
        for component in self.components.iter_mut() {
            component.register_config_handler(self.config.clone())?;
            component.init(tui)?;
        }

        let action_tx = self.action_tx.clone();
        loop {
            self.handle_events(tui).await?;
            self.handle_actions(tui).await?;
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
            if !matches!(action, Action::Tick | Action::Render) {
                debug!("{action:?}");
            }

            match &action {
                Action::Tick => {
                    self.last_tick_key_events.drain(..);
                }
                Action::Quit => self.should_quit = true,
                Action::Suspend => self.should_suspend = true,
                Action::Resume => self.should_suspend = false,
                Action::ClearScreen => tui.terminal.clear()?,
                Action::Resize { w, h } => self.handle_resize(tui, *w, *h)?,
                Action::Render => self.render(tui)?,
                Action::Cargo(cargo_action) => {
                    self.handle_cargo_actions(tui, cargo_action.clone()).await?
                }
                Action::Error(message) => {
                    error!("{message}");
                    self.action_tx
                        .send(Action::Status(StatusCommand::UpdateStatus(
                            StatusLevel::Error,
                            message.clone(),
                        )))?;
                }
                _ => {}
            }

            for component in self.components.iter_mut() {
                if let Some(sub_action) = component.update(&action, tui).await? {
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
                self.run_cargo_action(
                    tui,
                    OutputMode::Inherit,
                    progress,
                    success,
                    failure,
                    move |out| cargo::add(&name, Some(version), out),
                )
                .await?;
            }
            CargoCommand::Remove(name) => {
                let progress = format!("Removing {name}");
                let success = format!("Removed {name}");
                let failure = format!("Failed to remove {name}");
                self.run_cargo_action(
                    tui,
                    OutputMode::Capture,
                    progress,
                    success,
                    failure,
                    move |out| cargo::remove(name, out),
                )
                .await?;
            }
            CargoCommand::Install { name, version } => {
                let progress = format!("Installing {name} v{version}");
                let success = format!("Installed {name} v{version}");
                let failure = format!("Failed to install {name}");
                self.run_cargo_action(
                    tui,
                    OutputMode::Inherit,
                    progress,
                    success,
                    failure,
                    move |out| cargo::install(name, Some(version), out),
                )
                .await?;
            }
            CargoCommand::Uninstall(name) => {
                let progress = format!("Uninstalling {name}");
                let success = format!("Uninstalled {name}");
                let failure = format!("Failed to uninstall {name}");
                self.run_cargo_action(
                    tui,
                    OutputMode::Capture,
                    progress,
                    success,
                    failure,
                    move |out| cargo::uninstall(name, out),
                )
                .await?;
            }
            CargoCommand::Refresh => {
                // The cargo subprocesses block, so gather off the event-loop task — running them
                // here (under the write lock) would freeze rendering. Only the fast apply locks.
                let cargo_env = self.cargo_env.clone();
                let (project_dir, project) = {
                    let env = cargo_env.read().await;
                    (env.project_dir(), env.project.clone())
                };
                let tx = self.action_tx.clone();
                tokio::spawn(async move {
                    match tokio::task::spawn_blocking(move || {
                        CargoEnv::gather(project_dir, project)
                    })
                    .await
                    {
                        Ok(gathered) => {
                            cargo_env.write().await.apply(gathered);
                            tx.send(Action::CargoEvent(CargoEvent::Refreshed)).ok();
                        }
                        Err(err) => error!("cargo environment refresh failed: {err}"),
                    }
                });
            }
        }

        Ok(())
    }

    /// Runs a cargo command, reporting progress/success/failure to the status bar and refreshing
    /// the cargo environment on success. A second command is rejected while one is running.
    ///
    /// `out` drives both how cargo connects to the terminal and how the loop runs it:
    /// `OutputMode::Inherit` (add/install) releases the terminal and awaits (nothing renders
    /// meanwhile); `OutputMode::Capture` (remove/uninstall) keeps the TUI up and runs detached. The
    /// same `out` reaches `op`, so terminal handling and output mode can't diverge.
    async fn run_cargo_action<F>(
        &mut self,
        tui: &mut Tui,
        out: OutputMode,
        progress: String,
        success: String,
        failure: String,
        op: F,
    ) -> AppResult<()>
    where
        F: FnOnce(OutputMode) -> AppResult<()> + Send + 'static,
    {
        // Reject re-entry while a command runs (swap returns the previous value).
        if self.cargo_busy.swap(true, Ordering::SeqCst) {
            self.action_tx
                .send(Action::Status(StatusCommand::UpdateStatus(
                    StatusLevel::Info,
                    "A cargo command is already running".into(),
                )))?;
            return Ok(());
        }

        self.action_tx
            .send(Action::Status(StatusCommand::UpdateStatus(
                StatusLevel::Info,
                progress,
            )))?;

        let tx = self.action_tx.clone();
        let busy = self.cargo_busy.clone();

        match out {
            OutputMode::Inherit => {
                let result = Self::with_terminal_released(tui, move || op(out)).await;
                busy.store(false, Ordering::SeqCst);
                Self::report_cargo_result(&tx, result, success, failure);
            }
            OutputMode::Capture => {
                tokio::spawn(async move {
                    let result = tokio::task::spawn_blocking(move || op(out))
                        .await
                        .unwrap_or_else(|err| Err(err.into()));
                    busy.store(false, Ordering::SeqCst);
                    Self::report_cargo_result(&tx, result, success, failure);
                });
            }
        }

        Ok(())
    }

    /// Runs the blocking `op` on a blocking thread with the TUI exited, re-entering afterwards. The
    /// terminal is restored even if `op` fails, so an attached cargo command can never run with the
    /// alternate screen still up.
    async fn with_terminal_released<F, T>(tui: &mut Tui, op: F) -> AppResult<T>
    where
        F: FnOnce() -> AppResult<T> + Send + 'static,
        T: Send + 'static,
    {
        tui.exit()?;
        let joined = tokio::task::spawn_blocking(op).await;
        tui.enter()?;
        tui.terminal.clear()?;
        match joined {
            Ok(result) => result,
            Err(err) => Err(err.into()),
        }
    }

    fn report_cargo_result(
        tx: &mpsc::UnboundedSender<Action>,
        result: AppResult<()>,
        success: String,
        failure: String,
    ) {
        match result {
            Ok(()) => {
                tx.send(Action::Status(StatusCommand::UpdateStatus(
                    StatusLevel::Info,
                    success,
                )))
                .ok();
                tx.send(Action::Cargo(CargoCommand::Refresh)).ok();
            }
            Err(report) => {
                error!("{failure}: {report:?}");

                // Prefer cargo's own diagnostics (e.g. "the crate `x` could not be found") when the
                // failure came from the subprocess; otherwise show the error itself.
                let detail = report
                    .downcast_ref::<CargoError>()
                    .map(CargoError::summary)
                    .unwrap_or_else(|| format!("{report:#}"));

                tx.send(Action::Status(StatusCommand::UpdateStatus(
                    StatusLevel::Error,
                    format!("{failure}: {detail}"),
                )))
                .ok();
            }
        }
    }

    fn render(&mut self, tui: &mut Tui) -> AppResult<()> {
        tui.draw(|frame| {
            let [main_content_area, status_bar_area] =
                Layout::vertical([Constraint::Min(1), Constraint::Length(1)]).areas(frame.area());

            for component in self.components.iter_mut() {
                let area = match component.placement() {
                    Placement::Main => main_content_area,
                    Placement::StatusBar => status_bar_area,
                };

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
