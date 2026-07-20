//! End-to-end integration test for `canon ingest artifacts [--repo]
//! [--watch]` (S14 `s14-artifact-ingest-cli`), invoking the actually-
//! built `canon` binary against an offline fixture repo seeded with
//! BOTH source shapes the driver must feed differently:
//!
//! - a PATH-source `code-review` ledger record (`artifacts.ledger_root`,
//!   the config-driven scan `canon_ingest::artifact_registry::
//!   resolve_and_parse` already drives unmodified), and
//! - a RECORDS-source `Handoff` row planted straight into canon-store's
//!   own git tier (`canon.yaml` `routing.handoff: git`) — the exact
//!   input `canon_ingest::artifact_adapters::handoff`'s own module doc
//!   names as "a driver living OUTSIDE this crate ... resolving canon's
//!   own Postgres-tier `Handoff` table through `canon_store::Tier::read`"
//!   (git tier here, offline, mirroring `canon-ingest`'s own
//!   `tests/handoff_fixture.rs`).
//!
//! This proves the join spine is ACTUALLY connected, not merely that
//! each half compiles in isolation (operator directive, 2026-07-11):
//! (a) the handoff records-source adapter was actually DRIVEN — its
//! `ArtifactAdapterSummary.status` is `"read"`, never `"unavailable"`,
//! and it contributes real events (no
//! `ArtifactDispatchOutcome::UnsupportedSource` silent drop); (b) a
//! trajectory parquet row is actually WRITTEN under canon-learn's own
//! `ParquetTrajectoryStore`, read back through that crate's own public
//! API; (c) `canon-report`'s `mart_role_memory` AND
//! `mart_flywheel_funnel` — the exact two panels this whole change
//! exists to feed — render NON-EMPTY from that freshly-ingested data,
//! queried through the real `duckdb` CLI `canon-report` itself shells
//! out to (`canon_report::marts`), never a parallel Rust re-derivation.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use canon_ingest::verdict::Polarity;
use canon_learn::{ParquetTrajectoryStore, TrajectoryStore};
use canon_model::{
    Actor, Divergence, DivergenceStatus, DomainId, Envelope, Handoff, HandoffBody, HandoffId, HandoffState, ProjectId, ProvenanceRef,
    RecordKind, RegimeKey, Review, RoleId, ScenarioId, Sha, TotalOrder,
};
use canon_store::git_tier::GitTier;
use canon_store::tier::Tier;
use serde_json::Value;
use uuid::Uuid;

