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
    ///
    /// A holder may unlink the file it made while it still holds the lock
    /// (`canon`'s repository write lock does, for a refused first call), so
    /// a waiter that gets the lock checks that the path still names the
    /// file it locked and opens again when it does not: two holders can
    /// never lock two files of one name.
    pub fn acquire(path: &Path, wait: Duration) -> Result<FileLock, FileLockError> {
        let deadline = Instant::now() + wait;
        loop {
            if let Some(parent) = path.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let file = OpenOptions::new().create(true).truncate(false).write(true).open(path)?;
            loop {
                match file.try_lock() {
                    Ok(()) => break,
                    Err(TryLockError::WouldBlock) if Instant::now() < deadline => std::thread::sleep(POLL),
                    Err(TryLockError::WouldBlock) => return Err(FileLockError::Busy(wait)),
                    Err(TryLockError::Error(e)) => return Err(e.into()),
                }
            }
            if still_named(&file, path) {
                return Ok(FileLock { _file: file });
            }
        }
    }
}

/// Whether `path` still names the open `file` (Unix: the same device and inode; elsewhere a holder cannot unlink a file another
/// handle has open without the OS refusing new opens of it, so the file is the one named).
fn still_named(file: &File, path: &Path) -> bool {
    #[cfg(unix)]
    {
        use std::os::unix::fs::MetadataExt as _;
        match (file.metadata(), std::fs::metadata(path)) {
            (Ok(open), Ok(named)) => open.dev() == named.dev() && open.ino() == named.ino(),
            _ => false,
        }
    }
    #[cfg(not(unix))]
    {
        let _ = (file, path);
        true
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

    #[cfg(unix)]
    #[test]
    fn a_waiter_that_gets_a_lock_on_an_unlinked_file_locks_the_file_that_is_there_instead() {
        let dir = tempfile::TempDir::new().unwrap();
        let path = dir.path().join("writer.lock");
        let first = FileLock::acquire(&path, Duration::ZERO).unwrap();
        let waiting = {
            let path = path.clone();
            std::thread::spawn(move || FileLock::acquire(&path, Duration::from_secs(10)).map(|lock| (lock, path.is_file())))
        };
        std::thread::sleep(Duration::from_millis(200));
        std::fs::remove_file(&path).unwrap();
        drop(first);
        let (_lock, named) = waiting.join().unwrap().unwrap();
        assert!(named, "the waiter holds the lock of the file the path names, not of the unlinked one");
    }
}
