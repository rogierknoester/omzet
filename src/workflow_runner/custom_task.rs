use std::{
    collections::HashMap,
    io::{BufRead, BufReader},
    path::{Path, PathBuf},
};

use run_script::ScriptOptions;
use tracing::debug;

use crate::{job_orchestration::TaskReport, workflow::CustomTask};

use super::common::{ProbeResult, ProbeRunner, ProbingContext, TaskRunner};

impl ProbeRunner for CustomTask {
    fn run_probe(&self, context: ProbingContext) -> ProbeResult {
        // if no probe was defined the task should always run
        let probe = match &self.probe {
            Some(probe) => probe,
            None => return ProbeResult::Run,
        };

        match run_script(
            probe.as_str(),
            HashMap::from([
                (
                    "OMZET_INPUT".to_owned(),
                    context.path.to_string_lossy().to_string(),
                ),
                ("OMZET_TASK".to_owned(), self.id.to_owned()),
            ]),
            context.directory,
        ) {
            Ok((exit_code, ..)) => match exit_code {
                0 => ProbeResult::Run,
                _ => ProbeResult::Skip,
            },
            Err(_) => ProbeResult::Abort,
        }
    }
}

impl TaskRunner for CustomTask {
    fn run_task(
        &self,
        context: super::common::TaskContext,
    ) -> crate::job_orchestration::TaskReport {
        let env_vars: HashMap<String, String> = HashMap::from([
            (
                "OMZET_INPUT".to_owned(),
                context.input_path.to_string_lossy().to_string(),
            ),
            (
                "OMZET_OUTPUT".to_owned(),
                context.output_path.to_string_lossy().to_string(),
            ),
        ]);

        let result = run_script(&self.command, env_vars, context.directory)
            .expect("failed to run task script"); // @todo use error type

        TaskReport::new(Some(result.0), result.1, result.2)
    }
}

/// Run a script. For example a task's command or probe.
fn run_script(
    script: &str,
    env_vars: HashMap<String, String>,
    working_directory: &Path,
) -> Result<(i32, String, String), String> {
    let mut options = ScriptOptions::new();

    options.exit_on_error = true;
    options.print_commands = true;
    options.env_vars = Some(env_vars);
    options.working_directory = Some(PathBuf::from(working_directory));

    let _args = Vec::new();

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
