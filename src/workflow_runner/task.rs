use crate::{job_orchestration, workflow::Task};

use super::{
    builtin_task,
    common::{self, ProbeRunner, ProbingContext, TaskContext, TaskRunner},
};

impl ProbeRunner for Task {
    fn run_probe(&self, context: ProbingContext) -> common::ProbeResult {
        // delegate the running to the actual task
        match self {
            Task::Custom(custom_task) => custom_task.run_probe(context),
            Task::Builtin(builtin_task) => builtin_task.run_probe(context),
        }
    }
}

impl TaskRunner for Task {
    fn run_task(&self, context: TaskContext) -> job_orchestration::TaskReport {
        match self {
            Task::Custom(custom_task) => custom_task.run_task(context),
            Task::Builtin(builtin_task) => builtin_task.run_task(context),
        }
    }
}
