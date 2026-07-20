//! `canon selftest`'s 9th suite (spec-ledger-selftest, s15 P5, tasks
//! 6.1): fixture corpora proving inventory-sync's own violation
//! detection is trustworthy, plus one "frozen-incident" fold fixture —
//! mirroring [`canon_gate::selftest`]'s two-sided exact-set-match
//! discipline exactly (`Fixture`/`FixtureOutcome`/`parse_expected`/
//! `include_str!`-embedded oracle files), generalized from that crate's
//! closed 8-`FailureClass` domain to inventory-sync's open `<tag>
//! <subject>` violation space (`fmt:<FmtFailureClass>` from S11,
//! `sync:duplicate-scenario` from D5's per-root abort, `fold:*`/
//! `fold-live:*` from the frozen-incident fixture below) — the SAME
//! two-sided (`missing` AND `extra`) diff, the SAME "a malformed
//! EXPECTED file is itself a dirty outcome" rule.
//!
//! # Every fixture runs through [`crate::inventory::SyncCtx::from_fixture`]
//! (spec Req 2's "fixture constructor runs fully offline against a
//! tempdir" scenario): a fresh [`ScratchDir`] per fixture, no network,
//! no dependency on this repo's own checkout layout — mirroring
//! `canon-gate`'s own `ScratchDir` (module doc: no `tempfile` dependency
//! added to this crate's production graph; `canon-cli`'s own `tempfile`
//! entry is `[dev-dependencies]`-only, and this module runs inside the
//! release `canon` binary via `canon selftest`, not only under `cargo
//! test`). Every sync fixture calls [`crate::inventory::run_sync_with_ctx`]
//! — the SAME downstream entry point [`crate::inventory::run_sync`]
//! (production) calls, never a second, selftest-only sync path.
//!
//! # The frozen-incident fixture (spec Req 3)
//! Pins a REAL past divergence-fold case, not a synthetic one:
//! a donor parity-harness divergence log (§3.3)
//! documents a real backfilled donor run file
//! (`8-1-87c1fc57-1c26f4.jsonl`, scenario `world.firstbuy-hotdeal.26`,
//! **round 8**, `run_seq: 1`, status `open`) that a later fresh review
//! campaign (`3-3-8c81f9e1-2ea830.jsonl`, **round 3**, `run_seq: 3`,
//! status `resolved`) correctly outranks — "the numerically *smaller*
//! round (3 < 8) carries the numerically *greater* run_seq (3 > 1) and
//! correctly wins the fold... A naive 'sort by round' fold would
//! silently resurrect a stale `open` divergence over a genuinely
//! `resolved` one." [`frozen_incident_divergences`] reproduces these
//! exact two records (real `app_sha`/`round`/`run_seq`/`status`/
//! `reviewer`/timestamp values); [`run_frozen_incident`] folds them
//! through the REAL [`canon_model::fold_to_current_state`] twice — once
//! with no live-binding re-check supplied (trusted as-is: `resolved`)
//! and once with a live-binding snapshot showing the app moved off the
//! resolved `sha` (design D8's resolved-then-invalidated re-check:
//! `resolved-invalid`) — covering BOTH example cases spec Req 3 names
//! ("a real `run_seq`/`round` ordering, or a resolved-then-invalidated
//! binding") in one pinned corpus. `tests::a_round_primary_ordering_regression_would_flip_the_frozen_incident_winner`
//! is the inline proof this fixture is actually DISCRIMINATING (spec
//! Req 3 Scenario 2): a `(round, run_seq)`-primary comparator picks a
//! DIFFERENT winner (`open`, round 8) than the real `run_seq`-primary
//! `fold_to_current_state` (`resolved`, run_seq 3) — so a future
//! regression to non-`run_seq`-primary ranking would change
//! [`run_frozen_incident`]'s actual output and break this fixture's
//! checked-in oracle.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use canon_model::ids::TotalOrder;
use canon_model::{Actor, BindingSnapshot, Divergence, DivergenceStatus, Envelope, FoldedState, ProjectId, RecordKind, RoleId, ScenarioId, Sha, fold_to_current_state};
use chrono::{DateTime, Utc};

