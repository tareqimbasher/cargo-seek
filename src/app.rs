use crossterm::event::KeyEvent;
use ratatui::layout::{Constraint, Layout, Rect};
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use std::sync::Arc;
use tokio::sync::{mpsc, RwLock};
use tracing::{debug, info};

use crate::action::{Action, CargoAction};
use crate::cargo::{add, install, remove, uninstall, CargoEnv};
use crate::components::{AppId, Component, FpsCounter, Home, Settings, StatusBar, StatusLevel};
use crate::config::Config;
use crate::errors::{AppError, AppResult};
use crate::tui::{Event, Tui};

pub struct App {
    cargo_env: Arc<RwLock<CargoEnv>>,
    config: Config,
    tick_rate: f64,
    frame_rate: f64,
    mode: Mode,
    components: Vec<Box<dyn Component>>,
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
    Settings,
}

impl App {
    pub fn new(
        tick_rate: f64,
        frame_rate: f64,
        show_counter: bool,
        proj_path: Option<PathBuf>,
        initial_search_term: Option<String>,
    ) -> AppResult<Self> {
        let (action_tx, action_rx) = mpsc::unbounded_channel();

        let cargo_env = Arc::new(RwLock::new(CargoEnv::new(proj_path)));

        let mut components: Vec<Box<dyn Component>> = vec![
            Box::new(Home::new(
                initial_search_term,
                Arc::clone(&cargo_env),
                action_tx.clone(),
            )?),
            Box::new(Settings::new()),
            Box::new(AppId::new()), // Should be after other components so it gets drawn on top of them
            Box::new(StatusBar::new(action_tx.clone())),
        ];

        if show_counter {
            components.push(Box::new(FpsCounter::default()));
        }

        Ok(Self {
            cargo_env,
            tick_rate,
            frame_rate,
            components,
            should_quit: false,
            should_suspend: false,
            config: Config::new()?,
            mode: Mode::Home,
            last_tick_key_events: Vec::new(),
            action_tx,
            action_rx,
        })
    }

