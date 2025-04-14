use serde::Deserialize;

/// A workflow defines which things need to happen when a new file is detected
#[derive(Debug, Deserialize, Clone)]
pub(crate) struct Workflow {
    pub(crate) name: String,
    pub(crate) scratchpad_directory: String,
    pub(crate) included_extensions: Vec<String>,
    pub(crate) tasks: Vec<Task>,
}

#[derive(Debug, Clone, Deserialize)]
pub(crate) struct Task {
    /// The name of a task
    pub(crate) name: String,
    /// The probe is a CLI command to check if the command should be executed
    pub(crate) probe: Option<Runnable>,
    /// The command is a CLI command to actually perform the task
    pub(crate) command: Runnable,
}

type Runnable = String;

impl Task {
    pub(crate) fn new(name: String, probe: Option<Runnable>, command: Runnable) -> Self {
        Self {
            name,
            probe,
            command,
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_can_be_created() {
        let task = Task::new(
            "test-task".to_owned(),
            Some("echo probe".to_owned()),
            "echo done".to_owned(),
        );

        assert_eq!("test-task", task.name.as_str());
        assert!(task.probe.is_some());
        assert_eq!("echo probe", task.probe.unwrap().as_str());
        assert_eq!("echo done", task.command.as_str());
    }
}
