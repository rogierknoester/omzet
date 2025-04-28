use std::path::Path;

use crate::job_orchestration::TaskReport;

/// Blocks or allows running of a task
#[derive(Debug, PartialEq)]
pub(super) enum ProbeResult {
    Run,
    Skip,
    Abort,
}

pub(super) trait ProbeRunner {
    fn run_probe(&self, context: ProbingContext) -> ProbeResult;
}

#[derive(Copy, Clone)]
pub(super) struct ProbingContext<'a> {
    pub(super) path: &'a Path,
    pub(super) directory: &'a Path,
}

impl<'a> ProbingContext<'a> {
    pub(super) fn new(path: &'a Path, directory: &'a Path) -> Self {
        Self { path, directory }
    }
}

pub(super) trait TaskRunner {
    fn run_task(&self, context: TaskContext) -> TaskReport;
}

#[derive(Copy, Clone)]
pub(super) struct TaskContext<'a> {
    pub(super) input_path: &'a Path,
    pub(super) output_path: &'a Path,
    pub(super) directory: &'a Path,
}

impl<'a> TaskContext<'a> {
    pub(super) fn new(input_path: &'a Path, output_path: &'a Path, directory: &'a Path) -> Self {
        Self {
            input_path,
            output_path,
            directory,
        }
    }
}
