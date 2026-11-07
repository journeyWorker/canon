//! Atomic file replacement — the donor's atomic-write pattern (C6) — write to a unique sibling
//! temp file, `fsync` it, then `rename` it over the target, so a
//! process kill mid-write can never leave `path` holding a torn/
//! partial JSON file. `rename` within the same directory (hence same
//! filesystem) is a POSIX-atomic replace: any concurrent reader of
//! `path` observes either the fully-old or the fully-new bytes, never
//! a mix.
//!
//! Ported from the vendored upstream session-parser project's atomic
//! replace (`replace_file`) combined with the donor's
//! write/fsync/rename/cleanup-on-failure shape (`atomic_write_bytes`),
//! including that same donor's temp-name convention
//! (`<filename>.<pid>.<nanos-hex>.tmp`) adapted here to
//! `<filename>.tmp.<pid>.<nanos-hex>` per this crate's own naming
//! preference.
//!
//! Durability: on Unix [`write_atomic`] returns only after the file's bytes
//! (`sync_all` on the temp file) and the directory entry the `rename`
//! created ([`sync_dir`] on the parent) are on disk. canon is also built for
//! Windows (`x86_64-pc-windows-msvc`); there a directory cannot be opened with
//! `File::open`, so the write is the file sync and the `rename` and nothing
//! more, exactly as before. This module does not claim anything stronger for
//! Windows.

use std::io::{self, Write};
use std::path::Path;

/// Write `bytes` to `path` atomically and durably (Unix). Creates `path`'s
/// parent directories first, then writes+syncs a unique sibling temp file,
/// `rename`s it over `path`, and syncs the parent directory so the new entry
/// survives a power loss. On any failure before the `rename` the temp file is
/// best-effort removed rather than left as litter and `path` is untouched.
///
/// A failure of the final directory sync is an error, not a warning: by then
/// `path` already holds the new bytes, but they are not known to be on disk.
/// A caller treats it like any other failed write.
///
/// Limit: directories this call creates (`create_dir_all`) are not themselves
/// synced, so the first record in a brand-new directory can vanish with that
/// directory after a crash; every later write into it is covered.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, format!("write_atomic: path has no parent directory: {}", path.display())))?;
    // A bare file name has the parent `""`, which means the working directory.
    let dir = if dir.as_os_str().is_empty() { Path::new(".") } else { dir };
    std::fs::create_dir_all(dir)?;

    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let tmp_name = format!(
        "{}.tmp.{}.{:x}",
        path.file_name().and_then(|f| f.to_str()).unwrap_or("canon-store-record"),
        std::process::id(),
        nanos
    );
    let tmp_path = dir.join(tmp_name);

    let result = (|| -> io::Result<()> {
        let mut file = std::fs::File::create(&tmp_path)?;
        file.write_all(bytes)?;
        file.sync_all()?;
        trace::record(trace::Event::FileSync(tmp_path.clone()));
        std::fs::rename(&tmp_path, path)?;
        trace::record(trace::Event::Rename { from: tmp_path.clone(), to: path.to_path_buf() });
        Ok(())
    })();

    if result.is_err() {
        let _ = std::fs::remove_file(&tmp_path);
        return result;
    }
    // After the rename the temp name is gone, so there is nothing to clean up if this fails.
    sync_dir(dir)
}

/// Flush `dir`'s entries (a `rename` or create made inside it) to disk.
///
/// Unix only: Windows cannot open a directory with `File::open`, so there this does nothing and the
/// callers behave as they did before it existed. The error keeps the underlying kind and names the directory.
pub fn sync_dir(dir: &Path) -> io::Result<()> {
    #[cfg(unix)]
    {
        std::fs::File::open(dir)
            .and_then(|handle| handle.sync_all())
            .map_err(|e| io::Error::new(e.kind(), format!("could not sync directory {}: {e}", dir.display())))?;
        trace::record(trace::Event::DirSync(dir.to_path_buf()));
    }
    #[cfg(not(unix))]
    let _ = dir;
    Ok(())
}

/// The tests' durability trace: debug builds only, a per-thread log of the sync and rename steps
/// [`write_atomic`] and the artifact store take, in order, so a test can assert "file sync, then rename,
/// then directory sync". A release build records nothing.
pub mod trace {
    use std::path::PathBuf;

