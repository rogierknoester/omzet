use std::{
    io::{self},
    process::exit,
};

use app::App;
use config::read_config;
use runner::{DefaultRunner, Runner, SourceFilePath};
use tracing::{debug, error, info, level_filters::LevelFilter};
use tracing_subscriber::EnvFilter;
use workflow::Workflow;

mod app;
mod config;
mod runner;
mod workflow;
mod job_orchestration;

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

    let runner = DefaultRunner::new();

    let source_file_path = SourceFilePath::new("/tmp/omzet/bob.mkv".to_string());

    let workflow = config
        .workflows
        .get("example")
        .expect("unable to take workflow");

    let result = runner.run_workflow(workflow, source_file_path);

    match result {
        Ok(_) => info!("performed workflow"),
        Err(err) => error!("unable to perform workflow: {}", err),
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
