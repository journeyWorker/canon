//! `canon gate selftest` (design D9 / D17's DO-330 tool-qualification
//! rationale, the donor parity-harness audit's fixtures-selftest notes):
//! the fixture corpus that proves THIS crate's own gate is trustworthy —
//! one fixture per [`crate::FAILURE_CLASSES`] entry, each pairing a
//! deliberately-broken corpus with a checked-in `expected_failures.txt`
//! oracle (`<failure-class> <subject>`, `#`-comments allowed — the exact
//! plain-text format `tools/parity.py`'s own fixtures use). [`run`]
//! diffs ACTUAL vs EXPECTED for every fixture, exact-set-match (both
//! `missing` — under-detection — and `extra` — over-triggering, per the
//! audit's own "extra half is the important, easy-to-omit half" warning)
//! — never a one-sided "still catches the known-bad case" assertion.
//!
//! # `GateCtx`-pattern rebindable roots, corpus built in Rust
//! Every fixture binds every root into one fresh directory exactly like
//! [`crate::GateCtx::from_fixture`]'s own doc describes (the direct port
//! of parity.py's `fixture_ctx(fx)` — audit §3.1). Unlike parity.py's
//! literal, hand-authored JSON fixture files, `canon_store::GitTier`
//! derives each record's own on-disk filename from a content digest
//! (`canon_store::partition::content_digest12`) — hand-authoring a
//! literal fixture JSON file at exactly that path would either drift the
//! moment the digest algorithm changes, or need re-deriving by hand for
//! every edit. Each fixture's CORPUS is therefore built by ordinary Rust
//! code calling the exact same `GitTier::write`/`RawWrite` production
//! path every other test in this crate already uses (`ledger.rs`,
//! `staleness.rs`, … own `#[cfg(test)]` modules); only the
//! EXPECTED-violations oracle (`crates/canon-gate/fixtures/<class>/
//! expected_failures.txt`) is a literal, checked-in, hand-authored file —
//! the one artifact a reviewer needs to read to know what a fixture's
//! REQUIRED behavior is, embedded here via `include_str!` (mirroring
//! `hooks::PRE_COMMIT_SCRIPT`'s own "embedded verbatim ... so the two can
//! never drift" discipline) so `canon gate selftest` works identically
//! whether run via `cargo test -p canon-gate` or the installed binary,
//! with no runtime dependency on this repo's own checkout layout.
//!
//! # Two fixture shapes, one oracle format
//! Six of the eight classes (`uncovered-cell`, `unreviewed-promotion`,
//! `trust-below-required`, `stale-evidence`, `malformed-evidence`,
//! `flagged`) are reachable through the assembled [`crate::dispatch::check_set`]
//! over a loaded [`crate::GateContext`] — `canon gate check`'s own
//! dispatcher, engaged here with `release: true` so the release-scoped
//! `trust-below-required` class can fire too (this crate's OWN corpus
//! proving every class fires, unlike an ordinary non-release `canon gate
//! check` run, spec.md "does not block ordinary (non-release)
//! evaluation"). The remaining two (`unevidenced-flip`,
//! `fabricated-evidence`) are `gated-task-completion`'s own territory —
//! never a registered [`crate::GateCheck`] at all (`checkbox::gate_task`/
//! `markers::scan_fake_markers` are pure functions over a `tasks.md`
//! document, not over a `GateContext`) — so their fixtures build a
//! `(document, task_id, notes)` triple instead of relying on the
//! assembled check set. Both shapes reduce to the identical `(FailureClass,
//! String)` pair oracle (`Violation::pair()`) before comparison, so
//! [`run`]'s diff logic is exactly one function regardless of which path
//! produced a fixture's violations.
//!
//! # No `tempfile` dependency added to this crate's production graph
//! [`ScratchDir`] is a minimal, standard-library-only scratch-directory
//! helper (unique-suffixed subdir under `std::env::temp_dir()`, deleted
//! best-effort on drop) rather than pulling `tempfile` (today a
//! dev-dependency only) into this crate's regular dependency graph just
//! for `canon gate selftest`'s runtime corpus scratch space — this
//! change's allowed-edit-root excludes the workspace root
//! `Cargo.toml`/`Cargo.lock`.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use canon_model::{Actor, Envelope, EvidenceRecord, EvidenceVerdict, FlaggedOverlay, ProjectId, ProvenanceRef, RecordKind, Review, RoleId, ScenarioId, Sha, TaskId, TrustLifecycle};
use canon_policy::SchemaRegistry;
use canon_store::git_tier::GitTier;
use canon_store::tier::{RawWrite, Tier};
use chrono::{DateTime, Utc};