fn run_canon(args: &[&str], cwd: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(cwd).output().expect("spawn canon binary")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn duckdb_available() -> bool {
    Command::new("duckdb").arg("--version").output().is_ok()
}

/// The PATH-source half: one `kind=code-review` ledger finding
/// (`verdict` absent → an open finding, `derive_verdict`'s
/// `CodeReviewFinding` row → `dev`/failure/guardrail-candidate),
/// laid out exactly as `canon_ingest::artifact_adapters::ledger`'s own
/// module doc requires (`kind=<kind>/area=<area>/<scenario_id>.json`).
fn plant_ledger_code_review_finding(ledger_scan_root: &Path) {
    let dir = ledger_scan_root.join("kind=code-review").join("area=world");
    std::fs::create_dir_all(&dir).unwrap();
    let body = serde_json::json!({
        "kind": "code-review",
        "scenario_id": "world.firstbuy-hotdeal.26",
        "at": "2026-07-01T00:00:00Z",
    });
    std::fs::write(dir.join("world.firstbuy-hotdeal.26.json"), serde_json::to_vec_pretty(&body).unwrap()).unwrap();
}

/// The RECORDS-source half: a real, typed `Handoff` — created, claimed,
/// then completed — written through canon-store's own `GitTier`
/// (`tiers.git.root`/`routing.handoff: git` in the fixture's
/// `canon.yaml`), so `crate::artifact_ingest`'s `TierRegistry::query`
/// read step has a genuine row to return. `events_for` (handoff.rs)
/// emits exactly THREE events for this shape: created, claimed, done —
/// all `NonVerdict` (design D3: "a handoff is management plumbing"),
/// so this contributes zero verdicts on its own; its sole job here is
/// proving the records-source READ itself actually ran.
fn plant_handoff_in_git(git_root: &Path) -> HandoffId {
    let id = HandoffId::parse("20260701-0910-s14-fixture-a1b2").unwrap();
    let envelope = Envelope::new(
        1,
        RecordKind::Handoff,
        "2026-07-01T09:00:00Z".parse().unwrap(),
        Actor::new("s14-fixture", RoleId::parse("implementer").unwrap()),
    );
    let body = HandoffBody { domain: DomainId::parse("development").unwrap(), template_version: 1, fields: serde_json::json!({}) };
    let mut handoff = Handoff::new(envelope, id.clone(), Uuid::new_v4(), None, 1, "S14 fixture handoff", None, body);
    handoff.transition_to(HandoffState::InProgress, "2026-07-01T09:11:00Z".parse().unwrap(), Some("s14-agent")).unwrap();
    handoff.transition_to(HandoffState::Done, "2026-07-01T09:20:00Z".parse().unwrap(), None).unwrap();
    GitTier::new(git_root).write(&handoff).expect("write handoff fixture into the git tier");
    id
}

/// S15 P4 (design D7) fixture: a native `Review` record, planted
/// straight into canon-store's own git tier (`canon.yaml`
/// `routing.review: git`) — `canon_ingest::artifact_adapters::review`'s
/// real input. Actor role is deliberately `content` — NOT `dev`/
/// `design` — the exact non-hard-coded-role proof spec
/// `native-record-flywheel` Requirement 2 requires (`content` is one
/// of canon-learn's `BUILTIN_ROLES`, so it registers without a
/// `learn.roles` extension).
fn plant_review_in_git(git_root: &Path) {
    let envelope = Envelope::new(
        1,
        RecordKind::Review,
        "2026-07-01T09:30:00Z".parse().unwrap(),
        Actor::new("s15-fixture", RoleId::parse("content").unwrap()),
    );
    let review = Review::new(
        envelope,
        ProjectId::parse("acme-repo").unwrap(),
        ScenarioId::parse("world.firstbuy-hotdeal.26").unwrap(),
        "reviewer-1",
        "abcdef123456",
        ProvenanceRef::UpstreamRef("upstream://scenario/1".to_string()),
    );
    GitTier::new(git_root).write(&review).expect("write review fixture into the git tier");
}

/// S15 P4 (design D7) fixture: a native `Divergence` record (status
/// `Resolved`), planted the same way —
/// `canon_ingest::artifact_adapters::native_divergence`'s real input.
/// Actor role is deliberately `test` — NOT `dev` — proving
/// `derive_native_divergence_verdict`'s role is the RECORD's own
/// actor, never `derive_verdict`'s `RemediationResolved` row's
/// hard-coded `dev` constant.
fn plant_divergence_resolved_in_git(git_root: &Path) {
    let envelope = Envelope::new(
        1,
        RecordKind::Divergence,
        "2026-07-01T09:40:00Z".parse().unwrap(),
        Actor::new("s15-fixture", RoleId::parse("test").unwrap()),
    );
    let divergence = Divergence::new(
        envelope,
        ProjectId::parse("acme-repo").unwrap(),
        ScenarioId::parse("world.firstbuy-hotdeal.27").unwrap(),
        Sha::parse("b".repeat(40)).unwrap(),
        DivergenceStatus::Resolved,
        TotalOrder::new(1),
        1,
        "reviewer-1",
        "",
    );
    GitTier::new(git_root).write(&divergence).expect("write divergence fixture into the git tier");
}

/// Builds `<tmp>/acme-repo-native` with `artifacts.native_records:
/// true` and BOTH `review`/`divergence` routed to the git tier, seeded
/// with one `Review` AND one resolved `Divergence` — the S15 P4
/// flywheel fixture (design D7 Scenario "A Review and a Divergence in
/// the same run both produce trajectories"). No raw-artifact path
/// field is set, satisfying the XOR.
fn build_native_flywheel_repo(tmp: &Path) -> PathBuf {
    let repo = tmp.join("acme-repo-native");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(
        repo.join("canon.yaml"),
        "tiers:\n  local: { backend: git, root: canon/ledger }\n\nrouting:\n  review: local\n  divergence: local\n\nartifacts:\n  native_records: true\n",
    )
    .unwrap();

    plant_review_in_git(&repo.join("canon/ledger"));
    plant_divergence_resolved_in_git(&repo.join("canon/ledger"));

    repo
}

/// Builds `<tmp>/acme-repo` (a FIXED basename — `regime_key`'s `<repo>`
/// segment is this directory's basename, module doc; a random tempdir
/// name would make the expected key unpredictable) with a `canon.yaml`
/// routing `handoff` to the git tier and pointing
/// `artifacts.ledger_root` at the seeded ledger fixture, then plants
/// both fixture halves. Returns the repo root.
fn build_fixture_repo(tmp: &Path) -> PathBuf {
    let repo = tmp.join("acme-repo");
    std::fs::create_dir_all(&repo).unwrap();

    std::fs::write(
        repo.join("canon.yaml"),
        "tiers:\n  local: { backend: git, root: canon/ledger }\n\nrouting:\n  handoff: local\n\nartifacts:\n  ledger_root: fixtures/ledger\n",
    )
    .unwrap();

    plant_ledger_code_review_finding(&repo.join("fixtures/ledger"));
    plant_handoff_in_git(&repo.join("canon/ledger"));

    repo
}

#[test]
fn ingest_artifacts_drives_both_source_shapes_persists_trajectories_and_feeds_real_marts() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = build_fixture_repo(tmp.path());

    // ── Run the actually-built `canon` binary. ──
    let output = run_canon(&["ingest", "artifacts", "--repo", ".", "--json"], &repo);
    assert!(output.status.success(), "canon ingest artifacts must exit 0; stderr: {}", stderr(&output));

    let payload: Value = serde_json::from_str(stdout(&output).trim()).expect("--json prints a single JSON outcome document");

    // ── (a) the handoff RECORDS-source adapter was actually DRIVEN. ──
    let adapters = payload["adapters"].as_array().expect("adapters array");
    let handoff_summary = adapters
        .iter()
        .find(|a| a["adapter_id"] == "handoff")
        .unwrap_or_else(|| panic!("no `handoff` adapter entry in outcome: {payload}"));
    assert_eq!(handoff_summary["source_kind"], "records", "handoff must be reported as the records-source adapter");
    assert_eq!(
        handoff_summary["status"], "read",
        "handoff's records source must have been READ, not `unavailable` \
         (an `unavailable` status here would mean this driver silently dropped \
         the records-source input, exactly the ArtifactDispatchOutcome::\
         UnsupportedSource collapse the operator directive forbids): {payload}"
    );
    let handoff_events = handoff_summary["events_parsed"].as_u64().expect("events_parsed is a number");
    assert_eq!(handoff_events, 3, "created + claimed + done, from the planted handoff's own state transitions: {payload}");

    // The ledger PATH-source adapter contributed its one open finding.
    let ledger_summary = adapters.iter().find(|a| a["adapter_id"] == "ledger").expect("ledger adapter entry");
    assert_eq!(ledger_summary["status"], "read");
    assert_eq!(ledger_summary["events_parsed"], 1);

    // Exactly one verdict was derivable (the ledger finding; every
    // handoff event is NonVerdict by design D3 and derives none).
    assert_eq!(payload["verdicts_derived"], 1, "{payload}");

    let persisted = payload["trajectories_persisted"].as_array().expect("trajectories_persisted array");
    assert_eq!(persisted.len(), 1, "one regime-keyed trajectory persisted: {payload}");
    let regime_key_str = persisted[0]["regime_key"].as_str().expect("regime_key string").to_string();
    assert!(regime_key_str.starts_with("dev/acme-repo/world/"), "regime_key: {regime_key_str}");
    assert_eq!(payload["strategy_items_rebuilt"], 1, "the distilled tier must also gain exactly one item: {payload}");

    // ── (b) a trajectory parquet row was actually WRITTEN, read back
    //        through canon-learn's own public store API. ──
    let regime_key = RegimeKey::parse(&regime_key_str).unwrap();
    let learn_root = repo.join("canon/learn");
    let trajectory_store = ParquetTrajectoryStore::open(learn_root.join("trajectories"));
    let rows = trajectory_store.query_by_regime_key(&regime_key).expect("query_by_regime_key must not error");
    assert_eq!(rows.len(), 1, "exactly one trajectory row must be readable back from the parquet store");
    let row = &rows[0];
    assert_eq!(row.regime_key, regime_key);
    assert_eq!(row.verdicts.len(), 1);
    assert_eq!(row.verdicts[0].role.as_str(), "dev");
    assert_eq!(row.verdicts[0].polarity, Polarity::Failure, "an open code-review finding is a failure-polarity verdict");

    // ── (c) canon-report's mart_role_memory AND mart_flywheel_funnel
    //        render NON-EMPTY from that freshly-ingested data, via the
    //        real `duckdb` CLI canon-report itself shells out to. ──
    assert!(
        duckdb_available(),
        "duckdb required for the S14 end-to-end mart proof: the mart_role_memory / \
         mart_flywheel_funnel assertions below are the ONLY thing in this test that proves \
         the S9 mart path is actually connected to this ingest (the binary+parquet assertions \
         above only prove the write side) — a missing `duckdb` CLI must FAIL this test, never \
         silently skip past the one proof this capstone test exists to give"
    );
    let roots = canon_report::roots::Roots::new(repo.join("canon/ledger"), repo.join("canon/r2"), &learn_root);
    roots.ensure_seeded().expect("seed empty-glob placeholders for r2 (unused by these two marts)");

    let role_memory = canon_report::marts::fetch_role_memory(&roots).expect("mart_role_memory query must succeed");
    assert!(!role_memory.rows.is_empty(), "mart_role_memory must render non-empty after this ingest");
    let dev_row = role_memory
        .rows
        .iter()
        .find(|r| r.get("role").and_then(|v| v.as_str()) == Some("dev") && r.get("regime_key").and_then(|v| v.as_str()) == Some(regime_key_str.as_str()))
        .unwrap_or_else(|| panic!("no mart_role_memory row for regime_key {regime_key_str}: {:?}", role_memory.rows));
    assert_eq!(dev_row.get("strategy_count").and_then(|v| v.as_i64()), Some(1));

    let flywheel = canon_report::marts::fetch_flywheel_funnel(&roots).expect("mart_flywheel_funnel query must succeed");
    assert!(!flywheel.rows.is_empty(), "mart_flywheel_funnel must render non-empty after this ingest");
    let dev_funnel = flywheel.rows.iter().find(|r| r.get("role").and_then(|v| v.as_str()) == Some("dev")).unwrap_or_else(|| {
        panic!("no mart_flywheel_funnel row for role `dev`: {:?}", flywheel.rows)
    });
    assert_eq!(dev_funnel.get("verdicts").and_then(|v| v.as_i64()), Some(1));
    assert_eq!(dev_funnel.get("distilled").and_then(|v| v.as_i64()), Some(1));
}

