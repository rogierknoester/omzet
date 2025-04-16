use std::{
    io::{self},
    process::exit,
};

use app::App;
use config::read_config;
use runner::{DefaultRunner, SourceFilePath};
use tracing::{debug, error, level_filters::LevelFilter};
use tracing_subscriber::EnvFilter;
use workflow::Workflow;

mod app;
mod config;
mod job_orchestration;
mod runner;
mod workflow;

fn main() {
    setup_logging();

    let config = match read_config() {
        Ok(config) => config,
        Err(err) => {
            error!("error occurred while trying to read configuration");
            eprintln!("{}", err);

            exit(1);
        }
    };

    let app = App::new(config);

    match app.run() {
        Ok(_) => {
            debug!("exiting omzet");
            exit(0);
        }
        Err(err) => {
            error!("exiting omzet because an error occurred, see below.");
            error!("{}", err);
            exit(1);
        }
    }
}

fn setup_logging() {
    tracing_subscriber::fmt()
        .with_writer(io::stderr)
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .with_thread_names(true)
        .init();
}
