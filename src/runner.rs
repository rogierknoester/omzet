use run_script::ScriptOptions;
use std::{
    collections::HashMap,
    fs::{self},
    io::{BufRead, BufReader},
    ops::Deref,
    path::{Path, PathBuf},
    process::Output,
};
use uuid::Uuid;

use tracing::{debug, error, info};

use crate::{workflow::Task, Workflow};

pub(crate) struct SourceFilePath(String);

impl Deref for SourceFilePath {
    type Target = String;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

impl SourceFilePath {
    pub(crate) fn new(path: String) -> Self {
        Self(path)
    }
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum RunnerError {
    #[error("preparation for the workflow failed: {0}")]
    PreparationFailed(String),

    #[error("a task probe was aborted")]
    ProbeAborted,
}

/// A runner is able to execute a workflow for a given file. It should provide a
/// [`WorkflowReport`] once it finishes the workflow.
pub(crate) trait Runner {
    fn run_workflow(
        &self,
        workflow: Workflow,
        source_file: SourceFilePath,
    ) -> Result<WorkflowReport, RunnerError>;
}

#[derive(Debug)]
pub(crate) struct WorkflowReport {
    workflow: Workflow,
    task_reports: Vec<TaskReport>,
}

impl WorkflowReport {
    fn new(workflow: Workflow) -> Self {
        Self {
            workflow,
            task_reports: vec![],
        }
    }

