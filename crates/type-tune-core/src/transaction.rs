//! Recoverable file transactions for native rewrites.

use std::{
    collections::BTreeMap,
    fs,
    io::{ErrorKind, Write},
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};

use crate::{Result, TypeTuneError, path::normalize_project_path};

/// A staged set of project-relative file changes that can be committed or rolled back.
pub struct FileTransaction {
    project_root: PathBuf,
    staged: Vec<StagedChange>,
    committed_count: usize,
    created_directories: Vec<PathBuf>,
    commit_completed: bool,
    rollback_attempted: bool,
}

/// Stages project-relative rewritten files for a later commit.
///
/// # Errors
///
/// Returns an error when paths are unsafe or files cannot be staged.
pub fn stage_file_transaction(
    project_root: &Path,
    changes: &BTreeMap<String, String>,
) -> Result<FileTransaction> {
    let project_root = fs::canonicalize(project_root).map_err(|error| {
        TypeTuneError::new(format!(
            "Could not resolve project root {}: {error}",
            project_root.display()
        ))
    })?;
    let mut transaction = FileTransaction {
        project_root,
        staged: Vec::new(),
        committed_count: 0,
        created_directories: Vec::new(),
        commit_completed: false,
        rollback_attempted: false,
    };
    for (project_path, content) in changes {
        match stage_change(
            &transaction.project_root,
            project_path,
            content,
            &mut transaction.created_directories,
        ) {
            Ok(change) => transaction.staged.push(change),
            Err(error) => {
                let cleanup_error = transaction.rollback().err();
                return Err(TypeTuneError::new(format!(
                    "{error}{}",
                    cleanup_error.map_or_else(String::new, |error| format!(
                        " Staging cleanup failed: {error}"
                    ))
                )));
            }
        }
    }
    Ok(transaction)
}

impl FileTransaction {
    /// Atomically replaces every staged file, rolling back if any rename fails.
    ///
    /// # Errors
    ///
    /// Returns an error when a staged file cannot be committed.
    pub fn commit(&mut self) -> Result<()> {
        for index in self.committed_count..self.staged.len() {
            let (target_path, result) = {
                let change = &self.staged[index];
                (
                    change.target_path.display().to_string(),
                    ensure_original_parent(&self.project_root, &change.target_path).and_then(
                        |()| {
                            fs::rename(&change.temporary_path, &change.target_path)
                                .map_err(|error| TypeTuneError::new(error.to_string()))
                        },
                    ),
                )
            };
            if let Err(error) = result {
                let rollback_error = self.rollback().err();
                return Err(TypeTuneError::new(format!(
                    "Could not commit {}: {error}{}",
                    target_path,
                    rollback_error
                        .map_or_else(String::new, |error| format!(" Rollback failed: {error}"))
                )));
            }
            self.committed_count += 1;
        }
        self.commit_completed = true;
        Ok(())
    }

    /// Restores every committed file to its state before this transaction was staged.
    ///
    /// # Errors
    ///
    /// Returns an error when an original file cannot be restored.
    pub fn rollback(&mut self) -> Result<()> {
        self.rollback_attempted = true;
        while self.committed_count > 0 {
            let change = &self.staged[self.committed_count - 1];
            ensure_original_parent(&self.project_root, &change.target_path)?;
            match &change.original {
                Some(original) => restore_original(&change.target_path, original)?,
                None => fs::remove_file(&change.target_path).map_err(|error| {
                    TypeTuneError::new(format!(
                        "Could not remove created {}: {error}",
                        change.target_path.display()
                    ))
                })?,
            }
            self.committed_count -= 1;
        }
        cleanup_temporary_files(&self.project_root, &self.staged)?;
        cleanup_created_directories(&self.project_root, &mut self.created_directories)?;
        Ok(())
    }
}

