use std::{fs, path::PathBuf};

use dirs::data_dir;
use rusqlite::Connection;
use rusqlite_migration::{Migrations, M};

const DB_FILE_NAME: &str = "state.db";

/// Will create a connection to the local DB.
pub(crate) fn get_connection() -> Connection {
    let db_file = get_state_directory().join(DB_FILE_NAME);
    let mut connection = Connection::open(db_file.to_string_lossy().to_string()).unwrap();

    let migrations = get_migrations();

    migrations
        .to_latest(&mut connection)
        .expect("unable to run migrations");

    connection
}

/// Get the directory that stores the sqlite DB file
/// Ensures that the directory exists if it does not yet exist.
fn get_state_directory() -> PathBuf {
    let directory = data_dir().map(|path| path.join("omzet")).unwrap();

    fs::create_dir_all(&directory).unwrap();

    directory
}

fn get_migrations<'m>() -> Migrations<'m> {
    Migrations::new(vec![M::up(
        r#"
        CREATE TABLE job_report (
            id INTEGER PRIMARY KEY,
            source_file_path TEXT,
            output_file_fingerprint TEXT
        )
        "#,
    )])
}