use crate::inventory::{RootSyncOutcome, SyncCtx, run_sync_with_ctx};

/// A scratch directory for one selftest fixture, deleted best-effort on
/// drop — module doc's "no `tempfile` dependency" discipline, ported
/// verbatim from `canon_gate::selftest::ScratchDir`.
struct ScratchDir(PathBuf);

impl ScratchDir {
    fn new(label: &str) -> Self {
        use std::sync::atomic::{AtomicU64, Ordering};
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let n = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("canon-cli-inventory-selftest-{label}-{}-{n}", std::process::id()));
        std::fs::create_dir_all(&path).expect("create canon-cli inventory selftest scratch dir");
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

fn write(dir: &Path, rel: &str, content: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().expect("fixture file path has a parent dir")).expect("create fixture corpus dir");
    std::fs::write(path, content).expect("write fixture corpus file");
}

fn provenance_comment() -> String {
    "  # canon: {\"schema\":1,\"at\":\"2026-07-10T00:00:00Z\",\"actor\":{\"agent_id\":\"canon-fmt\"}}".to_string()
}

/// A single well-formed `.feature` file under `<root>/features/`, Hive-
/// laid-out (`kind=feature/area=<area>/<surface>.feature`) with a
/// provenance comment on both headers, so `canon-fmt::check` reports
/// zero violations for it (mirrors `crate::inventory`'s own
/// `#[cfg(test)]` helper of the identical name/shape).
fn write_clean_feature(spec_root: &Path, area: &str, surface: &str, nn: &str, title: &str) {
    let prov = provenance_comment();
    let text = format!("Feature: {area} {surface}\n{prov}\n\n  @{area}.{surface}.{nn}\n  Scenario: {title}\n{prov}\n    Given a precondition\n");
    write(spec_root, &format!("features/kind=feature/area={area}/{surface}.feature"), &text);
}

// ── the three sync fixture corpus builders (spec Req 1) ──

/// A clean root: one well-formed scenario, zero violations, exactly one
/// `Scenario` record materialized.
fn build_clean_root(dir: &Path) {
    write_clean_feature(&dir.join("specs"), "world", "hotdeal", "01", "Opening the hotdeal overlay");
}

/// An S11-violation root: both headers lack a provenance comment —
/// `canon-fmt::check` reports exactly one `missing-provenance`
/// violation (Hive-laid-out so no incidental `layout-grammar` violation
/// also fires), aborting the whole root (0 writes).
fn build_missing_provenance(dir: &Path) {
    write(
        &dir.join("specs"),
        "features/kind=feature/area=world/hotdeal.feature",
        "Feature: world hotdeal\n\n  @world.hotdeal.01\n  Scenario: Opening the hotdeal overlay\n    Given a precondition\n",
    );
}

/// A duplicate-scenario root: two well-formed features declaring the
/// SAME `scenario_id` — passes S11 `canon-fmt::check` (never dedups
/// scenario_ids) but violates D5's one-record-per-key contract, so this
/// root aborts (0 writes) via `RootSyncOutcome.sync_errors`, never a
/// frozen `FmtFailureClass` violation (mirrors
/// `crate::inventory`'s own `a_duplicate_scenario_id_in_one_root_aborts_that_root_but_siblings_continue`
/// test corpus).
fn build_duplicate_scenario(dir: &Path) {
    let prov = provenance_comment();
    let root = dir.join("specs");
    write(
        &root,
        "features/kind=feature/area=world/hotdeal.feature",
        &format!("Feature: world hotdeal\n{prov}\n\n  @world.hotdeal.01\n  Scenario: First\n{prov}\n    Given x\n"),
    );
    write(
        &root,
        "features/kind=feature/area=world/hotdeal-again.feature",
        &format!("Feature: world hotdeal again\n{prov}\n\n  @world.hotdeal.01\n  Scenario: Second\n{prov}\n    Given y\n"),
    );
}

