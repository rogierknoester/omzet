use std::io::{self};

use runner::{DefaultRunner, Runner, SourceFilePath};
use tracing::{error, info, level_filters::LevelFilter};
use tracing_subscriber::EnvFilter;
use workflow::Workflow;

mod runner;
mod workflow;

fn main() {
    tracing_subscriber::fmt()
        .with_writer(io::stderr)
        .with_env_filter(
            EnvFilter::builder()
                .with_default_directive(LevelFilter::INFO.into())
                .from_env_lossy(),
        )
        .init();

    let runner = DefaultRunner::new();

    let mut workflow = Workflow::new("tester".to_owned(), "/tmp/omzet".to_owned());
    workflow.register_task(
        "Copier".to_owned(),
        Some(
            r#"
                echo "Probing copy $OMZET_INPUT"
                sleep 2
                exit 0
            "#
            .to_owned(),
        ),
        r#"

        echo "performing task 1"
        echo "input: $OMZET_INPUT"
        echo "output: $OMZET_OUTPUT"

        ffmpeg -i $OMZET_INPUT -c:v libx265 -c:a copy -t 5 $OMZET_OUTPUT
        "#
        .to_owned(),
    );
    workflow.register_task(
        "264 Encoder".to_owned(),
        Some(r"exit 1".to_string()),
        "ffmpeg -i /tmp/test/bob.mkv -c:v libx264 /tmp/test/bob_new.mkv".to_owned(),
    );

    let source_file_path = SourceFilePath::new("/home/rogier/Downloads/bob.mkv".to_string());

    let result = runner.run_workflow(workflow, source_file_path);

    match result {
        Ok(_) => info!("performed workflow"),
        Err(err) => error!("unable to perform workflow: {}", err),
    }
}