use crate::checkbox::gate_task;
use crate::context::{GateContext, GateCtx, DEFAULT_LEDGER_RELATIVE_PATH};
use crate::dispatch::check_set;
use crate::failure_class::FailureClass;
use crate::markers::EvidenceNote;

/// A scratch directory for one selftest fixture, deleted best-effort on
/// drop (module doc — no `tempfile` dependency).
struct ScratchDir(PathBuf);

impl ScratchDir {
    fn new(label: &str) -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("canon-gate-selftest-{label}-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&path).expect("create canon-gate selftest scratch dir");
        Self(path)
    }

    fn path(&self) -> &Path {
        &self.0
    }
}

impl Drop for ScratchDir {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.0);
    }
}

fn ledger_root(dir: &Path) -> PathBuf {
    dir.join(DEFAULT_LEDGER_RELATIVE_PATH)
}

fn envelope(role: &str) -> Envelope {
    Envelope::new(1, RecordKind::EvidenceRecord, Utc::now(), Actor::new("selftest-agent", RoleId::parse(role).expect("selftest role is a valid RoleId")))
}

fn write_policy_yaml(dir: &Path, contents: &str) {
    let canon_dir = dir.join(".canon");
    std::fs::create_dir_all(&canon_dir).expect("create fixture .canon/ dir");
    std::fs::write(canon_dir.join("policy.yaml"), contents).expect("write fixture policy.yaml");
}

/// Insert an extra raw-JSON companion key (`class` — the ONE field
/// `ReleaseTrustCheck` still reads raw, s15 P3b's `trust`/`promote`
/// module docs) into one already-serialized `EvidenceRecord` body and
/// write it through `RawWrite` — `lifecycle`/`flagged`/`evidence_sha`/
/// `surface_ref` are now `EvidenceRecord`'s own native, typed fields
/// (`.with_lifecycle`/`.with_flagged`/`.with_evidence_sha`/
/// `.with_surface_ref`) and no longer need this raw-injection path.
fn write_evidence_with(ledger_root: &Path, record: &EvidenceRecord, extra: &[(&str, serde_json::Value)]) {
    let mut body = serde_json::to_value(record).expect("EvidenceRecord always serializes");
    let obj = body.as_object_mut().expect("EvidenceRecord serializes to a JSON object");
    for (key, value) in extra {
        obj.insert((*key).to_string(), value.clone());
    }
    GitTier::new(ledger_root).write(&RawWrite(canon_model::RawRecord(body))).expect("write selftest fixture evidence record");
}

fn git(dir: &Path, args: &[&str]) {
    let status = std::process::Command::new("git").arg("-C").arg(dir).args(args).status().expect("git must be on PATH for the stale-evidence selftest fixture");
    assert!(status.success(), "git {args:?} failed while building the stale-evidence selftest fixture");
}

fn git_output(dir: &Path, args: &[&str]) -> String {
    let output = std::process::Command::new("git").arg("-C").arg(dir).args(args).output().expect("git must be on PATH for the stale-evidence selftest fixture");
    assert!(output.status.success(), "git {args:?} failed while building the stale-evidence selftest fixture");
    String::from_utf8(output.stdout).expect("git output is UTF-8")
}

// ── the eight fixture corpus builders ──

fn build_uncovered_cell(dir: &Path) {
    write_policy_yaml(dir, "risk_routing:\n  reviewer: true\n");
    let task_id = TaskId::parse("s5-selftest-uncovered-cell#1").expect("valid TaskId");
    let record = EvidenceRecord::new(envelope("implementer"), Some(task_id), None, None, EvidenceVerdict::Faithful);
    GitTier::new(ledger_root(dir)).write(&record).expect("write uncovered-cell fixture evidence");
}

fn build_unreviewed_promotion(dir: &Path) {
    let task_id = TaskId::parse("s5-selftest-unreviewed-promotion#1").expect("valid TaskId");
    let record = EvidenceRecord::new(envelope("implementer"), Some(task_id), None, None, EvidenceVerdict::Faithful).with_lifecycle(TrustLifecycle::Reviewed);
    GitTier::new(ledger_root(dir)).write(&record).expect("write unreviewed-promotion fixture evidence");
}