/// Build a fixture dir, sync it through [`SyncCtx::from_fixture`] +
/// [`run_sync_with_ctx`] (the SAME entry point production `canon
/// inventory sync` uses), and return its single default root's outcome.
fn run_root(build: fn(&Path)) -> RootSyncOutcome {
    let scratch = ScratchDir::new("inventory");
    build(scratch.path());
    let ctx = SyncCtx::from_fixture(scratch.path());
    let outcome = run_sync_with_ctx(&ctx, None).expect("selftest fixture must sync without a config error");
    outcome.roots.into_iter().next().expect("SyncCtx::from_fixture always resolves exactly one default root")
}

/// Reduce a [`RootSyncOutcome`] to its `(tag, subject)` violation set —
/// the actual set every sync fixture's oracle is diffed against
/// (module doc): `fmt:<class>` for each S11 `canon-fmt::check`
/// violation (subject = its git-tier-relative path, mirroring
/// `canon-gate`'s own path-as-subject convention — never the free-text
/// `detail`, which is not a stable identifier), `sync:duplicate-scenario`
/// for each D5 per-root abort (subject = the exact deterministic
/// message `sync_one_root` produces for this fixture's fixed inputs).
fn outcome_pairs(outcome: &RootSyncOutcome) -> BTreeSet<(String, String)> {
    let mut set = BTreeSet::new();
    for v in &outcome.violations {
        set.insert((format!("fmt:{}", v.class.as_str()), v.path.display().to_string()));
    }
    for e in &outcome.sync_errors {
        set.insert(("sync:duplicate-scenario".to_string(), e.clone()));
    }
    set
}

fn run_clean_root() -> BTreeSet<(String, String)> {
    outcome_pairs(&run_root(build_clean_root))
}
fn run_missing_provenance() -> BTreeSet<(String, String)> {
    outcome_pairs(&run_root(build_missing_provenance))
}
fn run_duplicate_scenario() -> BTreeSet<(String, String)> {
    outcome_pairs(&run_root(build_duplicate_scenario))
}

// ── the frozen-incident fold fixture (spec Req 3) ──

fn frozen_project_id() -> ProjectId {
    ProjectId::parse("root").expect("literal ProjectId")
}

/// The real donor `scenario_id` the pinned incident occurred on
/// (module doc) — already used elsewhere in this workspace
/// (`crate::artifact_ingest::selftest`) as a canonical example id.
fn frozen_scenario_id() -> ScenarioId {
    ScenarioId::parse("world.firstbuy-hotdeal.26").expect("valid ScenarioId")
}

fn rfc3339(s: &str) -> DateTime<Utc> {
    s.parse::<DateTime<Utc>>().expect("literal RFC-3339 timestamp")
}

/// The two REAL `Divergence` records `divergence-log.md` §3.3 audits
/// (module doc) — the round-8 backfill (lower `run_seq`) and the
/// round-3 fresh campaign (higher `run_seq`, must win).
fn frozen_incident_divergences() -> Vec<Divergence> {
    vec![
        Divergence::new(
            Envelope::new(1, RecordKind::Divergence, rfc3339("2026-07-07T12:00:00Z"), Actor::new("backfill-w1-retro", RoleId::parse("reviewer").expect("valid RoleId"))),
            frozen_project_id(),
            frozen_scenario_id(),
            Sha::parse("87c1fc578467437918410502e9be5ea71cae315e").expect("valid Sha"),
            DivergenceStatus::Open,
            TotalOrder::new(1),
            8,
            "backfill-w1-retro",
            "round-8 backfill of an earlier campaign (frozen incident, divergence-log.md §3.3)",
        ),
        Divergence::new(
            Envelope::new(
                1,
                RecordKind::Divergence,
                rfc3339("2026-07-08T20:05:49Z"),
                Actor::new("code-reviewer-world-firstbuy-hotdeal", RoleId::parse("reviewer").expect("valid RoleId")),
            ),
            frozen_project_id(),
            frozen_scenario_id(),
            Sha::parse("8c81f9e13e9bda0a6a5ee29ba1b6b5137e7bf552").expect("valid Sha"),
            DivergenceStatus::Resolved,
            TotalOrder::new(3),
            3,
            "code-reviewer-world-firstbuy-hotdeal",
            "round-3 fresh review campaign, run_seq 3 (frozen incident, divergence-log.md §3.3)",
        ),
    ]
}

