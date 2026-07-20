//! canon-ingest's plan-import selftest entry point (s17 P4, tasks.md
//! §4.1; extended s30 `plan-dialect-superpowers` task 1.4): the 11th
//! `canon selftest` suite (registered as `"plan-import"` in
//! `canon-cli`'s `crate::selftest::suites`). TWO synthetic fixture
//! trees -- an openspec change tree (live/archive/malformed/
//! proposal-only dirs) and a superpowers plan-doc tree
//! (done/mixed-with-malformed/goal-missing/not-a-plan-doc/invalid-slug
//! docs) -- both built entirely inside a scratch directory at RUN time
//! (no checked-in fixture files; this module never reads or writes
//! anything under this repo's own `openspec/changes/` or
//! `docs/superpowers/plans/`, nor the checked-in dev-test corpus at
//! `crates/canon-ingest/tests/fixtures/plan_openspec` task 2.7 already
//! owns) -- each drives the REAL P1-P3 plan-import machinery in one
//! pipeline: [`crate::plan_registry::find`] resolves the dialect's
//! entry (the EXACT seam `canon ingest plans`'s driver,
//! `crates/canon-cli/src/plans.rs`, uses), whose
//! [`crate::plan_adapter::PlanAdapter::resolve_source`] +
//! [`crate::plan_adapter::PlanAdapter::parse`] produce one
//! [`PlanParseOutcome`] over that dialect's fixture root.
//!
//! # Two-sided exact-set fact oracle
//! Mirrors `canon_plugin::selftest`'s and canon-cli's
//! `crate::inventory_selftest`'s two-sided (missing AND extra both
//! fail) exact-set discipline, generalized to this crate's own
//! `(tag, subject)` fact pair: [`expected_facts`] (openspec) and
//! [`expected_superpowers_facts`] each name exactly every
//! `Change`/`Task` their fixture's docs must import
//! (`change:<change_id>:<status>` / `task:<task_id>:<status>`), every
//! named design-D3/D6 `unmapped` drop count (`drop:<construct>:<count>`),
//! and the one `malformed` scalar (`malformed:<count>`). [`diff_fact_sets`]
//! diffs a fixture's ACTUAL fact set ([`outcome_facts`]) against its
//! exact expectation both ways -- a fact a fixture no longer produces
//! (a Task silently stops importing, a status derivation regresses, a
//! drop count shrinks) and a fact it produces BEYOND what's expected (a
//! new, unaccounted-for Change/Task/drop) both fail the suite.
//! `tests::a_regressed_expected_fact_would_be_reported_as_missing` /
//! `tests::an_unexpected_extra_fact_would_be_reported` (and their
//! `_superpowers` siblings) prove the oracle is actually
//! discriminating, not a tautology that always agrees with whatever the
//! adapter currently emits.
//!
//! # Rebindable scratch root, no `tempfile` dependency
//! [`ScratchDir`] is a minimal `std`-only equivalent of
//! `tempfile::TempDir` (mirrors `canon_plugin::selftest::ScratchDir` /
//! canon-cli's `crate::inventory_selftest::ScratchDir` verbatim) --
//! `tempfile` is this crate's `[dev-dependencies]` only (task 1.4's
//! S4 handoff fixture), and this module compiles into the release
//! `canon` binary via `canon selftest`, not only under `cargo test`.
//! Every read/write is scoped to a fresh scratch directory under
//! `std::env::temp_dir()`, `Drop`-cleaned, side-effect-free against
//! this repo's own checkout.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use canon_model::records::{ChangeStatus, TaskStatus};

use crate::plan_adapter::{PlanParseOutcome, PlanSourceConfig};
use crate::plan_adapters::openspec::{DIAG_DESIGN_DOC, DIAG_PROPOSAL_MISSING_WHY, DIAG_SPEC_DELTA_SCENARIO};
use crate::plan_adapters::superpowers::{DIAG_GOAL_MISSING, DIAG_NOT_A_PLAN_DOC};
use crate::plan_registry;

/// A `std`-only, `Drop`-cleaned scratch directory -- see module doc.
struct ScratchDir(PathBuf);

impl ScratchDir {
    fn new() -> Result<Self, String> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("canon-ingest-plan-selftest-{}-{nanos}-{unique}", std::process::id()));
        std::fs::create_dir_all(&path).map_err(|e| format!("create scratch dir {}: {e}", path.display()))?;
        Ok(Self(path))
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

