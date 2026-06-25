//! `cargo-seek` is a terminal UI for searching, adding, and installing cargo crates.
//!
//! It runs standalone (`cargo-seek`) or as a cargo subcommand (`cargo seek`), and is built on
//! `ratatui`, `crossterm`, and `tokio`.
//!
//! # Architecture
//!
//! The app follows an Elm-style component + action message loop: nothing calls between components
//! directly, and everything communicates by sending `Action`s over a single tokio mpsc channel.
//! `App` (in `app`) owns the components, the channel, and the shared cargo environment; on each
//! iteration it drains terminal events into actions, dispatches them, and renders.

mod action;
mod app;
mod cargo;
mod cli;
mod components;
mod config;
mod errors;
mod logging;
mod search;
mod tui;
mod util;

use clap::Parser;
use cli::Cli;

use crate::app::App;

#[tokio::main]
async fn main() -> color_eyre::Result<()> {
    errors::init()?;
    logging::init()?;

    let args = filter_subcommand(std::env::args().collect());
    let args = Cli::parse_from(args);
    let mut app = App::new(
        args.tick_rate,
        args.frame_rate,
        args.counter,
        args.project_dir,
        args.search_term,
    )?;
    app.run().await?;
    Ok(())
}

fn filter_subcommand(args: Vec<String>) -> Vec<String> {
    // Check if the binary was invoked as a Cargo subcommand
    if args.get(1).map(String::as_str) == Some("seek") {
        // Skip the seek subcommand
        std::iter::once(args[0].clone())
            .chain(args.into_iter().skip(2))
            .collect()
    } else {
        args
    }
}