impl Drop for FileTransaction {
    fn drop(&mut self) {
        if cleanup_temporary_files(&self.project_root, &self.staged).is_ok()
            && !self.commit_completed
            && !self.rollback_attempted
            && self.committed_count == 0
        {
            let _ = cleanup_created_directories(&self.project_root, &mut self.created_directories);
        }
    }
}

/// Writes a project-relative set of rewritten files in one commit.
///
/// # Errors
///
/// Returns an error when paths are unsafe, files cannot be staged, or a commit cannot complete.
pub fn write_file_transaction(
    project_root: &Path,
    changes: &BTreeMap<String, String>,
) -> Result<()> {
    let mut transaction = stage_file_transaction(project_root, changes)?;
    transaction.commit()
}

struct StagedChange {
    target_path: PathBuf,
    temporary_path: PathBuf,
    original: Option<OriginalFile>,
}

struct OriginalFile {
    content: Vec<u8>,
    permissions: fs::Permissions,
}

fn create_parent_directories(
    project_root: &Path,
    parent: &Path,
    created_directories: &mut Vec<PathBuf>,
) -> Result<PathBuf> {
    let relative = parent.strip_prefix(project_root).map_err(|_| {
        TypeTuneError::new(format!(
            "Directory {} is outside the project root.",
            parent.display()
        ))
    })?;
    let mut current = project_root.to_path_buf();
    for component in relative.components() {
        current.push(component);
        match fs::create_dir(&current) {
            Ok(()) => created_directories.push(current.clone()),
            Err(error) if error.kind() == ErrorKind::AlreadyExists => {}
            Err(error) => {
                return Err(TypeTuneError::new(format!(
                    "Could not create {}: {error}",
                    current.display()
                )));
            }
        }
        current = fs::canonicalize(&current).map_err(|error| {
            TypeTuneError::new(format!(
                "Could not resolve directory {}: {error}",
                current.display()
            ))
        })?;
        if !current.starts_with(project_root) {
            return Err(TypeTuneError::new(format!(
                "Directory {} escapes the project root.",
                current.display()
            )));
        }
    }
    Ok(current)
}

fn cleanup_created_directories(project_root: &Path, created: &mut Vec<PathBuf>) -> Result<()> {
    while let Some(directory) = created.last() {
        // Do not follow a parent or final symlink that was substituted after staging.
        match fs::canonicalize(directory) {
            Ok(resolved) if resolved == *directory && resolved.starts_with(project_root) => {}
            Ok(_) => {
                return Err(TypeTuneError::new(format!(
                    "Could not safely clean created directory {}: path changed or escapes the project root.",
                    directory.display()
                )));
            }
            Err(error) if error.kind() == ErrorKind::NotFound => {
                created.pop();
                continue;
            }
            Err(error) => {
                return Err(TypeTuneError::new(format!(
                    "Could not inspect created directory {}: {error}",
                    directory.display()
                )));
            }
        }
        match fs::remove_dir(directory) {
            Ok(()) => {}
            Err(error)
                if matches!(
                    error.kind(),
                    ErrorKind::NotFound | ErrorKind::DirectoryNotEmpty
                ) => {}
            Err(error) => {
                return Err(TypeTuneError::new(format!(
                    "Could not remove created directory {}: {error}",
                    directory.display()
                )));
            }
        }
        created.pop();
    }
    Ok(())
}

