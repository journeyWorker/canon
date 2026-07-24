//! The staleness check (design decision 4/D4/A3): a GREEN evidence
//! record degrades to `stale-evidence` once the surface it covers has
//! moved out from under it. Direct port of `tools/parity.py`'s
//! two-tier `_is_stale` (the donor parity-harness audit's staleness notes
//! §3.1) — surface-scoped git-diff SHORT-CIRCUITS to
//! stale when the record declares a surface ref; the
//! `max_commits_behind` ceiling ALWAYS applies underneath it as a
//! backstop, never bypassed even when the precise diff comes back
//! clean (staleness.md §4's own "Silent-downgrade risk" warning: "if
//! `surface_scoped` is on and the diff returns `False` ..., execution
//! still falls through to the `max_commits_behind` check below it ...
//! easy to misport as 'if `port_files` exist, skip the ceiling
//! entirely' — canon's reimplementation must preserve the ... control
//! flow").
//!
//! # s15 P3b: native `evidence_sha` / `surface_ref`, read off `ctx.evidence`
//! `evidence_sha`/`surface_ref` used to be an interim, canon-gate-owned
//! raw-JSON companion this module independently re-scanned the ledger
//! for (`crate::trust_ladder`'s own migration note named the identical
//! move for `lifecycle`/`flagged`). s15 P1 moved both onto
//! `canon_model::EvidenceRecord` natively — `evidence_sha:
//! Option<Sha>`, `surface_ref: Vec<String>` (the one `Vec` exception:
//! defaults to empty when absent rather than `None`, design D9/
//! `gate-native-record-fields` spec) — so this module now reads them
//! as plain field accesses on the already-typed `ctx.evidence`, never
//! a second `GitTier` construction. Absent `evidence_sha` (`None`)
//! still means the SAME "unresolvable" third state as before (neither
//! git-diff tier has a starting ref) — this check silently skips such
//! a record (stays at whatever verdict [`crate::ledger`] already
//! reported), never manufacturing a `stale-evidence` violation from an
//! absent signal; a present-malformed `evidence_sha`/`surface_ref`
//! never reaches `ctx.evidence` at all (it already failed
//! [`crate::context::GateContext::load`]'s deserialize and lives in
//! `ctx.violations`, surfaced by `crate::ledger::LedgerCheck`).
//!
//! # Only the LATEST verdict per cell is ever checked
//! Staleness only ever DEGRADES a passing verdict (spec.md "Staleness
//! detection": "degrade a PASSING evidence record to stale") — a
//! `Divergent`/`NotApplicable` cell is already not green;
//! [`crate::ledger::LedgerEntry::is_green`] is the exact predicate
//! this module's own fold mirrors, applied to the WINNER of a
//! last-wins-by-`at` fold ([`fold_latest_green_cells`]'s own doc) —
//! never to any individual historical record in isolation. An older
//! `Faithful` record superseded by a newer non-green record for the
//! SAME (subject, role) cell is not green either: the ledger's
//! current answer for that cell is whatever its LATEST record says,
//! exactly [`crate::ledger::latest_verdicts`]'s own read model.

use std::collections::HashSet;
use std::path::Path;
use std::process::Command;

use canon_model::{EvidenceRecord, EvidenceVerdict, Sha};
use canon_store::fold_latest_by_key;
use chrono::{DateTime, Utc};

use crate::context::{GateCheck, GateContext};
use crate::coverage::CellSubject;
use crate::failure_class::{FailureClass, Violation};
use crate::policy::{DEFAULT_MAX_COMMITS_BEHIND, DEFAULT_SURFACE_SCOPED};

/// One (subject, role) cell's LATEST record's native staleness
/// signals, read directly off `ctx.evidence` (module doc) — no longer
/// a separate companion type. `raw` is the record's own
/// `serde_json::to_value` — the exact [`crate::PolicyResolution`] CEL
/// binding shape (`policy.rs` module doc), reused as-is for
/// `surface_scoped`/`max_commits_behind` evaluation. Only ever
/// constructed for a cell whose LATEST record's verdict is `Faithful`
/// — [`fold_latest_green_cells`] filters AFTER, never BEFORE, its
/// last-wins fold.
struct GreenCell {
    subject: String,
    evidence_sha: Option<Sha>,
    surface_ref: Vec<String>,
    raw: serde_json::Value,
}

