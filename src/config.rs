use std::{
    collections::HashMap,
    env,
    fs::{self, create_dir, exists},
    iter::Map,
    string::FromUtf8Error,
};

use serde::Deserialize;
use tracing::{debug, error};

use crate::Workflow;

#[derive(Debug, thiserror::Error)]
pub(crate) enum ConfigError {
    #[error("no HOME environment variable is set -- cannot know where configuration lives")]
    MissingHomeEnvironmentVariable,
    #[error("unable to access directory of configuration: {0}")]
    UnableToAccessDirectory(std::io::Error),
    #[error("unable to create directory for configuration: {0}")]
    UnableToCreateDirectory(std::io::Error),
    #[error("unable to write example configuration: {0}")]
    UnableToWriteExampleConfiguration(std::io::Error),
    #[error("unable to read the configuration file as utf-8 string: {0}")]
    UnableToReadConfigAsUtf8(FromUtf8Error),
    #[error("unable to read the configuration file: {0}")]
    UnableToReadConfiguration(std::io::Error),
    #[error("unable to deserialize config toml: {0}")]
    UnableToDeserialize(toml::de::Error),
    #[error("workflow with name \"{0}\", referenced in config, does not exist")]
    WorkflowDoesNotExist(String),
}

const EXAMPLE_CONFIG: &str = include_str!("../example/config.toml");

pub(crate) fn read_config() -> Result<Config, ConfigError> {
    let home_dir = env::var_os("HOME")
        .ok_or(ConfigError::MissingHomeEnvironmentVariable)?
        .to_string_lossy()
        .to_string();

    let config_dir = format!("{}/.config/omzet", home_dir);
    let config_file_path = format!("{}/omzet.toml", config_dir);

    let exists = exists(&config_dir).map_err(ConfigError::UnableToAccessDirectory)?;

    if !exists {
        debug!("directory for configuration does not exist yet, creating it");
        create_dir(&config_dir)
            .map_err(ConfigError::UnableToCreateDirectory)
            .and_then(|_| {
                debug!("writing example config because none exists");
                fs::write(&config_file_path, EXAMPLE_CONFIG)
                    .map_err(ConfigError::UnableToWriteExampleConfiguration)
            })?;
    }

    let config = fs::read(config_file_path)
        .map_err(ConfigError::UnableToReadConfiguration)
        .and_then(|bytes| String::from_utf8(bytes).map_err(ConfigError::UnableToReadConfigAsUtf8))
        .and_then(|data| {
            toml::from_str::<Config>(&data).map_err(ConfigError::UnableToDeserialize)
        })?;

    debug!("loaded config:");
    debug!("{:?}", &config);

    Ok(config)
}

#[derive(Debug, Deserialize)]
pub(crate) struct Config {
    pub(crate) libraries: HashMap<String, Library>,
    pub(crate) workflows: HashMap<String, Workflow>,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct Library {
    pub(crate) directory: String,
    pub(crate) workflow: String,
}

impl Config {
    pub(crate) fn get_workflow(&self, name: &str) -> Result<&Workflow, ConfigError> {
        self.workflows
            .get(name)
            .ok_or(ConfigError::WorkflowDoesNotExist(name.to_string()))
    }
}
