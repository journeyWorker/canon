//! Small helpers `check.rs` uses: corpus-relative paths and a family
//! subdirectory walker, so every check agrees on "how do I find every
//! `.json` under `ledger/`".

use std::path::{Path, PathBuf};

use canon_model::family::FamilyKind;

pub fn relative(root: &Path, path: &Path) -> PathBuf {
    path.strip_prefix(root).unwrap_or(path).to_path_buf()
}

pub fn walk_files(root: &Path, subdir: impl AsRef<Path>) -> impl Iterator<Item = PathBuf> {
    let base = root.join(subdir);
    walkdir::WalkDir::new(base).into_iter().filter_map(Result::ok).filter(|e| e.file_type().is_file()).map(|e| e.path().to_path_buf())
}

/// Every `.feature` file of a spec root's corpus: everything under
/// `<root>/kind=feature/` (0.17 D1, [`FamilyKind::Feature`]'s
/// [`canon_model::family::LayoutDescriptor::corpus_dir`]). The one walk
/// `canon format`, `canon inventory sync`, the readers' index refresh and
/// the scaffold's duplicate-tag check share. A plain directory walk: it
/// never consults `.gitignore` and never skips hidden directories, so a
/// root inside `.canon/` is read like any other.
pub fn feature_files(root: &Path) -> impl Iterator<Item = PathBuf> {
    walk_files(root, FamilyKind::Feature.layout_descriptor().corpus_dir()).filter(|path| path.extension().and_then(|e| e.to_str()) == Some("feature"))
}

/// The directory, under a spec root, that held its `.feature` corpus
/// before 0.17 (`features/kind=feature/…`).
pub const PRE_0_17_FEATURES_DIR: &str = "features";

/// The first `.feature` file (root-relative, in path order) under a spec root's
/// pre-0.17 `features/` directory, or `None` when there is none. A root
/// holding one is refused whole (0.17 D1, no dual-layout reading):
/// `canon format`, `canon inventory sync`, the readers' index refresh and
/// `canon gate check` all fail on it and name `canon migrate`
/// ([`pre_0_17_layout_detail`]). Only `.feature` files count, so an empty
/// leftover directory is not a refusal.
pub fn pre_0_17_feature_file(root: &Path) -> Option<PathBuf> {
    walk_files(root, PRE_0_17_FEATURES_DIR).filter(|path| path.extension().and_then(|e| e.to_str()) == Some("feature")).map(|path| relative(root, &path)).min()
}

/// Why a spec root on the pre-0.17 layout is refused: names the root
/// (`root_label`, already quoted the way the caller shows roots), the
/// first offending file, the layout 0.17 reads, and `canon migrate`.
pub fn pre_0_17_layout_detail(root_label: &str, first: &Path) -> String {
    format!(
        "spec root {root_label} is on the pre-0.17 layout (`{}` sits under `features/`); canon reads `kind=feature/area=<area>/<surface>.feature` directly under the root, and `canon migrate` moves `features/kind=feature/` there",
        first.display()
    )
}

pub const ENVELOPE_KEYS: [&str; 4] = ["schema", "kind", "at", "actor"];