/// One `ctx.evidence` record folded to its (subject, role) cell — the
/// input [`fold_latest_by_key`] (design D11/s21 D3) folds to the
/// greatest-`(at, digest)` winner per cell, mirroring
/// [`crate::ledger::latest_verdicts`]'s identical shape and total,
/// machine-independent tie-break.
struct Candidate<'a> {
    subject: String,
    role: Option<String>,
    at: DateTime<Utc>,
    digest: String,
    record: &'a EvidenceRecord,
}

/// Folds `ctx.evidence` to the LATEST record per (subject, role) cell
/// FIRST — the exact last-wins-by-`(at, digest)` discipline
/// [`crate::ledger::latest_verdicts`] applies, via the SAME hoisted
/// [`fold_latest_by_key`] (design D11/s21 D3) — THEN keeps only the
/// cells whose WINNING record's verdict is `Faithful`. Filtering out
/// non-`Faithful` records BEFORE the fold, instead of after, would let
/// an OLDER green record survive here even after a NEWER
/// `Divergent`/`NotApplicable` record for the SAME cell already
/// superseded it in the ledger's own last-wins read model — staleness
/// must only ever degrade the verdict the ledger ACTUALLY reports as
/// current for that cell, never a stale winner of its own that the
/// real ledger no longer agrees with (module doc's "Only the LATEST
/// verdict per cell is ever checked"). `ctx.evidence` already excludes
/// every content-malformed record (module doc) — no separate skip
/// logic needed here.
fn fold_latest_green_cells(ctx: &GateContext) -> Vec<GreenCell> {
    let candidates = ctx.evidence.iter().filter_map(|record| {
        let subject = CellSubject::of(record)?;
        let role = record.envelope.actor.role.as_ref().map(|r| r.as_str().to_string());
        let digest = canon_store::partition::content_digest12(&serde_json::to_value(record).unwrap_or_default());
        Some(Candidate { subject: subject.as_str().to_string(), role, at: record.envelope.at, digest, record })
    });

    fold_latest_by_key(candidates, |c| (c.subject.clone(), c.role.clone()), |c| c.at, |c| c.digest.as_str())
        .into_values()
        .filter(|c| matches!(c.record.verdict, EvidenceVerdict::Faithful))
        .map(|c| GreenCell {
            subject: c.subject,
            evidence_sha: c.record.evidence_sha.clone(),
            surface_ref: c.record.surface_ref.clone(),
            raw: serde_json::to_value(c.record).unwrap_or(serde_json::Value::Null),
        })
        .collect()
}

/// Memoized `git` subprocess boundary (staleness.md §3.3: "5k+ ledger
/// records share ~15 unique `app_sha`s ... one fork per unique query,
/// not per ledger row") — process-lifetime-per-`run()`-call, keyed by
/// the exact `(subcommand, sha)` pair; swallows every failure to
/// `None`, never panics (module doc's "unresolvable" third state).
#[derive(Default)]
struct GitMemo {
    changed: std::collections::HashMap<String, Option<HashSet<String>>>,
    behind: std::collections::HashMap<String, Option<u32>>,
}

impl GitMemo {
    fn changed_files_since(&mut self, repo: &Path, sha: &str) -> Option<HashSet<String>> {
        self.changed.entry(sha.to_string()).or_insert_with(|| git_diff_names(repo, sha)).clone()
    }

    fn commits_behind(&mut self, repo: &Path, sha: &str) -> Option<u32> {
        *self.behind.entry(sha.to_string()).or_insert_with(|| git_rev_list_count(repo, sha))
    }
}

/// The ONE subprocess boundary this module ever crosses — swallows
/// every failure (missing binary, unresolvable sha, non-zero exit,
/// invalid UTF-8) into `None`, never raises (staleness.md §3.3/`_git`;
/// module doc's "unresolvable" third state).
fn run_git(repo: &Path, args: &[&str]) -> Option<String> {
    let output = Command::new("git").arg("-C").arg(repo).args(args).output().ok()?;
    if !output.status.success() {
        return None;
    }
    String::from_utf8(output.stdout).ok()
}

/// The precise tier: `git diff --name-only --no-renames <sha>..HEAD`
/// (staleness.md §3.1/§6, `_changed_files_since` +
/// `_git_diff_touches`).
fn git_diff_names(repo: &Path, sha: &str) -> Option<HashSet<String>> {
    let range = format!("{sha}..HEAD");
    let out = run_git(repo, &["diff", "--name-only", "--no-renames", &range])?;
    Some(out.lines().map(str::to_string).collect())
}