fn folded_state_tag(state: Option<&FoldedState>) -> &'static str {
    match state {
        Some(FoldedState::Open) => "open",
        Some(FoldedState::Resolved) => "resolved",
        Some(FoldedState::StillDivergent) => "still-divergent",
        Some(FoldedState::Deferred { .. }) => "deferred",
        Some(FoldedState::ResolvedInvalid) => "resolved-invalid",
        None => "absent",
    }
}

/// Folds [`frozen_incident_divergences`] through the REAL
/// [`fold_to_current_state`] twice (module doc): no live-binding
/// re-check supplied (`fold:<scenario_id>`, trusted `resolved`), and a
/// live-binding snapshot whose `app_sha` has moved off the winner's
/// resolved `sha` (`fold-live:<scenario_id>`, `resolved-invalid`).
fn run_frozen_incident() -> BTreeSet<(String, String)> {
    let records = frozen_incident_divergences();
    let key = (frozen_project_id(), frozen_scenario_id());
    let mut pairs = BTreeSet::new();

    let folded = fold_to_current_state(&records, &std::collections::BTreeMap::new(), Utc::now());
    pairs.insert((format!("fold:{}", frozen_scenario_id().as_str()), folded_state_tag(folded.get(&key)).to_string()));

    let mut live = std::collections::BTreeMap::new();
    live.insert(key.clone(), BindingSnapshot { app_sha: Sha::parse("0".repeat(40)).expect("valid Sha"), reserved_digest: None });
    let folded_stale = fold_to_current_state(&records, &live, Utc::now());
    pairs.insert((format!("fold-live:{}", frozen_scenario_id().as_str()), folded_state_tag(folded_stale.get(&key)).to_string()));

    pairs
}

// ── the two-sided exact-set oracle machinery (mirrors canon-gate/src/selftest.rs) ──

/// One fixture's identity, runner, and checked-in EXPECTED oracle
/// (module doc; `canon_gate::selftest::Fixture`'s identical shape,
/// generalized from a closed `FailureClass` enum to an open `String`
/// tag).
struct Fixture {
    name: &'static str,
    run: fn() -> BTreeSet<(String, String)>,
    expected: &'static str,
}

fn fixtures() -> Vec<Fixture> {
    vec![
        Fixture { name: "clean-root", run: run_clean_root, expected: include_str!("../fixtures/inventory/clean-root/expected_violations.txt") },
        Fixture { name: "missing-provenance", run: run_missing_provenance, expected: include_str!("../fixtures/inventory/missing-provenance/expected_violations.txt") },
        Fixture { name: "duplicate-scenario", run: run_duplicate_scenario, expected: include_str!("../fixtures/inventory/duplicate-scenario/expected_violations.txt") },
        Fixture { name: "frozen-incident", run: run_frozen_incident, expected: include_str!("../fixtures/inventory/frozen-incident/expected_state.txt") },
    ]
}

/// Parse a fixture's `expected_*.txt` 2-column `<tag> <subject>` format
/// — blank lines and `#`-comment lines skipped. Returns `Err` naming
/// the first malformed non-comment line instead of silently filtering
/// it out — a fixture's checked-in EXPECTED file carrying a typo'd
/// line must never exact-set-match a dirty corpus just because its
/// well-formed pairs happen to agree with `actual` (mirrors
/// `canon_gate::selftest::parse_expected`'s identical "a malformed
/// oracle is a dirty outcome on its own" rule).
fn parse_expected(text: &str) -> Result<BTreeSet<(String, String)>, String> {
    let mut set = BTreeSet::new();
    for line in text.lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with('#')) {
        let (tag, subject) = line.split_once(' ').ok_or_else(|| format!("malformed EXPECTED line (expected `<tag> <subject>`): {line:?}"))?;
        set.insert((tag.to_string(), subject.trim().to_string()));
    }
    Ok(set)
}

