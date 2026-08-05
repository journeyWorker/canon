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
    ///
    /// It is a fingerprint of the DIGESTED INPUTS, not of the exported
    /// tables. On the git tier it covers the eight core kinds
    /// `crate::digest`'s `digest_side` assigns to a digest —
    /// `change`/`task`/`scenario`/`subject` (corpus) and
    /// `review`/`divergence`/`evidence_record`/`finding` (ledger head)
    /// — PLUS every namespaced overlay kind that tier holds, whatever
    /// it is named, discovered from the tier's own `kind=`
    /// directories rather than from any list
    /// (`crate::digest`'s `overlay_kind_dirs`). It excludes exactly six
    /// core kinds: `session`, `run`, `event`, `handoff`, `trajectory`,
    /// `strategy_item`.
    ///
    /// So a snapshot whose `mart_session_costs`/`mart_role_memory`/
    /// `mart_flywheel_funnel` rows moved — those are built from the
    /// excluded six — can carry an UNCHANGED `source_digest`. A
    /// snapshot whose `mart_scope_status` rows moved cannot, over
    /// VALIDATED core records and overlay inputs: that mart's inputs
    /// are `Scenario` (its driving side since s45), `Task.scenario_refs`
    /// and the folded `task` rows behind `mart_trust_matrix` (both
    /// covered core kinds), and `porting.coverage` (a namespaced
    /// overlay) — all of them digested.
    ///
    /// The ONE exception belongs right here and not four paragraphs
    /// down, because a guarantee whose counterexample sits further
    /// along the same file misleads everyone who stops reading at the
    /// guarantee. A file under a CORE `kind=<k>/` directory whose BODY
    /// `GitTier::read` refuses is absent from the digest and present
    /// in the mart anyway: a `kind=task` body carrying `kind`,
    /// `task_id`, `title`, `status` and `scenario_refs` but no `actor`
    /// fails `canon_store::partition::validate_body`, so the validated
    /// read this digest is computed from never sees it — while
    /// `stg_git_records`' glob hands it to `int_task_scenario_refs`
    /// and `mart_scope_status` grows a row. Pinned executably, both
    /// halves, by `crates/canon-report/tests/core_body_residual.rs`.
    ///
    /// The exception is one-directional: this value can UNDER-cover a
    /// mart that moved, never claim provenance over a corpus it did
    /// not read. Read it as "which authored corpus and which verdict
    /// ledger produced this", never as "these bytes are unchanged".
    ///
    /// # The standing asymmetry: Rust validates, DuckDB globs
    /// That exception is one instance of a permanent property of this
    /// system, stated here AS a property because rediscovering it case
    /// by case is what produced this finding — and the digest-rung
    /// residual in `crates/canon-store/sql/views.sql`'s header before
    /// it. Canon has TWO readers over one corpus. The Rust side
    /// VALIDATES: `GitTier::scan_kind_where` checks a file's body
    /// `kind`, then its content-resolved layout, then its full schema,
    /// and soft-skips whatever fails as a violation. The SQL side
    /// GLOBS: `read_text('kind=*/**/*.json')` parses JSON and asks
    /// nothing else. Validation is strictly stronger than a glob, so
    /// over the corpus the validated read REFUSES, anything derived
    /// from that read (this digest, `canon query`, `canon gate check`)
    /// and anything derived from the glob (every `mart_*`) are free to
    /// disagree.
    ///
    /// POSITION on the malformed-but-globbable file, recorded so the
    /// question is answered once instead of re-argued every round:
    /// this is CORPUS HYGIENE, not a digest defect, and it is
    /// deliberately NOT closed here. Three reasons.
    ///
    /// - Byte-hashing the core half — the symmetric-looking fix, since
    ///   [`crate::digest`]'s overlay half already hashes bytes —
    ///   trades a false claim for a false alarm. The core half hashes
    ///   the canonical re-serialization of the PARSED record, which is
    ///   exactly as whitespace-insensitive as DuckDB's own JSON parse,
    ///   so the two agree on every reformat of a valid file. Byte-
    ///   based, `source_digest` would move on a whitespace-only
    ///   reformat while every exported table stood still. The overlay
    ///   half hashes bytes because it has no schema to normalize
    ///   through and no validated read in its path; that difference
    ///   between the halves is deliberate, not an inconsistency to be
    ///   tidied away.
    /// - A refused file is not a record. This value answers "which
    ///   authored corpus produced this snapshot"; hashing content no
    ///   canon reader will admit redefines it as "the bytes under the
    ///   git tier" — a different and much weaker claim.
    /// - Covering it would HIDE it. A digest that moved for a
    ///   `GitTier::read`-refused file would let a corpus canon cannot
    ///   read ship a green report under fresh provenance. Failing is
    ///   the only outcome that surfaces the file to whoever authored
    ///   it.
    ///
    /// Where it DOES belong: a gate that refuses the file. Nothing
    /// blocks one today — `canon query --kind task` reports it as
    /// `(N violation(s) reported, excluded)` and still exits zero, and
    /// `canon gate check` never sees it at all, because
    /// `canon_gate::context::GateContext::load` reads only
    /// `RecordKind::EvidenceRecord`, so `LedgerCheck`'s
    /// `malformed-evidence` surfacing covers the ledger kind alone.
    /// Widening that load to every core kind is the closure, and it is
    /// canon-gate's change rather than this crate's: a report can only
    /// ever describe the corpus it was handed.
    ///
    /// Both past holes are on the record rather than left to be
    /// rediscovered. `Finding`/`Subject` sat outside the covered set
    /// until s43 round 2 (finding 8), because the covered set was a
    /// hand-kept array. The ENTIRE namespaced-overlay namespace sat
    /// outside it until s43 round 3 (finding 1), because that array's
    /// replacement was a `match` over a closed enum and the `kind=`
    /// namespace is open.
    ///
    /// What is still NOT hashed, and so could move an exported table
    /// under an unchanged `source_digest`:
    ///
    /// - report inputs that are not git-tier record files at all —
    ///   `canon.yaml` itself, and the r2/parquet tier that
    ///   `stg_r2_records` unions into `stg_records` (this crate
    ///   digests the git tier only, by design: `crate::digest`'s
    ///   module doc);
    /// - the refused-core-body exception, stated in full beside the
    ///   guarantee above rather than first disclosed down here.
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
