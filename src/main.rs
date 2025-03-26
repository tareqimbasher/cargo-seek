mod action;
mod app;
mod cargo;
mod cli;
mod config;
mod errors;
mod logging;
mod search;
mod tui;
mod util;
mod components;

use clap::Parser;
use cli::Cli;
use color_eyre::Result;

use crate::app::App;

#[tokio::main]
async fn main() -> Result<()> {
    errors::init()?;
    logging::init()?;

    let args = Cli::parse();
    let mut app = App::new(args.tick_rate, args.frame_rate, args.counter)?;
    app.run().await?;
    Ok(())
}
