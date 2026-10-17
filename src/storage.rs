//! Same-directory atomic replacement for explicit user exports/settings.
use std::{io::Write, path::Path};

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
