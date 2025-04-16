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

use tracing::{debug, error, info, warn};

use crate::{workflow::Task, Workflow};

pub(crate) struct SourceFilePath(String);

impl Deref for SourceFilePath {
    type Target = Path;

    fn deref(&self) -> &Self::Target {
        Path::new(&self.0)
    }
}

/// a new type
impl SourceFilePath {
    pub(crate) fn new(path: String) -> Self {
        Self(path)
    }
}

#[derive(thiserror::Error, Debug)]
pub(crate) enum RunnerError {
    #[error(transparent)]
    PreparationFailed(#[from] PreparationError),

    #[error("a task probe was aborted")]
    ProbeAborted,

    #[error(transparent)]
    CompletionFailed(#[from] CompletionError),
}

/// A runner is able to execute a workflow for a given file. It should provide a
/// [`WorkflowReport`] once it finishes the workflow.
pub(crate) trait WorkflowRunner {
    fn run_workflow(
        &self,
        workflow: &Workflow,
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
    Run,
    Skip,
    Abort,
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum PreparationError {
    #[error("unable to create scratchpad directory: {0}")]
    UnableToCreateScratchpad(#[source] std::io::Error),
    #[error("unable to copy in source file: {0}")]
    UnableToCopySourceFile(#[source] std::io::Error),
}

#[derive(Debug, thiserror::Error)]
pub(crate) enum CompletionError {
    #[error("unable to move transformed file to source path")]
    UnableToMoveFile(#[source] std::io::Error),
}

struct DefaultRunnerContext {
    scratchpad_directory: String,
    source_file_path: String,
    target_file_name: String,
    output_file_name: String,
}

impl DefaultRunner {
    pub(crate) fn new() -> Self {
        Self {}
    }

    /// Create the area where file transformations can be done
    fn prepare(
        &self,
        scratchpad_directory: &str,
        source_file_path: &SourceFilePath,
    ) -> Result<DefaultRunnerContext, PreparationError> {
        debug!("creating scratchpad directory at {}", scratchpad_directory);

        let scratchpad_path = Path::new(&scratchpad_directory);
        fs::create_dir_all(scratchpad_path).map_err(PreparationError::UnableToCreateScratchpad)?;

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
            .map_err(PreparationError::UnableToCopySourceFile)?;

        Ok(DefaultRunnerContext {
            scratchpad_directory: scratchpad_directory.to_owned(),
            source_file_path: source_file_path.0.clone(),
            target_file_name: target_file_name.to_owned(),
            output_file_name: generate_output_file_name(&target_file_name),
        })
    }

    /// Complete a run which will make sure that no artifacts are left behind
    /// and that the transformed file replaces the original source file
    fn complete_run(&self, context: &DefaultRunnerContext) -> Result<(), CompletionError> {
        let source_file_path = Path::new(&context.source_file_path);

        let target_file_path =
            Path::new(&context.scratchpad_directory).join(&context.target_file_name);

        debug!("copying target file back to source file");
        fs::rename(target_file_path, source_file_path).map_err(CompletionError::UnableToMoveFile)
    }

    /// Runs the probe
    fn run_probe(
        &self,
        probe: &str,
        task_name: &str,
        context: &DefaultRunnerContext,
    ) -> ProbeResult {
        match run_script(
            probe,
            HashMap::from([
                ("OMZET_INPUT".to_owned(), context.target_file_name.clone()),
                ("OMZET_TASK".to_owned(), String::from(task_name)),
            ]),
            &context.scratchpad_directory,
        ) {
            Ok((exit_code, _output, _stderr)) => match exit_code {
                0 => ProbeResult::Run,
                _ => ProbeResult::Skip,
            },
            Err(_) => ProbeResult::Abort,
        }
    }

    /// Run the actual task
    fn run_task(&self, task: &Task, context: &DefaultRunnerContext) -> TaskReport {
        let env_vars: HashMap<String, String> = HashMap::from([
            ("OMZET_INPUT".to_owned(), context.target_file_name.clone()),
            ("OMZET_OUTPUT".to_owned(), context.output_file_name.clone()),
        ]);

        let result = run_script(&task.command, env_vars, &context.scratchpad_directory)
            .expect("failed to run task script");

        // the file that the task should write to output
        let output_file_path =
            PathBuf::from(&context.scratchpad_directory).join(&context.output_file_name);

        // the file that the task uses as input
        let target_file_path =
            PathBuf::from(&context.scratchpad_directory).join(&context.target_file_name);

        // move the output file so it becomes the input file of any next task
        let task_completion_rename = fs::exists(&output_file_path)
            .map_err(|_| false)
            .and_then(|exists| {
                if exists {
                    return fs::rename(output_file_path, target_file_path)
                        .map_err(|_| false)
                        .map(|_| true);
                }

                Ok(false)
            })
            .unwrap_or(false);

        if !task_completion_rename {
            warn!(
                "task \"{}\" did not output any file, following task will work on the same source",
                task.name
            );
        }

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
    env_vars: HashMap<String, String>,
    working_directory: &str,
) -> Result<(i32, String, String), String> {
    let mut options = ScriptOptions::new();

    options.exit_on_error = true;
    options.print_commands = true;
    options.env_vars = Some(env_vars);
    options.working_directory = Some(PathBuf::from(working_directory));

    let _args = vec![];

    let mut child = run_script::spawn(script, &_args, &options)
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

/// The public interface of the the default runner
impl WorkflowRunner for DefaultRunner {
    /// Will synchronously run the workflow's tasks
    /// and produce a [`WorkflowReport`]
    fn run_workflow(
        &self,
        workflow: &Workflow,
        source_file: SourceFilePath,
    ) -> Result<WorkflowReport, RunnerError> {
        info!("starting workflow: {}", &workflow.name);

        let context = self.prepare(&workflow.scratchpad_directory, &source_file)?;

        info!("running probes to determine tasks");
        let tasks = workflow.tasks.clone();

        let probe_results: Vec<(&Task, ProbeResult)> = tasks
            .iter()
            .map(|task| match &task.probe {
                Some(probe) => (task, self.run_probe(probe, &task.name, &context)),
                None => (task, ProbeResult::Run),
            })
            .collect();

        let has_aborted_probe_result = probe_results
            .iter()
            .any(|(_, probe_result)| *probe_result == ProbeResult::Abort);

        if has_aborted_probe_result {
            return Err(RunnerError::ProbeAborted);
        }

        let tasks_to_run: Vec<&Task> = probe_results
            .into_iter()
            .filter(|(_, probe_result)| match probe_result {
                ProbeResult::Run => true,
                ProbeResult::Skip => false,
                ProbeResult::Abort => false,
            })
            .map(|(task, _)| task)
            .collect();

        if tasks_to_run.is_empty() {
            info!("no probes requested to run");
            return Ok(WorkflowReport::new(workflow.clone()));
        }

        info!("running {} tasks", tasks_to_run.len());

        let mut task_reports: Vec<TaskReport> = vec![];

        for task in tasks_to_run.into_iter() {
            info!("task \"{}\" started", task.name);

            let task_run_result = self.run_task(task, &context);

            task_reports.push(task_run_result);

            info!("task \"{}\" completed", task.name);
        }

        self.complete_run(&context)?;

        Ok(WorkflowReport::new(workflow.clone()).with_reports(task_reports))
    }
}

/// Generate a target file from the source file
fn generate_target_file(source_file_path: &Path) -> String {
    let uuid = Uuid::new_v4();

    let file_name = source_file_path
        .file_stem()
        .expect("failed to take file name");
    let extension = source_file_path
        .extension()
        .expect("failed to take file extension");

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
        let path = Path::new(source_file);
        let subject_file = generate_target_file(path);

        assert!(subject_file.len() == "test_file.mkv".len() + 37);
    }
}