/// S4 tasks.md group 6 (write-time idempotence): a SECOND `canon
/// ingest artifacts` pass over an UNCHANGED corpus must persist ZERO
/// new trajectories — `crate::artifact_ingest::run`'s existence check
/// (`trajectory_content_digest`, the `regime_key` + ordered
/// `VerdictRow` contents) recognizes the identical digest and skips
/// the write, rather than minting a fresh random `TrajectoryId` and
/// double-writing the SAME evidence into `ParquetTrajectoryStore` —
/// the genuine data-corruption bug this test guards against (an
/// unbounded re-run would otherwise silently double verdict/mart
/// counts every time `canon ingest artifacts` is re-invoked over an
/// unchanged repo, e.g. a cron/CI re-run).
#[test]
fn a_second_ingest_over_an_unchanged_corpus_persists_zero_new_trajectories() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = build_fixture_repo(tmp.path());

    // ── First pass: the baseline write. ──
    let first = run_canon(&["ingest", "artifacts", "--repo", ".", "--json"], &repo);
    assert!(first.status.success(), "first canon ingest artifacts must exit 0; stderr: {}", stderr(&first));
    let first_payload: Value = serde_json::from_str(stdout(&first).trim()).unwrap();
    assert_eq!(first_payload["trajectories_persisted"].as_array().unwrap().len(), 1, "{first_payload}");
    assert_eq!(first_payload["trajectories_skipped_duplicate"], 0, "{first_payload}");
    let regime_key_str = first_payload["trajectories_persisted"][0]["regime_key"].as_str().unwrap().to_string();

    let learn_root = repo.join("canon/learn");
    let trajectory_store = ParquetTrajectoryStore::open(learn_root.join("trajectories"));
    let regime_key = RegimeKey::parse(&regime_key_str).unwrap();
    let after_first = trajectory_store.query_by_regime_key(&regime_key).expect("query_by_regime_key must not error");
    assert_eq!(after_first.len(), 1, "exactly one trajectory row after the first pass");
    let first_id = after_first[0].id;

    // ── Second pass over the SAME, unchanged corpus. ──
    let second = run_canon(&["ingest", "artifacts", "--repo", ".", "--json"], &repo);
    assert!(second.status.success(), "second canon ingest artifacts must exit 0; stderr: {}", stderr(&second));
    let second_payload: Value = serde_json::from_str(stdout(&second).trim()).unwrap();

    // No NEW trajectory persisted, no verdict double-counted.
    assert_eq!(
        second_payload["trajectories_persisted"].as_array().unwrap().len(),
        0,
        "an unchanged corpus must persist ZERO new trajectories on re-ingest: {second_payload}"
    );
    assert_eq!(
        second_payload["trajectories_skipped_duplicate"], 1,
        "the one unchanged trajectory must be reported as a skipped duplicate, never silently dropped: {second_payload}"
    );
    assert_eq!(
        second_payload["verdicts_derived"], 1,
        "the same single verdict is still DERIVED (parsed) every pass — only the PERSIST step dedups: {second_payload}"
    );
    assert_eq!(second_payload["strategy_items_rebuilt"], 0, "no distilled-tier rebuild needed for an unchanged corpus: {second_payload}");

    // ── The parquet store itself is unchanged: still exactly ONE row
    //    under this regime_key, the SAME id, the SAME content — a
    //    double-write would make this 2 (the exact corruption S4 6.1/
    //    6.2 exists to prevent). ──
    let after_second = trajectory_store.query_by_regime_key(&regime_key).expect("query_by_regime_key must not error");
    assert_eq!(after_second.len(), 1, "the store must still hold exactly ONE trajectory row for this regime after the second pass");
    assert_eq!(after_second[0].id, first_id, "the SAME trajectory row must be found, never a fresh duplicate with a new id");
    assert_eq!(after_second[0].verdicts, after_first[0].verdicts, "verdict content must be byte-identical across both passes");
}

