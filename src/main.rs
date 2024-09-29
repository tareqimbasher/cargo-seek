mod action;
mod app;
mod cli;
mod components;
mod config;
mod errors;
mod http_client;
mod logging;
mod tui;
mod util;

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
