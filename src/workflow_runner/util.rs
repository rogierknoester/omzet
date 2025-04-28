use std::path::Path;

use uuid::Uuid;

/// Generate a target file from the source file
pub(super) fn generate_target_file(source_file_path: &Path) -> String {
    let uuid = Uuid::new_v4();

    let file_name = source_file_path
        .file_stem()
        .expect("failed to take file name");
    let extension = source_file_path
        .extension()
        .expect("failed to take file extension");

    format!(
        "{}-{}.{}",
        file_name.to_string_lossy(),
        uuid,
        extension.to_string_lossy()
    )
}

pub(super) fn generate_output_file_name(target_file_name: &str) -> String {
    let path = Path::new(target_file_name);

    let file_name = path.file_stem().expect("failed to take file name");
    let extension = path.extension().expect("failed to take file extension");

    format!(
        "{}.out.{}",
        file_name.to_string_lossy(),
        extension.to_string_lossy()
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn test_subject_file_generation() {
        let source_file = "/tmp/test_file.mkv";
        let path = Path::new(source_file);
        let subject_file = generate_target_file(path);

        assert!(subject_file.len() == "test_file.mkv".len() + 37);
    }
}
