use std::{
    fmt::Display,
    thread::{self, current, sleep, sleep_ms, JoinHandle},
    time::Duration,
};

use tracing::debug;

use crate::{
    config::{self, Config, ConfigError, Library},
    Workflow,
};

pub(crate) struct App {
    config: Config,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    Config(#[from] ConfigError),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Config(config_error) => config_error.fmt(f),
        }
    }
}

impl App {
    pub(crate) fn new(config: Config) -> Self {
        Self { config }
    }

    pub(crate) fn run(&self) -> Result<(), Error> {
        let libraries = &self.config.libraries;

        let mut library_threads = vec![];

        for (library_name, library_config) in libraries.iter() {
            debug!("starting library monitor for library {}", library_name);

            let workflow = self
                .config
                .get_workflow(&library_config.workflow)?
                .to_owned();

            let library = library_config.to_owned();

            let thread_handle = thread::spawn(move || {
                let library_monitor = LibraryMonitor::new(library, workflow);

                library_monitor.start();
            });

            library_threads.push(thread_handle);
        }

        for thread in library_threads {
            thread.join();
        }

        Ok(())
    }
}

struct LibraryMonitor {
    library: Library,
    workflow: Workflow,
}

impl LibraryMonitor {
    fn new(library: Library, workflow: Workflow) -> Self {
        Self { library, workflow }
    }
}

impl LibraryMonitor {
    fn start(&self) {
        loop {
            println!("hi {:?}", current().id());
            sleep(Duration::from_millis(3000));
        }
    }
}
