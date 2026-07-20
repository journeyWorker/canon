//! Integration tests for `canon plugin sync <plugin-id> [--spec-root <dir>]`
//! (s16 P4, `openspec/changes/s16-plugin-extensibility/`, tasks.md 4.4,
//! design.md D5, `porting-plugin` spec) — the porting plugin as the
//! acceptance vehicle for `canon-plugin`'s P1-P3 machinery. Invokes the
//! actually-built `canon` binary (`env!("CARGO_BIN_EXE_canon")`) against
//! an offline git-tier fixture repo, zero network, no credentials
//! (mirrors `tests/gate.rs`'s own subprocess-boundary discipline).
//!
//! Covers every scenario tasks.md 4.4 names: a covered scenario projects
//! `covered: true` + its `surface_ref`; an uncovered scenario projects
//! `covered: false` + an empty `surface_ref`; a second `canon plugin
//! sync porting` run over an unchanged inventory writes zero new
//! overlay records; `canon gate check`'s verdicts are byte-identical
//! with and without a porting sync run; a core `Scenario` record's
//! on-disk bytes are byte-identical before/after a porting sync +
//! `canon query --plugin` round-trip; and canon-gate's own crate source
//! carries no reference to the plugin-specific names `porting`/
//! `porting.coverage`/`scan_namespaced_kind`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;

const PORTING_YAML: &str = "id: porting\nnamespace: porting\noverlays:\n  - kind: coverage\n    attaches_to:\n      core_kind: scenario\n      join_key: [project_id, scenario_id]\n    fields:\n      - name: covered\n        type: bool\n      - name: surface_ref\n        type: { list: string }\n";

fn run_canon(args: &[&str], cwd: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(cwd).output().expect("spawn canon binary")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn write(dir: &Path, rel: &str, content: &str) {
    let path = dir.join(rel);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, content).unwrap();
}

fn provenance() -> String {
    "  # canon: {\"schema\":1,\"at\":\"2026-07-10T00:00:00Z\",\"actor\":{\"agent_id\":\"canon-fmt\"}}".to_string()
}

/// A fixture repo carrying: `canon.yaml` (git tier + `scenario: git`
/// routing), `canon/plugins/porting/plugin.yaml` (task 4.1's own
/// manifest, byte-identical to the checked-in one), a clean two-scenario
/// `.feature` corpus (`idolive.hub.01` covered, `idolive.hub.02` not),
/// and an `inventory/` entry whose `covered_by` names ONLY
/// `idolive.hub.01` — the exact "one covered, one not" shape
/// `porting-plugin` spec's own scenarios describe.
fn write_repo(dir: &Path) {
    write(dir, "canon.yaml", "tiers:\n  local: { backend: git, root: canon/ledger }\nrouting:\n  scenario: local\n");
    write(dir, "canon/plugins/porting/plugin.yaml", PORTING_YAML);

    let prov = provenance();
    let feature_text = format!(
        "Feature: idolive hub\n{prov}\n\n  @idolive.hub.01\n  Scenario: Opening the hub\n{prov}\n    Given a step\n\n  @idolive.hub.02\n  Scenario: Never covered\n{prov}\n    Given a step\n"
    );
    write(dir, "specs/features/kind=feature/area=idolive/hub.feature", &feature_text);

    write(
        dir,
        "specs/inventory/kind=inventory/area=idolive/surface=hub/hub.yaml",
        "schema: 1\nkind: inventory\nat: \"2026-07-10T00:00:00Z\"\nactor:\n  agent_id: test\nidolive.hub.hub-header:\n  upstream:\n    pin: a\n    file: f.tsx\n    symbol: S\n    lines: \"1-2\"\n  covered_by: [idolive.hub.01]\n",
    );
}

fn sync_inventory(dir: &Path) {
    let out = run_canon(&["inventory", "sync", "--repo", "."], dir);
    assert!(out.status.success(), "canon inventory sync failed: {}", stderr(&out));
}

fn sync_porting(dir: &Path) -> Output {
    let out = run_canon(&["plugin", "sync", "porting", "--repo", "."], dir);
    assert!(out.status.success(), "canon plugin sync porting failed: {}", stderr(&out));
    out
}