/// The documented seam (module doc of `crate::artifact_ingest`): when
/// `handoff` is NOT routed (no `routing.handoff` entry in `canon.yaml`),
/// the records-source read fails BEFORE `parse` ever runs — reported as
/// `status: "unavailable"` with an explicit reason, never silently
/// folded into the same shape a genuinely-empty read would produce.
/// Every PATH-source adapter (and persistence) still completes
/// normally — an absent records source degrades only its OWN
/// contribution, never the whole pass.
#[test]
fn an_unrouted_handoff_source_degrades_to_unavailable_without_aborting_the_rest_of_the_pass() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("acme-repo-unrouted");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(repo.join("canon.yaml"), "tiers:\n  local: { backend: git, root: canon/ledger }\n\nartifacts:\n  ledger_root: fixtures/ledger\n").unwrap();
    plant_ledger_code_review_finding(&repo.join("fixtures/ledger"));

    let output = run_canon(&["ingest", "artifacts", "--repo", ".", "--json"], &repo);
    assert!(output.status.success(), "stderr: {}", stderr(&output));

    let payload: Value = serde_json::from_str(stdout(&output).trim()).unwrap();
    let adapters = payload["adapters"].as_array().unwrap();
    let handoff_summary = adapters.iter().find(|a| a["adapter_id"] == "handoff").expect("handoff adapter entry present");
    assert_eq!(handoff_summary["status"], "unavailable", "{payload}");
    assert_eq!(handoff_summary["events_parsed"], 0);
    assert!(
        handoff_summary["unavailable_reason"].as_str().is_some_and(|s| !s.is_empty()),
        "an unavailable records source must carry a non-empty reason: {payload}"
    );

    // The ledger PATH-source adapter, and persistence, still ran.
    let ledger_summary = adapters.iter().find(|a| a["adapter_id"] == "ledger").unwrap();
    assert_eq!(ledger_summary["status"], "read");
    assert_eq!(payload["trajectories_persisted"].as_array().unwrap().len(), 1, "{payload}");
}