fn build_trust_below_required(dir: &Path) {
    write_policy_yaml(dir, "trust_required:\n  p1: human\n");
    let task_id = TaskId::parse("s5-selftest-trust-below-required#1").expect("valid TaskId");
    let scenario_id = ScenarioId::parse("selftest.trust-below-required.01").expect("valid ScenarioId");
    let record = EvidenceRecord::new(envelope("implementer"), Some(task_id), Some(scenario_id.clone()), None, EvidenceVerdict::Faithful).with_lifecycle(TrustLifecycle::Reviewed);
    let ledger_root = ledger_root(dir);
    // `class` is the one field still a raw companion, never migrated
    // (module doc) — `lifecycle` is the native field above.
    write_evidence_with(&ledger_root, &record, &[("class", serde_json::json!("p1"))]);

    let review = Review::new(
        Envelope::new(1, RecordKind::Review, Utc::now(), Actor::new("selftest-reviewer", RoleId::parse("reviewer").expect("valid RoleId"))),
        ProjectId::parse("root").unwrap(),
        scenario_id,
        "selftest-reviewer",
        "abc123abc123",
        ProvenanceRef::UpstreamRef("selftest#trust-below-required-fixture".to_string()),
    );
    GitTier::new(&ledger_root).write(&review).expect("write trust-below-required fixture review record");
}

fn build_stale_evidence(dir: &Path) {
    git(dir, &["init", "-q"]);
    git(dir, &["config", "user.email", "selftest@example.com"]);
    git(dir, &["config", "user.name", "canon-gate-selftest"]);
    std::fs::write(dir.join("surface.txt"), "one\n").expect("write fixture surface file");
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", "initial"]);
    let sha = git_output(dir, &["rev-parse", "HEAD"]).trim().to_string();

    let task_id = TaskId::parse("s5-selftest-stale-evidence#1").expect("valid TaskId");
    let record = EvidenceRecord::new(envelope("implementer"), Some(task_id), None, None, EvidenceVerdict::Faithful)
        .with_evidence_sha(Sha::parse(sha).expect("git rev-parse HEAD is a valid Sha"))
        .with_surface_ref(vec!["surface.txt".to_string()]);
    GitTier::new(ledger_root(dir)).write(&record).expect("write stale-evidence fixture evidence");

    // Move HEAD past `evidence_sha` by touching the DECLARED surface —
    // the ledger evidence file above is never `git add`-ed, so this
    // commit's diff is exactly the surface change.
    std::fs::write(dir.join("surface.txt"), "two\n").expect("modify fixture surface file");
    git(dir, &["add", "-A"]);
    git(dir, &["commit", "-q", "-m", "surface changed"]);
}

fn build_malformed_evidence(dir: &Path) {
    let path = ledger_root(dir).join("kind=evidence_record").join("broken.json");
    std::fs::create_dir_all(path.parent().expect("has a parent dir")).expect("create fixture ledger dir");
    std::fs::write(&path, b"{ this is not valid json").expect("write malformed fixture file");
}

fn build_flagged(dir: &Path) {
    let task_id = TaskId::parse("s5-selftest-flagged#1").expect("valid TaskId");
    let flagged_by = Actor::new("human-operator", RoleId::parse("human").expect("valid RoleId"));
    let record = EvidenceRecord::new(envelope("implementer"), Some(task_id), None, None, EvidenceVerdict::Faithful)
        .with_lifecycle(TrustLifecycle::Ratified)
        .with_flagged(FlaggedOverlay::set(flagged_by, Utc::now()));
    GitTier::new(ledger_root(dir)).write(&record).expect("write flagged fixture evidence");
}

fn build_unevidenced_flip(_dir: &Path) -> (String, TaskId, Vec<EvidenceNote>) {
    let task_id = TaskId::parse("s5-selftest-unevidenced-flip#1").expect("valid TaskId");
    ("- [ ] 1 Do the selftest thing\n".to_string(), task_id, Vec::new())
}

fn build_fabricated_evidence(dir: &Path) -> (String, TaskId, Vec<EvidenceNote>) {
    let task_id = TaskId::parse("s5-selftest-fabricated-evidence#1").expect("valid TaskId");
    let record = EvidenceRecord::new(envelope("implementer"), Some(task_id.clone()), None, None, EvidenceVerdict::Faithful);
    GitTier::new(ledger_root(dir)).write(&record).expect("write fabricated-evidence fixture evidence");
    let note = EvidenceNote::new(task_id.clone(), "TBD", None);
    ("- [ ] 1 Do the selftest thing\n".to_string(), task_id, vec![note])
}

