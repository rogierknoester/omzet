use std::{
    fs,
    path::{Path, PathBuf},
};

use tracing::{debug, info, warn};

use crate::{
    job_orchestration::{TaskReport, WorkflowReport},
    workflow::Task,
    workflow_runner::util::{generate_output_file_name, generate_target_file},
    Workflow,
};

use super::common::{ProbeResult, ProbeRunner, ProbingContext, TaskContext, TaskRunner};

#[derive(thiserror::Error, Debug)]
pub(crate) enum RunnerError {
    #[error(transparent)]
    PreparationFailed(#[from] PreparationError),

    #[error("a task probe was aborted")]
    ProbeAborted,

    #[error(transparent)]
    CompletionFailed(#[from] CompletionError),
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

struct Context {
    /// Directory where tasks are executed
    scratchpad_directory: PathBuf,
    /// Path to the original source file
    source_file_path: PathBuf,
    /// Path to the file each task should use as input
    input_file: PathBuf,
    /// Path where each task should output
    output_file: PathBuf,
}

pub(crate) struct Runner {}

impl Runner {
    pub(crate) fn new() -> Self {
        Self {}
    }
}

/// The public interface of the the default runner
impl Runner {
    /// Will synchronously run the workflow's tasks
    /// and produce a [`WorkflowReport`]
    pub(crate) fn run_workflow(
        &self,
        workflow: &Workflow,
        source_file: PathBuf,
    ) -> Result<WorkflowReport, RunnerError> {
        info!("starting workflow: {}", &workflow.name);

        let context = self.prepare(Path::new(&workflow.scratchpad_directory), &source_file)?;

        info!("running probes to determine tasks");

        let tasks_to_run = self.probe_tasks(&workflow.tasks, &context)?;

        if tasks_to_run.is_empty() {
            info!("no probes requested to run");
            return Ok(WorkflowReport::new(workflow.clone()));
        }

        info!("running {} tasks", tasks_to_run.len());

        let task_reports = self.run_tasks(tasks_to_run, &context)?;

        self.complete_run(&context)?;

        Ok(WorkflowReport::new_with_reports(
            workflow.clone(),
            task_reports,
        ))
    }

    /// Probe each task to see if it needs to run for the file
    fn probe_tasks<'a>(
        &self,
        tasks: &'a [Task],
        context: &Context,
    ) -> Result<Vec<&'a Task>, RunnerError> {
        let probing_context =
            ProbingContext::new(&context.input_file, &context.scratchpad_directory);

        let probe_results: Vec<(&Task, ProbeResult)> = tasks
            .iter()
            .map(|task| (task, task.run_probe(probing_context)))
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

        Ok(tasks_to_run)
    }

    fn run_tasks(
        &self,
        tasks: Vec<&Task>,
        context: &Context,
    ) -> Result<Vec<TaskReport>, RunnerError> {
        let mut task_reports: Vec<TaskReport> = Vec::with_capacity(tasks.len());

        for task in tasks.iter() {
            let task_context = TaskContext::new(
                &context.input_file,
                &context.output_file,
                &context.scratchpad_directory,
            );

            // @todo handle task failure properly
            let task_report = task.run_task(task_context);

            if !fs::exists(&context.output_file).unwrap_or(false) {
                continue;
            }
            // move the output file so it becomes the input file of any next task
            let move_result = fs::rename(&context.output_file, &context.input_file)
                .map(|_| true)
                .unwrap_or(false);

            if !move_result {
                warn!(
                "task \"{}\" did not output any file, following task will work on the same source",
                task.description()
            );
            }

            task_reports.push(task_report);
        }

        Ok(task_reports)
    }
}

/// Logic related to the start and cleanup of a run
impl Runner {
    /// Create the area where file transformations can be done
    fn prepare(
        &self,
        scratchpad_directory: &Path,
        source_file_path: &Path,
    ) -> Result<Context, PreparationError> {
        debug!(
            "creating scratchpad directory at {}",
            scratchpad_directory.to_string_lossy()
        );

        fs::create_dir_all(scratchpad_directory)
            .map_err(PreparationError::UnableToCreateScratchpad)?;

        let input_file_name = generate_target_file(source_file_path);
        debug!("generated target file name: {}", input_file_name);

        let input_file = scratchpad_directory.join(PathBuf::from(&input_file_name));

        debug!(
            "copying source file into scratchpad directory at {}",
            input_file.to_string_lossy()
        );

        fs::copy(source_file_path, &input_file)
            .map_err(PreparationError::UnableToCopySourceFile)?;

        let output_file = scratchpad_directory.join(generate_output_file_name(&input_file_name));

        Ok(Context {
            scratchpad_directory: scratchpad_directory.to_owned(),
            source_file_path: source_file_path.to_path_buf(),
            input_file,
            output_file,
        })
    }

    /// Complete a run which will make sure that no artifacts are left behind
    /// and that the transformed file replaces the original source file
    fn complete_run(&self, context: &Context) -> Result<(), CompletionError> {
        debug!("copying transformed file back to source file");
        fs::rename(&context.input_file, &context.source_file_path)
            .map_err(CompletionError::UnableToMoveFile)
    }
}
