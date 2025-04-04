use std::{
    fs,
    io::{stdout, Read, Write},
    path::Path,
    process::{Command, Output, Stdio},
    thread,
};

use runner::{DefaultRunner, Runner, SourceFile};
use workflow::Workflow;

mod runner;
mod workflow;

fn main() {
    tracing_subscriber::fmt::init();

    let runner = DefaultRunner::new();

    let mut workflow = Workflow::new("tester".to_owned(), "/tmp/omzet".to_owned());
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

    let source_file = SourceFile::new("/home/rogier/Downloads/bob.mkv".to_string());

    // @todo generate "scratchpad" area
    runner.run_workflow(workflow, source_file);
}