fn query_scenarios_with_plugin(dir: &Path) -> Vec<Value> {
    let out = run_canon(&["query", "--kind", "scenario", "--canon-yaml", "canon.yaml", "--plugin", "porting", "--json"], dir);
    assert!(out.status.success(), "canon query --plugin porting failed: {}", stderr(&out));
    let json: Value = serde_json::from_str(&stdout(&out)).unwrap_or_else(|e| panic!("query --json didn't parse: {e}\n{}", stdout(&out)));
    json["records"].as_array().cloned().unwrap_or_default()
}

fn scenario_record<'a>(records: &'a [Value], scenario_id: &str) -> &'a Value {
    records.iter().find(|r| r["scenario_id"] == scenario_id).unwrap_or_else(|| panic!("no scenario `{scenario_id}` in {records:?}"))
}

/// Every regular file under `dir`, recursively — a hand-rolled walker
/// (no extra dev-dependency) reused by both the overlay-file-count
/// (idempotence) and core-Scenario-byte-identity assertions below.
fn files_under(dir: &Path) -> Vec<PathBuf> {
    let mut files = Vec::new();
    if !dir.exists() {
        return files;
    }
    let mut stack = vec![dir.to_path_buf()];
    while let Some(d) = stack.pop() {
        for entry in std::fs::read_dir(&d).unwrap() {
            let entry = entry.unwrap();
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                files.push(path);
            }
        }
    }
    files
}

fn snapshot_bytes(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    files_under(dir).into_iter().map(|p| { let bytes = std::fs::read(&p).unwrap(); (p, bytes) }).collect()
}

// ── covered / uncovered projection ──

#[test]
fn a_covered_scenario_projects_covered_true_with_its_surface_ref() {
    let dir = tempfile::tempdir().unwrap();
    write_repo(dir.path());
    sync_inventory(dir.path());
    sync_porting(dir.path());

    let records = query_scenarios_with_plugin(dir.path());
    let record = scenario_record(&records, "idolive.hub.01");
    assert_eq!(record["overlay"]["porting.coverage"]["covered"], true, "{record:?}");
    assert_eq!(record["overlay"]["porting.coverage"]["surface_ref"], serde_json::json!(["idolive.hub.hub-header"]), "{record:?}");
}

#[test]
fn an_uncovered_scenario_projects_covered_false_with_an_empty_surface_ref() {
    let dir = tempfile::tempdir().unwrap();
    write_repo(dir.path());
    sync_inventory(dir.path());
    sync_porting(dir.path());

    let records = query_scenarios_with_plugin(dir.path());
    let record = scenario_record(&records, "idolive.hub.02");
    assert_eq!(record["overlay"]["porting.coverage"]["covered"], false, "{record:?}");
    assert_eq!(record["overlay"]["porting.coverage"]["surface_ref"], serde_json::json!([]), "{record:?}");
}

// ── idempotence ──

#[test]
fn a_second_porting_sync_over_an_unchanged_inventory_writes_zero_new_overlay_records() {
    let dir = tempfile::tempdir().unwrap();
    write_repo(dir.path());
    sync_inventory(dir.path());

    let first = sync_porting(dir.path());
    let overlay_dir = dir.path().join("canon/ledger/kind=porting.coverage");
    let count_after_first = files_under(&overlay_dir).len();
    assert_eq!(count_after_first, 2, "one overlay record per scanned scenario; stdout: {}", stdout(&first));
    assert!(stdout(&first).contains("2 written"), "{}", stdout(&first));

    let second = sync_porting(dir.path());
    let count_after_second = files_under(&overlay_dir).len();
    assert_eq!(count_after_second, count_after_first, "a second sync over an unchanged inventory must write zero NEW overlay records");
    assert!(stdout(&second).contains("0 written"), "{}", stdout(&second));
    assert!(stdout(&second).contains("2 deduped"), "{}", stdout(&second));
}

// ── canon-gate isolation: verdicts never move ──

#[test]
fn gate_check_verdicts_are_byte_identical_with_and_without_a_porting_sync_run() {
    let dir = tempfile::tempdir().unwrap();
    write_repo(dir.path());
    sync_inventory(dir.path());

    let before = run_canon(&["gate", "check", "--repo", "."], dir.path());

    sync_porting(dir.path());

    let after = run_canon(&["gate", "check", "--repo", "."], dir.path());

    assert_eq!(before.status.code(), after.status.code(), "exit code must be unaffected by a porting sync run");
    assert_eq!(stdout(&before), stdout(&after), "canon gate check stdout must be byte-identical with and without a porting sync run");
    assert_eq!(stderr(&before), stderr(&after), "canon gate check stderr must be byte-identical with and without a porting sync run");
}