/// ReviewS14 finding 1: `canon_cli::artifact_ingest::run` must
/// propagate `LearnConfig::from_manifest`'s `Err` — a malformed
/// `learn:` section (here, a `roles:` entry that is not a valid
/// kebab-slug `RoleId`, `canon_model::ids::RoleId::GRAMMAR`) — as the
/// command's own error, exiting nonzero, rather than silently falling
/// back to `LearnConfig::default()` and persisting this run's
/// trajectories into the WRONG store (`<repo>/canon/learn`, the
/// built-in default) when the repo actually configured a different
/// `learn.root`. Contrast with `LearnConfig::from_manifest`'s OWN
/// documented default-on-absent case (crates/canon-learn/src/
/// config.rs): a genuinely ABSENT `learn:` section is not this test's
/// concern and is untouched — every OTHER test in this file configures
/// no `learn:` section at all and still ingests + persists
/// successfully into `<repo>/canon/learn` (the clean-default path).
#[test]
fn a_malformed_learn_config_fails_loud_instead_of_silently_defaulting() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("acme-repo-malformed-learn");
    std::fs::create_dir_all(&repo).unwrap();
    // `learn.root` names a store OTHER than the built-in default
    // (`canon/learn`) — proving a silent default-fallback would land
    // trajectories in the WRONG place, not merely skip a custom root.
    std::fs::write(
        repo.join("canon.yaml"),
        "learn:\n  root: canon/learn-custom\n  roles:\n    - \"Not A Valid Role!\"\n",
    )
    .unwrap();

    let output = run_canon(&["ingest", "artifacts", "--repo", ".", "--json"], &repo);
    assert!(
        !output.status.success(),
        "a malformed `learn:` section must exit nonzero, never silently succeed: stdout: {} stderr: {}",
        stdout(&output),
        stderr(&output)
    );
    let err = stderr(&output);
    assert!(err.contains("canon ingest artifacts:"), "stderr must carry the command-prefixed error line: {err}");
    assert!(
        err.contains("RoleId") || err.to_lowercase().contains("role"),
        "stderr must surface the underlying RoleId grammar error, not a generic failure: {err}"
    );

    // The whole point of this test: a malformed config must NOT fall
    // through to silently opening (and writing into) the built-in
    // default store, nor the configured-but-unreached custom root.
    assert!(
        !repo.join("canon/learn").exists(),
        "a malformed learn config must never fall through to writing the default `<repo>/canon/learn` store"
    );
    assert!(!repo.join("canon/learn-custom").exists(), "nor may it write to the configured-but-unreached custom root");
}

