//! Integration tests for s22 `query-tier-degradation` /
//! `uniform-lenient-tier-build` (`openspec/changes/
//! s22-query-tier-degradation/`): `canon query` (both `run` and
//! `run_with_plugin`) now attaches only the tier(s) the requested
//! `--kind` actually needs, via `canon-cli::tiers::
//! build_lenient_tiers_for_kind` -- an unreachable-but-irrelevant
//! `pg`/`r2` tier degrades to a no-op instead of hard-failing every
//! `--kind` (the exact SYNTHESIS-ROUND2 #1 repro this change fixes);
//! a query whose OWN routed tier is unavailable still fails, named.
//!
//! Every fixture here writes its OWN `canon.yaml` (rather than reusing
//! `support::Fixture::new`, which never declares a `pg` tier) so each
//! test controls precisely which tiers are configured/reachable --
//! zero network, no credentials, mirroring `tests/plans_ingest.rs`'s
//! own "own write_canon_yaml/run_canon helpers" discipline.
//!
//! A genuinely LIVE `PgTier::connect` requires network (`pg_tier.rs`'s
//! own module doc: only `tests/pg_tier_live.rs`, gated behind the
//! `live-pg` feature, exercises real Postgres) -- so the "pg succeeds,
//! r2 fails" half of an aging pair's fan-out (spec task 3.5's full
//! scenario) and `--plugin`'s git-unconditional-attach proof (task
//! 3.6) are both exercised here with `r2` standing in for `pg` (via
//! the `CANON_R2_LOCAL_ROOT` debug-build test seam `tiers.rs` already
//! documents) -- the SAME tier-agnostic mechanism (design.md D2/D4),
//! offline. `crates/canon-cli/src/tiers.rs`'s own
//! `lenient_tier_tests::kind_scoped_build_attempts_both_tiers_for_a_pg_routed_r2_aged_kind`
//! proves the `pg`-specific half (both tiers independently attempted
//! at BUILD time, never narrowed to the routed tier alone) without
//! needing a live connection.

use std::path::Path;
use std::process::{Command, Output};

use canon_model::envelope::{Actor, Envelope, RecordKind};
use canon_model::ids::{ChangeId, ProjectId, RoleId, ScenarioId, SpecDigest};
use canon_model::records::{Change, ChangeStatus, Scenario};
use canon_plugin::manifest::schema::FieldDecl;
use canon_plugin::manifest::snapshot::OverlayDecl;
use canon_plugin::manifest::types::Type;
use canon_plugin::overlay::{compose_overlay_body, write_overlay, OverlayEnvelope};
use canon_store::git_tier::GitTier;
use canon_store::r2_tier::R2Tier;
use canon_store::tier::Tier;
use chrono::Utc;
use serde_json::{json, Value};

fn write_canon_yaml(root: &Path, body: &str) {
    std::fs::write(root.join("canon.yaml"), body).unwrap();
}

/// Spawn the built `canon` binary with a FULLY EXPLICIT environment:
/// `CANON_R2_LOCAL_ROOT` is always removed first (the debug-build
/// offline-r2 test seam `tiers.rs` documents), so `r2` genuinely falls
/// through to `R2Tier::connect_live` and fails loud on an unset
/// `bucket_env`, UNLESS a caller re-adds it via `extra_env` -- no
/// ambient-environment leakage into what each test asserts about
/// tier reachability.
fn run_canon(args: &[&str], cwd: &Path, extra_env: &[(&str, &str)]) -> Output {
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_canon"));
    cmd.args(args).current_dir(cwd).env_remove("CANON_R2_LOCAL_ROOT");
    for (k, v) in extra_env {
        cmd.env(k, v);
    }
    cmd.output().expect("spawn canon binary")
}

fn stdout(o: &Output) -> String {
    String::from_utf8_lossy(&o.stdout).into_owned()
}

fn stderr(o: &Output) -> String {
    String::from_utf8_lossy(&o.stderr).into_owned()
}

fn actor() -> Actor {
    Actor::new("test-agent", RoleId::parse("implementer").unwrap())
}

fn plant_change(git_root: &Path, change_id: &str, title: &str) {
    let git = GitTier::new(git_root);
    let record = Change::new(Envelope::new(1, RecordKind::Change, Utc::now(), actor()), ChangeId::parse(change_id).unwrap(), title, "", ChangeStatus::Proposed);
    git.write(&record).unwrap();
}

