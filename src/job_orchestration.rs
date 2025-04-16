use std::{
    sync::mpsc::{channel, Receiver, Sender},
    thread::sleep,
    time::Duration,
};

use tracing::{error, info};

use crate::{runner::WorkflowRunner, DefaultRunner, SourceFilePath, Workflow};

pub(crate) struct Job {
    /// The absolute path to the file for this job
    file_path: String,

    /// The library to which this job belongs
    library: String,

    /// The workflow that is requested for this job
    workflow: Workflow,
}

impl Job {
    pub(crate) fn new(library: String, file_path: String, workflow: Workflow) -> Self {
        Self {
            library,
            file_path,
            workflow,
        }
    }
}

pub(crate) struct JobOrchestrator {
    job_receiver: Receiver<Box<Job>>,
}

impl JobOrchestrator {
    /// Create a new orchestrator and a sender to be used to communicate with it
    pub(crate) fn new() -> (Self, Sender<Box<Job>>) {
        let (sender, receiver) = create_channel();
        (
            Self {
                job_receiver: receiver,
            },
            sender,
        )
    }

    pub(crate) fn start(&self) {
        let runner = DefaultRunner::new();

        loop {
            if let Ok(job) = self.job_receiver.recv() {
                sleep(Duration::from_secs(5));
                info!(
                    "received job for library {} with file {}",
                    job.library, job.file_path
                );

                let result = runner.run_workflow(&job.workflow, SourceFilePath::new(job.file_path));

                match result {
                    Ok(_) => info!("performed workflow"),
                    Err(err) => error!("unable to perform workflow: {}", err),
                }
            }
        }
    }
}

/// Create a channel that can be used for job dispatching
fn create_channel() -> (Sender<Box<Job>>, Receiver<Box<Job>>) {
    channel::<Box<Job>>()
}