fn stage_change(
    project_root: &Path,
    project_path: &str,
    content: &str,
    created_directories: &mut Vec<PathBuf>,
) -> Result<StagedChange> {
    let normalized = normalize_project_path(project_path)?;
    if normalized != project_path {
        return Err(TypeTuneError::new(format!(
            "Expected canonical project-relative path: {project_path}."
        )));
    }
    let project_target = project_root.join(project_path);
    let parent = project_target.parent().ok_or_else(|| {
        TypeTuneError::new(format!(
            "Could not determine parent for {}.",
            project_target.display()
        ))
    })?;
    let parent = create_parent_directories(project_root, parent, created_directories)?;
    let target_path = parent.join(
        project_target
            .file_name()
            .expect("normalized project path has a file name"),
    );
    let original = match fs::read(&target_path) {
        Ok(content) => {
            let permissions = fs::metadata(&target_path)
                .map_err(|error| {
                    TypeTuneError::new(format!(
                        "Could not read metadata for {}: {error}",
                        target_path.display()
                    ))
                })?
                .permissions();
            Some(OriginalFile {
                content,
                permissions,
            })
        }
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => None,
        Err(error) => {
            return Err(TypeTuneError::new(format!(
                "Could not read {}: {error}",
                target_path.display()
            )));
        }
    };
    let temporary_path = stage_temporary_file(&target_path, content.as_bytes())?;
    if let Some(original) = &original
        && let Err(error) = fs::set_permissions(&temporary_path, original.permissions.clone())
    {
        let cleanup = remove_failed_temporary_file(&temporary_path);
        return Err(TypeTuneError::new(format!(
            "Could not preserve permissions for {}: {error}{cleanup}",
            target_path.display(),
        )));
    }
    Ok(StagedChange {
        target_path,
        temporary_path,
        original,
    })
}

fn restore_original(target_path: &Path, original: &OriginalFile) -> Result<()> {
    let temporary_path = stage_temporary_file(target_path, &original.content)?;
    let result = fs::set_permissions(&temporary_path, original.permissions.clone())
        .and_then(|()| fs::rename(&temporary_path, target_path));
    if let Err(error) = result {
        let _ = fs::remove_file(&temporary_path);
        return Err(TypeTuneError::new(format!(
            "Could not restore {}: {error}",
            target_path.display()
        )));
    }
    Ok(())
}

fn stage_temporary_file(target_path: &Path, content: &[u8]) -> Result<PathBuf> {
    let nonce = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .map_err(|error| TypeTuneError::new(format!("Could not read system clock: {error}")))?
        .as_nanos();
    stage_temporary_file_with_nonce(target_path, content, nonce)
}

fn stage_temporary_file_with_nonce(
    target_path: &Path,
    content: &[u8],
    nonce: u128,
) -> Result<PathBuf> {
    for attempt in 0..16 {
        let temporary_path = temporary_path(target_path, nonce, attempt)?;
        let file = match fs::OpenOptions::new()
            .create_new(true)
            .write(true)
            .open(&temporary_path)
        {
            Ok(file) => file,
            Err(error) if error.kind() == std::io::ErrorKind::AlreadyExists => continue,
            Err(error) => {
                return Err(TypeTuneError::new(format!(
                    "Could not stage {}: {error}",
                    temporary_path.display()
                )));
            }
        };
        let mut file = file;
        if let Err(error) = file.write_all(content) {
            drop(file);
            let cleanup = remove_failed_temporary_file(&temporary_path);
            return Err(TypeTuneError::new(format!(
                "Could not stage {}: {error}{cleanup}",
                temporary_path.display()
            )));
        }
        return Ok(temporary_path);
    }
    Err(TypeTuneError::new(format!(
        "Could not allocate a temporary rewrite file for {}.",
        target_path.display()
    )))
}

fn remove_failed_temporary_file(path: &Path) -> String {
    match fs::remove_file(path) {
        Ok(()) => String::new(),
        Err(error) if error.kind() == ErrorKind::NotFound => String::new(),
        Err(error) => format!(
            " Could not remove temporary file {}: {error}",
            path.display()
        ),
    }
}

fn temporary_path(target_path: &Path, nonce: u128, attempt: usize) -> Result<PathBuf> {
    let name = target_path
        .file_name()
        .and_then(|name| name.to_str())
        .ok_or_else(|| {
            TypeTuneError::new(format!(
                "Could not create temporary path for {}.",
                target_path.display()
            ))
        })?;
    Ok(target_path.with_file_name(format!(".{name}.type-tune-{nonce}-{attempt}")))
}

