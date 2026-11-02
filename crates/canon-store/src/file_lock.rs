//! A bounded wait for an exclusive OS advisory lock on a file — the one
//! primitive behind canon's repo-level writer locks (the scenario-index
//! lock, the artifact-store lock).
//!
//! The lock is the OS's (`File::try_lock`: `flock` on Unix, `LockFileEx`
//! on Windows), not the file's existence: closing the file releases it,
//! and so does the holder's death, however it died, so a killed writer
//! never leaves a stale lock behind and the file itself is never
//! deleted. Waiting is bounded and polled; running out is
//! [`FileLockError::Busy`], which each caller words for its own lock.

use std::fs::{File, OpenOptions, TryLockError};
use std::path::Path;
use std::time::{Duration, Instant};

/// How often a waiting writer retries the lock.
const POLL: Duration = Duration::from_millis(50);

/// An exclusive lock on one file, held until this value is dropped.
#[derive(Debug)]
pub struct FileLock {
    _file: File,
}

/// Why [`FileLock::acquire`] holds nothing.
#[derive(Debug, thiserror::Error)]
pub enum FileLockError {
    /// Another holder kept the lock for the whole wait.
    #[error("the lock was held for the whole {}s wait", .0.as_secs())]
    Busy(Duration),
    /// The lock file (or its parent directory) could not be created or
    /// opened, or the OS refused the lock for a reason other than
    /// another holder.
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

impl FileLock {
    /// Open `path` (creating it and its parent directories when absent,
    /// never truncating it) and take its exclusive lock, retrying every
    /// 50ms until `wait` has passed.
    pub fn acquire(path: &Path, wait: Duration) -> Result<FileLock, FileLockError> {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        let file = OpenOptions::new().create(true).truncate(false).write(true).open(path)?;
        let deadline = Instant::now() + wait;
        loop {
            match file.try_lock() {
                Ok(()) => return Ok(FileLock { _file: file }),
                Err(TryLockError::WouldBlock) if Instant::now() < deadline => std::thread::sleep(POLL),
                Err(TryLockError::WouldBlock) => return Err(FileLockError::Busy(wait)),
                Err(TryLockError::Error(e)) => return Err(e.into()),
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_second_holder_waits_out_its_bound_then_succeeds_once_the_first_is_dropped() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("nested/dir/writer.lock");
        let first = FileLock::acquire(&path, Duration::ZERO).unwrap();
        assert!(path.is_file(), "the lock file and its parents are created");
        assert!(matches!(FileLock::acquire(&path, Duration::from_millis(120)), Err(FileLockError::Busy(_))));
        drop(first);
        // A test that forks a child in parallel briefly holds a copy of the
        // fd until the child execs, so the release is not instant: retry
        // within a bound, as every caller does.
        let _second = FileLock::acquire(&path, Duration::from_secs(5)).unwrap();
        assert!(path.is_file(), "the lock file is never deleted");
    }
}