/// One fixture's diff outcome — both halves of the two-sided exact-set
/// oracle (spec Req 1): `missing` (under-detection, expected but not
/// produced) and `extra` (over-triggering, produced but not expected).
pub struct FixtureOutcome {
    pub name: &'static str,
    pub missing: BTreeSet<(String, String)>,
    pub extra: BTreeSet<(String, String)>,
    /// Set when this fixture's checked-in EXPECTED file itself failed
    /// to parse — a malformed oracle is a dirty outcome on its own,
    /// independent of `missing`/`extra` (both stay empty: there is no
    /// `expected` set to diff against).
    pub expected_parse_error: Option<String>,
}

impl FixtureOutcome {
    pub fn is_clean(&self) -> bool {
        self.expected_parse_error.is_none() && self.missing.is_empty() && self.extra.is_empty()
    }

    fn describe(&self) -> String {
        let mut parts = Vec::new();
        if let Some(err) = &self.expected_parse_error {
            parts.push(format!("malformed EXPECTED file: {err}"));
        }
        for (tag, subject) in &self.missing {
            parts.push(format!("missing (under-detection): {tag} {subject}"));
        }
        for (tag, subject) in &self.extra {
            parts.push(format!("extra (over-triggering): {tag} {subject}"));
        }
        format!("{}: {}", self.name, parts.join("; "))
    }
}

/// Every fixture's outcome from one run.
pub struct SelftestReport {
    pub outcomes: Vec<FixtureOutcome>,
}

impl SelftestReport {
    pub fn is_clean(&self) -> bool {
        self.outcomes.iter().all(FixtureOutcome::is_clean)
    }
}

/// Run every fixture (module doc) — the ONE entry point both `cargo
/// test -p canon-cli` and [`selftest`] (the `canon selftest` aggregator
/// registration) call.
pub fn run() -> SelftestReport {
    let outcomes = fixtures()
        .into_iter()
        .map(|fixture| {
            let actual = (fixture.run)();
            match parse_expected(fixture.expected) {
                Ok(expected) => {
                    let missing = expected.difference(&actual).cloned().collect();
                    let extra = actual.difference(&expected).cloned().collect();
                    FixtureOutcome { name: fixture.name, missing, extra, expected_parse_error: None }
                }
                Err(err) => FixtureOutcome { name: fixture.name, missing: BTreeSet::new(), extra: BTreeSet::new(), expected_parse_error: Some(err) },
            }
        })
        .collect();
    SelftestReport { outcomes }
}

