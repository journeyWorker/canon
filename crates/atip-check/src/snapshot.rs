//! The files under a directory, before and after an invocation.
//!
//! A snapshot lists every entry under a root by its path relative to the
//! root: a file by size and SHA-256, a directory by its existence, a symlink
//! by its target. Two snapshots differ in exactly the paths that were
//! created, removed or changed (spec 8.2: a refusal changes nothing).

use std::collections::BTreeMap;
use std::fs;
use std::io;
use std::path::Path;

use sha2::{Digest, Sha256};

/// One entry of a snapshot.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Entry {
    File { size: u64, sha256: [u8; 32] },
    Dir,
    Symlink(String),
}

/// The entries under a root, keyed by their path relative to it.
pub type Snapshot = BTreeMap<String, Entry>;

/// Every entry under `root`, which must exist; links are recorded, never followed.
pub fn take(root: &Path) -> io::Result<Snapshot> {
    let mut snapshot = Snapshot::new();
    walk(root, "", &mut snapshot)?;
    Ok(snapshot)
}

fn walk(dir: &Path, prefix: &str, snapshot: &mut Snapshot) -> io::Result<()> {
    let mut names: Vec<_> = fs::read_dir(dir)?.collect::<io::Result<Vec<_>>>()?;
    names.sort_by_key(fs::DirEntry::file_name);
    for entry in names {
        let relative = format!("{prefix}/{}", entry.file_name().to_string_lossy());
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            snapshot.insert(
                relative,
                Entry::Symlink(fs::read_link(entry.path())?.to_string_lossy().into_owned()),
            );
        } else if kind.is_dir() {
            snapshot.insert(relative.clone(), Entry::Dir);
            walk(&entry.path(), &relative, snapshot)?;
        } else {
            let bytes = fs::read(entry.path())?;
            snapshot.insert(
                relative,
                Entry::File {
                    size: bytes.len() as u64,
                    sha256: Sha256::digest(&bytes).into(),
                },
            );
        }
    }
    Ok(())
}

/// The paths created, removed or changed between `before` and `after`, in path order.
pub fn changed(before: &Snapshot, after: &Snapshot) -> Vec<String> {
    let mut paths: Vec<&String> = before.keys().chain(after.keys()).collect();
    paths.sort();
    paths.dedup();
    paths
        .into_iter()
        .filter(|path| before.get(*path) != after.get(*path))
        .map(|path| path.trim_start_matches('/').to_string())
        .collect()
}

/// Copies the tree at `from` to `to` (created), keeping links as links and permission bits.
pub fn copy_tree(from: &Path, to: &Path) -> io::Result<()> {
    fs::create_dir_all(to)?;
    for entry in fs::read_dir(from)? {
        let entry = entry?;
        let target = to.join(entry.file_name());
        let kind = entry.file_type()?;
        if kind.is_symlink() {
            std::os::unix::fs::symlink(fs::read_link(entry.path())?, &target)?;
        } else if kind.is_dir() {
            copy_tree(&entry.path(), &target)?;
            fs::set_permissions(&target, fs::metadata(entry.path())?.permissions())?;
        } else {
            fs::copy(entry.path(), &target)?;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::fs;

    use super::{changed, copy_tree, take};

    #[test]
    fn atip_check_30_a_snapshot_names_what_was_created_changed_and_removed() {
        let dir = tempfile::tempdir().expect("tempdir");
        fs::write(dir.path().join("kept"), "a").expect("write");
        fs::write(dir.path().join("edited"), "a").expect("write");
        fs::write(dir.path().join("gone"), "a").expect("write");
        let before = take(dir.path()).expect("snapshot");
        fs::write(dir.path().join("edited"), "b").expect("write");
        fs::remove_file(dir.path().join("gone")).expect("remove");
        fs::create_dir(dir.path().join("made")).expect("mkdir");
        fs::write(dir.path().join("made").join("inside"), "").expect("write");
        let after = take(dir.path()).expect("snapshot");
        assert_eq!(
            changed(&before, &after),
            ["edited", "gone", "made", "made/inside"]
        );
        assert!(changed(&after, &after).is_empty());
    }

    #[test]
    fn atip_check_24_a_copy_is_independent_of_its_source() {
        let from = tempfile::tempdir().expect("tempdir");
        let to = tempfile::tempdir().expect("tempdir");
        fs::create_dir(from.path().join("sub")).expect("mkdir");
        fs::write(from.path().join("sub").join("file"), "x").expect("write");
        std::os::unix::fs::symlink("sub/file", from.path().join("link")).expect("symlink");
        let copy = to.path().join("copy");
        copy_tree(from.path(), &copy).expect("copy");
        assert_eq!(
            changed(
                &take(from.path()).expect("snapshot"),
                &take(&copy).expect("snapshot")
            ),
            Vec::<String>::new()
        );
        fs::write(copy.join("sub").join("file"), "y").expect("write");
        assert_eq!(
            fs::read_to_string(from.path().join("sub").join("file")).expect("read"),
            "x"
        );
    }
}
