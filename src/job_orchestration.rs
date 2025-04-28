//! Job Orchestration exists to have one single module responsible for receiving things that might
//! need to be queued and actually queueing and starting them.
//!

use crate::workflow_runner::{Runner, RunnerError};
use std::{
    collections::VecDeque,
    ops::Deref,
    path::PathBuf,
    process::Output,
    sync::mpsc::{channel, Receiver, Sender},
    thread::{self, sleep, JoinHandle},
    time::Duration,
};

use rusqlite::Connection;
use tracing::{debug, warn};

use crate::{db, Workflow};

#[derive(PartialEq, Eq, Debug)]
pub(crate) struct JobRequest {
    /// The absolute path to the file for this job
    file_path: PathBuf,

    /// The library to which this job belongs
    library: String,

    /// The workflow that is requested for this job
    workflow: Workflow,
}

impl JobRequest {
    /// Create a new job request that can be passed to a [`JobOrchestrator`]
    pub(crate) fn new(library: String, file_path: PathBuf, workflow: Workflow) -> Self {
        Self {
            library,
            file_path,
            workflow,
        }
    }
}

/// A Runnable Job is created once a [`JobRequest`] is determined to be valid and needed
#[derive(PartialEq, Eq, Debug)]
struct RunnableJob(JobRequest);

#[derive(Debug)]
#[allow(dead_code)]
struct RunningJob(JobRequest);

impl Deref for RunnableJob {
    type Target = JobRequest;

    fn deref(&self) -> &Self::Target {
        &self.0
    }
}

/// A job outputs a report that contains information about the tasks that were executed
/// and the logs of those processes, per task.
#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct WorkflowReport {
    workflow: Workflow,
    task_reports: Vec<TaskReport>,
}

impl WorkflowReport {
    pub(crate) fn new(workflow: Workflow) -> Self {
        Self {
            workflow,
            task_reports: vec![],
        }
    }

    pub(crate) fn new_with_reports(workflow: Workflow, task_reports: Vec<TaskReport>) -> Self {
        Self {
            workflow,
            task_reports,
        }
    }
}

/// Contains information about the execution of a single task. Its full output to stderr and stdout is collected.
#[derive(Debug)]
#[allow(dead_code)]
pub(crate) struct TaskReport {
    exit_code: Option<i32>,
    stdout: String,
    stderr: String,
}

impl TaskReport {
    pub(crate) fn new(exit_code: Option<i32>, stdout: String, stderr: String) -> Self {
        TaskReport {
            exit_code,
            stdout,
            stderr,
        }
    }
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

pub(crate) struct JobOrchestrator {
    job_receiver: Receiver<Box<JobRequest>>,
    connection: Connection,
    queue: VecDeque<RunnableJob>,
    current_running_job: Option<(RunningJob, JoinHandle<Result<WorkflowReport, RunnerError>>)>,
}

impl JobOrchestrator {
    /// Create a new orchestrator and a sender to be used to communicate with it
    pub(crate) fn new() -> (Self, Sender<Box<JobRequest>>) {
        let (sender, receiver) = channel::<Box<JobRequest>>();
        (
            Self {
                job_receiver: receiver,
                connection: db::get_connection(),
                queue: VecDeque::new(),
                current_running_job: None,
            },
            sender,
        )
    }

    pub(crate) fn start(&mut self) {
        loop {
            debug!("tick tock");
            self.handle_incoming_job_requests();
            self.handle_runner();

            sleep(Duration::from_secs(5));
        }
    }

    /// Check if any job requests have been sent, if so, enqueue them
    fn handle_incoming_job_requests(&mut self) {
        // handle items that have been dispatched, queue them up

        for incoming_job in self.job_receiver.try_iter() {
            let queueable = RunnableJob(*incoming_job);

            if self.queue.contains(&queueable) {
                continue;
            }

            // @todo check file fingerprint to see if it was already done by us

            debug!("enqueueing new item {queueable:?}");
            self.queue.push_back(queueable);
        }
    }

    /// Handle the runner.
    /// Starts a new job if nothing is running and jobs are queued.
    /// If something is running, check the status and finish it when it has completed
    fn handle_runner(&mut self) {
        // nothing is running
        if self.current_running_job.is_none() {
            self.start_job();
            return;
        }

        // something is running but not finished yet
        if let Some((_, handle)) = &self.current_running_job {
            if !handle.is_finished() {
                return;
            }
        }

        // something has finished
        // we know that the job has finished so we can take ownership of the job and handle
        let (running_job, handle) = self.current_running_job.take().unwrap();

        let result = handle.join();

        debug!("job  finished",);
        debug!("job: {running_job:?}");
        debug!("result: {result:?}");
    }

    /// Start a new job based on the first requested job in the queue
    fn start_job(&mut self) {
        if self.current_running_job.is_some() {
            warn!("trying to start job but one is already running");
            return;
        }

        let job_request = match self.queue.pop_front() {
            Some(job_request) => job_request,
            None => {
                debug!("nothing in queue; cannot start a new job");
                return;
            }
        };

        debug!(
            "starting job for file {}",
            job_request.file_path.to_string_lossy()
        );

        let workflow = job_request.workflow.clone();
        let file_path = job_request.file_path.clone();

        let handle = thread::Builder::new()
            .name(String::from("runner"))
            .spawn(move || {
                let runner = Runner::new();
                runner.run_workflow(&workflow, PathBuf::from(file_path))
            })
            .expect("unable to start worker");

        self.current_running_job = Some((RunningJob(job_request.0), handle));
    }
}