#[test]
fn ingest_artifacts_help_smoke() {
    let tmp = tempfile::tempdir().unwrap();
    let output = run_canon(&["ingest", "artifacts", "--help"], tmp.path());
    assert!(output.status.success());
    assert!(stdout(&output).contains("--repo"));
    assert!(stdout(&output).contains("--watch"));
}

/// S15 P4 (design D7) acceptance (a): a `Review` AND a resolved
/// `Divergence` in the SAME run both produce `Trajectory` records —
/// neither native kind is dropped because a single-kind dispatch could
/// read only one (spec `native-record-flywheel` Scenario "A Review and
/// a Divergence in the same run both produce trajectories"), each
/// reachable from `ParquetTrajectoryStore` exactly like an
/// S4-raw-artifact-derived trajectory, with the `regime_key` role
/// equal to the RECORD's own `actor.role` (`content`/`test`) — never a
/// `derive_verdict`-hard-coded `dev`/`design` constant.
#[test]
fn native_flywheel_review_and_divergence_resolved_in_same_run_both_produce_trajectories() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = build_native_flywheel_repo(tmp.path());

    let output = run_canon(&["ingest", "artifacts", "--repo", ".", "--json"], &repo);
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    let payload: Value = serde_json::from_str(stdout(&output).trim()).expect("--json prints a single JSON outcome document");

    let adapters = payload["adapters"].as_array().expect("adapters array");
    let review_summary = adapters.iter().find(|a| a["adapter_id"] == "review").unwrap_or_else(|| panic!("no `review` adapter entry: {payload}"));
    assert_eq!(review_summary["source_kind"], "records");
    assert_eq!(review_summary["status"], "read", "native_records:true must actually DRIVE `review`, never `disabled`/`unavailable`: {payload}");
    assert_eq!(review_summary["events_parsed"], 1, "{payload}");

    let divergence_summary = adapters
        .iter()
        .find(|a| a["adapter_id"] == "divergence-native")
        .unwrap_or_else(|| panic!("no `divergence-native` adapter entry: {payload}"));
    assert_eq!(divergence_summary["status"], "read", "{payload}");
    assert_eq!(divergence_summary["events_parsed"], 1, "{payload}");

    assert_eq!(
        payload["verdicts_derived"], 2,
        "both the Review and the resolved Divergence must derive exactly one verdict each: {payload}"
    );

    let persisted = payload["trajectories_persisted"].as_array().expect("trajectories_persisted array");
    assert_eq!(persisted.len(), 2, "neither native record's trajectory may be dropped by the other's dispatch: {payload}");

    let regime_keys: Vec<String> = persisted.iter().map(|p| p["regime_key"].as_str().unwrap().to_string()).collect();
    let review_regime = regime_keys.iter().find(|k| k.starts_with("content/acme-repo-native/world/")).unwrap_or_else(|| {
        panic!("no regime_key with role `content` (the Review's own actor.role, never a hard-coded `dev`/`design`): {regime_keys:?}")
    });
    let divergence_regime = regime_keys.iter().find(|k| k.starts_with("test/acme-repo-native/world/")).unwrap_or_else(|| {
        panic!("no regime_key with role `test` (the Divergence's own actor.role, never derive_verdict's hard-coded `dev`): {regime_keys:?}")
    });

    // ── Each is reachable from the parquet trajectory store, exactly
    //    like an S4-derived trajectory (`ingest_artifacts_drives_
    //    both_source_shapes...` above proves the same read-back shape
    //    for the raw-artifact path). ──
    let learn_root = repo.join("canon/learn");
    let trajectory_store = ParquetTrajectoryStore::open(learn_root.join("trajectories"));

    let review_key = RegimeKey::parse(review_regime).unwrap();
    let review_rows = trajectory_store.query_by_regime_key(&review_key).expect("query_by_regime_key must not error");
    assert_eq!(review_rows.len(), 1);
    assert_eq!(review_rows[0].verdicts.len(), 1);
    assert_eq!(review_rows[0].verdicts[0].role.as_str(), "content");
    assert_eq!(review_rows[0].verdicts[0].polarity, Polarity::Success, "a Review's mere existence is always a positive verdict");

    let divergence_key = RegimeKey::parse(divergence_regime).unwrap();
    let divergence_rows = trajectory_store.query_by_regime_key(&divergence_key).expect("query_by_regime_key must not error");
    assert_eq!(divergence_rows.len(), 1);
    assert_eq!(divergence_rows[0].verdicts.len(), 1);
    assert_eq!(divergence_rows[0].verdicts[0].role.as_str(), "test");
    assert_eq!(divergence_rows[0].verdicts[0].polarity, Polarity::Success, "a Resolved divergence is a success-polarity verdict");
}

