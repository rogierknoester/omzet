use run_script::ScriptOptions;
use std::{
    fs::{self},
    io::{stdout, Read, Write},
    path::Path,
    process::{Command, Output, Stdio},
    thread,
};
use uuid::Uuid;

use anyhow::Result;
use tracing::{debug, error, info};

use crate::{workflow::Task, Workflow};

pub(crate) struct SourceFile(String);

impl SourceFile {
    pub(crate) fn new(path: String) -> Self {
        Self(path)
    }
}

pub(crate) trait Runner {
    fn run_workflow(&self, workflow: Workflow, source_file: SourceFile) -> Result<WorkflowReport>;
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

    fn register_report(&mut self, report: TaskReport) {
        self.task_reports.push(report);
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
}

impl DefaultRunner {
    pub(crate) fn new() -> Self {
        Self {}
    }

    fn prepare_scratchpad(&self, scratchpad_directory: &str, source_file: &SourceFile) {
        debug!("creating scratchpad directory at {}", scratchpad_directory);

        let scratchpad_path = Path::new(&scratchpad_directory);
        fs::create_dir_all(scratchpad_path).expect("able to create scratchpad directory");

        let target_file_name = generate_target_file(&source_file.0);

        let target_file_path = format!("{}{}", scratchpad_directory, target_file_name);

        debug!(
            "copying source file into scratchpad directory at {}",
            target_file_path
        );
        fs::copy(&source_file.0, target_file_path)
            .expect("able to copy source file into scratchpad");
    }

    /// Runs the probe
    fn run_probe(&self, probe: &str) -> ProbeResult {
        let options = ScriptOptions::new();
        let args = vec![];
        let mut child = run_script::spawn(probe, &args, &options).expect("able to spawn child");

        let mut child_stdout = child.stdout.take().unwrap();

        let thread = thread::spawn(move || {
            let mut buffer = [0; 1024];
            loop {
                let n = child_stdout.read(&mut buffer);
                match n {
                    Ok(n) if n > 0 => {
                        stdout().write_all(&buffer[..n]).unwrap();
                        stdout().flush().unwrap();
                    }
                    Ok(0) => break,
                    Ok(_) => continue,
                    Err(error) => {
                        error!("Error while reading from child process output. {}", error);
                        break;
                    }
                }
            }
        });

        let result = child.wait();
        let _ = thread.join();

        match result {
            Err(_) => ProbeResult::ShouldSkip,
            Ok(exit_code) => match exit_code.code().unwrap() {
                0 => ProbeResult::ShouldRun,
                _ => ProbeResult::ShouldSkip,
            },
        }
    }

    fn run_task(&self, task: &Task) -> TaskReport {
        let stdout = Stdio::piped();
        let stderr = Stdio::piped();
        let child = Command::new("sh")
            .arg("-c")
            .arg(&task.command)
            .stdout(stdout)
            .stderr(stderr)
            .output()
            .expect("unable to run task");

        TaskReport::from(child)
    }
}

impl Runner for DefaultRunner {
    /// Will synchronously run the workflow's tasks
    /// and produce a [`WorkflowReport`]
    fn run_workflow(&self, workflow: Workflow, source_file: SourceFile) -> Result<WorkflowReport> {
        info!("starting workflow: {}", &workflow.name);

        self.prepare_scratchpad(&workflow.scratchpad_directory, &source_file);

        let tasks = workflow.tasks.clone();

        let mut workflow_report = WorkflowReport::new(workflow);

        info!("running probes to determine tasks");
        let tasks_to_run: Vec<Task> = tasks
            .into_iter()
            .filter(|task| match &task.probe {
                Some(probe) => self.run_probe(probe.as_str()) == ProbeResult::ShouldRun,
                None => true,
            })
            .collect();

        if tasks_to_run.is_empty() {
            info!("no probes requested to run");
            return Ok(workflow_report);
        }

        info!("running {} tasks", tasks_to_run.len());

        for task in tasks_to_run.into_iter() {
            info!("running task \"{}\"", &task.name);

            workflow_report.register_report(self.run_task(&task));
            info!("completed task \"{}\"", &task.name);
        }

        Ok(workflow_report)
    }
}

/// Generate a target file from the source file
fn generate_target_file(source_file: &str) -> String {
    let uuid = Uuid::new_v4();

    format!("{}.{}", source_file, uuid)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subject_file_generation() {
        let source_file = "/tmp/test_file.mkv";
        let subject_file = generate_target_file(source_file);

        assert!(subject_file.len() == source_file.len() + 37);
    }
}