fn write_file(root: &Path, rel: &str, contents: &str) -> Result<(), String> {
    let path = root.join(rel);
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("create dir {}: {e}", parent.display()))?;
    }
    std::fs::write(&path, contents).map_err(|e| format!("write {}: {e}", path.display()))
}

// ── the openspec fixture tree (module doc) ──

const LIVE_CHANGE: &str = "widget-import";
const ARCHIVED_CHANGE: &str = "legacy-import";
const PROPOSAL_ONLY_CHANGE: &str = "proposal-only-import";
const MISSING_WHY_CHANGE: &str = "missing-why-import";
/// Fails `ChangeId::parse` on purpose (uppercase + underscore + `!`) --
/// the whole dir must import as `malformed`, never as a `Change`.
const BAD_BASENAME_DIR: &str = "Bad_Slug!";

/// Build the synthetic change tree under `root` -- see module doc for
/// the shape each dir proves. `LIVE_CHANGE` alone carries a
/// `design.md` and a `specs/**/spec.md` (2 `#### Scenario:` blocks)
/// plus one bad-`<n>` row, so every design-D3 drop diagnostic and the
/// `malformed` scalar are both exercised in one pass.
fn build_fixture(root: &Path) -> Result<(), String> {
    write_file(
        root,
        &format!("openspec/changes/{LIVE_CHANGE}/proposal.md"),
        "# widget-import\n\n## Why\nImports a widget capability for the plan-import selftest fixture.\n\n## What Changes\n- add a widget\n",
    )?;
    write_file(
        root,
        &format!("openspec/changes/{LIVE_CHANGE}/tasks.md"),
        "## 1. Widget\n- [x] 1.1 wire the parser — ✅ selftest evidence\n- [ ] 1.2 add docs\n- [ ] 1.a bad task number row\n",
    )?;
    write_file(root, &format!("openspec/changes/{LIVE_CHANGE}/design.md"), "# Design — widget-import\n\nDesign prose never maps onto a core record.\n")?;
    write_file(
        root,
        &format!("openspec/changes/{LIVE_CHANGE}/specs/widget/spec.md"),
        "# Spec deltas\n\n#### Scenario: widget renders\n- fixture only\n\n#### Scenario: widget dismisses\n- fixture only\n",
    )?;

    write_file(
        root,
        &format!("openspec/changes/archive/{ARCHIVED_CHANGE}/proposal.md"),
        "# legacy-import\n\n## Why\nAn archived change pinning the D6 archive-wins-unconditionally rule.\n",
    )?;
    write_file(root, &format!("openspec/changes/archive/{ARCHIVED_CHANGE}/tasks.md"), "## 1. Legacy\n- [ ] 1.1 keep the old rows\n")?;

    write_file(
        root,
        &format!("openspec/changes/{PROPOSAL_ONLY_CHANGE}/proposal.md"),
        "# proposal-only-import\n\n## Why\nA proposal-stage change with no tasks.md yet.\n",
    )?;

    write_file(
        root,
        &format!("openspec/changes/{MISSING_WHY_CHANGE}/proposal.md"),
        "# missing-why-import\n\n## What Changes\n- nothing yet, no Why heading at all\n",
    )?;

    write_file(root, &format!("openspec/changes/{BAD_BASENAME_DIR}/proposal.md"), "# Bad_Slug!\n\n## Why\nThis basename fails ChangeId::parse on purpose.\n")?;

    Ok(())
}

/// Build the fixture tree, then drive it through the REAL `openspec`
/// entry in the PRODUCTION [`plan_registry`] -- the exact
/// `resolve_source` + `parse` seam `canon ingest plans`'s driver uses
/// (module doc), never a direct `OpenspecPlanAdapter` construction
/// that could silently diverge from the registered entry.
fn run_fixture() -> Result<PlanParseOutcome, String> {
    let scratch = ScratchDir::new()?;
    build_fixture(scratch.path())?;

    let entry = plan_registry::find("openspec").ok_or_else(|| "the `openspec` dialect is not registered in plan_registry".to_string())?;
    let config = PlanSourceConfig { root: Some(scratch.path().to_path_buf()) };
    let handle = entry.adapter.resolve_source(&config).ok_or_else(|| "resolve_source returned None for a configured (Some) root".to_string())?;
    Ok(entry.adapter.parse(&handle))
}