// ── core Scenario records are read-only ──

#[test]
fn a_porting_sync_and_query_round_trip_leaves_core_scenario_files_byte_identical() {
    let dir = tempfile::tempdir().unwrap();
    write_repo(dir.path());
    sync_inventory(dir.path());

    let scenario_dir = dir.path().join("canon/ledger/kind=scenario");
    let before = snapshot_bytes(&scenario_dir);
    assert_eq!(before.len(), 2, "fixture must have materialized both Scenario records before this round-trip");

    sync_porting(dir.path());
    query_scenarios_with_plugin(dir.path());

    let after = snapshot_bytes(&scenario_dir);
    assert_eq!(before, after, "a `canon plugin sync porting` + `canon query --plugin porting` round-trip must never alter a core Scenario file's bytes");
}

// ── canon-gate source isolation (non-negotiable, tasks.md 4.4/design.md D5) ──

fn rust_files_under(dir: &Path) -> Vec<PathBuf> {
    files_under(dir).into_iter().filter(|p| p.extension().and_then(|e| e.to_str()) == Some("rs")).collect()
}

/// `canon-gate`'s crate source SHALL carry zero reference to the three
/// plugin-specific names `porting-plugin` spec names: the bare plugin
/// id (checked as a QUOTED string literal, `"porting"` — not a bare
/// substring/word match, since canon-gate's own PRE-EXISTING,
/// s16-unrelated `policy.rs` doc comment happens to use the English
/// gerund "porting" ["... explicit SKIP on porting donor-specific
/// routing semantics verbatim"] and `checkbox.rs` contains "importing"
/// — neither is a reference to the s16 plugin, and a bare-substring
/// check would false-positive on both), the overlay identity
/// `porting.coverage`, and the store primitive `scan_namespaced_kind`
/// canon-gate would need to read ANY overlay record at all. Also
/// confirms canon-gate's `Cargo.toml` never depends on `canon-plugin` —
/// canon-gate's own `coverage.rs`/`CoverageCheck`/`uncovered-cell`
/// authority is untouched and out of this check's scope (spec.md).
#[test]
fn canon_gate_source_carries_no_porting_plugin_reference() {
    let gate_src = Path::new(env!("CARGO_MANIFEST_DIR")).join("../canon-gate/src");
    assert!(gate_src.is_dir(), "expected canon-gate/src at {}", gate_src.display());

    let forbidden = ["\"porting\"", "porting.coverage", "scan_namespaced_kind", "canon_plugin"];
    for path in rust_files_under(&gate_src) {
        let text = std::fs::read_to_string(&path).unwrap();
        for needle in forbidden {
            assert!(
                !text.contains(needle),
                "canon-gate source file {} contains forbidden plugin-specific reference `{needle}` -- canon-gate must carry ZERO code path that can read a porting overlay record",
                path.display()
            );
        }
    }

    let cargo_toml = std::fs::read_to_string(Path::new(env!("CARGO_MANIFEST_DIR")).join("../canon-gate/Cargo.toml")).unwrap();
    assert!(!cargo_toml.contains("canon-plugin"), "canon-gate/Cargo.toml must never depend on canon-plugin (design.md D5 isolation)");
}

// ── coverage change supersedes the stale overlay (source-version `at`) ──

/// Rewrite the fixture's single inventory file with a given envelope
/// `at` and `covered_by` list -- the S11 authoring discipline for a
/// coverage change: edit the entry AND bump the record's own `at`.
fn write_inventory_at(dir: &Path, at: &str, covered_by: &str) {
    write(
        dir,
        "specs/inventory/kind=inventory/area=idolive/surface=hub/hub.yaml",
        &format!("schema: 1\nkind: inventory\nat: \"{at}\"\nactor:\n  agent_id: test\nidolive.hub.hub-header:\n  upstream:\n    pin: a\n    file: f.tsx\n    symbol: S\n    lines: \"1-2\"\n  covered_by: {covered_by}\n"),
    );
}