/// S15 P4 (design D7) acceptance (b): `artifacts.native_records: true`
/// together with a raw-artifact path field (`ledger_root`) fails
/// config validation BEFORE any read runs (spec
/// `native-record-flywheel` Scenario "native_records with a
/// raw-artifact path fails config validation") — nonzero exit, no
/// trajectory written (not even the ledger's own, otherwise-valid
/// finding).
#[test]
fn native_records_true_with_a_raw_path_fails_the_xor_before_any_read() {
    let tmp = tempfile::tempdir().unwrap();
    let repo = tmp.path().join("acme-repo-xor");
    std::fs::create_dir_all(&repo).unwrap();
    std::fs::write(
        repo.join("canon.yaml"),
        "tiers:\n  local: { backend: git, root: canon/ledger }\n\nartifacts:\n  native_records: true\n  ledger_root: fixtures/ledger\n",
    )
    .unwrap();
    plant_ledger_code_review_finding(&repo.join("fixtures/ledger"));

    let output = run_canon(&["ingest", "artifacts", "--repo", ".", "--json"], &repo);
    assert!(
        !output.status.success(),
        "native_records:true + ledger_root must fail before any read, never silently succeed: stdout: {} stderr: {}",
        stdout(&output),
        stderr(&output)
    );
    let err = stderr(&output);
    assert!(err.contains("canon ingest artifacts:"), "stderr must carry the command-prefixed error line: {err}");
    assert!(err.contains("native_records"), "stderr must name the XOR conflict by field name: {err}");

    // The whole point: config validation runs BEFORE any adapter read,
    // so not even the ledger's own (otherwise perfectly valid) finding
    // is ever parsed — no learn store is created at all.
    assert!(
        !repo.join("canon/learn").exists(),
        "no trajectory may be written when config validation rejects the run before any read"
    );
}

