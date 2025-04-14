use std::{
    fmt::Display,
    fs,
    path::{Path, PathBuf},
    thread::{self, sleep},
    time::Duration,
};

use globset::Glob;
use tracing::{debug, error, info};

use crate::{
    config::{Config, ConfigError, Library},
    Workflow,
};

pub(crate) struct App {
    config: Config,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum Error {
    Config(#[from] ConfigError),
    CannotStartLibraryMonitor(std::io::Error),
}

impl Display for Error {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Error::Config(config_error) => config_error.fmt(f),
            Error::CannotStartLibraryMonitor(io_error) => io_error.fmt(f),
        }
    }
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

        for (library_name, library_config) in libraries.iter() {
            debug!("starting library monitor for library {}", library_name);

            let workflow = self
                .config
                .get_workflow(&library_config.workflow)?
                .to_owned();

            let library = library_config.to_owned();

            let thread_builder = thread::Builder::new().name(library_name.to_owned());

            let handle = thread_builder
                .spawn(move || {
                    LibraryMonitor::new(library, workflow).start();
                })
                .map_err(Error::CannotStartLibraryMonitor)?;

            library_threads.push(handle);
        }

        for thread in library_threads {
            let _ = thread.join();
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
            debug!("performing library scan");
            if let Err(err) = self.tick() {
                error!("error occurred during library monitoring, see below");
                error!("{}", err);
            }
            sleep(Duration::from_secs(60));
        }
    }

    fn tick(&self) -> Result<(), MonitorError> {
        let library_path = Path::new(&self.library.directory);

        scan_library(library_path, self.get_directory_glob())?;

        Ok(())
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

    info!("files: {:?}", files);

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

struct ScannedFile {
    path: String,
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
