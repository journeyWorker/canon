//! Parallel directory-walk primitive.
//!
//! Ported/adapted from the donor's `scan_directory`:
//! same `WalkDir` + `rayon::par_bridge` shape and the same
//! deterministic `sort_unstable()` tail, but the donor's hardcoded
//! `match pattern { "*.jsonl" => …, "*.json|*.jsonl" => …, … }` string
//! arms are replaced with a per-adapter `matches(path) -> bool`
//! closure (`registry::AdapterEntry::file_matches` supplies it) — S3's
//! adapter set doesn't need a shared pattern-string vocabulary the way
//! the donor's 37-client registry does.

use std::path::{Path, PathBuf};

use rayon::prelude::*;
use walkdir::WalkDir;

/// Walk `root` recursively and return every regular file `matches`
/// accepts, in deterministic (byte-lexical) sorted order. A
/// non-existent `root` yields an empty `Vec`, never an error — an
/// absent/unconfigured adapter source root is a non-fatal, zero-record
/// skip (S3 tasks.md 1.6), not a scan failure.
///
/// Ported from the donor session scanner.
pub fn scan_dir<F>(root: &Path, matches: F) -> Vec<PathBuf>
where
    F: Fn(&Path) -> bool + Sync,
{
    if !root.exists() {
        return Vec::new();
    }

    let mut paths: Vec<PathBuf> = WalkDir::new(root)
        .into_iter()
        .par_bridge()
        .filter_map(|entry| entry.ok())
        .filter(|entry| entry.path().is_file() && matches(entry.path()))
        .map(|entry| entry.path().to_path_buf())
        .collect();

    // Deterministic ordering (ported comment, scanner.rs:378-380):
    // sort_unstable() is sufficient (no stability requirement for
    // PathBuf) and avoids allocation; ordering is byte-lexical, not
    // case-normalized.
    paths.sort_unstable();
    paths
}

/// Union several root directories into one deterministic, deduped
/// file list — the shared primitive behind any adapter whose
/// `scan_roots` returns more than one directory (design D5's Codex
/// live+archived precedent; Wave 1's omp adapter unions `.omp` and
/// `.pi` home directories the same way, `adapters::omp` module doc).
/// Dedup is by canonicalized path where possible (falls back to the
/// raw path when canonicalization fails, e.g. a root that doesn't
/// exist) — mirrors the donor's `push_unique_scan_task`
/// canonicalized-path dedup.
pub fn scan_roots<F>(roots: &[PathBuf], matches: F) -> Vec<PathBuf>
where
    F: Fn(&Path) -> bool + Sync,
{
    let mut seen_canonical = std::collections::HashSet::new();
    let mut out = Vec::new();
    for root in roots {
        let canonical = root.canonicalize().unwrap_or_else(|_| root.clone());
        if !seen_canonical.insert(canonical) {
            continue;
        }
        out.extend(scan_dir(root, &matches));
    }
    out.sort_unstable();
    out.dedup();
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scan_dir_returns_empty_for_missing_root() {
        let missing = PathBuf::from("/definitely/does/not/exist/canon-ingest-test");
        assert!(scan_dir(&missing, |_| true).is_empty());
    }

    #[test]
    fn scan_dir_filters_and_sorts_deterministically() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("b.jsonl"), "").unwrap();
        std::fs::write(dir.path().join("a.jsonl"), "").unwrap();
        std::fs::write(dir.path().join("c.txt"), "").unwrap();
        let sub = dir.path().join("sub");
        std::fs::create_dir(&sub).unwrap();
        std::fs::write(sub.join("d.jsonl"), "").unwrap();

        let found = scan_dir(dir.path(), |p| p.extension().is_some_and(|ext| ext == "jsonl"));
        let names: Vec<_> = found.iter().map(|p| p.file_name().unwrap().to_string_lossy().to_string()).collect();
        assert_eq!(names, vec!["a.jsonl", "b.jsonl", "d.jsonl"]);
    }

    #[test]
    fn scan_roots_dedups_overlapping_and_missing_roots() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("one.jsonl"), "").unwrap();
        let missing = dir.path().join("does-not-exist");

        let roots = vec![dir.path().to_path_buf(), dir.path().to_path_buf(), missing];
        let found = scan_roots(&roots, |p| p.extension().is_some_and(|ext| ext == "jsonl"));
        assert_eq!(found.len(), 1);
    }
}