// ── the superpowers fixture tree (s30 task 1.4) ──

const SUPERPOWERS_DONE_CHANGE: &str = "2026-07-14-superpowers-done";
/// One doc carrying every checkbox-matrix shape (all-checked, mixed,
/// zero-checkbox) PLUS a duplicate `Task 1` heading and an invalid
/// `Task x` heading -- mirrors `LIVE_CHANGE`'s "every diagnostic in one
/// pass" density, s30 design D3.
const SUPERPOWERS_RICH_CHANGE: &str = "2026-07-14-superpowers-rich";
const SUPERPOWERS_GOAL_MISSING_CHANGE: &str = "2026-07-14-superpowers-goal-missing";
/// Slugifies to the empty string on purpose (design D2) -- the whole
/// doc must import as `malformed`, never as a `Change`.
const SUPERPOWERS_BAD_SLUG_FILE: &str = "!!!.md";

/// Build the synthetic superpowers plan-doc tree under `root`, at
/// `docs/superpowers/plans/` (design D5's ordinary consumer-repo
/// shape) -- see the module doc and this fn's own per-doc comments for
/// the diagnostic each one proves.
fn build_superpowers_fixture(root: &Path) -> Result<(), String> {
    write_file(
        root,
        &format!("docs/superpowers/plans/{SUPERPOWERS_DONE_CHANGE}.md"),
        "# Superpowers Done Implementation Plan\n\n\
         **Goal:** Prove an all-checked task section imports as Completed.\n\n\
         ### Task 1: Adapter\n- [x] **Step 1:** wire the parser\n- [x] **Step 2:** register the entry\n",
    )?;

    write_file(
        root,
        &format!("docs/superpowers/plans/{SUPERPOWERS_RICH_CHANGE}.md"),
        "# Superpowers Rich Implementation Plan\n\n\
         **Goal:** Prove the checkbox matrix, a duplicate task number, and an invalid task number all in one document.\n\n\
         ### Task 1: Adapter\n- [x] **Step 1:** wire the parser\n\n\
         ### Task 2: Docs\n- [x] **Step 1:** draft the docs\n- [ ] **Step 2:** review the docs\n\n\
         ### Task 3: Empty\nNo checkbox lines in this section at all.\n\n\
         ### Task 1: Duplicate Adapter\n- [ ] **Step 1:** never counted, first wins\n\n\
         ### Task x: Bad Number\n- [x] **Step 1:** never emitted, invalid task number\n",
    )?;

    write_file(
        root,
        &format!("docs/superpowers/plans/{SUPERPOWERS_GOAL_MISSING_CHANGE}.md"),
        "# Superpowers Goal Missing Implementation Plan\n\n### Task 1: Adapter\n- [ ] **Step 1:** not yet done\n",
    )?;

    write_file(
        root,
        &format!("docs/superpowers/plans/{SUPERPOWERS_BAD_SLUG_FILE}"),
        "# Invalid Slug Plan\n\n**Goal:** This filename slugs to empty and must be malformed.\n\n### Task 1: Adapter\n- [x] **Step 1:** irrelevant, file never imports\n",
    )?;

    write_file(
        root,
        "docs/superpowers/plans/README.md",
        "# Plans Directory\n\nJust an ordinary docs README with no Goal line and no Task headings at all.\n",
    )?;

    Ok(())
}

/// Build the superpowers fixture tree, then drive it through the REAL
/// `superpowers` entry in the PRODUCTION [`plan_registry`] -- mirrors
/// [`run_fixture`]'s discipline exactly, never a direct
/// `SuperpowersPlanAdapter` construction that could silently diverge
/// from the registered entry.
fn run_superpowers_fixture() -> Result<PlanParseOutcome, String> {
    let scratch = ScratchDir::new()?;
    build_superpowers_fixture(scratch.path())?;

    let entry = plan_registry::find("superpowers").ok_or_else(|| "the `superpowers` dialect is not registered in plan_registry".to_string())?;
    let config = PlanSourceConfig { root: Some(scratch.path().to_path_buf()) };
    let handle = entry.adapter.resolve_source(&config).ok_or_else(|| "resolve_source returned None for a configured (Some) root".to_string())?;
    Ok(entry.adapter.parse(&handle))
}

// ── the two-sided exact-set fact oracle (module doc) ──

