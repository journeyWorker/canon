//! Canonical repo-relative locations of everything canon produces in a
//! consumer repo.
//!
//! Every canon-made artifact lives under the single `.canon/` directory
//! so a user can opt out by deleting `canon.yaml` + `.canon/` (plus the
//! `.claude`/`.codex` hook entries `canon gate install-hooks` wrote).
//! Only `canon.yaml` itself stays at the repo root — it is the anchor
//! the nearest-ancestor repo-root walk discovers.
//!
//! These are DEFAULTS: where a `canon.yaml` key can override a location
//! (`tiers.local.root`, `learn.root`, …), the explicit value wins. But
//! every default is defined HERE, once — a crate hardcoding its own
//! `".canon/…"` literal instead of naming a constant from this module
//! is a bug (the pre-`.canon` layout drifted exactly that way, with
//! `canon/ledger` duplicated across crates).

/// The single directory holding everything canon produces.
pub const CANON_DIR: &str = ".canon";

/// Git-tier record root (Hive `kind=…/` partitions + the divergence
/// staging dir). Overridable via `canon.yaml` `tiers.local.root`.
pub const LEDGER_DIR: &str = ".canon/ledger";

/// Hot-tier sqlite database file. Overridable via `tiers.hot.path`.
pub const HOT_DB_FILE: &str = ".canon/hot.db";

/// The `.gitignore` line covering the sqlite db + its WAL/SHM siblings.
pub const HOT_DB_GITIGNORE: &str = ".canon/hot.db*";

/// Warm trajectory tier (rebuildable parquet, gitignored). Overridable
/// via `learn.root`.
pub const LEARN_DIR: &str = ".canon/learn";

/// Promoted, git-tracked strategy tier. Overridable via
/// `learn.demotion.strategies_root`.
pub const STRATEGIES_DIR: &str = ".canon/strategies";

/// Typed-vocabulary plugin root (authored). Overridable via
/// `canon.project.yaml`'s `vocabDir`.
pub const VOCAB_DIR: &str = ".canon/vocab/";

/// Ledger-overlay plugin root (authored).
pub const PLUGINS_DIR: &str = ".canon/plugins";

/// The gate's policy expressions (authored).
pub const POLICY_FILE: &str = ".canon/policy.yaml";

/// `canon report`'s generated-never-edited markdown output.
pub const REPORT_FILE: &str = ".canon/REPORT.md";

/// `canon report --snapshot` / `canon dashboard` parquet snapshot dir.
pub const DASHBOARD_SNAPSHOT_DIR: &str = ".canon/dashboard-snapshot";

/// Local cold-tier mirror root (CLI convention; the live cold rung is a
/// remote bucket).
pub const R2_LOCAL_DIR: &str = ".canon/r2";

/// Ingest bookkeeping root (per-source watermark cursors live below).
pub const INGEST_DIR: &str = ".canon/ingest";

/// Per-source ingest watermark cursors.
pub const INGEST_CURSORS_DIR: &str = ".canon/ingest/cursors";

/// The pre-commit gate script `canon gate install-hooks` materializes.
pub const PRE_COMMIT_SCRIPT: &str = ".canon/scripts/canon-gate-pre-commit.sh";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_product_path_lives_under_the_canon_dir() {
        let all = [
            LEDGER_DIR,
            HOT_DB_FILE,
            HOT_DB_GITIGNORE,
            LEARN_DIR,
            STRATEGIES_DIR,
            VOCAB_DIR,
            PLUGINS_DIR,
            POLICY_FILE,
            REPORT_FILE,
            DASHBOARD_SNAPSHOT_DIR,
            R2_LOCAL_DIR,
            INGEST_DIR,
            INGEST_CURSORS_DIR,
            PRE_COMMIT_SCRIPT,
        ];
        for path in all {
            assert!(
                path.strip_prefix(CANON_DIR)
                    .is_some_and(|rest| rest.is_empty() || rest.starts_with('/') || rest.starts_with('.')),
                "{path} escapes {CANON_DIR}/ — the opt-out contract (delete canon.yaml + .canon/) would break"
            );
        }
    }
}