// ── runners: reduce one fixture's build to its actual `(class, subject)` set ──

fn run_check_fixture(build: fn(&Path), now: DateTime<Utc>) -> BTreeSet<(FailureClass, String)> {
    let scratch = ScratchDir::new("check");
    build(scratch.path());
    let ctx = GateCtx::from_fixture(scratch.path());
    let registry = SchemaRegistry::load();
    let gate_context = GateContext::load(ctx, &registry, now).expect("selftest fixture ledger must load");
    check_set(true).iter().flat_map(|check| check.run(&gate_context)).map(|v| v.pair()).collect()
}

fn run_taskflip_fixture(build: fn(&Path) -> (String, TaskId, Vec<EvidenceNote>), now: DateTime<Utc>) -> BTreeSet<(FailureClass, String)> {
    let scratch = ScratchDir::new("taskflip");
    let (_document, task_id, notes) = build(scratch.path());
    let ctx = GateCtx::from_fixture(scratch.path());
    let registry = SchemaRegistry::load();
    let gate_context = GateContext::load(ctx, &registry, now).expect("selftest fixture ledger must load");
    // s35: `gate_task` is the pure evidence DECISION — no document. The
    // fixture's `document` field is retained for shape parity with the
    // pre-s35 fixtures but is now the write-back layer's concern, never
    // this crate's; the selftest oracle only cares about the blocked
    // violations the decision emits.
    match gate_task(&task_id, &gate_context.evidence, &notes) {
        crate::checkbox::TaskFlipDecision::Blocked { violations } => violations.into_iter().map(|v| v.pair()).collect(),
        crate::checkbox::TaskFlipDecision::Approved { .. } => BTreeSet::new(),
    }
}


/// One fixture's identity, corpus builder, and checked-in EXPECTED
/// oracle (module doc).
struct Fixture {
    class: FailureClass,
    run: fn(DateTime<Utc>) -> BTreeSet<(FailureClass, String)>,
    expected: &'static str,
}

fn run_uncovered_cell(now: DateTime<Utc>) -> BTreeSet<(FailureClass, String)> {
    run_check_fixture(build_uncovered_cell, now)
}
fn run_unreviewed_promotion(now: DateTime<Utc>) -> BTreeSet<(FailureClass, String)> {
    run_check_fixture(build_unreviewed_promotion, now)
}
fn run_trust_below_required(now: DateTime<Utc>) -> BTreeSet<(FailureClass, String)> {
    run_check_fixture(build_trust_below_required, now)
}
fn run_stale_evidence(now: DateTime<Utc>) -> BTreeSet<(FailureClass, String)> {
    run_check_fixture(build_stale_evidence, now)
}
fn run_malformed_evidence(now: DateTime<Utc>) -> BTreeSet<(FailureClass, String)> {
    run_check_fixture(build_malformed_evidence, now)
}
fn run_flagged(now: DateTime<Utc>) -> BTreeSet<(FailureClass, String)> {
    run_check_fixture(build_flagged, now)
}
fn run_unevidenced_flip(now: DateTime<Utc>) -> BTreeSet<(FailureClass, String)> {
    run_taskflip_fixture(build_unevidenced_flip, now)
}
fn run_fabricated_evidence(now: DateTime<Utc>) -> BTreeSet<(FailureClass, String)> {
    run_taskflip_fixture(build_fabricated_evidence, now)
}

fn fixtures() -> Vec<Fixture> {
    vec![
        Fixture { class: FailureClass::UncoveredCell, run: run_uncovered_cell, expected: include_str!("../fixtures/uncovered-cell/expected_failures.txt") },
        Fixture { class: FailureClass::UnreviewedPromotion, run: run_unreviewed_promotion, expected: include_str!("../fixtures/unreviewed-promotion/expected_failures.txt") },
        Fixture { class: FailureClass::TrustBelowRequired, run: run_trust_below_required, expected: include_str!("../fixtures/trust-below-required/expected_failures.txt") },
        Fixture { class: FailureClass::StaleEvidence, run: run_stale_evidence, expected: include_str!("../fixtures/stale-evidence/expected_failures.txt") },
        Fixture { class: FailureClass::MalformedEvidence, run: run_malformed_evidence, expected: include_str!("../fixtures/malformed-evidence/expected_failures.txt") },
        Fixture { class: FailureClass::Flagged, run: run_flagged, expected: include_str!("../fixtures/flagged/expected_failures.txt") },
        Fixture { class: FailureClass::UnevidencedFlip, run: run_unevidenced_flip, expected: include_str!("../fixtures/unevidenced-flip/expected_failures.txt") },
        Fixture { class: FailureClass::FabricatedEvidence, run: run_fabricated_evidence, expected: include_str!("../fixtures/fabricated-evidence/expected_failures.txt") },
    ]
}

