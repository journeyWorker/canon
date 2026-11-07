//! The staging temp files a dead blob write leaves in the artifact store (0.39): found by `canon artifacts verify` and `canon artifacts prune`,
//! deleted only by `prune --commit`.
//!
//! A blob upload writes `.canon/artifacts/sha256/.tmp-<sha256>-<pid>-<nanos>` and renames it when it is complete
//! ([`canon_gate::artifact_store::ArtifactStore`]); a writer killed in between leaves the file. A **stale temp file** is a regular file of that
//! exact name shape whose modification time is more than [`STALE_AFTER`] before now. Everything else under `.tmp-` (a name of another shape, a
//! symlink, a directory, a file written less than five minutes ago, a file stamped in the future) is not one: it is neither listed nor touched.
//!
//! The age floor covers a writer that does not take the artifact-store lock (an older canon); a live write keeps updating the file's time. The
//! process number in the name is never consulted: it may belong to another host.

use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use canon_gate::artifact_store::{is_sha256_hex, store_dir};

/// A temp file is stale when it was last modified before `now` minus this. A constant: an option would open a way to delete a live write.
pub(crate) const STALE_AFTER: Duration = Duration::from_secs(5 * 60);

/// Prefix of a staging temp file's name.
const PREFIX: &str = ".tmp-";

/// One stale temp file of the local store.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(crate) struct StaleTemp {
    /// The file's name in the store directory.
    pub(crate) name: String,
    pub(crate) path: PathBuf,
    pub(crate) bytes: u64,
}

/// Whether `name` is the name a staging write gives its temp file: `.tmp-<64 lowercase hex>-<digits>-<digits>`.
pub(crate) fn is_staging_name(name: &str) -> bool {
    let Some(rest) = name.strip_prefix(PREFIX) else { return false };
    let mut parts = rest.split('-');
    let (Some(sha), Some(pid), Some(nanos), None) = (parts.next(), parts.next(), parts.next(), parts.next()) else { return false };
    let digits = |text: &str| !text.is_empty() && text.bytes().all(|b| b.is_ascii_digit());
    is_sha256_hex(sha) && digits(pid) && digits(nanos)
}

/// Whether a file with `meta` is a stale temp file at `now`: a regular file (a link is not followed, so `meta` must come from `symlink_metadata`)
/// last modified before `now - STALE_AFTER`. A time in the future, or one the file system does not give, is not stale.
fn is_stale(meta: &std::fs::Metadata, now: SystemTime) -> bool {
    let Some(cutoff) = now.checked_sub(STALE_AFTER) else { return false };
    meta.file_type().is_file() && meta.modified().is_ok_and(|modified| modified < cutoff)
}

/// The stale temp files of `repo`'s local artifact store, in name order. No store directory is no temp files; a store directory that is no real
/// directory or cannot be read is an error: the count is not known.
pub(crate) fn find(repo: &Path, now: SystemTime) -> std::io::Result<Vec<StaleTemp>> {
    let dir = store_dir(repo);
    match std::fs::symlink_metadata(&dir) {
        Ok(meta) if meta.file_type().is_dir() => {}
        Ok(_) => return Err(std::io::Error::other(format!("{} is not a real directory", dir.display()))),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(Vec::new()),
        Err(e) => return Err(e),
    }
    let mut found = Vec::new();
    for entry in canon_store::eio::read_dir(&dir)? {
        let entry = entry?;
        let name = entry.file_name().to_string_lossy().into_owned();
        if !is_staging_name(&name) {
            continue;
        }
        let path = entry.path();
        match std::fs::symlink_metadata(&path) {
            Ok(meta) if is_stale(&meta, now) => found.push(StaleTemp { name, path, bytes: meta.len() }),
            Ok(_) => {}
            // Gone since the listing: a write finished or another sweep ran.
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e),
        }
    }
    found.sort_by(|a, b| a.name.cmp(&b.name));
    Ok(found)
}

