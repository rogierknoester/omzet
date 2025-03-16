use std::process::{Command, Output, Stdio};

fn main() {
    let runner = Runner::new();

    let mut workflow = Workflow::new("tester".to_owned());
    workflow.register_task("Echo-er".to_owned(), None, "echo 123".to_owned());
    workflow.register_task("Echo-er".to_owned(), None, "echo 567".to_owned());

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

/// The Runner orchestrates the execution of a workflow
struct Runner {}

impl Runner {
    fn new() -> Self {
        Self {}
    }

    /// Will synchronously run the workflow's tasks
    /// and produce a [`WorkflowReport`]
    fn run_workflow(&self, workflow: Workflow) {
        let tasks = workflow.tasks.clone();

        let mut workflow_report = WorkflowReport::new(workflow);

        for task in tasks.iter() {
            println!("Running task \"{}\"", task.name);

            let stdout = Stdio::piped();
            let stderr = Stdio::piped();
            let child = Command::new("sh")
                .arg("-c")
                .arg(&task.command)
                .stdout(stdout)
                .stderr(stderr)
                .spawn()
                .expect("unable to run task");

            workflow_report.register_report(TaskReport::from(child));
        }

        println!("{:?}", workflow_report)
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
