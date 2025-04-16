use std::{
    fs,
    path::{Path, PathBuf},
    sync::mpsc::Sender,
    thread::{self, sleep},
    time::Duration,
};

use globset::Glob;
use tracing::{debug, error, info};

use crate::{
    config::{Config, ConfigError, Library},
    job_orchestration::{Job, JobOrchestrator},
    Workflow,
};

pub(crate) struct App {
    config: Config,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    #[error(transparent)]
    Config(#[from] ConfigError),
    #[error(transparent)]
    CannotStartLibraryMonitor(std::io::Error),
}

impl App {
    pub(crate) fn new(config: Config) -> Self {
        Self { config }
    }

    /// Start the actual application.
    /// This will make sure that each configured library will be monitored, each in its separate
    /// thread.
    pub(crate) fn run(&self) -> Result<(), Error> {
        let libraries = &self.config.libraries;

        let mut library_threads = vec![];

        let (job_orchestrator, sender) = JobOrchestrator::new();

        let orchestrator_handle = thread::Builder::new()
            .name(String::from("job_orchestrator"))
            .spawn(move || {
                job_orchestrator.start();
            });

        for (library_name, library_config) in libraries.iter() {
            debug!("starting library monitor for library {library_name}");

            let workflow = self
                .config
                .get_workflow(&library_config.workflow)?
                .to_owned();

            let library = library_config.clone();
            let name = library_name.clone();
            let job_sender = sender.clone();

            let thread_builder = thread::Builder::new().name(name.clone());

            let handle = thread_builder
                .spawn(move || {
                    LibraryMonitor::new(name, library, workflow, job_sender).start();
                })
                .map_err(Error::CannotStartLibraryMonitor)?;

            library_threads.push(handle);
        }

        // let's not keep an instance after starting the threads
        drop(sender);

        for thread in library_threads {
            let _ = thread.join();
        }

        Ok(())
    }
}

struct LibraryMonitor {
    name: String,
    library: Library,
    workflow: Workflow,
    job_sender: Sender<Box<Job>>,
}

impl LibraryMonitor {
    fn new(
        name: String,
        library: Library,
        workflow: Workflow,
        job_sender: Sender<Box<Job>>,
    ) -> Self {
        Self {
            name,
            library,
            workflow,
            job_sender,
        }
    }

    fn get_directory_glob(&self) -> String {
        let extensions_part = format!(".{{{}}}", self.workflow.included_extensions.join(","));
        PathBuf::from(&self.library.directory)
            .join(format!("**/*{}", extensions_part))
            .to_string_lossy()
            .to_string()
    }
}

#[derive(Debug, thiserror::Error)]
enum MonitorError {
    #[error(transparent)]
    Scanning(#[from] ScanningError),
}

impl LibraryMonitor {
    fn start(&self) {
        loop {
            if let Err(err) = self.tick() {
                error!("error occurred during library monitoring, see below");
                error!("{err}");
            }
            sleep(Duration::from_secs(60 * 60));
        }
    }

    /// Perform a "monitoring tick" for the library.
    /// Comes down to scanning all files within
    fn tick(&self) -> Result<(), MonitorError> {
        let library_path = Path::new(&self.library.directory);

        info!("starting library scan");

        let files = scan_library(library_path, self.get_directory_glob())?;

        info!("library scan completed, found {} files", files.len());

        for file_path in files {
            self.dispatch_job(
                self.name.clone(),
                file_path.to_string_lossy().to_string(),
                self.workflow.clone(),
            );
        }

        Ok(())
    }

    /// Dispatches a job so that a [`JobOrchestrator`] can pick it up
    /// and start doing something
    fn dispatch_job(&self, library: String, file_path: String, workflow: Workflow) {
        let job = Box::new(Job::new(library, file_path, workflow));

        if let Err(err) = self.job_sender.send(job) {
            error!("unable to dispatch job for scanned file\n {err}");
        }
    }
}

#[derive(Debug, thiserror::Error)]
enum ScanningError {
    #[error("unable to iterate over library directory \"{1}\": {0}")]
    IterateDirectory(std::io::Error, PathBuf),
    #[error("unable to read entry in library directory \"{1}\": {0}")]
    ReadEntry(std::io::Error, PathBuf),
    #[error("unable to form glob to scan directory: {0}")]
    FormGlob(#[from] globset::Error),
}

/// Scan the library for matching files
fn scan_library(path: &Path, glob_pattern: String) -> Result<Vec<PathBuf>, ScanningError> {
    debug!("{}", glob_pattern);

    let paths = scan_directory_for_files(path)?;

    let globset = Glob::new(&glob_pattern)?.compile_matcher();
    let mut files: Vec<PathBuf> = paths
        .into_iter()
        .filter(|path| globset.is_match(path))
        .collect();

    files.sort();

    Ok(files)
}

/// Recursively scan the given directory for files
fn scan_directory_for_files(directory: &Path) -> Result<Vec<PathBuf>, ScanningError> {
    let mut paths: Vec<PathBuf> = vec![];

    for entry in fs::read_dir(directory)
        .map_err(|err| ScanningError::IterateDirectory(err, directory.to_path_buf()))?
    {
        let entry = entry.map_err(|err| ScanningError::ReadEntry(err, directory.to_path_buf()))?;

        if entry.path().is_dir() {
            let mut children = scan_directory_for_files(&entry.path())?;
            paths.append(&mut children);
        } else {
            paths.push(entry.path());
        }
    }

    Ok(paths)
}

#[cfg(test)]
mod tests {
    use tempdir::TempDir;

    use super::*;

    #[test]
    fn directory_is_scanned_properly() {
        let temp_test_dir = TempDir::new("omzet-test").unwrap();

        let temp_dir_path = temp_test_dir.path().to_path_buf();

        println!("{:?}", temp_dir_path);
        let test_file_a = temp_dir_path.join("a.txt");
        let test_file_b = temp_dir_path.join("b.txt");
        let test_dir_c = temp_dir_path.join("c/");
        let test_file_c = test_dir_c.join("c.txt");

        fs::write(test_file_a, "a")
            .and(fs::write(test_file_b, "b"))
            .and(fs::create_dir(test_dir_c))
            .and(fs::write(test_file_c, "c"))
            .expect("unable to setup test files");

        let files = scan_directory_for_files(&temp_dir_path).unwrap();

        temp_test_dir.close().unwrap();

        assert_eq!(files.len(), 3);
    }
}
