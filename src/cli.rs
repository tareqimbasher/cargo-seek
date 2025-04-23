use clap::Parser;
use std::path::PathBuf;

use crate::config::{get_config_dir, get_data_dir};

fn get_current_dir() -> Option<PathBuf> {
    std::env::current_dir().ok()
}

#[derive(Parser, Debug)]
#[command(bin_name = "crate-seek", author, version = version(), about)]
pub struct Cli {
    /// Path to a directory containing (or one of its parents) a Cargo.toml file
    #[arg(default_value=get_current_dir().unwrap_or_default().into_os_string())]
    pub project_dir: Option<PathBuf>,

    /// Search term to trigger search at startup
    #[arg(short, long = "search", value_name = "TERM")]
    pub search_term: Option<String>,

    /// Frame rate, i.e. number of frames per second
    #[arg(short, long = "fps", value_name = "FLOAT", default_value_t = 30.0)]
    pub frame_rate: f64,

    /// Tick rate, i.e. number of ticks per second
    #[arg(short, long = "tps", value_name = "FLOAT", default_value_t = 4.0)]
    pub tick_rate: f64,

    /// Show TPS/FPS counter
    #[arg(long)]
    pub counter: bool,
}

const VERSION_MESSAGE: &str = concat!(
    env!("CARGO_PKG_VERSION"),
    " -",
    env!("VERGEN_GIT_DESCRIBE"),
    " (",
    env!("VERGEN_BUILD_DATE"),
    ")"
);

pub fn version() -> String {
    let description = clap::crate_description!();

    // let current_exe_path = PathBuf::from(clap::crate_name!()).display().to_string();
    let config_dir_path = get_config_dir().display().to_string();
    let data_dir_path = get_data_dir().display().to_string();

    format!(
        "\
{VERSION_MESSAGE}

{description}

Config dir: {config_dir_path}
Data dir:   {data_dir_path}"
    )
}
