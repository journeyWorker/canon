//! [`kinds_not_read_directly`]: the set of record kinds whose routed
//! rung's configured backend is not one of `canon-report`'s own local
//! read roots — the marts never read that backend's store directly, so
//! their data appears only if separately materialized into the local
//! roots (it may be incomplete or stale), never proven complete or
//! current from `canon.yaml` alone (round-
//! 3 dogfood finding F2, `openspec/changes/s25-report-pg-tier-
//! boundary/proposal.md` "Why"; reframed from tier-IDENTITY to BACKEND
//! CAPABILITY by `openspec/changes/s27-tier-role-backend-split/
//! design.md` D2; corrected from "offline-file-readable" to "read
//! directly by report" — S3 now correctly excluded — by `openspec/
//! changes/s28-rung-backend-capability/design.md` D2/D3) — computed
//! PURELY from `canon.yaml`'s STATIC `TierPolicy.routing`/
//! `TierPolicy.tiers` tables (`canon_store::policy::TierPolicy::
//! from_yaml`, a pure YAML parse, no socket, no live connection or row
//! count — s25 design D1). This is the single derivation both
//! [`render_note`] (the markdown `## Kinds not read directly` section,
//! `crate::render::render`) and [`warn_line`] (the CLI's stderr
//! `WARN`, `canon-cli`'s `run_report`) read off — the note and the
//! warning can never disagree because both are one function call away
//! from the exact same `Vec<RecordKind>` (s25 design D3/R2).
//!
//! # s27 `tier-role-backend-split`: backend capability, not tier identity
//! This module originally (s25) filtered on `TierKind::Pg` directly —
//! correct only because `pg` happened to be the one non-file-scannable
//! backend AND the identity of the one live-queryable tier at the same
//! time. s27's `Rung`/`Backend` split makes those two facts
//! independent: a rung's ROLE (`local`/`hot`/`cold`) and its
//! configured `Backend` (`git`/`postgres`/`s3`) are no longer the same
//! axis. A routed rung with NO `tiers.<rung>` entry at all (never
//! configured) is treated identically to "backend unknown, exclude
//! conservatively" — this module never assumes an unconfigured rung is
//! read directly.
//!
//! # s28 `rung-backend-capability`: `read_directly_by_report`, not `offline_file_readable`
//! s27's `!Backend::offline_file_readable()` filter WRONGLY treated
//! `S3` as report-visible: `canon report`'s marts read ONLY `canon-
//! report`'s own local roots — the git ledger (`stg_git_records`) and
//! a LOCAL `.canon/r2` parquet directory (`stg_r2_records`, rooted at
//! `CANON_R2_ROOT`) — never a live S3 bucket. `canon tier age` writes
//! cold/S3 records to the LIVE bucket, not to local `.canon/r2`; a
//! local mirror only exists if an operator separately materializes
//! one, which canon has no automatic sync for today. This module now
//! filters on `!Backend::read_directly_by_report()`
//! (`canon_store::policy::Backend::read_directly_by_report`, s28
//! design D2) — the ONE capability method every report-inclusion
//! decision in the codebase reads — so a `RecordKind` is named here
//! whenever its ROUTED RUNG's configured backend's OWN store is not
//! one `canon-report` opens directly, REGARDLESS of which rung it
//! happens to be. `Backend::class()` (s28 design D1) is a SEPARATE,
//! parse-time compatibility check `TierPolicy::from_yaml` already
//! performed by the time this module ever sees a `TierPolicy` — this
//! module reads only `read_directly_by_report`, never `class`.
//!
//! # Why this set is a CONSERVATIVE lower bound on "invisible to the marts", not an exact list
//! `crates/canon-store/sql/views.sql`'s `stg_records` is exhaustively
//! `stg_git_records UNION ALL stg_r2_records` — no third source, and
//! `stg_r2_records` scans whatever parquet happens to sit at
//! `CANON_R2_ROOT` (default `<repo>/.canon/r2`), REGARDLESS of whether
//! that directory is a live-synced mirror of an S3-backed cold rung, a
//! stale one, or empty. So a kind named here is GUARANTEED to route to
//! a backend whose own live store the report never opens directly —
//! but its data MAY still incidentally appear in a panel if a local
//! `.canon/r2` mirror happens to hold it (s28 design D3: this is why
//! the rendered note and stderr WARN never claim an absolute "not
//! reflected" — they say "not read directly", true whether or not a
//! mirror exists).
//!
//! # Fail-soft posture
//! A missing or malformed `canon.yaml` degrades to an empty `Vec` —
//! never a panic, never an `Err` — mirroring
//! `crate::digest::DigestHeader::compute`'s existing `<repo_root>/
//! .canon/policy.yaml` precedent exactly (s25 design D1/R1): a config
//! error ELSEWHERE in `canon.yaml` (e.g. a malformed `aging` duration,
//! unrelated to `routing`) must never turn an otherwise-successful
//! `canon report` run into a hard failure. `canon fmt`/`canon gate
//! check` remain the blessed surfaces for a malformed `canon.yaml` to
//! fail loud.
//!
//! # No live, non-directly-readable-backend read anywhere in this module
//! Explicit non-goal (s25/s27/s28 proposals): no `PgTier::connect`, no
//! row count, no `stg_pg_records`/`stg_s3_records`, no automatic
//! `.canon/r2` materialization. Data routed to a backend the report
//! does not read directly stays reachable exclusively through `canon
//! query --kind <kind>` (s22 `query-tier-degradation`) — this module
//! only names the boundary, it never crosses it.

