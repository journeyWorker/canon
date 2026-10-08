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
//! - git root — `canon.yaml`'s `local` rung's `root`, resolved by
//!   [`canon_gate::GateCtx::from_repo`] (the SAME resolution `canon
//!   gate check` uses, over `canon_store::policy::TierPolicy`), falling
//!   back to `.canon/ledger` only when `canon.yaml` is absent or
//!   declares no git-backed `local` rung. A PRESENT `canon.yaml` that
//!   cannot be read or parsed is a [`ReportConfigError`] (exit 2):
//!   rendering the default ledger instead would publish a report of a
//!   ledger the repo never named.
//! - r2 root — `.canon/r2`. `canon.yaml`'s `cold` rung only names a LIVE
//!   bucket (`bucket_env`/`prefix`) — there is no local-sync-root
//!   config key today, so this mirrors the standalone `canon-report`
//!   binary's own `--r2-root` default exactly (module doc of
//!   `canon_report::roots`: "the r2 tier's local (or synced) parquet
//!   root").
//! - learn root — `canon.yaml`'s `learn:` section
//!   (`canon_learn::LearnConfig`), `.canon/learn` when the file or the
//!   section is absent; a present file whose `learn:` section does not
//!   parse is a [`ReportConfigError`] too, for the same reason.

use std::path::{Path, PathBuf};

use canon_gate::{CanonYamlError, GateCtx};
use canon_learn::{LearnConfig, LearnError};
use canon_model::paths;
use canon_report::{ReportInputs, Roots};

use crate::context::resolve_repo_root;

/// The default r2-tier local sync root relative to a resolved repo
/// root (module doc: no `canon.yaml` config key exists for this yet).
const DEFAULT_R2_LOCAL_ROOT: &str = paths::R2_LOCAL_DIR;

/// `<repo>/canon.yaml` is present but cannot resolve the report's
/// roots. Worded like [`CanonYamlError`] (`parsing `<path>`: …`) so
/// every verb that refuses the file names it the same way.
#[derive(Debug, thiserror::Error)]
pub enum ReportConfigError {
    #[error(transparent)]
    CanonYaml(#[from] CanonYamlError),
    #[error("parsing `{}`: {source}", path.display())]
    Learn { path: PathBuf, source: LearnError },
}

fn resolve_roots(repo: &Path) -> Result<Roots, ReportConfigError> {
    // Tier policy first: an unreadable or syntactically broken file
    // surfaces through the same `CanonYamlError` `canon gate check`
    // prints, and once it has parsed, the file is known to be readable.
    let git_root = GateCtx::from_repo(repo)?.ledger_root;

    let canon_yaml = repo.join("canon.yaml");
    let learn_config = match std::fs::read_to_string(&canon_yaml) {
        Ok(text) => LearnConfig::from_manifest(&text).map_err(|source| ReportConfigError::Learn { path: canon_yaml, source })?,
        Err(_) => LearnConfig::default(),
    };
    let learn_root = repo.join(learn_config.root);

    let r2_root = repo.join(DEFAULT_R2_LOCAL_ROOT);

    Ok(Roots::new(git_root, r2_root, learn_root))
}

/// Resolves `--repo` (design D7 ancestor walk) and builds the
/// [`ReportInputs`] every `canon report` mode needs. Returns the
/// resolved repo root alongside the inputs — callers need it to
/// compute the default report path.
pub fn resolve_inputs(repo: &Path) -> Result<(PathBuf, ReportInputs), ReportConfigError> {
    let repo = resolve_repo_root(repo);
    let roots = resolve_roots(&repo)?;
    let inputs = ReportInputs::new(repo.clone(), roots);
    Ok((repo, inputs))
}

/// `<repo>/.canon/REPORT.md` — `canon-report`'s own conventional
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
        let roots = resolve_roots(dir.path()).unwrap();
        assert_eq!(roots.git_root, dir.path().join(".canon/ledger"));
        assert_eq!(roots.r2_root, dir.path().join(".canon/r2"));
        assert_eq!(roots.learn_root, dir.path().join(".canon/learn"));
    }

    #[test]
    fn resolve_roots_honors_canon_yaml_tiers_local_root_override() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("canon.yaml"), "tiers:\n  local: { backend: git, root: custom/ledger }\n").unwrap();
        let roots = resolve_roots(dir.path()).unwrap();
        assert_eq!(roots.git_root, dir.path().join("custom/ledger"));
    }

    #[test]
    fn resolve_roots_honors_canon_yaml_learn_root_override() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("canon.yaml"), "learn:\n  root: custom/learn\n").unwrap();
        let roots = resolve_roots(dir.path()).unwrap();
        assert_eq!(roots.learn_root, dir.path().join("custom/learn"));
    }

    /// A present but unparseable `canon.yaml` refuses, naming the file,
    /// instead of reporting on the default ledger.
    #[test]
    fn resolve_roots_refuses_a_malformed_canon_yaml() {
        let dir = tempfile::tempdir().unwrap();
        let canon_yaml = dir.path().join("canon.yaml");
        std::fs::write(&canon_yaml, "not: [valid: yaml").unwrap();
        let err = resolve_roots(dir.path()).unwrap_err();
        assert!(matches!(err, ReportConfigError::CanonYaml(_)), "{err:?}");
        assert!(err.to_string().starts_with(&format!("parsing `{}`: ", canon_yaml.display())), "{err}");
    }

    /// A tier policy that parses but a `learn:` section that does not is
    /// refused too, never resolved to `.canon/learn`.
    #[test]
    fn resolve_roots_refuses_a_malformed_learn_section() {
        let dir = tempfile::tempdir().unwrap();
        let canon_yaml = dir.path().join("canon.yaml");
        std::fs::write(&canon_yaml, "learn:\n  roles: [\"Not A Role\"]\n").unwrap();
        let err = resolve_roots(dir.path()).unwrap_err();
        assert!(matches!(err, ReportConfigError::Learn { .. }), "{err:?}");
        let message = err.to_string();
        assert!(message.starts_with(&format!("parsing `{}`: ", canon_yaml.display())) && message.contains("Not A Role"), "{message}");
    }

    #[test]
    fn default_report_path_is_canon_report_md_under_the_repo_root() {
        let repo = PathBuf::from("/some/repo");
        assert_eq!(default_report_path(&repo), PathBuf::from("/some/repo/.canon/REPORT.md"));
    }
}
