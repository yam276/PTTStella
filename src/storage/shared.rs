//! Cross-process read/merge/backup/replace transactions for shared documents.
use super::{ReadError, read_bounded, replace_backed_up};
use std::{
    fs::{File, OpenOptions, TryLockError},
    path::PathBuf,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

#[derive(Clone)]
pub struct Document {
    path: PathBuf,
    observed: Arc<Mutex<Option<Vec<u8>>>>,
}
impl Document {
    pub fn new(path: PathBuf) -> Self {
        Self {
            path,
            observed: Arc::default(),
        }
    }
    pub fn read(&self, maximum: usize) -> Result<Option<Vec<u8>>, ReadError> {
        let mut observed = self
            .observed
            .lock()
            .map_err(|_| ReadError::Io(std::io::Error::other("document owner failed")))?;
        let bytes = read_bounded(&self.path, maximum)?;
        *observed = bytes.clone();
        Ok(bytes)
    }
    /// Convenience for callers building a single sequential edit. UI requests
    /// must carry their own base, captured before enqueueing, instead.
    pub fn baseline<T: Default>(
        &self,
        decode: impl FnOnce(&[u8]) -> Result<T, String>,
    ) -> Result<T, String> {
        let bytes = self.observed.lock().map_err(|_| conflict())?;
        Ok(bytes
            .as_deref()
            .and_then(|bytes| decode(bytes).ok())
            .unwrap_or_default())
    }
    pub fn update<T>(
        &self,
        maximum: usize,
        backup_override: Option<Vec<u8>>,
        edit: impl FnOnce(Option<&[u8]>, bool) -> Result<(T, Vec<u8>), String>,
    ) -> Result<T, String> {
        let mut observed = self.observed.lock().map_err(|_| conflict())?;
        if let Some(parent) = self.path.parent().filter(|p| !p.as_os_str().is_empty()) {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let _lock = lock(self.path.with_extension("json.lock"))?;
        let current = match read_bounded(&self.path, maximum) {
            // Explicit history erasure also discards an oversized old document;
            // its empty backup replaces both copies without copying old data.
            Err(ReadError::TooLarge) if backup_override.is_some() => None,
            result => result.map_err(|error| match error {
                ReadError::Io(e) => e.to_string(),
                ReadError::TooLarge => crate::msg!("storage-document-too-large"),
            })?,
        };
        let (result, bytes) = edit(current.as_deref(), *observed == current)?;
        if bytes.len() > maximum {
            return Err(crate::msg!("storage-document-too-large"));
        }
        replace_backed_up(
            &self.path,
            &bytes,
            backup_override.as_deref().or(current.as_deref()),
        )?;
        *observed = Some(bytes);
        Ok(result)
    }
}

fn lock(path: PathBuf) -> Result<File, String> {
    let mut options = OpenOptions::new();
    options.read(true).write(true).create(true).truncate(false);
    #[cfg(unix)]
    {
        use std::os::unix::fs::OpenOptionsExt;
        options.mode(0o600);
    }
    // The sidecar is never unlinked: replacing the JSON cannot replace the lock
    // inode, and a process exit releases the kernel lock without stale leases.
    let file = options.open(path).map_err(|e| e.to_string())?;
    let deadline = Instant::now() + Duration::from_secs(1);
    loop {
        match file.try_lock() {
            Ok(()) => return Ok(file),
            Err(TryLockError::WouldBlock) if Instant::now() < deadline => {
                std::thread::sleep(Duration::from_millis(10))
            }
            Err(TryLockError::WouldBlock) => return Err(crate::msg!("storage-document-busy")),
            Err(TryLockError::Error(error)) => return Err(error.to_string()),
        }
    }
}

pub fn conflict() -> String {
    crate::msg!("storage-document-conflict")
}

pub fn field<T: Clone + PartialEq>(base: &T, draft: &T, current: &T) -> Result<T, String> {
    if draft == base {
        Ok(current.clone())
    } else if current == base || current == draft {
        Ok(draft.clone())
    } else {
        Err(conflict())
    }
}

/// Stable identities are supplied by the document owner, never guessed from JSON.
/// Deletions participate in the same three-way check as updates and additions.
pub fn entries<T: Clone + PartialEq>(
    base: &[T],
    draft: &[T],
    current: &[T],
    same: impl Fn(&T, &T) -> bool,
) -> Result<Vec<T>, String> {
    let mut result = Vec::new();
    for item in draft.iter().chain(current) {
        if result.iter().any(|old| same(old, item)) {
            continue;
        }
        let before = base.iter().find(|old| same(old, item));
        let proposed = draft.iter().find(|old| same(old, item));
        let latest = current.iter().find(|old| same(old, item));
        if let Some(merged) = field(&before, &proposed, &latest)? {
            result.push(merged.clone());
        }
    }
    // A locally deleted item absent from both output lists requires no write.
    Ok(result)
}