fn change_status_tag(status: ChangeStatus) -> &'static str {
    match status {
        ChangeStatus::Proposed => "proposed",
        ChangeStatus::InProgress => "in_progress",
        ChangeStatus::Completed => "completed",
        ChangeStatus::Archived => "archived",
    }
}

fn task_status_tag(status: TaskStatus) -> &'static str {
    match status {
        TaskStatus::Open => "open",
        TaskStatus::Done => "done",
    }
}

/// Reduce one [`PlanParseOutcome`] to its `(tag, subject)` fact set --
/// every emitted `Change` id+status, every emitted `Task` id+status,
/// every named `unmapped` drop count, and the `malformed` list's own
/// (module doc). Dialect-agnostic -- both [`check_oracle`] and
/// [`check_superpowers_oracle`] diff its output against their own
/// dialect's expectation.
fn outcome_facts(outcome: &PlanParseOutcome) -> BTreeSet<(String, String)> {
    let mut facts = BTreeSet::new();
    for change in &outcome.changes {
        facts.insert(("change".to_string(), format!("{}:{}", change.change_id.as_str(), change_status_tag(change.status))));
    }
    for task in &outcome.tasks {
        facts.insert(("task".to_string(), format!("{}:{}", task.task_id.as_str(), task_status_tag(task.status))));
    }
    for (construct, count) in &outcome.unmapped {
        facts.insert(("drop".to_string(), format!("{construct}:{count}")));
    }
    facts.insert(("malformed".to_string(), outcome.malformed.len().to_string()));
    facts
}

/// This fixture's checked expectation (module doc): the 4 well-formed
/// dirs import (`BAD_BASENAME_DIR` never does -- malformed); 3 Tasks
/// (`LIVE_CHANGE`'s bad-`<n>` row never emits one); the 3 named drops
/// [`build_fixture`] exercises, referenced through the SAME
/// `DIAG_*` constants `plan_adapters::openspec` emits them under, so
/// this expectation can never drift from that module's own diagnostic
/// names; and exactly 2 malformed constructs (`BAD_BASENAME_DIR`'s bad
/// basename + `LIVE_CHANGE`'s bad-`<n>` row).
fn expected_facts() -> BTreeSet<(String, String)> {
    [
        ("change".to_string(), format!("{LIVE_CHANGE}:in_progress")),
        ("change".to_string(), format!("{ARCHIVED_CHANGE}:archived")),
        ("change".to_string(), format!("{PROPOSAL_ONLY_CHANGE}:proposed")),
        ("change".to_string(), format!("{MISSING_WHY_CHANGE}:proposed")),
        ("task".to_string(), format!("{LIVE_CHANGE}#1.1:done")),
        ("task".to_string(), format!("{LIVE_CHANGE}#1.2:open")),
        ("task".to_string(), format!("{ARCHIVED_CHANGE}#1.1:open")),
        ("drop".to_string(), format!("{DIAG_SPEC_DELTA_SCENARIO}:2")),
        ("drop".to_string(), format!("{DIAG_DESIGN_DOC}:1")),
        ("drop".to_string(), format!("{DIAG_PROPOSAL_MISSING_WHY}:1")),
        ("malformed".to_string(), "2".to_string()),
    ]
    .into_iter()
    .collect()
}

/// This fixture's checked expectation (s30 task 1.4): 3 well-formed
/// docs import (`SUPERPOWERS_BAD_SLUG_FILE` never does -- malformed);
/// 5 Tasks total (`SUPERPOWERS_RICH_CHANGE`'s duplicate + invalid
/// headings never emit one); the 2 named drops
/// [`build_superpowers_fixture`] exercises, referenced through the SAME
/// `DIAG_*` constants `plan_adapters::superpowers` emits them under;
/// and exactly 3 malformed constructs (the bad-slug doc + the rich
/// doc's duplicate heading + its invalid-number heading).
fn expected_superpowers_facts() -> BTreeSet<(String, String)> {
    [
        ("change".to_string(), format!("{SUPERPOWERS_DONE_CHANGE}:completed")),
        ("change".to_string(), format!("{SUPERPOWERS_RICH_CHANGE}:in_progress")),
        ("change".to_string(), format!("{SUPERPOWERS_GOAL_MISSING_CHANGE}:proposed")),
        ("task".to_string(), format!("{SUPERPOWERS_DONE_CHANGE}#1:done")),
        ("task".to_string(), format!("{SUPERPOWERS_RICH_CHANGE}#1:done")),
        ("task".to_string(), format!("{SUPERPOWERS_RICH_CHANGE}#2:open")),
        ("task".to_string(), format!("{SUPERPOWERS_RICH_CHANGE}#3:open")),
        ("task".to_string(), format!("{SUPERPOWERS_GOAL_MISSING_CHANGE}#1:open")),
        ("drop".to_string(), format!("{DIAG_GOAL_MISSING}:1")),
        ("drop".to_string(), format!("{DIAG_NOT_A_PLAN_DOC}:1")),
        ("malformed".to_string(), "3".to_string()),
    ]
    .into_iter()
    .collect()
}

