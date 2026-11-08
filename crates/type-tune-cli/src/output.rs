//! Safe text output helpers for CLI artifacts.

use std::{fs, path::Path};

/// Creates parent directories and writes a text artifact.
pub(crate) fn write_text_file(path: &Path, content: &str) -> Result<(), String> {
    if let Some(parent) = path.parent().filter(|path| !path.as_os_str().is_empty()) {
        fs::create_dir_all(parent)
            .map_err(|error| format!("Could not create {}: {error}", parent.display()))?;
    }
    fs::write(path, content).map_err(|error| format!("Could not write {}: {error}", path.display()))
}

/// Reads a text artifact with a path-specific CLI error.
pub(crate) fn read_text_file(path: &Path) -> Result<String, String> {
    fs::read_to_string(path).map_err(|error| format!("Could not read {}: {error}", path.display()))
}

#[cfg(test)]
mod tests {
    use std::{
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{read_text_file, write_text_file};

    #[test]
    fn creates_parent_directories_before_writing_text() {
        let root = std::env::temp_dir().join(format!(
            "type-tune-output-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let path = root.join("nested/artifact.json");

        write_text_file(&path, "content\n").expect("writes artifact");
        assert_eq!(
            fs::read_to_string(&path).expect("reads artifact"),
            "content\n"
        );

        fs::remove_dir_all(root).expect("removes fixture");
    }

    #[test]
    fn replaces_an_existing_text_artifact() {
        let root = std::env::temp_dir().join(format!(
            "type-tune-output-replace-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("creates fixture");
        let path = root.join("artifact.json");
        fs::write(&path, "old\n").expect("writes existing artifact");

        write_text_file(&path, "new\n").expect("replaces artifact");

        assert_eq!(fs::read_to_string(&path).expect("reads artifact"), "new\n");
        fs::remove_dir_all(root).expect("removes fixture");
    }

    #[test]
    fn reports_the_missing_text_artifact_path() {
        let path = std::env::temp_dir().join(format!(
            "type-tune-missing-artifact-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        let error = read_text_file(&path).expect_err("missing artifact fails");

        assert!(error.starts_with(&format!("Could not read {}:", path.display())));
    }
}