use std::path::Path;

use canon_model::envelope::RecordKind;
use canon_store::policy::TierPolicy;

/// `canon.yaml`'s fixed on-disk location relative to a repo root — the
/// SAME file `canon_cli::report::resolve_roots` already reads for the
/// `local` rung's `root` (s25 design D3), read here directly
/// rather than threaded through `ReportInputs` (s25 design D3's
/// accepted shape (ii): `canon-report` reads `canon.yaml` itself).
const CANON_YAML_RELATIVE_PATH: &str = "canon.yaml";

/// The set of [`RecordKind`]s `<repo_root>/canon.yaml`'s `routing`
/// table sends to a rung whose configured backend is NOT
/// `read_directly_by_report()` (s28 design D2) — sorted ascending by
/// [`RecordKind::as_str`] (never enum declaration order or `HashMap`
/// iteration order, the byte-stability property `crate::render::render`
/// depends on). Empty when `canon.yaml` is absent, unreadable, or
/// fails to parse as a [`TierPolicy`] (module doc: fail-soft, never a
/// panic/`Err`). A routed rung with no `tiers.<rung>` entry at all is
/// treated as not-read-directly (never assumed reachable) — the
/// same conservative-exclusion posture as an unattached rung anywhere
/// else in this codebase.
pub fn kinds_not_read_directly(repo_root: &Path) -> Vec<RecordKind> {
    let Ok(text) = std::fs::read_to_string(repo_root.join(CANON_YAML_RELATIVE_PATH)) else {
        return Vec::new();
    };
    let Ok(policy) = TierPolicy::from_yaml(&text) else {
        return Vec::new();
    };
    let mut kinds: Vec<RecordKind> = policy
        .routing
        .iter()
        .filter(|(_, rung)| match policy.tiers.get(rung) {
            Some(cfg) => !cfg.backend().read_directly_by_report(),
            None => true,
        })
        .map(|(kind, _)| *kind)
        .collect();
    kinds.sort_by_key(|k| k.as_str());
    kinds
}

/// The sentence both [`render_note`] and [`warn_line`] share verbatim
/// (module doc: one derivation, never two hand-authored copies whose
/// wording could drift apart independently). s28 design D3: truthful
/// WITH OR WITHOUT a local `.canon/r2` mirror — never an absolute "not
/// reflected" claim.
const ESCAPE_HATCH_SENTENCE: &str = "canon report reads its local roots directly (the git ledger + local `.canon/r2` + `.canon/learn` parquet); the kinds below route to a backend whose own store it does not read (a live database or object-store bucket), so their data appears only if materialized into the local report roots — it may be incomplete or stale. Read them live with `canon query --kind <kind>`.";

/// `## Kinds not read directly` — `None` when `kinds` is empty (design
/// D2: an empty set renders NOTHING at all, not an empty section, so
/// every existing git-only fixture's rendered output stays byte-
/// identical to before this change). Callers pass an already-sorted
/// `kinds` ([`kinds_not_read_directly`]'s own contract) — this
/// function never re-sorts.
pub fn render_note(kinds: &[RecordKind]) -> Option<String> {
    if kinds.is_empty() {
        return None;
    }
    let mut out = String::new();
    out.push_str("## Kinds not read directly\n\n");
    out.push_str(&format!("> {ESCAPE_HATCH_SENTENCE}\n\n"));
    for kind in kinds {
        out.push_str(&format!("- `{}`\n", kind.as_str()));
    }
    out.push('\n');
    Some(out)
}