    fn with_reports(self, task_reports: Vec<TaskReport>) -> Self {
        Self {
            workflow: self.workflow,
            task_reports,
        }
    }
}

/// Contains information about the execution of a single task. Its full output to stderr and stdout is collected.
#[derive(Debug)]
struct TaskReport {
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
}

impl From<Output> for TaskReport {
    fn from(value: Output) -> Self {
        Self {
            exit_code: value.status.code(),
            stdout: String::from_utf8(value.stdout).expect("cannot get out of task"),
            stderr: String::from_utf8(value.stderr).expect("cannot get out of task"),
        }
    }
}

/// The default runner
#[derive(Debug)]
pub(crate) struct DefaultRunner {}

#[derive(Debug, PartialEq)]
enum ProbeResult {
    ShouldRun,
    ShouldSkip,
    ShouldAbort,
}

#[derive(Debug, thiserror::Error)]
enum ScratchpadError {
    #[error("unable to create scratchpad directory")]
    UnableToCreate(#[source] std::io::Error),
    #[error("unable to copy in source file")]
    UnableToCopySourceFile(#[source] std::io::Error),
}

impl DefaultRunner {
    pub(crate) fn new() -> Self {
        Self {}
    }

    /// Create the area where file transformations can be done
    fn prepare_scratchpad(
        &self,
        scratchpad_directory: &str,
        source_file_path: &SourceFilePath,
    ) -> Result<String, ScratchpadError> {
        debug!("creating scratchpad directory at {}", scratchpad_directory);

        let scratchpad_path = Path::new(&scratchpad_directory);
        fs::create_dir_all(scratchpad_path).map_err(ScratchpadError::UnableToCreate)?;

        let target_file_name = generate_target_file(source_file_path);
        debug!("generated target file name: {}", target_file_name);

        let target_file_path = Path::new(scratchpad_directory)
            .join(PathBuf::from(&target_file_name))
            .into_os_string()
            .into_string()
            .expect("unable to create target file path");

        debug!(
            "copying source file into scratchpad directory at {}",
            target_file_path
        );
        fs::copy(&source_file_path.0, target_file_path)
            .map_err(ScratchpadError::UnableToCopySourceFile)
            .map(|_| target_file_name)
    }

    /// Runs the probe
    fn run_probe(
        &self,
        probe: &str,
        target_file_name: &str,
        scratchpad_directory: &str,
    ) -> ProbeResult {
        match run_script(
            probe,
            Some(HashMap::from([(
                "OMZET_INPUT".to_owned(),
                target_file_name.to_string(),
            )])),
            scratchpad_directory,
        ) {
            Ok((exit_code, _output, _stderr)) => match exit_code {
                0 => ProbeResult::ShouldRun,
                _ => ProbeResult::ShouldSkip,
            },
            Err(_) => ProbeResult::ShouldAbort,
        }
    }

    fn run_task(
        &self,
        task: &Task,
        target_file_name: &str,
        scratchpad_directory: &str,
    ) -> TaskReport {
        // @todo lift this up, it's deterministic for target name
        let output_file_name = generate_output_file_name(target_file_name);

        let env_vars: HashMap<String, String> = HashMap::from([
            ("OMZET_INPUT".to_owned(), target_file_name.to_string()),
            ("OMZET_OUTPUT".to_owned(), output_file_name),
        ]);

        let result = run_script(&task.command, Some(env_vars), scratchpad_directory)
            .expect("failed to run task script");

        // @todo check if output file exists now
        //       mv/rename it to target file, i.e. clean up and finish the task

        TaskReport {
            exit_code: Some(result.0),
            stdout: result.1,
            stderr: result.2,
        }
    }
}

/// Run a script. For example a task's command or probe.
fn run_script(
    script: &str,
    env_vars: Option<HashMap<String, String>>,
    working_directory: &str,
) -> Result<(i32, String, String), String> {
    let mut options = ScriptOptions::new();

    if let Some(env_vars) = env_vars {
        options.env_vars = Some(env_vars);
    }

    options.working_directory = Some(PathBuf::from(working_directory));

    let args = vec![];

    let mut child = run_script::spawn(script, &args, &options)
        .expect("failed to spawn child when running script");

    let child_stdout = child
        .stdout
        .take()
        .expect("failed to get stdout of child process");

    let child_stderr = child
        .stderr
        .take()
        .expect("failed to get stderr of child process");

    let mut stdout_reader = BufReader::new(child_stdout);
    let mut stderr_reader = BufReader::new(child_stderr);

    let mut stdout_lines = String::new();
    let mut stderr_lines = String::new();
    let mut current_line = String::new();

    while stdout_reader.read_line(&mut current_line).unwrap_or(0) > 0 {
        debug!("stdout: {}", current_line.trim_end());
        stdout_lines.push_str(&current_line);
        current_line.clear();
    }

    while stderr_reader.read_line(&mut current_line).unwrap_or(0) > 0 {
        debug!("stderr: {}", current_line.trim_end());
        stderr_lines.push_str(&current_line);
        current_line.clear();
    }

    let result = child.wait().expect("failed to wait for child");

    Ok((
        result.code().expect("child was terminal by a signal"),
        stdout_lines,
        stderr_lines,
    ))
}

impl Runner for DefaultRunner {
    /// Will synchronously run the workflow's tasks
    /// and produce a [`WorkflowReport`]
    fn run_workflow(
        &self,
        workflow: Workflow,
        source_file: SourceFilePath,
    ) -> Result<WorkflowReport, RunnerError> {
        info!("starting workflow: {}", &workflow.name);

        let target_file_name = self
            .prepare_scratchpad(&workflow.scratchpad_directory, &source_file)
            .map_err(|err| RunnerError::PreparationFailed(err.to_string()))?;

        info!("running probes to determine tasks");
        let tasks = workflow.tasks.clone();

        let probe_results: Vec<(&Task, ProbeResult)> = tasks
            .iter()
            .map(|task| match &task.probe {
                Some(probe) => (
                    task,
                    self.run_probe(probe, &target_file_name, &workflow.scratchpad_directory),
                ),
                None => (task, ProbeResult::ShouldRun),
            })
            .collect();

        let has_aborted_probe_result = probe_results
            .iter()
            .any(|(_, probe_result)| *probe_result == ProbeResult::ShouldAbort);

        if has_aborted_probe_result {
            return Err(RunnerError::ProbeAborted);
        }

        let tasks_to_run: Vec<&Task> = probe_results
            .into_iter()
            .filter(|(_task, probe_result)| match probe_result {
                ProbeResult::ShouldRun => true,
                ProbeResult::ShouldSkip => false,
                ProbeResult::ShouldAbort => false,
            })
            .map(|(task, _probe_result)| task)
            .collect();

        if tasks_to_run.is_empty() {
            info!("no probes requested to run");
            return Ok(WorkflowReport::new(workflow));
        }

        info!("running {} tasks", tasks_to_run.len());

        let mut task_reports: Vec<TaskReport> = vec![];

        for task in tasks_to_run.into_iter() {
            info!("running task \"{}\"", &task.name);

            let task_run_result =
                self.run_task(&task, &target_file_name, &workflow.scratchpad_directory);

            task_reports.push(task_run_result);

            info!("completed task \"{}\"", &task.name);
        }

        Ok(WorkflowReport::new(workflow).with_reports(task_reports))
    }
}

/// Generate a target file from the source file
fn generate_target_file(source_file_path: &str) -> String {
    let uuid = Uuid::new_v4();

    let path = Path::new(source_file_path);

    let file_name = path.file_stem().expect("failed to take file name");
    let extension = path.extension().expect("failed to take file extension");

    format!(
        "{}-{}.{}",
        file_name.to_string_lossy(),
        uuid,
        extension.to_string_lossy()
    )
}

fn generate_output_file_name(target_file_name: &str) -> String {
    let path = Path::new(target_file_name);

    let file_name = path.file_stem().expect("failed to take file name");
    let extension = path.extension().expect("failed to take file extension");

    format!(
        "{}.out.{}",
        file_name.to_string_lossy(),
        extension.to_string_lossy()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subject_file_generation() {
        let source_file = "/tmp/test_file.mkv";
        let subject_file = generate_target_file(source_file);

        assert!(subject_file.len() == "test_file.mkv".len() + 37);
    }
}