/// canon-cli's shared-contract selftest entry point (`canon selftest`'s
/// 9th suite, registered as `"spec-ledger-selftest"` in
/// `crate::selftest::suites`). Beyond the four fixtures' two-sided
/// oracle diff, also asserts the clean-root fixture actually
/// MATERIALIZES its scenario (spec Req 1's "N scenarios materialized" —
/// a write-count fact, not a violation, so it is checked directly here
/// rather than folded into the violation-set oracle above).
///
/// `Ok(n)` = checks passed; `Err(_)` = one line per failure, never
/// panics.
pub fn selftest() -> Result<usize, Vec<String>> {
    let mut passed = 0;
    let mut failures = Vec::new();

    let clean = run_root(build_clean_root);
    if clean.is_clean() && clean.scanned == 1 && clean.written == 1 {
        passed += 1;
    } else {
        failures.push(format!(
            "clean-root: expected exactly 1 scenario materialized cleanly, got scanned={} written={} violations={} sync_errors={}",
            clean.scanned,
            clean.written,
            clean.violations.len(),
            clean.sync_errors.len()
        ));
    }

    let report = run();
    for outcome in &report.outcomes {
        if outcome.is_clean() {
            passed += 1;
        } else {
            failures.push(outcome.describe());
        }
    }

    if failures.is_empty() { Ok(passed) } else { Err(failures) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn spec_ledger_selftest_is_clean_against_its_own_fixtures() {
        match selftest() {
            Ok(passed) => assert_eq!(passed, 5, "4 fixtures + 1 materialized-count check"),
            Err(failures) => panic!("spec-ledger-selftest fixtures are failing:\n{}", failures.join("\n")),
        }
    }

    #[test]
    fn fixture_table_has_one_entry_per_named_fixture() {
        let names: Vec<&str> = fixtures().iter().map(|f| f.name).collect();
        let unique: BTreeSet<&str> = names.iter().copied().collect();
        assert_eq!(names.len(), unique.len(), "no fixture registered twice");
        assert_eq!(names.len(), 4);
    }

    /// The bug this mirrors from `canon_gate::selftest` (assignment
    /// contract): a fixture whose EXPECTED file is otherwise correct
    /// but carries ONE extra malformed line (no space) must never
    /// silently pass — `parse_expected` rejects the whole file.
    #[test]
    fn a_malformed_extra_expected_line_fails_the_fixture_instead_of_silently_matching() {
        let clean_text = include_str!("../fixtures/inventory/clean-root/expected_violations.txt");
        let clean_expected = parse_expected(clean_text).expect("the shipped EXPECTED file is well-formed");
        assert_eq!(clean_expected, run_clean_root(), "precondition: the real fixture's EXPECTED set matches actual exactly");

        let dirty_text = format!("{clean_text}\nnomarker\n");
        let err = parse_expected(&dirty_text).expect_err("a spaceless line must be rejected, never silently dropped");
        assert!(err.contains("nomarker"), "the error must name the offending line: {err}");
    }

    /// spec Req 1's own two scenarios, directly: a fixture whose actual
    /// set omits an expected entry is reported `missing`; one with an
    /// extra unexpected entry is reported `extra` — never silently
    /// accepted as a superset.
    #[test]
    fn a_missing_or_extra_entry_is_reported_two_sided() {
        let actual = run_missing_provenance();
        let too_narrow = parse_expected("fmt:missing-provenance some/other/path.feature\n").unwrap();
        let missing: BTreeSet<_> = too_narrow.difference(&actual).cloned().collect();
        let extra: BTreeSet<_> = actual.difference(&too_narrow).cloned().collect();
        assert!(!missing.is_empty(), "the real subject must be reported missing against a wrong-subject oracle");
        assert!(!extra.is_empty(), "the real subject must ALSO be reported extra — over-triggering is never silently accepted");
    }

    /// Spec Req 3 Scenario 2, made concrete (assignment acceptance
    /// criterion): this fixture's two records must pick DIFFERENT
    /// winners under `run_seq`-primary (the real, correct ordering)
    /// vs. `round`-primary (the regression `fold_to_current_state`'s
    /// own module doc rules out) — proving a future ranking regression
    /// would change `run_frozen_incident`'s actual output and break
    /// its checked-in oracle, rather than this fixture being vacuously
    /// insensitive to the bug it exists to catch.
    #[test]
    fn a_round_primary_ordering_regression_would_flip_the_frozen_incident_winner() {
        let records = frozen_incident_divergences();

        let run_seq_primary_winner = records.iter().max_by_key(|d| (d.run_seq, d.round)).expect("non-empty fixture");
        let round_primary_winner = records.iter().max_by_key(|d| (d.round, d.run_seq)).expect("non-empty fixture");
        assert_ne!(
            run_seq_primary_winner.status, round_primary_winner.status,
            "the frozen incident must pick different winners under run_seq-primary vs round-primary ordering, or a regression to the latter could never be caught"
        );
        assert_eq!(run_seq_primary_winner.status, DivergenceStatus::Resolved, "run_seq 3 (round 3, the fresh campaign) must win");
        assert_eq!(round_primary_winner.status, DivergenceStatus::Open, "a round-primary bug would resurrect round 8's stale open backfill");

        let key = (frozen_project_id(), frozen_scenario_id());
        let folded = fold_to_current_state(&records, &std::collections::BTreeMap::new(), Utc::now());
        assert_eq!(folded.get(&key), Some(&FoldedState::Resolved), "the REAL fold_to_current_state must agree with run_seq-primary, never round-primary");
    }

    #[test]
    fn frozen_incident_resolved_invalid_branch_fires_on_a_stale_live_binding() {
        let actual = run_frozen_incident();
        assert!(actual.contains(&(format!("fold:{}", frozen_scenario_id().as_str()), "resolved".to_string())));
        assert!(actual.contains(&(format!("fold-live:{}", frozen_scenario_id().as_str()), "resolved-invalid".to_string())));
    }
}
