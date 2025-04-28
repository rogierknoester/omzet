use std::{
    collections::{HashMap, HashSet},
    env,
    fs::{self, create_dir, exists},
    string::FromUtf8Error,
};

use serde::Deserialize;
use tracing::{debug, error, info};

use crate::{
    workflow::{BuiltinTask, CustomTask, Library, Task, UnknownBuiltinTask},
    Workflow,
};

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
    UnknownWorkflow(String),
    #[error(transparent)]
    UnknownBuiltinTask(#[from] UnknownBuiltinTask),
    #[error("custom task with id \"{0}\" was referenced, but it is not configured")]
    UnknownCustomTask(String),
}

const EXAMPLE_CONFIG: &str = include_str!("../example/config.toml");

pub(crate) struct Config {
    pub(crate) libraries: Vec<Library>,
}

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

    let toml_config = fs::read(config_file_path)
        .map_err(ConfigError::UnableToReadConfiguration)
        .and_then(|bytes| String::from_utf8(bytes).map_err(ConfigError::UnableToReadConfigAsUtf8))
        .and_then(|data| {
            toml::from_str::<TomlConfig>(&data).map_err(ConfigError::UnableToDeserialize)
        })?;

    let config = Config {
        libraries: denormalize_config(toml_config)?,
    };

    Ok(config)
}

#[derive(Debug, Deserialize)]
pub struct TomlConfig {
    pub(crate) libraries: HashMap<String, LibraryConfig>,
    pub(crate) workflows: Vec<WorkflowConfig>,
    pub(crate) tasks: Vec<TaskConfig>,
}

#[derive(Debug, Deserialize, Clone)]
pub(crate) struct LibraryConfig {
    pub(crate) directory: String,
    pub(crate) workflow: String,
}

impl TomlConfig {
    pub(crate) fn build_workflow(&self, name: &str) -> Result<Workflow, ConfigError> {
        self.workflows
            .iter()
            .find(|workflow_config| workflow_config.name == name)
            .ok_or(ConfigError::UnknownWorkflow(name.to_string()))
            .and_then(|workflow_config| {
                let tasks = self.build_tasks(&workflow_config.tasks)?;

                Ok(Workflow {
                    name: workflow_config.name.clone(),
                    scratchpad_directory: workflow_config.scratchpad_directory.clone(),
                    included_extensions: workflow_config
                        .included_extensions
                        .clone()
                        .into_iter()
                        .collect(),
                    tasks,
                })
            })
    }

    pub fn build_tasks(&self, ids: &[TaskId]) -> Result<Vec<Task>, ConfigError> {
        let mut tasks = Vec::with_capacity(ids.len());
        // loop over names to ensure order
        for id in ids {
            if id.0.starts_with("builtin.") {
                let builtin_task = BuiltinTask::try_from(id.0.as_str())?;

                tasks.push(Task::Builtin(builtin_task));
            } else {
                let custom_task = self
                    .tasks
                    .iter()
                    .find(|t| t.id == *id)
                    .ok_or(ConfigError::UnknownCustomTask(id.0.clone()))?;

                tasks.push(Task::Custom(custom_task.into()));
            }
        }

        Ok(tasks)
    }
}

impl From<&TaskConfig> for CustomTask {
    fn from(value: &TaskConfig) -> Self {
        Self {
            id: value.id.0.clone(),
            description: value.description.clone(),
            probe: value.probe.clone(),
            command: value.command.clone(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct WorkflowConfig {
    name: String,
    scratchpad_directory: String,
    included_extensions: HashSet<String>,
    tasks: Vec<TaskId>,
}

#[derive(Debug, Deserialize, Eq, PartialEq)]
struct TaskId(String);

#[derive(Debug, Deserialize)]
struct TaskConfig {
    id: TaskId,
    description: String,
    probe: Option<String>,
    command: String,
}

/// Denormalize the config into libraries configured with their workflows
fn denormalize_config(config: TomlConfig) -> Result<Vec<Library>, ConfigError> {
    let mut libraries = Vec::with_capacity(config.libraries.len());

    for (name, library_config) in config.libraries.iter() {
        libraries.push(Library::new(
            name.clone(),
            config.build_workflow(&library_config.workflow)?,
            (&library_config.directory).into(),
        ));
    }

    info!("{:?}", libraries);
    Ok(libraries)
}