fn plant_scenario_in_r2(r2_root: &Path, project_id: &str, scenario_id: &str, title: &str) {
    let r2 = R2Tier::local(r2_root, "canon/").unwrap();
    let record = Scenario::new(
        Envelope::new(1, RecordKind::Scenario, Utc::now(), actor()),
        ProjectId::parse(project_id).unwrap(),
        ScenarioId::parse(scenario_id).unwrap(),
        title,
        "",
        SpecDigest::parse("a".repeat(64)).unwrap(),
    );
    r2.write(&record).unwrap();
}

fn write_plugin_manifest(root: &Path, id: &str, yaml: &str) {
    let dir = root.join("canon/plugins").join(id);
    std::fs::create_dir_all(&dir).unwrap();
    std::fs::write(dir.join("plugin.yaml"), yaml).unwrap();
}

const PORTING_YAML: &str = "id: porting\nnamespace: porting\noverlays:\n  - kind: coverage\n    attaches_to:\n      core_kind: scenario\n      join_key: [project_id, scenario_id]\n    fields:\n      - name: covered\n        type: bool\n      - name: surface_ref\n        type: { list: string }\n";

fn coverage_decl() -> OverlayDecl {
    OverlayDecl {
        namespace: "porting".to_string(),
        kind: "coverage".to_string(),
        identity: "porting.coverage".to_string(),
        core_kind: "scenario".to_string(),
        join_key: vec!["project_id".to_string(), "scenario_id".to_string()],
        fields: vec![
            FieldDecl { name: "covered".to_string(), ty: Type::Bool },
            FieldDecl { name: "surface_ref".to_string(), ty: Type::List(Box::new(Type::Str)) },
        ],
    }
}

/// Planted straight into the GIT tier -- `resolve_and_project`
/// (`query.rs`) always scans overlay records via the git tier
/// (`git.scan_namespaced_kind`), independent of `TierPolicy.routing`
/// for the CORE kind, which is exactly the mechanism a git-unconditional
/// `--plugin` attach must keep working.
fn plant_overlay(git_root: &Path, project_id: &str, scenario_id: &str, covered: bool, surface_ref: &[&str]) {
    let git = GitTier::new(git_root);
    let envelope = OverlayEnvelope::new(1, "porting.coverage", Utc::now(), actor());
    let mut fields = serde_json::Map::new();
    fields.insert("project_id".to_string(), json!(project_id));
    fields.insert("scenario_id".to_string(), json!(scenario_id));
    fields.insert("covered".to_string(), json!(covered));
    fields.insert("surface_ref".to_string(), json!(surface_ref));
    let body = compose_overlay_body(&envelope, fields);
    write_overlay(&git, &coverage_decl(), body).expect("plant a well-formed overlay record");
}

// ── query-tier-degradation Requirement 1/2: git-routed kinds never touch pg/r2 ──

/// The exact SYNTHESIS-ROUND2 #1 repro (task 3.3): a `canon.yaml`
/// declaring `git`+`pg`+`r2`, neither `pg`'s `dsn_env` nor `r2`'s
/// `bucket_env` reachable, `--kind change` (git-routed) still
/// succeeds and returns the `change` records.
#[test]
fn git_routed_kind_query_succeeds_when_pg_and_r2_are_both_unreachable() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(
        dir.path(),
        "tiers:\n  local: { backend: git, root: canon/ledger }\n  hot: { backend: postgres, dsn_env: CANON_PG_DSN_S22_QD_T1, schema: canon_v1 }\n  cold: { backend: s3, bucket_env: CANON_R2_BUCKET_S22_QD_T1, prefix: \"canon/\" }\nrouting:\n  change: local\n  task: hot\n",
    );
    plant_change(&dir.path().join("canon/ledger"), "add-widget", "Adds a widget");

    let output = run_canon(&["query", "--kind", "change", "--json"], dir.path(), &[]);
    assert!(output.status.success(), "a git-routed kind must never require live pg/r2 credentials: stderr={}", stderr(&output));

    let payload: Value = serde_json::from_str(&stdout(&output)).expect("valid JSON on stdout");
    assert_eq!(payload["kind"], "change");
    assert_eq!(payload["count"], 1, "{payload}");
    assert_eq!(payload["records"][0]["change_id"], "add-widget", "{payload}");
}

// ── query-tier-degradation Requirement 3: a query's OWN routed tier fails named ──

