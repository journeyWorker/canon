//! [`Manifest`]: `--snapshot <dir>`'s declared table→file map (design
//! D3) — `{generated_at, source_git_sha, source_digest, tables:
//! [{table, file}]}`, verbatim. Unlike the drift-checked markdown
//! report header ([`crate::digest::DigestHeader`], decision 11: NO
//! timestamp/git-sha), `manifest.json` is never drift-checked, so it
//! may safely carry `generated_at`/`source_git_sha` (D2's own
//! reconciliation note) — a browser-side dashboard reads this file to
//! render its freshness banner, exactly [`crate::digest::DigestHeader`]'s
//! module-level cross-reference already documents.

use std::path::Path;
use std::process::Command;

use serde::Serialize;

/// One `manifest.json` `tables[]` entry — `file` is always
/// `format!("{table}.parquet")` (design D3: filenames are
/// byte-identical to table names, never `EXPORT DATABASE`'s escaped
/// variant).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ManifestTable {
    pub table: String,
    pub file: String,
}

/// `--snapshot <dir>`'s `manifest.json` — the dashboard's declared
/// table→file map (module doc); it never enumerates the snapshot
/// directory itself.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct Manifest {
    /// RFC3339 (`chrono`'s default `DateTime<Utc>` JSON
    /// serialization) — the ONE place this crate embeds a wall-clock
    /// timestamp; the markdown report never does (decision 11).
    pub generated_at: chrono::DateTime<chrono::Utc>,
    /// `git rev-parse HEAD` run against the snapshot's `repo_root`,
    /// falling back to `"unknown"` outside a git checkout — never
    /// embedded in the markdown report header (D2's reconciliation
    /// note: a committed report can't hold the hash of the commit
    /// that adds it; a snapshot, never committed nor drift-checked,
    /// can).
    pub source_git_sha: String,
    /// [`crate::digest::DigestHeader::combined_digest`] — one 12-hex
    /// fingerprint over the same corpus/policy/ledger-head digests the
    /// report header renders, so a snapshot's provenance can be
    /// compared against a report's without re-deriving anything.
    pub source_digest: String,
    pub tables: Vec<ManifestTable>,
}

/// `git rev-parse HEAD` against `repo_root` — `"unknown"` outside a
/// git checkout (missing `git` binary, `repo_root` not a work tree,
/// etc.), never an `Err` a caller must handle: a snapshot's provenance
/// degrading to "unknown" is strictly better than aborting the whole
/// export over a repo that simply isn't a git checkout (e.g. a fixture
/// tempdir in a test).
pub fn git_head_sha(repo_root: &Path) -> String {
    Command::new("git")
        .args(["rev-parse", "HEAD"])
        .current_dir(repo_root)
        .output()
        .ok()
        .filter(|output| output.status.success())
        .and_then(|output| String::from_utf8(output.stdout).ok())
        .map(|sha| sha.trim().to_string())
        .filter(|sha| !sha.is_empty())
        .unwrap_or_else(|| "unknown".to_string())
}