    pub async fn run(&mut self) -> AppResult<()> {
        let mut tui = Tui::new()?
            // .mouse(true)
            // .paste(true)
            .tick_rate(self.tick_rate)
            .frame_rate(self.frame_rate);
        tui.enter()?;

        self.cargo_env.write().await.read().ok();

        for component in self.components.iter_mut() {
            component.register_config_handler(self.config.clone())?;
        }
        for component in self.components.iter_mut() {
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
            Event::Resize(x, y) => action_tx.send(Action::Resize(x, y))?,
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

            let clone = action.clone();

            match action {
                Action::Tick => {
                    self.last_tick_key_events.drain(..);
                }
                Action::Quit => self.should_quit = true,
                Action::Suspend => self.should_suspend = true,
                Action::Resume => self.should_suspend = false,
                Action::ClearScreen => tui.terminal.clear()?,
                Action::Resize(w, h) => self.handle_resize(tui, w, h)?,
                Action::Render => self.render(tui)?,
                Action::ToggleSettings => {
                    self.mode = if self.mode == Mode::Settings {
                        Mode::Home
                    } else {
                        Mode::Settings
                    };
                }
                Action::Cargo(action) => {
                    return match action {
                        CargoAction::Add(crate_name, version) => {
                            self.action_tx.send(Action::UpdateStatus(
                                StatusLevel::Info,
                                format!("Adding {} v{}", crate_name, version),
                            ))?;

                            tui.exit()?;
                            let tx = self.action_tx.clone();
                            tokio::spawn(async move {
                                if add(crate_name.clone(), Some(version.clone()), true).is_err() {
                                    tx.send(Action::UpdateStatus(
                                        StatusLevel::Error,
                                        format!("Failed to add {crate_name}"),
                                    ))?;
                                    // TODO should user full error message (in a popup maybe)
                                    return Ok(());
                                }
                                tx.send(Action::UpdateStatus(
                                    StatusLevel::Info,
                                    format!("Added {crate_name} v{version}"),
                                ))?;
                                tx.send(Action::RefreshCargoEnv)?;
                                Ok::<(), AppError>(())
                            })
                            .await??;

                            tui.enter()?;
                            tui.terminal.clear()?;
                            Ok(())
                        }
                        CargoAction::Remove(crate_name) => {
                            self.action_tx.send(Action::UpdateStatus(
                                StatusLevel::Info,
                                format!("Removing {}", crate_name),
                            ))?;

                            let tx = self.action_tx.clone();
                            tokio::spawn(async move {
                                if remove(crate_name.clone(), false).is_err() {
                                    tx.send(Action::UpdateStatus(
                                        StatusLevel::Error,
                                        format!("Failed to remove {crate_name}"),
                                    ))?;
                                    // TODO should user full error message (in a popup maybe)
                                    return Ok(());
                                }
                                tx.send(Action::UpdateStatus(
                                    StatusLevel::Info,
                                    format!("Removed {crate_name}"),
                                ))?;
                                tx.send(Action::RefreshCargoEnv)?;
                                Ok::<(), AppError>(())
                            });
                            Ok(())
                        }
                        // CargoAction::Update(crate_name) => {
                        //     let _ = crate_name;
                        //     Ok(Some(Action::RefreshCargoEnv))
                        // }
                        // CargoAction::UpdateAll => Ok(Some(Action::RefreshCargoEnv)),
                        CargoAction::Install(crate_name, version) => {
                            self.action_tx.send(Action::UpdateStatus(
                                StatusLevel::Info,
                                format!("Installing {crate_name} v{version}"),
                            ))?;

                            tui.exit()?;
                            let tx = self.action_tx.clone();
                            tokio::spawn(async move {
                                if install(crate_name.clone(), Some(version.clone()), true).is_err()
                                {
                                    tx.send(Action::UpdateStatus(
                                        StatusLevel::Error,
                                        format!("Failed to install {crate_name}"),
                                    ))?;
                                    // TODO should user full error message (in a popup maybe)
                                    return Ok(());
                                }
                                tx.send(Action::UpdateStatus(
                                    StatusLevel::Info,
                                    format!("Installed {crate_name} v{version}"),
                                ))?;
                                tx.send(Action::RefreshCargoEnv)?;
                                Ok::<(), AppError>(())
                            })
                            .await??;

                            tui.enter()?;
                            tui.terminal.clear()?;
                            Ok(())
                        }
                        CargoAction::Uninstall(crate_name) => {
                            self.action_tx.send(Action::UpdateStatus(
                                StatusLevel::Info,
                                format!("Uninstalling {crate_name}"),
                            ))?;

                            let tx = self.action_tx.clone();
                            tokio::spawn(async move {
                                if uninstall(crate_name.clone(), false).is_err() {
                                    tx.send(Action::UpdateStatus(
                                        StatusLevel::Error,
                                        format!("Failed to uninstall {crate_name}"),
                                    ))?;
                                    // TODO should user full error message (in a popup maybe)
                                    return Ok(());
                                }
                                tx.send(Action::UpdateStatus(
                                    StatusLevel::Info,
                                    format!("Uninstalled {crate_name}"),
                                ))?;
                                tx.send(Action::RefreshCargoEnv)?;
                                Ok::<(), AppError>(())
                            });
                            Ok(())
                        }
                    };
                }
                Action::RefreshCargoEnv => {
                    self.cargo_env.write().await.read()?;
                    self.action_tx.send(Action::CargoEnvRefreshed)?;
                }
                _ => {}
            }

            for component in self.components.iter_mut() {
                if let Some(sub_action) = component.update(clone.clone(), tui).await? {
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
                        .send(Action::Error(format!("Failed to draw: {:?}", err)));
                }
            }
        })?;
        Ok(())
    }
}
