//! `canon report [--repo <dir>] [--check] [--snapshot <dir>]` (S9
//! part2, tasks.md 3.1): the CLI surface over `canon-report`'s already-
//! shipped library API (`canon_report::{report, write_report,
//! check_report, snapshot}`, part1/part2's own `crates/canon-report`).
//! This module adds ONLY "resolve `--repo`'s `canon.yaml`-derived
//! [`canon_report::Roots`] + call the right library fn" — it never
//! re-implements rendering, digest, drift-checking, or Parquet-export
//! logic itself (design D1: `canon report` renders/exports the DuckDB
//! views S2 already computed, no second aggregation layer; every one
//! of those already lives in `canon-report`, reviewed clean at S9
//! part1's `beffc751`).
//!
//! # Roots resolution
//! `--repo` resolves through the same
//! [`crate::context::resolve_repo_root`] nearest-`canon.yaml`-ancestor
//! walk `canon retrieve`/`canon context`/`canon fmt`/`canon gate`
//! already use (design D7) — never a second root-resolution
//! convention. The three DuckDB view-layer roots
//! ([`canon_report::Roots`]) resolve off that repo root exactly like
//! `crates/canon-report/src/bin/canon-report.rs`'s own defaults:
//! - git root — `canon.yaml`'s `local` rung's `root`
//!   (`canon_store::policy::TierPolicy`, the SAME parse `canon_cli::
//!   tiers::build_tiers` uses for `canon tier age`/`canon query`),
//!   falling back to `canon/ledger` when `canon.yaml`/the `tiers.git`
//!   section is absent or unparseable — degrades rather than errors,
//!   the same "works with zero config" posture
//!   [`crate::retrieve::open_strategy_store`] already established for
//!   `canon-learn`'s root.
//! - r2 root — `canon/r2`. `canon.yaml`'s `cold` rung only names a LIVE
//!   bucket (`bucket_env`/`prefix`) — there is no local-sync-root
//!   config key today, so this mirrors the standalone `canon-report`
//!   binary's own `--r2-root` default exactly (module doc of
//!   `canon_report::roots`: "the r2 tier's local (or synced) parquet
//!   root").
//! - learn root — `canon.yaml`'s `learn:` section
//!   (`canon_learn::LearnConfig`), reusing
//!   [`crate::retrieve::open_strategy_store`]'s exact resolution.

use std::path::{Path, PathBuf};

use canon_learn::LearnConfig;
use canon_report::{ReportInputs, Roots};
use canon_store::policy::TierPolicy;

use crate::context::resolve_repo_root;

/// The default r2-tier local sync root relative to a resolved repo
/// root (module doc: no `canon.yaml` config key exists for this yet).
const DEFAULT_R2_LOCAL_ROOT: &str = "canon/r2";

/// The default git-tier root relative to a resolved repo root, used
/// only when `canon.yaml`/its git-backed `local` rung is absent or
/// fails to parse (module doc).
const DEFAULT_GIT_ROOT: &str = "canon/ledger";

fn resolve_roots(repo: &Path) -> Roots {
    let canon_yaml_text = std::fs::read_to_string(repo.join("canon.yaml")).ok();

    let git_root = canon_yaml_text
        .as_deref()
        .and_then(|text| TierPolicy::from_yaml(text).ok())
        .and_then(|policy| policy.local_git().cloned())
        .map(|git| repo.join(git.root))
        .unwrap_or_else(|| repo.join(DEFAULT_GIT_ROOT));

    let learn_config = canon_yaml_text.as_deref().and_then(|text| LearnConfig::from_manifest(text).ok()).unwrap_or_default();
    let learn_root = repo.join(learn_config.root);

    let r2_root = repo.join(DEFAULT_R2_LOCAL_ROOT);

    Roots::new(git_root, r2_root, learn_root)
}

/// Resolves `--repo` (design D7 ancestor walk) and builds the
/// [`ReportInputs`] every `canon report` mode needs. Returns the
/// resolved repo root alongside the inputs — callers need it to
/// compute the default report path.
pub fn resolve_inputs(repo: &Path) -> (PathBuf, ReportInputs) {
    let repo = resolve_repo_root(repo);
    let roots = resolve_roots(&repo);
    let inputs = ReportInputs::new(repo.clone(), roots);
    (repo, inputs)
}

/// `<repo>/canon/REPORT.md` — `canon-report`'s own conventional
/// default path ([`canon_report::render::DEFAULT_REPORT_PATH`]),
/// resolved against an already-`resolve_repo_root`-resolved `repo`.
pub fn default_report_path(repo: &Path) -> PathBuf {
    repo.join(canon_report::render::DEFAULT_REPORT_PATH)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn resolve_roots_defaults_to_the_canon_ledger_r2_learn_convention_with_no_canon_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let roots = resolve_roots(dir.path());
        assert_eq!(roots.git_root, dir.path().join("canon/ledger"));
        assert_eq!(roots.r2_root, dir.path().join("canon/r2"));
        assert_eq!(roots.learn_root, dir.path().join("canon/learn"));
    }

    #[test]
    fn resolve_roots_honors_canon_yaml_tiers_local_root_override() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("canon.yaml"), "tiers:\n  local: { backend: git, root: custom/ledger }\n").unwrap();
        let roots = resolve_roots(dir.path());
        assert_eq!(roots.git_root, dir.path().join("custom/ledger"));
    }

    #[test]
    fn resolve_roots_honors_canon_yaml_learn_root_override() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("canon.yaml"), "learn:\n  root: custom/learn\n").unwrap();
        let roots = resolve_roots(dir.path());
        assert_eq!(roots.learn_root, dir.path().join("custom/learn"));
    }

    #[test]
    fn resolve_roots_degrades_to_defaults_on_malformed_canon_yaml() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("canon.yaml"), "not: [valid: yaml").unwrap();
        let roots = resolve_roots(dir.path());
        assert_eq!(roots.git_root, dir.path().join("canon/ledger"));
        assert_eq!(roots.learn_root, dir.path().join("canon/learn"));
    }

    #[test]
    fn default_report_path_is_canon_report_md_under_the_repo_root() {
        let repo = PathBuf::from("/some/repo");
        assert_eq!(default_report_path(&repo), PathBuf::from("/some/repo/canon/REPORT.md"));
    }
}