fn check_dialect_registered(_outcome: &PlanParseOutcome) -> Result<(), String> {
    if plan_registry::find("openspec").is_none() {
        return Err("the `openspec` dialect must stay registered in plan_registry for this suite to mean anything".to_string());
    }
    Ok(())
}

fn check_superpowers_dialect_registered(_outcome: &PlanParseOutcome) -> Result<(), String> {
    if plan_registry::find("superpowers").is_none() {
        return Err("the `superpowers` dialect must stay registered in plan_registry for this suite to mean anything".to_string());
    }
    Ok(())
}

/// A coarse sanity floor BEFORE the fine-grained oracle runs -- catches
/// a totally broken pipeline (e.g. discovery finding zero dirs/docs)
/// with a distinct, more legible failure than an exact-set diff dump
/// would. Dialect-agnostic -- shared by both fixtures.
fn check_fixture_yields_records(outcome: &PlanParseOutcome) -> Result<(), String> {
    if outcome.changes.is_empty() || outcome.tasks.is_empty() {
        return Err(format!("expected a non-empty parse over the fixture tree, got {} changes / {} tasks", outcome.changes.len(), outcome.tasks.len()));
    }
    Ok(())
}

/// Diff `actual` against `expected` both ways (module doc) -- the one
/// mutator [`check_oracle`] and [`check_superpowers_oracle`] both call,
/// so the two dialects' oracles can never silently diverge on what
/// "matches" means.
fn diff_fact_sets(actual: &BTreeSet<(String, String)>, expected: &BTreeSet<(String, String)>) -> Result<(), String> {
    let missing: Vec<_> = expected.difference(actual).cloned().collect();
    let extra: Vec<_> = actual.difference(expected).cloned().collect();
    if missing.is_empty() && extra.is_empty() {
        Ok(())
    } else {
        Err(format!("fact set mismatch -- missing (expected, never produced): {missing:?}; extra (produced, never expected): {extra:?}"))
    }
}

fn check_oracle(outcome: &PlanParseOutcome) -> Result<(), String> {
    diff_fact_sets(&outcome_facts(outcome), &expected_facts())
}

fn check_superpowers_oracle(outcome: &PlanParseOutcome) -> Result<(), String> {
    diff_fact_sets(&outcome_facts(outcome), &expected_superpowers_facts())
}

