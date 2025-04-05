use std::sync::LazyLock;
use tracing_error::ErrorLayer;
use tracing_subscriber::{fmt, prelude::*, EnvFilter};

use crate::config;

static LOG_ENV: LazyLock<String> =
    LazyLock::new(|| format!("{}_LOGLEVEL", &*config::PROJECT_NAME));
static LOG_FILE: LazyLock<String> = LazyLock::new(|| format!("{}.log", env!("CARGO_PKG_NAME")));

pub fn init() -> color_eyre::Result<()> {
    let directory = config::get_data_dir();
    std::fs::create_dir_all(directory.clone())?;
    let log_path = directory.join(&*LOG_FILE);
    let log_file = std::fs::File::create(log_path)?;
    let env_filter = EnvFilter::builder().with_default_directive(tracing::Level::INFO.into());
    // If the `RUST_LOG` environment variable is set, use that as the default, otherwise use the
    // value of the `LOG_ENV` environment variable. If the `LOG_ENV` environment variable contains
    // errors, then this will return an error.
    let env_filter = env_filter
        .try_from_env()
        .or_else(|_| env_filter.with_env_var(&*LOG_ENV).from_env())?;
    let file_subscriber = fmt::layer()
        .with_file(true)
        .with_line_number(true)
        .with_writer(log_file)
        .with_target(false)
        .with_ansi(false)
        .with_filter(env_filter);
    tracing_subscriber::registry()
        .with(file_subscriber)
        .with(ErrorLayer::default())
        .try_init()?;
    Ok(())
}