/// task 3.4 / spec "A hot-routed kind's query fails naming the rung and
/// backend": `canon query --kind task` (hot-routed) with `CANON_PG_DSN`
/// unset fails non-zero, naming `hot`/`postgres` and "no live DSN" --
/// reached via `TierRegistry::query`'s existing named
/// `StoreError::TierUnavailable`, never pre-empted by a build-time
/// hard error, never silent.
#[test]
fn pg_routed_kind_query_fails_naming_pg_and_the_no_live_dsn_reason() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(
        dir.path(),
        "tiers:\n  local: { backend: git, root: canon/ledger }\n  hot: { backend: postgres, dsn_env: CANON_PG_DSN_S22_QD_T2, schema: canon_v1 }\nrouting:\n  change: local\n  task: hot\n",
    );

    let output = run_canon(&["query", "--kind", "task"], dir.path(), &[]);
    assert!(!output.status.success(), "an unavailable OWN-routed tier must fail the command, never a silent empty result");
    let err = stderr(&output);
    assert!(err.contains("hot"), "error must name the rung: {err}");
    assert!(err.contains("postgres"), "error must name the backend: {err}");
    assert!(err.contains("no live DSN"), "error must name the reason: {err}");
}

/// spec "A cold-routed kind's query fails naming the rung and backend,
/// distinctly from a hot failure": `canon query --kind trajectory`
/// (cold-routed) with `CANON_R2_BUCKET` unreachable fails non-zero,
/// naming `cold`/`s3` and "no live bucket" -- textually distinct from
/// the hot case above.
#[test]
fn r2_routed_kind_query_fails_naming_r2_distinctly_from_a_pg_failure() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(
        dir.path(),
        "tiers:\n  local: { backend: git, root: canon/ledger }\n  cold: { backend: s3, bucket_env: CANON_R2_BUCKET_S22_QD_T3, prefix: \"canon/\" }\nrouting:\n  change: local\n  trajectory: cold\n",
    );

    let output = run_canon(&["query", "--kind", "trajectory"], dir.path(), &[]);
    assert!(!output.status.success(), "an unavailable OWN-routed cold tier must fail the command");
    let err = stderr(&output);
    assert!(err.contains("cold"), "error must name the rung: {err}");
    assert!(err.contains("s3"), "error must name the backend: {err}");
    assert!(err.contains("no live bucket"), "error must name the reason: {err}");
    assert!(!err.contains("no live DSN"), "must never be confused with the hot-tier wording: {err}");
}

/// spec "The command's exit code is non-zero for an own-tier-unavailable
/// failure" (both scenarios above already assert `!success()`, this
/// pins the concrete non-zero code so a future regression to e.g. a
/// caught-and-swallowed `0` is caught).
#[test]
fn own_tier_unavailable_failure_exits_with_a_nonzero_code() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(dir.path(), "tiers:\n  local: { backend: git, root: canon/ledger }\n  hot: { backend: postgres, dsn_env: CANON_PG_DSN_S22_QD_T2B, schema: canon_v1 }\nrouting:\n  task: hot\n");

    let output = run_canon(&["query", "--kind", "task"], dir.path(), &[]);
    assert_eq!(output.status.code(), Some(1), "canon query must exit 1, not merely a nonzero-but-unspecified code");
}

// ── query-tier-degradation Requirement 2: malformed config still fails loud ──

/// spec "A malformed pg schema still fails loud regardless of DSN
/// presence": a `tiers.pg.schema` failing `validate_schema_ident`
/// fails the whole command loud -- validated BEFORE the `dsn_env`
/// lookup, so an unset-DSN degrade can never mask it.
#[test]
fn a_malformed_pg_schema_fails_loud_never_masked_by_the_unset_dsn_degrade() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(
        dir.path(),
        "tiers:\n  local: { backend: git, root: canon/ledger }\n  hot: { backend: postgres, dsn_env: CANON_PG_DSN_S22_QD_T4, schema: Bad-Schema }\nrouting:\n  task: hot\n",
    );

    let output = run_canon(&["query", "--kind", "task"], dir.path(), &[]);
    assert!(!output.status.success(), "a malformed tiers.pg.schema must fail loud, never degrade to unwritten/empty");
    let err = stderr(&output);
    assert!(err.contains("Bad-Schema"), "the fatal error must name the offending schema: {err}");
    assert!(err.contains("must match"), "must be the schema-validation error, not an unset-DSN degrade: {err}");
}

// ── uniform-lenient-tier-build / design.md R2: an aging pair attempts BOTH tiers ──