fn ensure_original_parent(project_root: &Path, path: &Path) -> Result<()> {
    let parent = path.parent().ok_or_else(|| {
        TypeTuneError::new(format!(
            "Could not determine parent for {}.",
            path.display()
        ))
    })?;
    let resolved = fs::canonicalize(parent).map_err(|error| {
        TypeTuneError::new(format!(
            "Could not inspect parent {}: {error}",
            parent.display()
        ))
    })?;
    if resolved != parent || !resolved.starts_with(project_root) {
        return Err(TypeTuneError::new(format!(
            "Could not safely access {}: parent changed or escapes the project root.",
            path.display()
        )));
    }
    Ok(())
}

fn cleanup_temporary_files(project_root: &Path, staged: &[StagedChange]) -> Result<()> {
    for change in staged {
        match ensure_original_parent(project_root, &change.temporary_path) {
            Ok(()) => {}
            Err(_)
                if change.temporary_path.parent().is_some_and(|parent| {
                    fs::symlink_metadata(parent)
                        .is_err_and(|error| error.kind() == ErrorKind::NotFound)
                }) =>
            {
                continue;
            }
            Err(error) => return Err(error),
        }
        match fs::remove_file(&change.temporary_path) {
            Ok(()) => {}
            Err(error) if error.kind() == ErrorKind::NotFound => {}
            Err(error) => {
                return Err(TypeTuneError::new(format!(
                    "Could not remove staged file {}: {error}",
                    change.temporary_path.display()
                )));
            }
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::{
        collections::BTreeMap,
        fs,
        time::{SystemTime, UNIX_EPOCH},
    };

    use super::{
        cleanup_created_directories, stage_file_transaction, stage_temporary_file_with_nonce,
        write_file_transaction,
    };

    #[test]
    fn writes_existing_and_new_project_files() {
        let root = std::env::temp_dir().join(format!(
            "type-tune-transaction-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(root.join("src")).expect("creates fixture");
        fs::write(root.join("src/source.ts"), "before").expect("writes fixture");
        write_file_transaction(
            &root,
            &BTreeMap::from([
                ("src/source.ts".into(), "after".into()),
                (
                    "src/types/shared.ts".into(),
                    "export type Shared = {};\n".into(),
                ),
            ]),
        )
        .expect("writes transaction");
        assert_eq!(
            fs::read_to_string(root.join("src/source.ts")).expect("reads"),
            "after"
        );
        assert_eq!(
            fs::read_to_string(root.join("src/types/shared.ts")).expect("reads"),
            "export type Shared = {};\n"
        );
        fs::remove_dir_all(root).expect("removes fixture");
    }

    #[test]
    fn rejects_unsafe_project_paths() {
        let root = std::env::temp_dir();
        assert!(
            write_file_transaction(
                &root,
                &BTreeMap::from([("../outside.ts".into(), "unsafe".into())]),
            )
            .is_err()
        );
    }

    #[test]
    fn rollback_removes_only_new_empty_parent_directories() {
        let root = tempfile::tempdir().expect("creates fixture");
        fs::create_dir(root.path().join("existing")).expect("creates existing directory");
        fs::write(root.path().join("existing/keep.txt"), "untouched").expect("writes fixture");
        let mut transaction = stage_file_transaction(
            root.path(),
            &BTreeMap::from([("existing/new/types/shared.ts".into(), "created".into())]),
        )
        .expect("stages");
        transaction.commit().expect("commits");
        transaction.rollback().expect("rolls back");
        assert!(!root.path().join("existing/new").exists());
        assert_eq!(
            fs::read_to_string(root.path().join("existing/keep.txt")).unwrap(),
            "untouched"
        );
        transaction.rollback().expect("repeated rollback");
    }

    #[test]
    fn rollback_keeps_concurrent_files_and_their_directories() {
        let root = tempfile::tempdir().expect("creates fixture");
        let mut transaction = stage_file_transaction(
            root.path(),
            &BTreeMap::from([("new/types/created.ts".into(), "created".into())]),
        )
        .expect("stages");
        transaction.commit().expect("commits");
        fs::write(root.path().join("new/types/other.txt"), "keep").expect("writes other file");
        transaction.rollback().expect("rolls back");
        assert_eq!(
            fs::read_to_string(root.path().join("new/types/other.txt")).unwrap(),
            "keep"
        );
        assert!(!root.path().join("new/types/created.ts").exists());
        transaction.rollback().expect("repeated rollback");
        assert!(root.path().join("new/types").is_dir());
    }

    #[test]
    fn commit_keeps_directories_and_dropping_staged_transaction_cleans_them() {
        let root = tempfile::tempdir().expect("creates fixture");
        {
            let mut transaction = stage_file_transaction(
                root.path(),
                &BTreeMap::from([("committed/types/file.ts".into(), "keep".into())]),
            )
            .expect("stages");
            transaction.commit().expect("commits");
        }
        assert_eq!(
            fs::read_to_string(root.path().join("committed/types/file.ts")).unwrap(),
            "keep"
        );
        {
            let _transaction = stage_file_transaction(
                root.path(),
                &BTreeMap::from([("staged/types/file.ts".into(), "temporary".into())]),
            )
            .expect("stages");
        }
        assert!(!root.path().join("staged").exists());
    }

    #[test]
    fn explicit_rollback_before_commit_cleans_staged_directories() {
        let root = tempfile::tempdir().expect("creates fixture");
        let mut transaction = stage_file_transaction(
            root.path(),
            &BTreeMap::from([("new/types/file.ts".into(), "staged".into())]),
        )
        .expect("stages");
        transaction.rollback().expect("rolls back staging");
        assert!(!root.path().join("new").exists());
        transaction.rollback().expect("repeated rollback");
    }

    #[test]
    fn staging_failure_cleans_parents_created_before_a_change_is_staged() {
        let root = tempfile::tempdir().expect("creates fixture");
        let invalid_name = format!("new/types/{}.ts", "a".repeat(512));
        let result = stage_file_transaction(
            root.path(),
            &BTreeMap::from([
                ("first/staged.ts".into(), "staged".into()),
                (invalid_name, "x".into()),
            ]),
        );
        assert!(result.is_err());
        assert!(!root.path().join("first").exists());
        assert!(!root.path().join("new").exists());
    }

    #[test]
    fn cleanup_error_is_reported_and_can_be_retried() {
        let root = tempfile::tempdir().expect("creates fixture");
        let project_root = fs::canonicalize(root.path()).unwrap();
        let directory = project_root.join("new");
        fs::create_dir(&directory).unwrap();
        let mut created = vec![directory.clone()];
        fs::remove_dir(&directory).unwrap();
        fs::write(&directory, "other data").unwrap();
        let error = cleanup_created_directories(&project_root, &mut created).unwrap_err();
        assert!(
            error
                .to_string()
                .contains("Could not remove created directory")
        );
        assert_eq!(fs::read_to_string(&directory).unwrap(), "other data");
        assert_eq!(created.len(), 1);
        fs::remove_file(&directory).unwrap();
        fs::create_dir(&directory).unwrap();
        cleanup_created_directories(&project_root, &mut created).expect("retries");
        assert!(!directory.exists());
    }

    #[test]
    fn partial_commit_failure_rolls_back_and_removes_created_directories() {
        let root = tempfile::tempdir().expect("creates fixture");
        fs::write(root.path().join("keep.txt"), "untouched").unwrap();
        let mut transaction = stage_file_transaction(
            root.path(),
            &BTreeMap::from([
                ("a/types/first.ts".into(), "first".into()),
                ("z/types/second.ts".into(), "second".into()),
            ]),
        )
        .expect("stages");
        fs::remove_file(&transaction.staged[1].temporary_path).unwrap();
        assert!(transaction.commit().is_err());
        assert!(!root.path().join("a").exists());
        assert!(!root.path().join("z").exists());
        assert_eq!(
            fs::read_to_string(root.path().join("keep.txt")).unwrap(),
            "untouched"
        );
        transaction.rollback().expect("repeated rollback");
    }

    #[test]
    fn retry_after_partial_rollback_preserves_other_files_and_cleans_directories() {
        let root = tempfile::tempdir().expect("creates fixture");
        let original = root.path().join("a.ts");
        fs::write(&original, "before").unwrap();
        let mut transaction = stage_file_transaction(
            root.path(),
            &BTreeMap::from([
                ("a.ts".into(), "after".into()),
                ("new/types/file.ts".into(), "created".into()),
            ]),
        )
        .expect("stages");
        transaction.commit().expect("commits");
        fs::write(root.path().join("new/other.txt"), "keep").unwrap();
        fs::remove_file(&original).unwrap();
        fs::create_dir(&original).unwrap();
        assert!(transaction.rollback().is_err());
        assert!(root.path().join("new/types").is_dir());
        fs::remove_dir(&original).unwrap();
        transaction.rollback().expect("retry");
        assert_eq!(fs::read_to_string(original).unwrap(), "before");
        assert!(!root.path().join("new/types").exists());
        assert_eq!(
            fs::read_to_string(root.path().join("new/other.txt")).unwrap(),
            "keep"
        );
        transaction.rollback().expect("repeated rollback");
    }

    #[test]
    fn dropping_a_failed_rollback_does_not_remove_its_directories() {
        let root = tempfile::tempdir().expect("creates fixture");
        let original = root.path().join("a.ts");
        fs::write(&original, "before").unwrap();
        {
            let mut transaction = stage_file_transaction(
                root.path(),
                &BTreeMap::from([
                    ("a.ts".into(), "after".into()),
                    ("new/types/file.ts".into(), "created".into()),
                ]),
            )
            .expect("stages");
            transaction.commit().expect("commits");
            fs::remove_file(&original).unwrap();
            fs::create_dir(&original).unwrap();
            assert!(transaction.rollback().is_err());
        }
        assert!(root.path().join("new/types").is_dir());
        assert!(original.is_dir());
    }

    #[cfg(unix)]
    #[test]
    fn rollback_refuses_a_substituted_symlink_to_outside_the_root() {
        use std::os::unix::fs::symlink;
        let root = tempfile::tempdir().expect("creates fixture");
        let outside = tempfile::tempdir().expect("creates outside fixture");
        let mut transaction = stage_file_transaction(
            root.path(),
            &BTreeMap::from([("new/types/file.ts".into(), "created".into())]),
        )
        .expect("stages");
        transaction.commit().expect("commits");
        fs::write(outside.path().join("file.ts"), "keep").unwrap();
        fs::rename(root.path().join("new/types"), root.path().join("new/moved")).unwrap();
        symlink(outside.path(), root.path().join("new/types")).unwrap();
        assert!(transaction.rollback().is_err());
        assert_eq!(
            fs::read_to_string(outside.path().join("file.ts")).unwrap(),
            "keep"
        );
        fs::remove_file(root.path().join("new/types")).unwrap();
        fs::rename(root.path().join("new/moved"), root.path().join("new/types")).unwrap();
        transaction
            .rollback()
            .expect("retry after restoring directory");
        assert!(!root.path().join("new").exists());
    }

    #[test]
    fn restores_committed_changes_on_rollback() {
        let root = std::env::temp_dir().join(format!(
            "type-tune-transaction-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(root.join("src")).expect("creates fixture");
        fs::write(root.join("src/source.ts"), "before").expect("writes fixture");
        let mut transaction = stage_file_transaction(
            &root,
            &BTreeMap::from([
                ("src/source.ts".into(), "after".into()),
                ("src/types/shared.ts".into(), "created".into()),
            ]),
        )
        .expect("stages transaction");
        transaction.commit().expect("commits transaction");
        transaction.rollback().expect("rolls back transaction");
        assert_eq!(
            fs::read_to_string(root.join("src/source.ts")).expect("reads"),
            "before"
        );
        assert!(!root.join("src/types/shared.ts").exists());
        fs::remove_dir_all(root).expect("removes fixture");
    }

    #[test]
    fn retries_rollback_after_a_partial_failure() {
        let root = tempfile::tempdir().expect("creates fixture");
        let existing = root.path().join("a.ts");
        let created = root.path().join("b.ts");
        fs::write(&existing, "before").expect("writes fixture");
        let mut transaction = stage_file_transaction(
            root.path(),
            &BTreeMap::from([
                ("a.ts".into(), "after".into()),
                ("b.ts".into(), "created".into()),
            ]),
        )
        .expect("stages transaction");
        transaction.commit().expect("commits transaction");
        fs::remove_file(&existing).expect("removes file");
        fs::create_dir(&existing).expect("blocks restoration");

        assert!(transaction.rollback().is_err());
        assert!(!created.exists());
        fs::remove_dir(&existing).expect("unblocks restoration");
        transaction.rollback().expect("retries rollback");
        assert_eq!(fs::read_to_string(existing).expect("reads"), "before");
    }

    #[test]
    #[cfg(unix)]
    fn preserves_existing_permissions_on_commit_and_rollback() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().expect("creates fixture");
        let target = root.path().join("source.ts");
        fs::write(&target, "before").expect("writes fixture");
        fs::set_permissions(&target, fs::Permissions::from_mode(0o640)).expect("sets permissions");
        let mut transaction = stage_file_transaction(
            root.path(),
            &BTreeMap::from([("source.ts".into(), "after".into())]),
        )
        .expect("stages");
        transaction.commit().expect("commits");
        assert_eq!(
            fs::metadata(&target)
                .expect("metadata")
                .permissions()
                .mode()
                & 0o777,
            0o640
        );
        transaction.rollback().expect("rolls back");
        assert_eq!(
            fs::metadata(&target)
                .expect("metadata")
                .permissions()
                .mode()
                & 0o777,
            0o640
        );
    }

    #[test]
    #[cfg(unix)]
    fn restores_read_only_files() {
        use std::os::unix::fs::PermissionsExt;

        let root = tempfile::tempdir().expect("creates fixture");
        let target = root.path().join("source.ts");
        let original = [0xff, 0x00, 0x80];
        fs::write(&target, original).expect("writes fixture");
        fs::set_permissions(&target, fs::Permissions::from_mode(0o440)).expect("sets read-only");
        let mut transaction = stage_file_transaction(
            root.path(),
            &BTreeMap::from([("source.ts".into(), "after".into())]),
        )
        .expect("stages");
        transaction.commit().expect("commits");
        transaction.rollback().expect("restores read-only file");
        assert_eq!(fs::read(&target).expect("reads"), original);
        assert_eq!(
            fs::metadata(target).expect("metadata").permissions().mode() & 0o777,
            0o440
        );
    }

    #[test]
    fn preserves_an_existing_temporary_file_on_collision() {
        let root = std::env::temp_dir().join(format!(
            "type-tune-transaction-{}",
            SystemTime::now()
                .duration_since(UNIX_EPOCH)
                .expect("clock")
                .as_nanos()
        ));
        fs::create_dir_all(&root).expect("creates fixture");
        let target = root.join("source.ts");
        let existing = root.join(".source.ts.type-tune-7-0");
        fs::write(&existing, "keep").expect("writes collision fixture");
        let staged = stage_temporary_file_with_nonce(&target, b"staged", 7).expect("stages retry");
        assert_eq!(staged, root.join(".source.ts.type-tune-7-1"));
        assert_eq!(
            fs::read_to_string(existing).expect("reads existing"),
            "keep"
        );
        assert_eq!(fs::read_to_string(&staged).expect("reads staged"), "staged");
        fs::remove_dir_all(root).expect("removes fixture");
    }
}