/// The coarse tier: `git rev-list --count <sha>..HEAD` (staleness.md
/// §3.1, the `max_commits_behind` ceiling's own git query).
fn git_rev_list_count(repo: &Path, sha: &str) -> Option<u32> {
    let range = format!("{sha}..HEAD");
    let out = run_git(repo, &["rev-list", "--count", &range])?;
    out.trim().parse().ok()
}

/// The staleness [`crate::GateCheck`] (D4) — see module doc for the
/// two-tier decision and the native `evidence_sha`/`surface_ref`
/// fields.
pub struct StalenessCheck;

impl GateCheck for StalenessCheck {
    fn name(&self) -> &'static str {
        "staleness"
    }

    fn run(&self, ctx: &GateContext) -> Vec<Violation> {
        let mut memo = GitMemo::default();
        let now = ctx.now;
        let mut violations = Vec::new();

        for cell in fold_latest_green_cells(ctx) {
            // No `evidence_sha` at all: neither tier has a starting
            // ref to query. Unresolvable, never assumed stale (module
            // doc).
            let Some(sha) = cell.evidence_sha.as_ref() else { continue };

            let mut reasons: Vec<String> = Vec::new();

            let surface_scoped = ctx.policy.surface_scoped(&cell.raw, now).unwrap_or(DEFAULT_SURFACE_SCOPED);
            if surface_scoped && !cell.surface_ref.is_empty() {
                if let Some(changed) = memo.changed_files_since(&ctx.ctx.repo, sha.as_str()) {
                    if cell.surface_ref.iter().any(|declared| changed.contains(declared)) {
                        reasons.push(format!("a declared surface file changed since {sha}"));
                    }
                }
                // `None` (unresolvable diff query) contributes nothing
                // to this tier — never assumed stale.
            }

            // The ceiling is an UNCONDITIONAL backstop (module doc) —
            // it always runs, even when the precise diff above came
            // back clean.
            if let Some(commits_behind) = memo.commits_behind(&ctx.ctx.repo, sha.as_str()) {
                let ceiling = ctx.policy.max_commits_behind(&cell.raw, now).unwrap_or(DEFAULT_MAX_COMMITS_BEHIND);
                if commits_behind > ceiling {
                    reasons.push(format!("{commits_behind} commits behind HEAD (max_commits_behind={ceiling})"));
                }
            }

            if !reasons.is_empty() {
                violations.push(Violation::new(FailureClass::StaleEvidence, cell.subject.clone(), reasons.join("; ")));
            }
        }

        violations
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use canon_model::{Actor, Envelope, RawRecord, RecordKind, RoleId, TaskId};
    use canon_store::git_tier::GitTier;
    use canon_store::tier::{RawWrite, Tier, TierQuery};
    use tempfile::TempDir;

    use super::*;
    use canon_policy::SchemaRegistry;
    use crate::context::GateCtx;
    use crate::policy::{PolicyField, PolicyResolution, StalenessPolicy};

    fn git(repo: &Path, args: &[&str]) {
        let status = Command::new("git").arg("-C").arg(repo).args(args).status().expect("git must be on PATH for this test");
        assert!(status.success(), "git {args:?} failed");
    }

    fn init_repo(repo: &Path) {
        git(repo, &["init", "-q"]);
        git(repo, &["config", "user.email", "test@example.com"]);
        git(repo, &["config", "user.name", "test"]);
    }

    fn write_and_commit(repo: &Path, relative: &str, content: &str, message: &str) -> String {
        let path = repo.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content).unwrap();
        git(repo, &["add", "-A"]);
        git(repo, &["commit", "-q", "-m", message]);
        run_git(repo, &["rev-parse", "HEAD"]).unwrap().trim().to_string()
    }

    /// Writes one green `EvidenceRecord` with its NATIVE
    /// `evidence_sha`/`surface_ref` fields set (s15 P3b — no longer a
    /// raw-JSON companion, module doc) into `ledger_root`, through the
    /// ordinary typed write path.
    fn write_green_record(ledger_root: &Path, task: &str, role: &str, evidence_sha: &str, surface_ref: &[&str]) {
        let envelope = Envelope::new(1, RecordKind::EvidenceRecord, Utc::now(), Actor::new("agent", RoleId::parse(role).unwrap()));
        let record = EvidenceRecord::new(envelope, Some(TaskId::parse(task).unwrap()), None, None, EvidenceVerdict::Faithful)
            .with_evidence_sha(Sha::parse(evidence_sha).unwrap())
            .with_surface_ref(surface_ref.iter().map(|s| s.to_string()).collect());
        GitTier::new(ledger_root).write(&record).expect("write evidence record");
    }

    /// Same as [`write_green_record`] but with an explicit `verdict`
    /// and `at` timestamp — needed to construct two records for the
    /// SAME (subject, role) cell where the caller controls which one
    /// is "latest" by `at`, rather than relying on `Utc::now()`'s
    /// natural ordering across two calls a few nanoseconds apart.
    fn write_record_at(ledger_root: &Path, task: &str, role: &str, verdict: EvidenceVerdict, at: DateTime<Utc>, evidence_sha: &str, surface_ref: &[&str]) {
        let envelope = Envelope::new(1, RecordKind::EvidenceRecord, at, Actor::new("agent", RoleId::parse(role).unwrap()));
        let record = EvidenceRecord::new(envelope, Some(TaskId::parse(task).unwrap()), None, None, verdict)
            .with_evidence_sha(Sha::parse(evidence_sha).unwrap())
            .with_surface_ref(surface_ref.iter().map(|s| s.to_string()).collect());
        GitTier::new(ledger_root).write(&record).expect("write evidence record");
    }

    fn policy_with(max_commits_behind: u32, surface_scoped: bool) -> PolicyResolution {
        PolicyResolution {
            trust_required: Default::default(),
            trust_sample: Default::default(),
            staleness: StalenessPolicy { max_commits_behind: PolicyField::Flat(max_commits_behind), surface_scoped: PolicyField::Flat(surface_scoped) },
            risk_routing: Default::default(),
            diagnostics: Vec::new(),
        }
    }

    /// A named, fixed UTC constant (s21 design.md R5: never
    /// `Utc::now()` in a test call site of `GateContext::load`) — none
    /// of this module's existing tests exercise a time-bearing CEL
    /// policy (`policy_with` builds `Flat`-only fields), so any fixed
    /// instant is equally valid here.
    fn fixed_now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z").unwrap().with_timezone(&Utc)
    }

    /// Builds a `GateContext` around a caller-supplied `policy`
    /// (bypassing a real `policy.yaml`) — `evidence`/`violations` are
    /// loaded for real off `ledger_root`, exactly as
    /// `GateContext::load` itself would (s15 P3b: `StalenessCheck` now
    /// reads `ctx.evidence` directly, so this fixture must actually
    /// populate it, never leave it empty).
    fn ctx_for(repo: &Path, ledger_root: &Path, policy: PolicyResolution, now: DateTime<Utc>) -> GateContext {
        let read = GitTier::new(ledger_root).read(&TierQuery::kind(RecordKind::EvidenceRecord)).unwrap_or_default();
        let (evidence, _validation_violations) = canon_model::validate_evidence_batch(&read.records);
        GateContext { ctx: GateCtx { repo: repo.to_path_buf(), ledger_root: ledger_root.to_path_buf() }, policy, evidence, violations: read.violations, now }
    }

    #[test]
    fn stale_evidence_fires_when_a_declared_surface_file_changes_after_the_evidence_sha() {
        let dir = TempDir::new().unwrap();
        let repo = dir.path();
        init_repo(repo);
        let sha = write_and_commit(repo, "crates/foo/src/bar.rs", "fn bar() {}", "initial");

        let ledger_root = repo.join(".canon/ledger");
        write_green_record(&ledger_root, "s5#1.7", "implementer", &sha, &["crates/foo/src/bar.rs"]);

        // Move HEAD by touching the DECLARED surface file.
        write_and_commit(repo, "crates/foo/src/bar.rs", "fn bar() { /* changed */ }", "surface changed");

        let ctx = ctx_for(repo, &ledger_root, policy_with(50, true), fixed_now());
        let violations = StalenessCheck.run(&ctx);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].class, FailureClass::StaleEvidence);
        assert_eq!(violations[0].subject, "s5#1.7");
        assert!(violations[0].detail.contains("declared surface file changed"));
    }

    #[test]
    fn not_stale_when_declared_surfaces_are_untouched_and_well_under_the_ceiling() {
        let dir = TempDir::new().unwrap();
        let repo = dir.path();
        init_repo(repo);
        let sha = write_and_commit(repo, "crates/foo/src/bar.rs", "fn bar() {}", "initial");

        let ledger_root = repo.join(".canon/ledger");
        write_green_record(&ledger_root, "s5#1.7", "implementer", &sha, &["crates/foo/src/bar.rs"]);

        // Move HEAD via an UNRELATED file — the declared surface never changes.
        write_and_commit(repo, "crates/other/src/unrelated.rs", "fn unrelated() {}", "unrelated change");

        let ctx = ctx_for(repo, &ledger_root, policy_with(50, true), fixed_now());
        assert!(StalenessCheck.run(&ctx).is_empty());
    }

    #[test]
    fn ceiling_applies_even_when_the_surface_scoped_diff_is_precisely_clean() {
        // staleness.md §4: the ceiling is an UNCONDITIONAL backstop —
        // never skipped just because the precise tier proved the
        // declared files untouched.
        let dir = TempDir::new().unwrap();
        let repo = dir.path();
        init_repo(repo);
        let sha = write_and_commit(repo, "crates/foo/src/bar.rs", "fn bar() {}", "initial");

        let ledger_root = repo.join(".canon/ledger");
        write_green_record(&ledger_root, "s5#1.7", "implementer", &sha, &["crates/foo/src/bar.rs"]);

        write_and_commit(repo, "crates/other/src/unrelated.rs", "fn unrelated() {}", "unrelated change 1");
        write_and_commit(repo, "crates/other/src/unrelated2.rs", "fn unrelated() {}", "unrelated change 2");

        // Ceiling of 1: two unrelated commits already exceed it, even
        // though the declared surface file is untouched.
        let ctx = ctx_for(repo, &ledger_root, policy_with(1, true), fixed_now());
        let violations = StalenessCheck.run(&ctx);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].detail.contains("commits behind HEAD"));
    }

    #[test]
    fn max_commits_behind_ceiling_fires_with_no_declared_surface_ref() {
        let dir = TempDir::new().unwrap();
        let repo = dir.path();
        init_repo(repo);
        let sha = write_and_commit(repo, "README.md", "hello", "initial");

        let ledger_root = repo.join(".canon/ledger");
        write_green_record(&ledger_root, "s5#1.7", "implementer", &sha, &[]);

        write_and_commit(repo, "README.md", "hello again", "second");
        write_and_commit(repo, "README.md", "hello thrice", "third");

        let ctx = ctx_for(repo, &ledger_root, policy_with(1, true), fixed_now());
        let violations = StalenessCheck.run(&ctx);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].detail.contains("commits behind HEAD"));
    }

    #[test]
    fn unresolvable_evidence_sha_is_never_assumed_stale() {
        let dir = TempDir::new().unwrap();
        let repo = dir.path();
        init_repo(repo);
        write_and_commit(repo, "README.md", "hello", "initial");

        let ledger_root = repo.join(".canon/ledger");
        // Well-formed 40-hex sha grammar, but it never existed in this repo's history.
        let fake_sha = "0".repeat(40);
        write_green_record(&ledger_root, "s5#1.7", "implementer", &fake_sha, &["README.md"]);

        let ctx = ctx_for(repo, &ledger_root, policy_with(50, true), fixed_now());
        assert!(StalenessCheck.run(&ctx).is_empty(), "an unresolvable sha must never be treated as stale");
    }

    #[test]
    fn a_record_with_no_evidence_sha_is_never_assumed_stale() {
        let dir = TempDir::new().unwrap();
        let repo = dir.path();
        init_repo(repo);
        write_and_commit(repo, "README.md", "hello", "initial");

        let ledger_root = repo.join(".canon/ledger");
        let envelope = Envelope::new(1, RecordKind::EvidenceRecord, Utc::now(), Actor::new("agent", RoleId::parse("implementer").unwrap()));
        let record = EvidenceRecord::new(envelope, Some(TaskId::parse("s5#1.7").unwrap()), None, None, EvidenceVerdict::Faithful);
        let value = serde_json::to_value(&record).unwrap();
        GitTier::new(&ledger_root).write(&RawWrite(RawRecord(value))).unwrap();

        let ctx = ctx_for(repo, &ledger_root, policy_with(0, true), fixed_now());
        assert!(StalenessCheck.run(&ctx).is_empty(), "no evidence_sha at all means neither tier can be evaluated");
    }

    #[test]
    fn a_non_green_verdict_is_never_checked_for_staleness() {
        let dir = TempDir::new().unwrap();
        let repo = dir.path();
        init_repo(repo);
        let sha = write_and_commit(repo, "crates/foo/src/bar.rs", "fn bar() {}", "initial");

        let ledger_root = repo.join(".canon/ledger");
        let envelope = Envelope::new(1, RecordKind::EvidenceRecord, Utc::now(), Actor::new("agent", RoleId::parse("implementer").unwrap()));
        let record = EvidenceRecord::new(envelope, Some(TaskId::parse("s5#1.7").unwrap()), None, None, EvidenceVerdict::Divergent);
        let mut value = serde_json::to_value(&record).unwrap();
        value.as_object_mut().unwrap().insert("evidence_sha".to_string(), serde_json::json!(sha));
        value.as_object_mut().unwrap().insert("surface_ref".to_string(), serde_json::json!(["crates/foo/src/bar.rs"]));
        GitTier::new(&ledger_root).write(&RawWrite(RawRecord(value))).unwrap();

        write_and_commit(repo, "crates/foo/src/bar.rs", "fn bar() { /* changed */ }", "surface changed");

        let ctx = ctx_for(repo, &ledger_root, policy_with(0, true), fixed_now());
        assert!(StalenessCheck.run(&ctx).is_empty(), "staleness only ever degrades an ALREADY-green record");
    }

    #[test]
    fn a_newer_non_green_latest_verdict_suppresses_staleness_for_an_older_superseded_green_record() {
        // Reordering regression: `fold_latest_green_cells` must fold
        // to the LATEST record per cell FIRST, THEN test verdict —
        // never filter to `Faithful` records before the last-wins
        // fold, or an OLD green record would survive here even after
        // a NEWER `Divergent` record for the SAME cell already
        // demoted it in the ledger's own last-wins read model.
        let dir = TempDir::new().unwrap();
        let repo = dir.path();
        init_repo(repo);
        let sha = write_and_commit(repo, "crates/foo/src/bar.rs", "fn bar() {}", "initial");

        let ledger_root = repo.join(".canon/ledger");
        let old_at = Utc::now() - chrono::Duration::hours(1);
        let new_at = Utc::now();

        // OLD Faithful record for this cell...
        write_record_at(&ledger_root, "s5#1.7", "implementer", EvidenceVerdict::Faithful, old_at, &sha, &["crates/foo/src/bar.rs"]);
        // ...superseded by a NEWER Divergent record for the SAME
        // (subject, role) cell — last-wins makes this the cell's
        // current answer, and it is not green.
        write_record_at(&ledger_root, "s5#1.7", "implementer", EvidenceVerdict::Divergent, new_at, &sha, &["crates/foo/src/bar.rs"]);

        // Move HEAD by touching the declared surface file — would
        // trip staleness for a genuinely green cell.
        write_and_commit(repo, "crates/foo/src/bar.rs", "fn bar() { /* changed */ }", "surface changed");

        let ctx = ctx_for(repo, &ledger_root, policy_with(50, true), fixed_now());
        assert!(
            StalenessCheck.run(&ctx).is_empty(),
            "the newer non-green latest verdict must suppress staleness for the older superseded green record"
        );
    }

    // ── s21 `deterministic-gate-clock`: a real `policy.yaml` carrying
    // an `age_days(...)` CEL predicate, loaded through the REAL
    // `GateContext::load` (never `ctx_for`'s hand-built
    // `PolicyResolution`, since the whole point here is that the CEL
    // predicate is compiled from an on-disk `policy.yaml`) at an
    // explicitly injected `now` — task 6. `evidence_at`/the two
    // injected-now helpers are EXPLICIT UTC constants (design.md R5:
    // never `Utc::now() - Duration::days(N)` computed at test-run
    // time), so this test's own expected verdict is reproducible.

    fn evidence_at() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z").unwrap().with_timezone(&Utc)
    }

    /// Age = 1 day: below the fixture's `age_days(record.at) > 5` threshold.
    fn now_before_threshold() -> DateTime<Utc> {
        evidence_at() + chrono::Duration::days(1)
    }

    /// Age = 10 days: above the threshold.
    fn now_after_threshold() -> DateTime<Utc> {
        evidence_at() + chrono::Duration::days(10)
    }

    /// A git repo with ONE green evidence record (fixed `at`,
    /// `evidence_sha` = the repo's initial commit, declaring the
    /// surface file as its own) followed by a SECOND commit that
    /// changes that exact surface file — plus a `.canon/policy.yaml`
    /// whose `staleness.surface_scoped` is a CEL predicate over
    /// `age_days(record.at)`. `surface_scoped` resolving `false`
    /// (age below threshold) means the surface-changed reason never
    /// even gets checked; resolving `true` (age above threshold)
    /// means it does, and the declared surface DID change since the
    /// evidence sha — the one violation this fixture is built to fire.
    fn time_bearing_fixture() -> (TempDir, PathBuf) {
        let dir = TempDir::new().unwrap();
        let repo = dir.path().to_path_buf();
        init_repo(&repo);
        let sha = write_and_commit(&repo, "crates/foo/src/bar.rs", "fn bar() {}", "initial");

        let ledger_root = repo.join(crate::context::DEFAULT_LEDGER_RELATIVE_PATH);
        write_record_at(&ledger_root, "s5#1.7", "implementer", EvidenceVerdict::Faithful, evidence_at(), &sha, &["crates/foo/src/bar.rs"]);

        write_and_commit(&repo, "crates/foo/src/bar.rs", "fn bar() { /* changed */ }", "surface changed");

        std::fs::create_dir_all(repo.join(".canon")).unwrap();
        std::fs::write(repo.join(".canon").join("policy.yaml"), "staleness:\n  surface_scoped:\n    cel: \"age_days(record.at) > 5\"\n").unwrap();

        (dir, repo)
    }

    #[test]
    fn time_bearing_policy_two_loads_at_the_same_injected_now_produce_byte_identical_reports() {
        let (_dir, repo) = time_bearing_fixture();
        let registry = SchemaRegistry::load();
        let now = now_after_threshold();

        let ctx1 = GateContext::load(GateCtx::from_repo(&repo), &registry, now).unwrap();
        assert!(ctx1.policy.is_clean(), "diagnostics: {:?}", ctx1.policy.diagnostics);
        let report1 = StalenessCheck.run(&ctx1);
        assert!(!report1.is_empty(), "sanity: this fixture at now_after_threshold must actually fire a violation, or this test proves nothing");

        let ctx2 = GateContext::load(GateCtx::from_repo(&repo), &registry, now).unwrap();
        let report2 = StalenessCheck.run(&ctx2);

        assert_eq!(report1, report2, "two independent GateContext::load calls at the SAME injected `now` must produce byte-identical reports");
    }

    #[test]
    fn time_bearing_policy_repeated_evaluation_at_a_fixed_now_never_drifts() {
        let (_dir, repo) = time_bearing_fixture();
        let registry = SchemaRegistry::load();
        let now = now_after_threshold();

        let reports: Vec<_> = (0..3).map(|_| StalenessCheck.run(&GateContext::load(GateCtx::from_repo(&repo), &registry, now).unwrap())).collect();
        assert_eq!(reports[0], reports[1], "run 1 vs run 2 must agree");
        assert_eq!(reports[1], reports[2], "run 2 vs run 3 must agree — no run-to-run drift at a fixed now");
    }

    #[test]
    fn time_bearing_policy_verdict_flips_purely_by_advancing_the_injected_now() {
        let (_dir, repo) = time_bearing_fixture();
        let registry = SchemaRegistry::load();

        let ctx_before = GateContext::load(GateCtx::from_repo(&repo), &registry, now_before_threshold()).unwrap();
        assert!(ctx_before.policy.is_clean(), "diagnostics: {:?}", ctx_before.policy.diagnostics);
        assert!(
            StalenessCheck.run(&ctx_before).is_empty(),
            "age below the age_days threshold: surface_scoped resolves false, the surface-changed reason is never checked"
        );

        let ctx_after = GateContext::load(GateCtx::from_repo(&repo), &registry, now_after_threshold()).unwrap();
        let violations = StalenessCheck.run(&ctx_after);
        assert_eq!(violations.len(), 1, "age above the age_days threshold: surface_scoped resolves true and the declared surface changed since the evidence sha");
        assert_eq!(violations[0].class, FailureClass::StaleEvidence);
        assert_eq!(violations[0].subject, "s5#1.7");
    }
}
