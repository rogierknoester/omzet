use std::sync::mpsc::{channel, Receiver, Sender};

use tracing::info;

use crate::Workflow;

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
        loop {
            if let Ok(x) = self.job_receiver.recv() {
                info!("received job for library {}", x.library);
            }
        }
    }
}

/// Create a channel that can be used for job dispatching
fn create_channel() -> (Sender<Box<Job>>, Receiver<Box<Job>>) {
    channel::<Box<Job>>()
}
