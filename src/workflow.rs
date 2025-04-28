use std::path::PathBuf;

#[derive(Debug, Clone)]
pub(crate) struct Library {
    pub(crate) name: String,
    pub(crate) workflow: Workflow,
    pub(crate) directory: PathBuf,
}

impl Library {
    pub(crate) fn new(name: String, workflow: Workflow, directory: PathBuf) -> Self {
        Self {
            name,
            workflow,
            directory,
        }
    }
}

/// A workflow defines which things need to happen when a new file is detected
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct Workflow {
    pub(crate) name: String,
    pub(crate) scratchpad_directory: String,
    pub(crate) included_extensions: Vec<String>,
    pub(crate) tasks: Vec<Task>,
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub(crate) enum Task {
    Custom(CustomTask),
    Builtin(BuiltinTask),
}

impl Task {
    pub(crate) fn description(&self) -> &str {
        match self {
            Task::Custom(custom_task) => custom_task.id.as_str(),
            Task::Builtin(builtin_task) => builtin_task.name(),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct CustomTask {
    /// identifier to easily reference the task
    pub(crate) id: String,
    /// A small description of what the task does
    pub(crate) description: String,
    /// The probe is a CLI command to check if the command should be executed
    pub(crate) probe: Option<Runnable>,
    /// The command is a CLI command to actually perform the task
    pub(crate) command: Runnable,
}

type Runnable = String;

impl CustomTask {
    pub(crate) fn new(
        id: String,
        description: String,
        probe: Option<Runnable>,
        command: Runnable,
    ) -> Self {
        Self {
            id,
            description,
            probe,
            command,
        }
    }
}

#[derive(Debug, PartialEq, Eq, Clone)]
pub(crate) enum BuiltinTask {
    TranscodeToH265,
}

impl BuiltinTask {
    fn name(&self) -> &str {
        match self {
            BuiltinTask::TranscodeToH265 => "transcode to h265 (builtin)",
        }
    }
}

#[derive(thiserror::Error, Debug)]
#[error("the referenced builtin task ({id}) does not exist")]
pub(crate) struct UnknownBuiltinTask {
    id: String,
}

impl TryFrom<&str> for BuiltinTask {
    type Error = UnknownBuiltinTask;

    fn try_from(value: &str) -> Result<Self, Self::Error> {
        match value {
            "builtin.transcode_to_h265" => Ok(BuiltinTask::TranscodeToH265),
            _ => Err(UnknownBuiltinTask {
                id: String::from(value),
            }),
        }
    }
}

#[cfg(test)]
mod tests {

    use super::*;

    #[test]
    fn task_can_be_created() {
        let task = CustomTask::new(
            "test-task".to_owned(),
            "some description".to_owned(),
            Some("echo probe".to_owned()),
            "echo done".to_owned(),
        );

        assert_eq!("test-task", task.id.as_str());
        assert_eq!("some description", task.description.as_str());
        assert!(task.probe.is_some());
        assert_eq!("echo probe", task.probe.unwrap().as_str());
        assert_eq!("echo done", task.command.as_str());
    }
}