/// Parse `expected_failures.txt`'s 2-column `<failure-class> <subject>`
/// format (module doc; `tools/parity.py::_parse_expected`'s exact
/// grammar) — blank lines and `#`-comment lines skipped. Returns `Err`
/// naming the first malformed non-comment line (unsplittable, or an
/// unknown failure-class column) instead of silently filtering it out —
/// a fixture's checked-in EXPECTED file carrying a typo'd/unknown extra
/// line must never exact-set-match a dirty corpus just because its
/// well-formed pairs happen to agree with `actual`; that would let a
/// broken oracle pass as clean.
fn parse_expected(text: &str) -> Result<BTreeSet<(FailureClass, String)>, String> {
    let mut set = BTreeSet::new();
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with('#')) {
        let (class_str, subject) = line.split_once(' ').ok_or_else(|| format!("malformed EXPECTED line (expected `<failure-class> <subject>`): {line:?}"))?;
        let class = FailureClass::from_str_exact(class_str).ok_or_else(|| format!("malformed EXPECTED line (unknown failure class {class_str:?}): {line:?}"))?;
        set.insert((class, subject.trim().to_string()));
    }
    Ok(set)
}

/// One fixture's diff outcome — both halves of parity.py's own
/// exact-set-match oracle (module doc): `missing` (under-detection,
/// expected but not produced) and `extra` (over-triggering, produced but
/// not expected).
pub struct FixtureOutcome {
    pub class: FailureClass,
    pub missing: BTreeSet<(FailureClass, String)>,
    pub extra: BTreeSet<(FailureClass, String)>,
    /// Set when this fixture's checked-in `expected_failures.txt` itself
    /// failed to parse (`parse_expected`'s own doc) — a malformed oracle
    /// is a dirty outcome on its own, independent of `missing`/`extra`,
    /// which both stay empty in this case (there is no `expected` set to
    /// diff against).
    pub expected_parse_error: Option<String>,
}

impl FixtureOutcome {
    pub fn is_clean(&self) -> bool {
        self.expected_parse_error.is_none() && self.missing.is_empty() && self.extra.is_empty()
    }
}

/// Every fixture's outcome from one `canon gate selftest` run.
pub struct SelftestReport {
    pub outcomes: Vec<FixtureOutcome>,
}

impl SelftestReport {
    pub fn is_clean(&self) -> bool {
        self.outcomes.iter().all(FixtureOutcome::is_clean)
    }

    /// parity.py's own two-way exit-code contract, reused (`GateReport::exit_code`'s
    /// identical shape): `0` clean, `1` on any fixture mismatch.
    pub fn exit_code(&self) -> i32 {
        if self.is_clean() {
            0
        } else {
            1
        }
    }

    pub fn format_human(&self) -> String {
        let mut out = String::new();
        for outcome in &self.outcomes {
            if outcome.is_clean() {
                out.push_str(&format!("ok    {}\n", outcome.class.as_str()));
                continue;
            }
            out.push_str(&format!("FAIL  {}\n", outcome.class.as_str()));
            if let Some(err) = &outcome.expected_parse_error {
                out.push_str(&format!("  malformed EXPECTED file: {err}\n"));
            }
            for (class, subject) in &outcome.missing {
                out.push_str(&format!("  missing (under-detection): {} {}\n", class.as_str(), subject));
            }
            for (class, subject) in &outcome.extra {
                out.push_str(&format!("  extra (over-triggering):   {} {}\n", class.as_str(), subject));
            }
        }
        out
    }
}

