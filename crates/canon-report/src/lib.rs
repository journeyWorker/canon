//! `canon-report`: the generated-never-edited markdown status report
//! over S2's `canon-store` DuckDB view layer (S9 part1, design D1/D2 —
//! `openspec/changes/s9-unified-surface/design.md`), plus S9 part2's
//! `--snapshot` Parquet export ([`snapshot::snapshot`],
//! [`manifest::Manifest`], design D3). `packages/dashboard` and the
//! `canon-cli` `canon report` arm are the REMAINING part2 surfaces —
//! `canon-cli` wires this crate's public API directly (`report()`/
//! `write_report()`/`check_report()`/`snapshot()`), never
//! reimplementing any of it.
//!
//! # Public API
//! - [`report`] — render the current inputs to markdown, in memory.
//! - [`write_report`] — render + persist to a path.
//! - [`check_report`] — `canon report --check`'s drift gate
//!   ([`check::CheckOutcome`]): regenerate in memory, byte-diff against
//!   the existing file (design D2, the donor parity harness's
//!   `cmd_report` lifted near-verbatim).
//! - [`snapshot`] — `canon report --snapshot <dir>`: export every
//!   panel mart to Parquet + a declared `manifest.json` (design D3).
//!
//! # Where the numbers come from
//! [`ReportInputs::roots`] names the three physical sources
//! `canon_store::VIEWS_SQL` reads (git tier, r2 tier, `canon-learn`'s
//! operator-local store — [`roots::Roots`]); [`report`] shells out to
//! the real `duckdb` CLI ([`query::run_query`]) with that view layer as
//! its `-init` file, never a parallel Rust-side computation of any mart
//! (design D1). [`snapshot`] shells out to the SAME `duckdb` CLI
//! ([`query::run_command`]) to `COPY` those views straight to Parquet.

pub mod check;
pub mod digest;
pub mod divergence;
pub mod error;
pub mod manifest;
pub mod marts;
pub mod query;
pub mod render;
pub mod roots;
pub mod snapshot;
pub mod tier_boundary;

use std::path::{Path, PathBuf};

pub use check::CheckOutcome;
pub use error::ReportError;
pub use manifest::Manifest;
pub use roots::Roots;
pub use snapshot::snapshot;

/// Everything [`report`] needs: where the corpus/policy/ledger digest
/// is computed from (`repo_root`), and the three DuckDB view-layer
/// roots (`roots`).
#[derive(Debug, Clone)]
pub struct ReportInputs {
    /// The consumer repo's root — where `.canon/policy.yaml` and the
    /// `.git` directory `git rev-parse HEAD` runs against both live.
    pub repo_root: PathBuf,
    pub roots: Roots,
}

impl ReportInputs {
    pub fn new(repo_root: impl Into<PathBuf>, roots: Roots) -> Self {
        Self { repo_root: repo_root.into(), roots }
    }
}

/// Renders the current report content, in memory — the single
/// generation path [`write_report`]/[`check_report`] both call, so a
/// write and a `--check` diff are guaranteed to compare against the
/// EXACT same rendering logic, never two subtly different code paths.
pub fn report(inputs: &ReportInputs) -> Result<String, ReportError> {
    let digest = digest::DigestHeader::compute(&inputs.repo_root, &inputs.roots.git_root)?;
    let trust_matrix = marts::fetch_trust_matrix(&inputs.roots)?;
    let marts = render::ReportMarts {
        trust_matrix,
        session_costs: marts::fetch_session_costs(&inputs.roots)?,
        role_memory: marts::fetch_role_memory(&inputs.roots)?,
        flywheel_funnel: marts::fetch_flywheel_funnel(&inputs.roots)?,
        review_burndown: marts::fetch_review_burndown(&inputs.roots)?,
        scope_status: marts::fetch_scope_status(&inputs.roots)?,
        subjects: marts::fetch_subjects(&inputs.roots)?,
    };
    // Design D3: `canon-report` reads `<repo_root>/canon.yaml` itself
    // (mirroring `digest::DigestHeader::compute`'s own direct
    // `.canon/policy.yaml` read) — an input `report()` DERIVES, never a
    // live, non-directly-readable-backend read (module doc of
    // `tier_boundary`, s28 design D2).
    let kinds_not_read_directly = tier_boundary::kinds_not_read_directly(&inputs.repo_root);
    Ok(render::render(&digest, &marts, &kinds_not_read_directly))
}

/// [`report`] + persist to `report_path` (parent directories created
/// as needed). Returns the written content.
pub fn write_report(inputs: &ReportInputs, report_path: &Path) -> Result<String, ReportError> {
    let content = report(inputs)?;
    if let Some(parent) = report_path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    std::fs::write(report_path, &content)?;
    Ok(content)
}

/// `canon report --check`: [`report`] regenerated in memory, byte-
/// diffed against `report_path` ([`check::check`]) — never a
/// heuristic freshness check (design D2).
pub fn check_report(inputs: &ReportInputs, report_path: &Path) -> Result<CheckOutcome, ReportError> {
    let content = report(inputs)?;
    check::check(report_path, &content)
}
