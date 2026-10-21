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

/// The default spec root (`specs.roots[]` entry `id: root`, 0.17 D1):
/// the authored `.feature` corpus, at
/// `kind=feature/area=<area>/<surface>.feature` beneath it. Used when
/// `canon.yaml` has no `specs:` section, and written by `canon init`.
/// Authored and git-tracked: no canon `.gitignore` line may cover it.
pub const SPECS_DIR: &str = ".canon/specs";

/// Hot-tier sqlite database file. Overridable via `tiers.hot.path`.
pub const HOT_DB_FILE: &str = ".canon/hot.db";

/// The `.gitignore` line covering the sqlite db + its WAL/SHM siblings.
pub const HOT_DB_GITIGNORE: &str = ".canon/hot.db*";

/// Warm trajectory tier (rebuildable parquet, gitignored). Overridable
/// via `learn.root`.
pub const LEARN_DIR: &str = ".canon/learn";

/// The `.gitignore` line covering [`LEARN_DIR`].
pub const LEARN_GITIGNORE: &str = ".canon/learn/";

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

/// Who approved the policy, and which bytes (`canon policy approve`;
/// authored, git-tracked).
pub const POLICY_APPROVAL_FILE: &str = ".canon/policy.approval.json";

/// Canon's own plan source (authored, git-tracked): one
/// `<CHANGES_DIR>/<slug>/` per change, holding its `brief.md` (the
/// human's `Approved:` line lives there) and `tasks.md`. `canon change
/// new` writes it; every plan reader reads it with no `plans:` entry. A
/// change imported from an external plan gets only a `brief.md` here,
/// whose `Source:` line points at the external document.
pub const CHANGES_DIR: &str = ".canon/changes";

/// `canon report`'s generated-never-edited markdown output.
pub const REPORT_FILE: &str = ".canon/REPORT.md";

/// `canon report --snapshot` / `canon dashboard` parquet snapshot dir.
pub const DASHBOARD_SNAPSHOT_DIR: &str = ".canon/dashboard-snapshot";

/// The `.gitignore` line covering [`DASHBOARD_SNAPSHOT_DIR`]: regenerated
/// by every `canon dashboard` and `canon report --snapshot`.
pub const DASHBOARD_SNAPSHOT_GITIGNORE: &str = ".canon/dashboard-snapshot/";

/// Local cold-tier mirror root (CLI convention; the live cold rung is a
/// remote bucket).
pub const R2_LOCAL_DIR: &str = ".canon/r2";

/// The `.gitignore` line covering [`R2_LOCAL_DIR`].
pub const R2_LOCAL_GITIGNORE: &str = ".canon/r2/";

/// Ingest bookkeeping root (per-source watermark cursors live below).
pub const INGEST_DIR: &str = ".canon/ingest";

/// Per-source ingest watermark cursors.
pub const INGEST_CURSORS_DIR: &str = ".canon/ingest/cursors";

/// The pre-commit gate script `canon gate install-hooks` materializes.
pub const PRE_COMMIT_SCRIPT: &str = ".canon/scripts/canon-gate-pre-commit.sh";

/// Content-addressed store of the bytes `canon evidence add --artifact/
/// --report` bound, one file per digest at `<ARTIFACTS_SHA256_DIR>/<hex>`.
/// Authored (no source regenerates a rewritten report's old bytes), so
/// it is git-tracked, never ignored.
pub const ARTIFACTS_SHA256_DIR: &str = ".canon/artifacts/sha256";

/// The read-access audit log `canon query`/`export`/`purge` append to.
/// Local operator state, never committed: `canon init` ignores its
/// directory ([`AUDIT_GITIGNORE`]).
pub const ACCESS_AUDIT_FILE: &str = ".canon/audit/access.jsonl";

/// The `.gitignore` line covering [`ACCESS_AUDIT_FILE`]'s directory.
pub const AUDIT_GITIGNORE: &str = ".canon/audit/";

/// The `.gitignore` line covering the default ledger's `Scenario` index
/// (`kind=scenario/` under [`LEDGER_DIR`]): derived from the `.feature`
/// files, and rebuilt by every reader of it when stale (0.16 D2), so
/// `canon init` keeps it out of git.
pub const SCENARIO_INDEX_GITIGNORE: &str = ".canon/ledger/kind=scenario/";

/// The advisory lock `canon inventory sync` and every reader's
/// stale-index refresh hold while they write the scenario index, so two
/// of them never index the same change twice. An OS file lock
/// (`flock`/`LockFileEx`), released when the holder exits — even when it
/// is killed — so the file never goes stale and is never deleted. Local
/// state, so `canon init` ignores it.
pub const SCENARIO_INDEX_LOCK_FILE: &str = ".canon/scenario-index.lock";

/// The advisory lock every write to the evidence artifact store
/// ([`ARTIFACTS_SHA256_DIR`]) holds from its "already stored?" check to
/// its last rename or rollback, so a rollback never removes a blob
/// another writer published or counted as stored. An OS file lock
/// (`flock`/`LockFileEx`), released when the holder exits — even when it
/// is killed — so the file never goes stale and is never deleted. Local
/// state, so `canon init` ignores it.
pub const ARTIFACT_STORE_LOCK_FILE: &str = ".canon/artifact-store.lock";

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_product_path_lives_under_the_canon_dir() {
        let all = [
            LEDGER_DIR,
            SPECS_DIR,
            HOT_DB_FILE,
            HOT_DB_GITIGNORE,
            LEARN_DIR,
            STRATEGIES_DIR,
            VOCAB_DIR,
            PLUGINS_DIR,
            POLICY_FILE,
            POLICY_APPROVAL_FILE,
            REPORT_FILE,
            DASHBOARD_SNAPSHOT_DIR,
            R2_LOCAL_DIR,
            INGEST_DIR,
            INGEST_CURSORS_DIR,
            PRE_COMMIT_SCRIPT,
            ARTIFACTS_SHA256_DIR,
            ACCESS_AUDIT_FILE,
            AUDIT_GITIGNORE,
            LEARN_GITIGNORE,
            DASHBOARD_SNAPSHOT_GITIGNORE,
            R2_LOCAL_GITIGNORE,
            SCENARIO_INDEX_GITIGNORE,
            SCENARIO_INDEX_LOCK_FILE,
            ARTIFACT_STORE_LOCK_FILE,
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
