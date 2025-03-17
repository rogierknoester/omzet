use std::{
    io::{stdout, Read, Write},
    process::{Command, Output, Stdio},
    thread,
};

use run_script::ScriptOptions;
use tracing::{debug, error, info};

fn main() {
    tracing_subscriber::fmt::init();

    let runner = Runner::new();

    let mut workflow = Workflow::new("tester".to_owned());
    workflow.register_task(
        "Copier".to_owned(),
        Some(
            r#"
                echo "Probing copy"
                sleep 2
                echo "awake"
                sleep 2
                exit 1
            "#
            .to_owned(),
        ),
        "cp /home/rogier/Downloads/bob.mkv /tmp/test/bob.mkv".to_owned(),
    );
    workflow.register_task(
        "264 Encoder".to_owned(),
        Some(r"exit 1".to_string()),
        "ffmpeg -i /tmp/test/bob.mkv -c:v libx264 /tmp/test/bob_new.mkv".to_owned(),
    );

    // @todo generate "scratchpad" area
    runner.run_workflow(workflow);
}

#[derive(Debug, Clone)]
struct Task {
    /// The name of a task
    name: String,
    /// The probe is a CLI command to check if the command should be executed
    probe: Option<Runnable>,
    /// The command is a CLI command to actually perform the task
    command: Runnable,
}

type Runnable = String;

impl Task {
    fn new(name: String, probe: Option<Runnable>, command: Runnable) -> Self {
        Self {
            name,
            probe,
            command,
        }
    }
}

/// A workflow defines which things need to happen when a new file is detected
/// @todo add some "context" in which variables can be stored
#[derive(Debug)]
struct Workflow {
    name: String,
    tasks: Vec<Task>,
}

impl Workflow {
    fn new(name: String) -> Self {
        Self {
            name,
            tasks: Vec::new(),
        }
    }

    fn register_task(&mut self, name: String, probe: Option<String>, command: String) {
        self.tasks.push(Task::new(name, probe, command));
    }
}

/// Contains some basic information to perform the tasks.
/// Such as the source file and the "current file" on which should be acted
struct WorkflowContext {
    source_file: String,
    working_directory: String,
    subject_file: String,
}

/// The Runner orchestrates the execution of a workflow
#[derive(Debug)]
struct Runner {}

#[derive(Debug, PartialEq)]
enum ProbeResult {
    ShouldRun,
    ShouldSkip,
}

impl Runner {
    fn new() -> Self {
        Self {}
    }

    /// Will synchronously run the workflow's tasks
    /// and produce a [`WorkflowReport`]
    fn run_workflow(&self, workflow: Workflow) {
        info!("Starting workflow: {}", &workflow.name);

        let tasks = workflow.tasks.clone();

        let mut workflow_report = WorkflowReport::new(workflow);

        info!("Running probes to determine tasks");
        let tasks_to_run = tasks
            .into_iter()
            .filter(|task| match &task.probe {
                Some(probe) => self.run_probe(probe.as_str()) == ProbeResult::ShouldRun,
                None => true,
            })
            .collect::<Vec<Task>>();

        if tasks_to_run.is_empty() {
            info!("No probes requested to run");
            return;
        }

        info!("Running {} tasks", tasks_to_run.len());

        for task in tasks_to_run.into_iter() {
            let task_name = task.name.clone();
            info!("Running task \"{}\"", task_name);

            workflow_report.register_report(self.run_task(task));
            info!("Completed task \"{}\"", task_name);
        }

        //info!("{:?}", workflow_report)
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

    #[tracing::instrument]
    fn run_task(&self, task: Task) -> TaskReport {
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

#[derive(Debug)]
struct WorkflowReport {
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