/// S15 P4 (design D7) acceptance (c): the `Handoff` adapter runs
/// IDENTICALLY whether `artifacts.native_records` is unset/false or
/// `true` (spec `native-record-flywheel` Scenario "The handoff adapter
/// is unaffected by the native_records switch") — its own
/// `ArtifactAdapterSummary` is byte-identical across both runs, while
/// the native `review`/`divergence-native` adapters visibly change
/// status (`"disabled"` when the switch is off, `"read"` when it's on
/// with their kinds routed) — proving the switch scopes ONLY the two
/// native adapters.
#[test]
fn handoff_adapter_is_unaffected_by_the_native_records_switch() {
    let tmp = tempfile::tempdir().unwrap();

    let repo_off = tmp.path().join("acme-repo-switch-off");
    std::fs::create_dir_all(&repo_off).unwrap();
    std::fs::write(repo_off.join("canon.yaml"), "tiers:\n  local: { backend: git, root: canon/ledger }\n\nrouting:\n  handoff: local\n").unwrap();
    plant_handoff_in_git(&repo_off.join("canon/ledger"));

    let repo_on = tmp.path().join("acme-repo-switch-on");
    std::fs::create_dir_all(&repo_on).unwrap();
    std::fs::write(
        repo_on.join("canon.yaml"),
        "tiers:\n  local: { backend: git, root: canon/ledger }\n\nrouting:\n  handoff: local\n  review: local\n  divergence: local\n\nartifacts:\n  native_records: true\n",
    )
    .unwrap();
    plant_handoff_in_git(&repo_on.join("canon/ledger"));

    let off = run_canon(&["ingest", "artifacts", "--repo", ".", "--json"], &repo_off);
    assert!(off.status.success(), "stderr: {}", stderr(&off));
    let off_payload: Value = serde_json::from_str(stdout(&off).trim()).unwrap();

    let on = run_canon(&["ingest", "artifacts", "--repo", ".", "--json"], &repo_on);
    assert!(on.status.success(), "stderr: {}", stderr(&on));
    let on_payload: Value = serde_json::from_str(stdout(&on).trim()).unwrap();

    let off_handoff = off_payload["adapters"].as_array().unwrap().iter().find(|a| a["adapter_id"] == "handoff").expect("handoff entry (off)");
    let on_handoff = on_payload["adapters"].as_array().unwrap().iter().find(|a| a["adapter_id"] == "handoff").expect("handoff entry (on)");
    assert_eq!(
        off_handoff, on_handoff,
        "the Handoff adapter's own summary must be BYTE-IDENTICAL whether native_records is off or on: off={off_payload} on={on_payload}"
    );
    assert_eq!(off_handoff["status"], "read");
    assert_eq!(off_handoff["events_parsed"], 3, "created + claimed + done, from the planted handoff's own state transitions");

    // The switch DOES visibly change the native adapters' own status —
    // proving it is not simply a no-op, only that it never touches
    // `handoff`.
    let review_off = off_payload["adapters"].as_array().unwrap().iter().find(|a| a["adapter_id"] == "review").expect("review entry (off)");
    assert_eq!(review_off["status"], "disabled", "native_records is off in repo_off — `review` must be disabled, never read/unavailable");

    let review_on = on_payload["adapters"].as_array().unwrap().iter().find(|a| a["adapter_id"] == "review").expect("review entry (on)");
    assert_eq!(review_on["status"], "read", "native_records is on and `review` is routed in repo_on — it must actually run");
    assert_eq!(review_on["events_parsed"], 0, "no Review record was planted in repo_on — zero events, but still a genuine READ, not disabled");
}