/// Run every fixture (module doc) — the ONE entry point both `cargo test
/// -p canon-gate` (below) and `canon gate selftest` (canon-cli) call.
/// `Utc::now()` is called exactly ONCE here (s21 `deterministic-gate-clock`
/// D6, mirroring `canon-cli/src/gate.rs`'s dispatch-boundary idiom) and
/// threaded into every fixture's `GateContext::load` — no fixture, and
/// no `GateCheck` it exercises, ever reads the wall clock itself.
pub fn run() -> SelftestReport {
    let now = Utc::now();
    let outcomes = fixtures()
        .into_iter()
        .map(|fixture| {
            let actual = (fixture.run)(now);
            match parse_expected(fixture.expected) {
                Ok(expected) => {
                    let missing = expected.difference(&actual).cloned().collect();
                    let extra = actual.difference(&expected).cloned().collect();
                    FixtureOutcome { class: fixture.class, missing, extra, expected_parse_error: None }
                }
                Err(err) => FixtureOutcome { class: fixture.class, missing: BTreeSet::new(), extra: BTreeSet::new(), expected_parse_error: Some(err) },
            }
        })
        .collect();
    SelftestReport { outcomes }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Tests lock this (assignment acceptance criterion): every one of
    /// the eight `FAILURE_CLASSES` strings fires on its own fixture,
    /// exactly (missing AND extra both empty, every fixture).
    #[test]
    fn every_failure_class_fires_exactly_on_its_own_fixture() {
        let report = run();
        assert_eq!(report.outcomes.len(), crate::FAILURE_CLASSES.len(), "one fixture per FAILURE_CLASSES entry");
        assert!(report.is_clean(), "{}", report.format_human());
        assert_eq!(report.exit_code(), 0);
    }

    /// The fixture table itself covers every `FailureClass::ALL` variant
    /// exactly once — a structural check independent of the corpus
    /// content above, so a future ninth class added to `FAILURE_CLASSES`
    /// without a matching fixture fails loudly here rather than being
    /// silently under-covered.
    #[test]
    fn fixture_table_covers_every_failure_class_exactly_once() {
        let classes: Vec<FailureClass> = fixtures().iter().map(|f| f.class).collect();
        let unique: BTreeSet<FailureClass> = classes.iter().copied().collect();
        assert_eq!(classes.len(), unique.len(), "no failure class fixtured twice");
        assert_eq!(unique, FailureClass::ALL.into_iter().collect(), "every FailureClass::ALL member has exactly one fixture");
    }

    /// A regressed `expected_failures.txt` (a class omitted, or an
    /// invented extra class the corpus never produces) must fail the
    /// selftest, matching spec.md's own "selftest fails when a fixture's
    /// expectations regress" scenario.
    #[test]
    fn a_mismatched_expected_set_produces_a_dirty_report() {
        let actual: BTreeSet<(FailureClass, String)> = run_check_fixture(build_uncovered_cell, Utc::now());
        let wrong_expected = parse_expected("uncovered-cell some-other-subject\n").expect("well-formed EXPECTED text parses");
        let missing: BTreeSet<_> = wrong_expected.difference(&actual).cloned().collect();
        let extra: BTreeSet<_> = actual.difference(&wrong_expected).cloned().collect();
        assert!(!missing.is_empty() || !extra.is_empty(), "a deliberately wrong expected set must not diff clean");
    }

    /// The bug this fix closes (assignment): a fixture whose EXPECTED
    /// file is otherwise byte-for-byte correct, but carries ONE extra
    /// malformed/unknown-class line, must never silently exact-set-match
    /// `actual` just because the well-formed pairs happen to agree —
    /// `parse_expected` must reject the whole file (not filter the line
    /// out), and the resulting `FixtureOutcome` (mirroring `run()`'s own
    /// `Err` branch) must report failing, never clean.
    #[test]
    fn a_malformed_extra_expected_line_fails_the_fixture_instead_of_silently_matching() {
        let actual = run_check_fixture(build_uncovered_cell, Utc::now());
        let clean_text = include_str!("../fixtures/uncovered-cell/expected_failures.txt");
        let clean_expected = parse_expected(clean_text).expect("the shipped EXPECTED file is well-formed");
        assert_eq!(clean_expected, actual, "precondition: the real fixture's EXPECTED set matches actual exactly");

        let dirty_text = format!("{clean_text}\nnot-a-real-class some-typo-subject\n");
        let err = parse_expected(&dirty_text).expect_err("an unknown failure-class column must be rejected, never silently dropped");
        assert!(err.contains("not-a-real-class"), "the error must name the offending line: {err}");

        // Mirror `run()`'s own `Err` branch to prove the reported outcome is dirty.
        let outcome = FixtureOutcome { class: FailureClass::UncoveredCell, missing: BTreeSet::new(), extra: BTreeSet::new(), expected_parse_error: Some(err) };
        assert!(!outcome.is_clean(), "a malformed EXPECTED file must never report as a clean fixture");
    }
}