    /// One durability step.
    #[derive(Debug, Clone, PartialEq, Eq)]
    pub enum Event {
        /// A file's bytes were synced (`sync_all`).
        FileSync(PathBuf),
        /// `from` was renamed to `to`.
        Rename { from: PathBuf, to: PathBuf },
        /// A directory's entries were synced.
        DirSync(PathBuf),
    }

    #[cfg(debug_assertions)]
    thread_local! {
        static LOG: std::cell::RefCell<Vec<Event>> = const { std::cell::RefCell::new(Vec::new()) };
    }

    /// Append `event` to this thread's log (a no-op in a release build).
    pub fn record(event: Event) {
        #[cfg(debug_assertions)]
        LOG.with(|log| log.borrow_mut().push(event));
        #[cfg(not(debug_assertions))]
        let _ = event;
    }

    /// Take and clear this thread's log (always empty in a release build).
    pub fn take() -> Vec<Event> {
        #[cfg(debug_assertions)]
        {
            LOG.with(|log| std::mem::take(&mut *log.borrow_mut()))
        }
        #[cfg(not(debug_assertions))]
        {
            Vec::new()
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn writes_final_content_and_leaves_no_tmp_residue() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("record.json");
        write_atomic(&path, br#"{"a":1}"#).unwrap();

        assert_eq!(std::fs::read(&path).unwrap(), br#"{"a":1}"#);

        let leftovers: Vec<_> =
            std::fs::read_dir(dir.path()).unwrap().filter_map(Result::ok).filter(|e| e.file_name().to_string_lossy().contains(".tmp.")).collect();
        assert!(leftovers.is_empty(), "temp file(s) left behind after a successful write: {leftovers:?}");
    }

    #[test]
    fn overwrites_an_existing_file_atomically() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("record.json");
        write_atomic(&path, b"first").unwrap();
        write_atomic(&path, b"second").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"second");
    }

    #[test]
    fn creates_missing_parent_directories() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("kind=change").join("area=x").join("record.json");
        write_atomic(&path, b"nested").unwrap();
        assert_eq!(std::fs::read(&path).unwrap(), b"nested");
    }

    #[test]
    fn rejects_a_path_with_no_parent() {
        let err = write_atomic(Path::new("/"), b"x").unwrap_err();
        assert_eq!(err.kind(), io::ErrorKind::InvalidInput);
    }

    #[cfg(all(unix, debug_assertions))]
    #[test]
    fn one_write_syncs_the_file_then_renames_then_syncs_the_parent_directory() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("record.json");
        trace::take();
        write_atomic(&path, b"x").unwrap();
        let events = trace::take();
        let [trace::Event::FileSync(synced), trace::Event::Rename { from, to }, trace::Event::DirSync(parent)] = events.as_slice() else {
            panic!("expected file sync, rename, directory sync in that order, got {events:?}");
        };
        assert_eq!(synced, from, "the file synced is the temp file that is then renamed");
        assert_eq!(to, &path);
        assert_eq!(parent, dir.path());
    }

    #[cfg(all(unix, debug_assertions))]
    #[test]
    fn a_refused_path_records_no_sync() {
        trace::take();
        write_atomic(Path::new("/"), b"x").unwrap_err();
        assert!(trace::take().is_empty());
    }

    /// Unix, not root: a parent that can be written and searched but not read lets the temp file and `rename` succeed and fails only the
    /// directory sync. Skipped (and says so) as root, who can open any directory, and on Windows.
    #[cfg(unix)]
    #[test]
    fn an_unreadable_parent_fails_the_directory_sync_naming_the_directory_skipped_as_root() {
        use std::os::unix::fs::PermissionsExt;
        let dir = tempfile::tempdir().unwrap();
        let parent = dir.path().join("closed");
        std::fs::create_dir(&parent).unwrap();
        std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(0o300)).unwrap();
        let restore = |mode| std::fs::set_permissions(&parent, std::fs::Permissions::from_mode(mode)).unwrap();
        if std::fs::File::open(&parent).is_ok() {
            restore(0o700);
            eprintln!("skipped: running as root, who can open an unreadable directory");
            return;
        }
        let path = parent.join("record.json");
        let err = write_atomic(&path, b"new").unwrap_err();
        restore(0o700);
        assert_eq!(err.kind(), io::ErrorKind::PermissionDenied);
        assert!(err.to_string().contains(&parent.display().to_string()), "the error names the directory: {err}");
        assert_eq!(std::fs::read(&path).unwrap(), b"new", "the rename had already placed the bytes: the error means not known to be on disk");
    }
}
