//! Same-directory atomic replacement for explicit user exports/settings.
use std::{
    fs::File,
    io::{self, Read, Write},
    path::Path,
};

pub mod shared;

pub enum ReadError {
    Io(io::Error),
    TooLarge,
}

/// Missing data differs from unreadable/oversized data. Callers own document
/// validation and recovery; reading never modifies the main file or backup.
pub fn read_bounded(path: &Path, maximum: usize) -> Result<Option<Vec<u8>>, ReadError> {
    let file = match File::open(path) {
        Ok(file) => file,
        Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(None),
        Err(error) => return Err(ReadError::Io(error)),
    };
    let mut bytes = Vec::new();
    file.take(maximum.saturating_add(1) as u64)
        .read_to_end(&mut bytes)
        .map_err(ReadError::Io)?;
    if bytes.len() > maximum {
        return Err(ReadError::TooLarge);
    }
    Ok(Some(bytes))
}

/// Each file is replaced atomically, separately. A failed backup leaves the
/// main file intact; a failed main replacement is still an error.
pub fn replace_backed_up(path: &Path, bytes: &[u8], backup: Option<&[u8]>) -> Result<(), String> {
    if let Some(parent) = path.parent().filter(|p| !p.as_os_str().is_empty()) {
        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
    }
    if let Some(backup) = backup {
        replace(&path.with_extension("json.bak"), backup)?;
    }
    replace(path, bytes)
}

pub fn replace(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let parent = path
        .parent()
        .filter(|p| !p.as_os_str().is_empty())
        .unwrap_or_else(|| Path::new("."));
    let mut temporary = tempfile::Builder::new()
        .prefix(".pttstella-")
        .tempfile_in(parent)
        .map_err(|e| crate::msg!("storage-temporary-failed", e = e.to_string()))?;
    if let Err(error) = temporary
        .write_all(bytes)
        .and_then(|()| temporary.as_file().sync_all())
    {
        return Err(cleanup_error(temporary, error));
    }
    temporary
        .persist(path)
        .map(|_| ())
        .map_err(|error| cleanup_error(error.file, error.error))
}

fn cleanup_error(file: tempfile::NamedTempFile, error: std::io::Error) -> String {
    let path = file.path().to_path_buf();
    match file.close() {
        Ok(()) => crate::msg!("storage-save-failed", error = error.to_string()),
        Err(cleanup) => crate::msg!(
            "storage-cleanup-failed",
            error = error.to_string(),
            value1 = path.display().to_string(),
            cleanup = cleanup.to_string()
        ),
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn bounded_reads_distinguish_missing_exact_limit_oversize_and_io_failure() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("data.json");
        assert!(matches!(super::read_bounded(&path, 3), Ok(None)));
        std::fs::write(&path, b"abc").unwrap();
        assert!(matches!(super::read_bounded(&path, 3), Ok(Some(bytes)) if bytes==b"abc"));
        assert!(matches!(
            super::read_bounded(&path, 2),
            Err(super::ReadError::TooLarge)
        ));
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        assert!(matches!(
            super::read_bounded(&path, 3),
            Err(super::ReadError::Io(_))
        ));
    }
    #[test]
    fn backup_and_main_failures_preserve_data_and_leave_no_temporary_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("nested/data.json");
        super::replace_backed_up(&path, b"first", None).unwrap();
        let backup = path.with_extension("json.bak");
        std::fs::create_dir(&backup).unwrap();
        assert!(super::replace_backed_up(&path, b"second", Some(b"first")).is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"first");
        std::fs::remove_dir(&backup).unwrap();
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        assert!(super::replace_backed_up(&path, b"second", Some(b"first")).is_err());
        assert_eq!(std::fs::read(&backup).unwrap(), b"first");
        assert!(path.is_dir());
        assert_eq!(
            std::fs::read_dir(path.parent().unwrap()).unwrap().count(),
            2
        );
    }
    #[test]
    fn replacement_is_complete_and_failure_preserves_other_files() {
        let directory = tempfile::tempdir().unwrap();
        let path = directory.path().join("export.png");
        std::fs::write(&path, b"old").unwrap();
        super::replace(&path, b"new").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"new");
        let blocked = directory.path().join("directory.png");
        std::fs::create_dir(&blocked).unwrap();
        assert!(super::replace(&blocked, b"cannot replace a directory").is_err());
        assert_eq!(std::fs::read(&path).unwrap(), b"new");
        assert_eq!(std::fs::read_dir(directory.path()).unwrap().count(), 2);
    }
}