/// One stderr line naming the same `kinds` [`render_note`] names — no
/// `canon report: WARN ` prefix (the CLI caller,
/// `canon-cli::main::run_report`, adds that itself, matching every
/// other stderr line's own prefix convention in that function). `None`
/// when `kinds` is empty — silent for a repo with nothing routed to a
/// not-directly-read backend.
pub fn warn_line(kinds: &[RecordKind]) -> Option<String> {
    if kinds.is_empty() {
        return None;
    }
    let names: Vec<&str> = kinds.iter().map(|k| k.as_str()).collect();
    Some(format!("{ESCAPE_HATCH_SENTENCE} Not read directly: {}.", names.join(", ")))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write_canon_yaml(dir: &Path, text: &str) {
        std::fs::write(dir.join("canon.yaml"), text).unwrap();
    }

    #[test]
    fn no_canon_yaml_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        assert!(kinds_not_read_directly(dir.path()).is_empty());
    }

    #[test]
    fn malformed_canon_yaml_is_empty_never_a_panic() {
        let dir = tempfile::tempdir().unwrap();
        write_canon_yaml(dir.path(), "routing:\n  not_a_real_kind: hot\n");
        assert!(kinds_not_read_directly(dir.path()).is_empty());
    }

    #[test]
    fn all_directly_read_routing_is_empty() {
        let dir = tempfile::tempdir().unwrap();
        write_canon_yaml(
            dir.path(),
            "tiers:\n  local: { backend: git, root: .canon/ledger }\nrouting:\n  task: local\n  change: local\n",
        );
        assert!(kinds_not_read_directly(dir.path()).is_empty());
    }

    #[test]
    fn multi_tier_routing_returns_the_correct_sorted_set() {
        let dir = tempfile::tempdir().unwrap();
        write_canon_yaml(
            dir.path(),
            "tiers:\n  local: { backend: git, root: .canon/ledger }\n  hot: { backend: postgres, dsn_env: CANON_PG_DSN, schema: canon_v1 }\nrouting:\n  task: hot\n  change: local\n  session: hot\n  event: hot\n",
        );
        let kinds = kinds_not_read_directly(dir.path());
        assert_eq!(kinds.iter().map(|k| k.as_str()).collect::<Vec<_>>(), vec!["event", "session", "task"]);
    }

    /// s28 `rung-backend-capability` spec scenario: a `cold` rung
    /// backed by `s3` (today's class-correct pairing, s28 design D1)
    /// now APPEARS here — s27's `Backend::offline_file_readable()`
    /// wrongly excluded S3, treating it as report-visible when
    /// `canon-report` never actually opens the live bucket directly.
    #[test]
    fn a_cold_rung_backed_by_s3_now_appears_in_kinds_not_read_directly() {
        let dir = tempfile::tempdir().unwrap();
        write_canon_yaml(
            dir.path(),
            "tiers:\n  cold: { backend: s3, bucket_env: CANON_R2_BUCKET_S3_TB, prefix: \"canon/\" }\nrouting:\n  trajectory: cold\n",
        );
        let kinds = kinds_not_read_directly(dir.path());
        assert_eq!(kinds, vec![RecordKind::Trajectory], "an s3-backed cold rung's own live bucket is never opened directly by canon report");
    }

    /// s28 spec: a routed rung with NO `tiers.<rung>` block at all is
    /// treated as not-read-directly (excluded from the marts, named
    /// here) — never assumed reachable.
    #[test]
    fn an_unconfigured_routed_rung_is_treated_as_not_read_directly() {
        let dir = tempfile::tempdir().unwrap();
        write_canon_yaml(dir.path(), "routing:\n  task: hot\n");
        let kinds = kinds_not_read_directly(dir.path());
        assert_eq!(kinds, vec![RecordKind::Task], "an unconfigured rung must never be assumed read-directly");
    }

    #[test]
    fn render_note_and_warn_line_name_the_identical_sorted_kinds() {
        let kinds = vec![RecordKind::Event, RecordKind::Session, RecordKind::Task];
        let note = render_note(&kinds).unwrap();
        let warn = warn_line(&kinds).unwrap();
        for kind in &kinds {
            assert!(note.contains(&format!("`{}`", kind.as_str())));
            assert!(warn.contains(kind.as_str()));
        }
    }

    #[test]
    fn empty_kinds_render_nothing() {
        assert!(render_note(&[]).is_none());
        assert!(warn_line(&[]).is_none());
    }

    #[test]
    fn two_calls_over_an_unchanged_file_are_equal() {
        let dir = tempfile::tempdir().unwrap();
        write_canon_yaml(dir.path(), "tiers:\n  hot: { backend: postgres, dsn_env: CANON_PG_DSN, schema: canon_v1 }\nrouting:\n  task: hot\n");
        assert_eq!(kinds_not_read_directly(dir.path()), kinds_not_read_directly(dir.path()));
    }
}
