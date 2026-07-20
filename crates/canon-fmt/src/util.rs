//! Small helpers `check.rs` uses: corpus-relative paths and a family
//! subdirectory walker, so every check agrees on "how do I find every
//! `.json` under `ledger/`".

use std::path::{Path, PathBuf};

pub fn relative(root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(root).unwrap_or(path).to_path_buf()
}

pub fn walk_files(root: &Path, subdir: &str) -> impl Iterator<Item = PathBuf> {
    let base = root.join(subdir);
    walkdir::WalkDir::new(base).into_iter().filter_map(Result::ok).filter(|e| e.file_type().is_file()).map(|e| e.path().to_path_buf())
}

pub const ENVELOPE_KEYS: [&str; 4] = ["schema", "kind", "at", "actor"];
