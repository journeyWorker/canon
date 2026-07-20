//! Atomic file replacement — the donor's atomic-write pattern (C6) — write to a unique sibling
//! temp file, `fsync` it, then `rename` it over the target, so a
//! process kill mid-write can never leave `path` holding a torn/
//! partial JSON file. `rename` within the same directory (hence same
//! filesystem) is a POSIX-atomic replace: any concurrent reader of
//! `path` observes either the fully-old or the fully-new bytes, never
//! a mix.
//!
//! Ported from the vendored upstream session-parser project's atomic
//! replace (`replace_file`'s `#[cfg(not(target_os = "windows"))]
//! std::fs::rename` branch — canon-store is not Windows-targeted, so the
//! donor's sibling `MoveFileExW` branch is not ported) combined with the
//! donor's write/fsync/rename/cleanup-on-failure shape
//! (`atomic_write_bytes`), including that same donor's temp-name
//! convention (`<filename>.<pid>.<nanos-hex>.tmp`) adapted here to
//! `<filename>.tmp.<pid>.<nanos-hex>` per this crate's own naming
//! preference.

use std::io::{self, Write};
use std::path::Path;

/// Write `bytes` to `path` atomically. Creates `path`'s parent
/// directories first, then writes+syncs a unique sibling temp file and
/// `rename`s it over `path`. On any failure the temp file is
/// best-effort removed rather than left as litter; `path` itself is
/// untouched until the final `rename` succeeds.
pub fn write_atomic(path: &Path, bytes: &[u8]) -> io::Result<()> {
    let dir = path
        .parent()
        .ok_or_else(|| io::Error::new(io::ErrorKind::InvalidInput, format!("write_atomic: path has no parent directory: {}", path.display())))?;
    std::fs::create_dir_all(dir)?;

    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
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
        std::fs::rename(&tmp_path, path)?;
        Ok(())
    })();

    if result.is_err() {
        let _ = std::fs::remove_file(&tmp_path);
    }
    result
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
}