/// task 3.5 (offline half): `canon query --kind handoff` (pg-routed,
/// r2-aged) with BOTH `pg`/`r2` unreachable still fails NAMED (never a
/// generic/unnamed error) -- `TierRegistry::tiers_for_read`'s own
/// tiers-in-order contract means the FIRST unavailable tier in the
/// fan-out (`pg`, the routed tier) is the one the error names. The
/// build-time proof that BOTH tiers are independently ATTEMPTED (never
/// narrowed to `pg` alone) lives in `tiers.rs`'s own
/// `kind_scoped_build_attempts_both_tiers_for_a_pg_routed_r2_aged_kind`
/// unit test -- a live-pg-succeeds/r2-fails CLI-level proof needs a
/// real Postgres, out of scope for this offline suite (module doc).
#[test]
fn pg_routed_r2_aged_kind_query_fails_named_never_silently() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(
        dir.path(),
        "tiers:\n  local: { backend: git, root: canon/ledger }\n  hot: { backend: postgres, dsn_env: CANON_PG_DSN_S22_QD_T5, schema: canon_v1 }\n  cold: { backend: s3, bucket_env: CANON_R2_BUCKET_S22_QD_T5, prefix: \"canon/\" }\nrouting:\n  handoff: hot\naging:\n  handoff: { after: 30d, to: cold }\n",
    );

    let output = run_canon(&["query", "--kind", "handoff"], dir.path(), &[]);
    assert!(!output.status.success(), "an aging pair with its routed tier unreachable must still fail loud, never a fabricated empty success");
    let err = stderr(&output);
    assert!(err.contains("hot"), "must name the routed rung that was actually unavailable: {err}");
}

// ── query-tier-degradation Requirement 1: --plugin's git attach is unconditional ──

/// task 3.6 / design.md R1 (offline-adapted, module doc): `canon query
/// --kind scenario --plugin porting` where `scenario` routes to `r2`
/// alone (so `git` is scoped OUT of `scenario`'s OWN read fan-out,
/// `tiers_needed_for(Scenario) == [R2]`) and `r2` IS reachable (the
/// `CANON_R2_LOCAL_ROOT` debug test seam) -- the plugin overlay still
/// resolves and projects, proving `git` was attached UNCONDITIONALLY,
/// never itself scoped by the queried kind's own routing.
#[test]
fn plugin_git_attachment_is_unconditional_even_when_the_queried_kinds_own_routing_excludes_git() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(
        dir.path(),
        "tiers:\n  local: { backend: git, root: canon/ledger }\n  hot: { backend: postgres, dsn_env: CANON_PG_DSN_S22_QD_T6, schema: canon_v1 }\n  cold: { backend: s3, bucket_env: CANON_R2_BUCKET_S22_QD_T6, prefix: \"canon/\" }\nrouting:\n  scenario: cold\n",
    );
    let r2_root = dir.path().join("r2-local");
    plant_scenario_in_r2(&r2_root, "root", "world.hotdeal.01", "a scenario");
    write_plugin_manifest(dir.path(), "porting", PORTING_YAML);
    plant_overlay(&dir.path().join("canon/ledger"), "root", "world.hotdeal.01", true, &["world.hotdeal.01"]);

    let output = run_canon(
        &["query", "--kind", "scenario", "--plugin", "porting", "--json"],
        dir.path(),
        &[("CANON_R2_LOCAL_ROOT", r2_root.to_str().unwrap())],
    );
    assert!(output.status.success(), "stderr: {}", stderr(&output));

    let payload: Value = serde_json::from_str(&stdout(&output)).expect("valid JSON on stdout");
    let records = payload["records"].as_array().expect("records array");
    let record = records.iter().find(|r| r["scenario_id"] == "world.hotdeal.01").expect("planted scenario present");
    assert_eq!(
        record["overlay"]["porting.coverage"]["covered"], true,
        "the overlay must have actually PROJECTED (not merely degraded to the unmodified core view), proving git was attached even though scenario's own routing never needs it: {record}"
    );
}

// ── s27 query-tier-degradation: an unconfigured rung fails naming the rung alone ──

/// s27 spec "A routed rung with no tiers.<rung> block at all fails
/// naming the rung alone": `canon.yaml`'s `routing.task` is `hot`, but
/// `tiers:` declares no `hot` entry at all -- the error names `hot`
/// and states it is not configured, never fabricating a backend name
/// it was never told.
#[test]
fn unconfigured_rung_query_fails_naming_the_rung_alone() {
    let dir = tempfile::tempdir().unwrap();
    write_canon_yaml(dir.path(), "tiers:\n  local: { backend: git, root: canon/ledger }\nrouting:\n  task: hot\n");

    let output = run_canon(&["query", "--kind", "task"], dir.path(), &[]);
    assert!(!output.status.success(), "an unconfigured routed rung must fail the command, never a silent empty result");
    let err = stderr(&output);
    assert!(err.contains("hot"), "error must name the rung: {err}");
    assert!(err.contains("not configured"), "error must state the rung is not configured, never fabricate a backend: {err}");
}