/// One named fixture check (mirrors `canon-ingest::selftest::Check` /
/// `canon_plugin::selftest::Check`).
type Check = (&'static str, fn(&PlanParseOutcome) -> Result<(), String>);

const OPENSPEC_CHECKS: &[Check] = &[
    ("dialect-registered", check_dialect_registered),
    ("fixture-yields-records", check_fixture_yields_records),
    ("fact-two-sided-exact-set", check_oracle),
];

const SUPERPOWERS_CHECKS: &[Check] = &[
    ("dialect-registered", check_superpowers_dialect_registered),
    ("fixture-yields-records", check_fixture_yields_records),
    ("fact-two-sided-exact-set", check_superpowers_oracle),
];

/// Run canon-ingest's plan-import fixture checks (module doc) over
/// BOTH the openspec and superpowers fixture trees. `Ok(n)` reports how
/// many independent checks (across both dialects) passed; `Err(_)`
/// carries one human-readable, dialect-prefixed line per failing check
/// -- never panics.
pub fn selftest() -> Result<usize, Vec<String>> {
    let openspec_outcome = match run_fixture() {
        Ok(outcome) => outcome,
        Err(e) => return Err(vec![format!("openspec fixture setup: {e}")]),
    };
    let superpowers_outcome = match run_superpowers_fixture() {
        Ok(outcome) => outcome,
        Err(e) => return Err(vec![format!("superpowers fixture setup: {e}")]),
    };

    let mut passed = 0usize;
    let mut failures = Vec::new();
    for (name, check) in OPENSPEC_CHECKS {
        match check(&openspec_outcome) {
            Ok(()) => passed += 1,
            Err(e) => failures.push(format!("openspec/{name}: {e}")),
        }
    }
    for (name, check) in SUPERPOWERS_CHECKS {
        match check(&superpowers_outcome) {
            Ok(()) => passed += 1,
            Err(e) => failures.push(format!("superpowers/{name}: {e}")),
        }
    }

    if failures.is_empty() { Ok(passed) } else { Err(failures) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selftest_is_clean_against_its_own_synthetic_fixtures() {
        let result = selftest();
        assert!(result.is_ok(), "canon-ingest plan-import selftest failed against its own fixtures: {result:?}");
        assert_eq!(result.unwrap(), OPENSPEC_CHECKS.len() + SUPERPOWERS_CHECKS.len());
    }

    #[test]
    fn a_regressed_expected_fact_would_be_reported_as_missing() {
        // Proves the two-sided oracle is actually discriminating
        // (module doc): an EXPECTED fact this fixture's real output
        // never produces (the real status is `in_progress`, never
        // `completed`) must fail loud, on the MISSING side.
        let outcome = run_fixture().expect("fixture setup");
        let actual = outcome_facts(&outcome);
        let bogus_expected: BTreeSet<(String, String)> = std::iter::once(("change".to_string(), format!("{LIVE_CHANGE}:completed"))).collect();
        assert!(!bogus_expected.difference(&actual).collect::<Vec<_>>().is_empty(), "a bogus expected fact must show up as missing");
    }

    #[test]
    fn an_unexpected_extra_fact_would_be_reported() {
        // The EXTRA side: every actual fact beyond an empty expected
        // set must surface, never silently pass.
        let outcome = run_fixture().expect("fixture setup");
        let actual = outcome_facts(&outcome);
        let empty_expected: BTreeSet<(String, String)> = BTreeSet::new();
        assert!(!actual.difference(&empty_expected).collect::<Vec<_>>().is_empty(), "an empty expected set must surface every actual fact as extra");
    }

    #[test]
    fn a_regressed_expected_superpowers_fact_would_be_reported_as_missing() {
        // The superpowers fixture's own version of the discriminating-
        // oracle proof above: the real status is `in_progress`, never
        // `completed`.
        let outcome = run_superpowers_fixture().expect("fixture setup");
        let actual = outcome_facts(&outcome);
        let bogus_expected: BTreeSet<(String, String)> = std::iter::once(("change".to_string(), format!("{SUPERPOWERS_RICH_CHANGE}:completed"))).collect();
        assert!(!bogus_expected.difference(&actual).collect::<Vec<_>>().is_empty(), "a bogus expected fact must show up as missing");
    }

    #[test]
    fn an_unexpected_extra_superpowers_fact_would_be_reported() {
        let outcome = run_superpowers_fixture().expect("fixture setup");
        let actual = outcome_facts(&outcome);
        let empty_expected: BTreeSet<(String, String)> = BTreeSet::new();
        assert!(!actual.difference(&empty_expected).collect::<Vec<_>>().is_empty(), "an empty expected set must surface every actual fact as extra");
    }

    #[test]
    fn the_fixture_tree_is_cleaned_up_after_the_run() {
        // ScratchDir's Drop-cleanup contract, pinned directly (module
        // doc: "side-effect-free against this repo's own checkout").
        let scratch = ScratchDir::new().expect("scratch dir");
        let path = scratch.path().to_path_buf();
        build_fixture(&path).expect("build fixture");
        assert!(path.join("openspec/changes").is_dir());
        drop(scratch);
        assert!(!path.exists(), "ScratchDir must remove its directory on drop");
    }

    #[test]
    fn the_superpowers_fixture_tree_is_cleaned_up_after_the_run() {
        let scratch = ScratchDir::new().expect("scratch dir");
        let path = scratch.path().to_path_buf();
        build_superpowers_fixture(&path).expect("build superpowers fixture");
        assert!(path.join("docs/superpowers/plans").is_dir());
        drop(scratch);
        assert!(!path.exists(), "ScratchDir must remove its directory on drop");
    }
}