/// Deletes `temp` when it is still a stale temp file at `now`; call it holding the artifact-store lock. `Ok(Some(bytes))` when it was deleted,
/// `Ok(None)` when it was not: it is gone, or it is no longer a regular file or no longer old enough (another writer touched it). Only a failed
/// delete is an error.
pub(crate) fn remove(temp: &StaleTemp, now: SystemTime) -> std::io::Result<Option<u64>> {
    let meta = match std::fs::symlink_metadata(&temp.path) {
        Ok(meta) => meta,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(e) => return Err(e),
    };
    if !is_stale(&meta, now) {
        return Ok(None);
    }
    match std::fs::remove_file(&temp.path) {
        Ok(()) => Ok(Some(meta.len())),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(None),
        Err(e) => Err(e),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const SHA: &str = "0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef";

    fn aged(path: &Path, bytes: &[u8], age: Option<Duration>) {
        std::fs::write(path, bytes).unwrap();
        let file = std::fs::File::options().write(true).open(path).unwrap();
        let time = match age {
            Some(age) => SystemTime::now() - age,
            None => SystemTime::now() + Duration::from_secs(3600),
        };
        file.set_modified(time).unwrap();
    }

    fn store(root: &Path) -> PathBuf {
        let dir = store_dir(root);
        std::fs::create_dir_all(&dir).unwrap();
        dir
    }

    /// Only the exact staging shape is a staging name.
    #[test]
    fn only_the_exact_shape_is_a_staging_name() {
        assert!(is_staging_name(&format!(".tmp-{SHA}-123-456")));
        for name in [".tmp-x", ".tmp-notes", &format!(".tmp-{SHA}"), &format!(".tmp-{SHA}-1"), &format!(".tmp-{SHA}-1-2-3"), &format!(".tmp-{SHA}--2"), &format!(".tmp-{SHA}-a-2"), &format!(".tmp-{}-1-2", SHA.to_uppercase()), &format!(".tmp-{}-1-2", &SHA[..63]), &format!("tmp-{SHA}-1-2")] {
            assert!(!is_staging_name(name), "{name}");
        }
    }

    /// A stale temp file is a regular file of the shape older than five minutes: a younger one, one stamped in the future, a link and a directory
    /// of the same name are not.
    #[test]
    fn find_lists_only_regular_staging_files_older_than_the_floor() {
        let root = tempfile::tempdir().unwrap();
        let dir = store(root.path());
        let old = format!(".tmp-{SHA}-1-1");
        aged(&dir.join(&old), b"abcd", Some(Duration::from_secs(6 * 60)));
        aged(&dir.join(format!(".tmp-{SHA}-2-2")), b"young", Some(Duration::from_secs(60)));
        aged(&dir.join(format!(".tmp-{SHA}-3-3")), b"future", None);
        aged(&dir.join(".tmp-x"), b"mine", Some(Duration::from_secs(3600)));
        std::fs::create_dir(dir.join(format!(".tmp-{SHA}-4-4"))).unwrap();
        std::os::unix::fs::symlink(dir.join(&old), dir.join(format!(".tmp-{SHA}-5-5"))).unwrap();
        let found = find(root.path(), SystemTime::now()).unwrap();
        assert_eq!(found.iter().map(|t| (t.name.as_str(), t.bytes)).collect::<Vec<_>>(), [(old.as_str(), 4)]);
    }

    /// No store directory is no temp files; a store that is no directory is an error, not zero.
    #[test]
    fn find_without_a_directory_is_empty_and_a_file_in_its_place_is_an_error() {
        let root = tempfile::tempdir().unwrap();
        assert_eq!(find(root.path(), SystemTime::now()).unwrap(), Vec::new());
        let dir = store(root.path());
        std::fs::remove_dir(&dir).unwrap();
        std::fs::write(&dir, b"not a directory").unwrap();
        find(root.path(), SystemTime::now()).unwrap_err();
    }

    /// The delete re-checks under the lock: a file that was touched since the plan, replaced by a link or a directory, or is gone is skipped
    /// (`None`) and kept; a stale one goes with its size.
    #[test]
    fn remove_rechecks_the_file_and_skips_what_changed() {
        let root = tempfile::tempdir().unwrap();
        let dir = store(root.path());
        let name = format!(".tmp-{SHA}-1-1");
        let path = dir.join(&name);
        aged(&path, b"abcd", Some(Duration::from_secs(6 * 60)));
        let listed = find(root.path(), SystemTime::now()).unwrap();
        let temp = &listed[0];

        // Touched since the plan: it is young again.
        aged(&path, b"abcd", Some(Duration::from_secs(10)));
        assert_eq!(remove(temp, SystemTime::now()).unwrap(), None);
        assert!(path.exists());
        // Stamped in the future.
        aged(&path, b"abcd", None);
        assert_eq!(remove(temp, SystemTime::now()).unwrap(), None);
        assert!(path.exists());
        // Replaced by a symlink to an old file: the link is not followed, and the target stays.
        let target = dir.join("target");
        aged(&target, b"precious", Some(Duration::from_secs(3600)));
        std::fs::remove_file(&path).unwrap();
        std::os::unix::fs::symlink(&target, &path).unwrap();
        assert_eq!(remove(temp, SystemTime::now()).unwrap(), None);
        assert!(path.symlink_metadata().is_ok() && target.exists());
        // Replaced by a directory.
        std::fs::remove_file(&path).unwrap();
        std::fs::create_dir(&path).unwrap();
        assert_eq!(remove(temp, SystemTime::now()).unwrap(), None);
        assert!(path.is_dir());
        std::fs::remove_dir(&path).unwrap();
        // Gone: not counted, not an error.
        assert_eq!(remove(temp, SystemTime::now()).unwrap(), None);
        // Stale: deleted, with its bytes.
        aged(&path, b"abcd", Some(Duration::from_secs(6 * 60)));
        assert_eq!(remove(temp, SystemTime::now()).unwrap(), Some(4));
        assert!(!path.exists());
    }
}
