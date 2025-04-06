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

    let args = Cli::parse();
    let mut app = App::new(
        args.tick_rate,
        args.frame_rate,
        args.counter,
        args.proj_dir,
        args.search_term,
    )?;
    app.run().await?;
    Ok(())
}