#[test]
fn a_coverage_change_uncovered_to_covered_supersedes_the_stale_overlay() {
    let dir = tempfile::tempdir().unwrap();
    write_repo(dir.path()); // inventory at 2026-07-10; hub.02 uncovered
    sync_inventory(dir.path());
    sync_porting(dir.path());
    let records = query_scenarios_with_plugin(dir.path());
    assert_eq!(scenario_record(&records, "idolive.hub.02")["overlay"]["porting.coverage"]["covered"], false);

    // Author covers hub.02 by adding it to covered_by AND bumping the
    // inventory record's own `at` -> the new overlay's source-version
    // `at` advances, so P3's latest-by-`at` fold picks it over the stale
    // covered:false record P2 left append-only on disk.
    write_inventory_at(dir.path(), "2026-07-11T00:00:00Z", "[idolive.hub.01, idolive.hub.02]");
    sync_porting(dir.path());

    let records = query_scenarios_with_plugin(dir.path());
    let record = scenario_record(&records, "idolive.hub.02");
    assert_eq!(record["overlay"]["porting.coverage"]["covered"], true, "the newer source-version overlay must win the fold: {record:?}");
    assert_eq!(record["overlay"]["porting.coverage"]["surface_ref"], serde_json::json!(["idolive.hub.hub-header"]), "{record:?}");

    // The now-unchanged inventory still dedupes on a third sync.
    let third = sync_porting(dir.path());
    assert!(stdout(&third).contains("0 written"), "a re-sync over the unchanged (bumped) inventory must dedupe: {}", stdout(&third));
}

#[test]
fn a_coverage_removal_covered_to_uncovered_supersedes_the_stale_positive() {
    let dir = tempfile::tempdir().unwrap();
    write_repo(dir.path()); // hub.01 covered by [idolive.hub.01]
    sync_inventory(dir.path());
    sync_porting(dir.path());
    let records = query_scenarios_with_plugin(dir.path());
    assert_eq!(scenario_record(&records, "idolive.hub.01")["overlay"]["porting.coverage"]["covered"], true);

    // Author removes hub.01's coverage by emptying covered_by AND
    // bumping the record's `at` -> the newer covered:false overlay wins
    // the fold, clearing the stale positive (the true->false direction).
    write_inventory_at(dir.path(), "2026-07-11T00:00:00Z", "[]");
    sync_porting(dir.path());

    let records = query_scenarios_with_plugin(dir.path());
    let record = scenario_record(&records, "idolive.hub.01");
    assert_eq!(record["overlay"]["porting.coverage"]["covered"], false, "clearing covered_by (with a bumped at) must clear the stale positive: {record:?}");
    assert_eq!(record["overlay"]["porting.coverage"]["surface_ref"], serde_json::json!([]), "{record:?}");
}

// ── repo-root resolution: `--repo .` from a subdirectory (ReviewS16P4 F2) ──

#[test]
fn plugin_sync_from_a_subdirectory_resolves_the_repo_root_tier() {
    // `canon plugin sync porting --repo .` run from a SUBDIRECTORY must
    // resolve the repo ROOT via the nearest-canon.yaml ancestor walk
    // (`resolve_repo_root`, the same one `canon inventory sync`/`canon
    // gate` use), writing into the repo root's git tier -- never a
    // subdir-local one, and never failing to find the plugin manifest.
    let dir = tempfile::tempdir().unwrap();
    write_repo(dir.path());
    sync_inventory(dir.path());

    let sub = dir.path().join("specs/features"); // a nested subdir, no canon.yaml of its own
    let out = run_canon(&["plugin", "sync", "porting", "--repo", "."], &sub);
    assert!(out.status.success(), "plugin sync --repo . from a subdir must resolve the repo root: {}", stderr(&out));
    assert!(stdout(&out).contains("2 written"), "must write into the repo-root tier: {}", stdout(&out));

    // Overlays land under the repo ROOT tier; no subdir-local tier appears.
    assert_eq!(files_under(&dir.path().join("canon/ledger/kind=porting.coverage")).len(), 2, "overlay records must be under the repo ROOT tier");
    assert!(!sub.join("canon").exists(), "no subdir-local canon/ tier may be created: {sub:?}");

    // A root-run query projects exactly the overlays the subdir sync wrote (same tier).
    let records = query_scenarios_with_plugin(dir.path());
    assert_eq!(scenario_record(&records, "idolive.hub.01")["overlay"]["porting.coverage"]["covered"], true);
}
