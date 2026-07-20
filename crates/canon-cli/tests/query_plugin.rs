//! Integration tests for `canon query --kind <k> --plugin <id> [--json]`
//! (s16 P3, `openspec/changes/s16-plugin-extensibility/`, tasks.md
//! 3.3/3.4, design.md D3, `plugin-overlay-projection` spec), invoking
//! the actually-built `canon` binary against an offline git-tier
//! fixture (`support::Fixture`) -- zero network, no credentials.
//!
//! Covers every scenario tasks.md 3.4 names: a matching overlay record
//! projects the declared fields; an unmatched core record projects
//! unmodified; a malformed sibling overlay record is skipped +
//! diagnosed without aborting the whole projection; a core record's
//! on-disk file is byte-identical before/after a projection read; and
//! `canon query` without `--plugin` is byte-identical to its pre-s16
//! output even when overlay data exists on disk. Also covers the
//! `--kind`/`--plugin` `core_kind` mismatch pin (parent-agent steer,
//! design.md's "s16 projects onto `Scenario` only" scope boundary).

mod support;

use canon_model::envelope::{Actor, Envelope, RecordKind};
use canon_model::handoff::{DomainId, Handoff, HandoffBody};
use canon_model::ids::{HandoffId, RoleId};
use canon_plugin::overlay::{OverlayEnvelope, compose_overlay_body, write_overlay};
use canon_plugin::manifest::snapshot::OverlayDecl;
use canon_plugin::manifest::schema::FieldDecl;
use canon_plugin::manifest::types::Type;
use canon_store::git_tier::GitTier;
use canon_store::tier::Tier;
use chrono::Utc;
use uuid::Uuid;
use serde_json::{Value, json};

const ROUTING: &str = "  scenario: local\n  handoff: local\n";
const AGING: &str = "  scenario: { after: 1d, to: cold }\n";

const PORTING_YAML: &str = "id: porting\nnamespace: porting\noverlays:\n  - kind: coverage\n    attaches_to:\n      core_kind: scenario\n      join_key: [project_id, scenario_id]\n    fields:\n      - name: covered\n        type: bool\n      - name: surface_ref\n        type: { list: string }\n";

fn actor() -> Actor {
    Actor::new("test-agent", RoleId::parse("implementer").unwrap())
}

/// Mirrors `PORTING_YAML`'s own declared shape exactly -- this is the
/// SAME `porting.coverage` overlay identity `canon query --plugin
/// porting` resolves from the manifest written by `write_plugin_manifest`,
/// hand-assembled here (rather than parsed) so overlay-record fixtures
/// can be composed directly via `canon_plugin::overlay::write_overlay`.
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

/// Write a well-formed `porting.coverage` overlay record straight into
/// the fixture's git tier, through `canon-plugin`'s own validating
/// writer (`write_overlay`) -- planted independently of the CLI under
/// test, mirroring `support::Fixture`'s own "planted directly via
/// canon-store's library" discipline.
fn plant_overlay(fixture: &support::Fixture, project_id: &str, scenario_id: &str, covered: bool, surface_ref: &[&str]) {
    let git = GitTier::new(fixture.git_root());
    let envelope = OverlayEnvelope::new(1, "porting.coverage", Utc::now(), actor());
    let mut fields = serde_json::Map::new();
    fields.insert("project_id".to_string(), json!(project_id));
    fields.insert("scenario_id".to_string(), json!(scenario_id));
    fields.insert("covered".to_string(), json!(covered));
    fields.insert("surface_ref".to_string(), json!(surface_ref));
    let body = compose_overlay_body(&envelope, fields);
    write_overlay(&git, &coverage_decl(), body).expect("plant a well-formed overlay record");
}

/// Plant a `porting.coverage` overlay record that DOES NOT pass the
/// CURRENT manifest's schema (`covered` is a string, not a bool) --
/// bypasses `write_overlay`'s own validation (which would refuse to
/// write it) via the lower-level `write_namespaced`, exactly as a
/// record that drifted out of sync with a later manifest edit would
/// look on disk (design.md R7).
fn plant_malformed_overlay(fixture: &support::Fixture, project_id: &str, scenario_id: &str) {
    let git = GitTier::new(fixture.git_root());
    let envelope = OverlayEnvelope::new(1, "porting.coverage", Utc::now(), actor());
    let mut fields = serde_json::Map::new();
    fields.insert("project_id".to_string(), json!(project_id));
    fields.insert("scenario_id".to_string(), json!(scenario_id));
    fields.insert("covered".to_string(), json!("not-a-bool")); // wrong type
    fields.insert("surface_ref".to_string(), json!([]));
    let body = compose_overlay_body(&envelope, fields);
    let natural_key = format!("{project_id}__{scenario_id}");
    git.write_namespaced("porting.coverage", &natural_key, body).expect("plant a malformed overlay record straight onto disk");
}

/// A `Handoff` record, planted straight into the git tier
/// (`routing.handoff: git` in `ROUTING`) -- a NON-`scenario` core kind,
/// used to exercise the `--kind`/`--plugin` `core_kind` mismatch pin.
fn plant_handoff(fixture: &support::Fixture) -> HandoffId {
    let id = HandoffId::parse("20260712-0900-s16-p3-fixture-a1b2").unwrap();
    let envelope = Envelope::new(1, RecordKind::Handoff, Utc::now(), actor());
    let body = HandoffBody { domain: DomainId::parse("development").unwrap(), template_version: 1, fields: serde_json::json!({}) };
    let handoff = Handoff::new(envelope, id.clone(), Uuid::new_v4(), None, 1, "s16 P3 fixture handoff", None, body);
    GitTier::new(fixture.git_root()).write(&handoff).expect("write handoff fixture into the git tier");
    id
}

fn scenario_record<'a>(records: &'a [Value], scenario_id: &str) -> &'a Value {
    records.iter().find(|r| r["scenario_id"] == scenario_id).unwrap_or_else(|| panic!("no scenario `{scenario_id}` in {records:?}"))
}

#[test]
fn a_scenario_with_a_matching_overlay_record_projects_the_declared_fields() {
    let fixture = support::Fixture::new(ROUTING, AGING);
    fixture.plant_scenario_in_git("root", "world.hotdeal.01", "a scenario", Utc::now());
    fixture.write_plugin_manifest("porting", PORTING_YAML);
    plant_overlay(&fixture, "root", "world.hotdeal.01", true, &["world.hotdeal.01"]);

    let output = fixture.run_canon(&["query", "--kind", "scenario", "--plugin", "porting", "--json"]);
    assert!(output.status.success(), "stderr: {}", support::stderr(&output));

    let payload: Value = serde_json::from_str(&support::stdout(&output)).expect("valid JSON on stdout");
    let records = payload["records"].as_array().expect("records array");
    let record = scenario_record(records, "world.hotdeal.01");
    assert_eq!(record["overlay"]["porting.coverage"]["covered"], true);
    assert_eq!(record["overlay"]["porting.coverage"]["surface_ref"], json!(["world.hotdeal.01"]));
    // Native fields untouched alongside the projected overlay.
    assert_eq!(record["title"], "a scenario");
}

#[test]
fn a_scenario_with_no_overlay_record_projects_unmodified() {
    let fixture = support::Fixture::new(ROUTING, AGING);
    fixture.plant_scenario_in_git("root", "world.hotdeal.02", "uncovered scenario", Utc::now());
    fixture.write_plugin_manifest("porting", PORTING_YAML);
    // No overlay record planted for this scenario at all.

    let output = fixture.run_canon(&["query", "--kind", "scenario", "--plugin", "porting", "--json"]);
    assert!(output.status.success(), "stderr: {}", support::stderr(&output));

    let payload: Value = serde_json::from_str(&support::stdout(&output)).expect("valid JSON on stdout");
    let records = payload["records"].as_array().expect("records array");
    let record = scenario_record(records, "world.hotdeal.02");
    assert!(record.get("overlay").is_none(), "no overlay record exists for this scenario -- no `overlay` key must be injected: {record}");
    assert_eq!(record["title"], "uncovered scenario");
}

#[test]
fn a_malformed_overlay_record_is_skipped_while_sibling_records_still_project() {
    let fixture = support::Fixture::new(ROUTING, AGING);
    fixture.plant_scenario_in_git("root", "world.hotdeal.03", "malformed-overlay scenario", Utc::now());
    fixture.plant_scenario_in_git("root", "world.hotdeal.04", "well-formed-overlay scenario", Utc::now());
    fixture.write_plugin_manifest("porting", PORTING_YAML);
    plant_malformed_overlay(&fixture, "root", "world.hotdeal.03");
    plant_overlay(&fixture, "root", "world.hotdeal.04", true, &["world.hotdeal.04"]);

    let output = fixture.run_canon(&["query", "--kind", "scenario", "--plugin", "porting", "--json"]);
    assert!(output.status.success(), "a malformed sibling record must never abort the whole projection -- stderr: {}", support::stderr(&output));

    let stderr = support::stderr(&output);
    assert!(stderr.contains("E-PLUGIN-BODY-TYPE") || stderr.contains("covered"), "the malformed record must be diagnosed on stderr: {stderr}");

    let payload: Value = serde_json::from_str(&support::stdout(&output)).expect("valid JSON on stdout");
    let records = payload["records"].as_array().expect("records array");

    let malformed_record = scenario_record(records, "world.hotdeal.03");
    assert!(malformed_record.get("overlay").is_none(), "the malformed record must never contribute a projected overlay: {malformed_record}");

    let sibling_record = scenario_record(records, "world.hotdeal.04");
    assert_eq!(sibling_record["overlay"]["porting.coverage"]["covered"], true, "the well-formed sibling must still project: {sibling_record}");
}

#[test]
fn projection_never_rewrites_the_core_record_on_disk() {
    let fixture = support::Fixture::new(ROUTING, AGING);
    let receipt = fixture.plant_scenario_in_git("root", "world.hotdeal.05", "a scenario", Utc::now());
    fixture.write_plugin_manifest("porting", PORTING_YAML);
    plant_overlay(&fixture, "root", "world.hotdeal.05", true, &["world.hotdeal.05"]);

    let scenario_path = fixture.git_root().join(&receipt.location);
    let before = std::fs::read(&scenario_path).expect("core scenario file exists before the query");

    let output = fixture.run_canon(&["query", "--kind", "scenario", "--plugin", "porting", "--json"]);
    assert!(output.status.success(), "stderr: {}", support::stderr(&output));
    // Sanity: the projection actually ran and found the overlay match --
    // otherwise this test would trivially pass without exercising
    // anything.
    let payload: Value = serde_json::from_str(&support::stdout(&output)).expect("valid JSON on stdout");
    let record = scenario_record(payload["records"].as_array().unwrap(), "world.hotdeal.05");
    assert_eq!(record["overlay"]["porting.coverage"]["covered"], true);

    let after = std::fs::read(&scenario_path).expect("core scenario file still exists after the query");
    assert_eq!(before, after, "a projection read must NEVER rewrite the core record's on-disk bytes");
}

#[test]
fn no_plugin_flag_is_byte_identical_to_the_pre_s16_output_even_with_overlay_data_on_disk() {
    let fixture = support::Fixture::new(ROUTING, AGING);
    fixture.plant_scenario_in_git("root", "world.hotdeal.06", "a scenario", Utc::now());

    let output_before_overlay_data = fixture.run_canon(&["query", "--kind", "scenario", "--json"]);
    assert!(output_before_overlay_data.status.success());

    // Now plant a plugin manifest AND a matching overlay record -- s16's
    // entire read-time-projection surface now has real data to project,
    // but this invocation never passes `--plugin`.
    fixture.write_plugin_manifest("porting", PORTING_YAML);
    plant_overlay(&fixture, "root", "world.hotdeal.06", true, &["world.hotdeal.06"]);

    let output_after_overlay_data = fixture.run_canon(&["query", "--kind", "scenario", "--json"]);
    assert!(output_after_overlay_data.status.success());

    assert_eq!(
        support::stdout(&output_before_overlay_data),
        support::stdout(&output_after_overlay_data),
        "`canon query` without `--plugin` must be byte-identical whether or not overlay data exists on disk"
    );

    // And structurally: no s16 surface leaks into the no-`--plugin`
    // payload shape at all.
    let payload: Value = serde_json::from_str(&support::stdout(&output_after_overlay_data)).unwrap();
    assert!(payload.get("plugin").is_none());
    assert!(payload.get("overlays").is_none());
    let record = scenario_record(payload["records"].as_array().unwrap(), "world.hotdeal.06");
    assert!(record.get("overlay").is_none(), "no `--plugin` flag -- no `overlay` key, even though a matching overlay record exists on disk: {record}");
}

#[test]
fn a_kind_plugin_core_kind_mismatch_never_projects_and_leaves_core_output_unchanged() {
    // s16 projects onto `core_kind: scenario` only (porting's own
    // manifest, PORTING_YAML) -- querying a DIFFERENT `--kind` with
    // `--plugin porting` must never attempt a projection at all, per
    // spec's `core_kind` scope boundary.
    let fixture = support::Fixture::new(ROUTING, AGING);
    let handoff_id = plant_handoff(&fixture);
    fixture.write_plugin_manifest("porting", PORTING_YAML);

    let no_plugin = fixture.run_canon(&["query", "--kind", "handoff", "--json"]);
    assert!(no_plugin.status.success());

    let with_mismatched_plugin = fixture.run_canon(&["query", "--kind", "handoff", "--plugin", "porting", "--json"]);
    assert!(with_mismatched_plugin.status.success());

    assert_eq!(
        support::stdout(&no_plugin),
        support::stdout(&with_mismatched_plugin),
        "a core_kind mismatch must leave the core `--kind handoff` output byte-identical to the no-`--plugin` path"
    );

    let stderr = support::stderr(&with_mismatched_plugin);
    assert!(
        stderr.contains("core_kind") && stderr.contains("scenario") && stderr.contains("handoff"),
        "a diagnostic naming the core_kind mismatch must be surfaced on stderr: {stderr}"
    );

    let payload: Value = serde_json::from_str(&support::stdout(&with_mismatched_plugin)).unwrap();
    let records = payload["records"].as_array().expect("records array");
    assert!(records.iter().any(|r| r["id"] == handoff_id.to_string()));
    for record in records {
        assert!(record.get("overlay").is_none(), "no overlay fields must ever be injected onto a non-scenario core kind: {record}");
    }
}

#[test]
fn plugin_flag_with_no_projectable_overlay_falls_back_byte_identical_to_no_plugin() {
    // ReviewS16P3 F1: an installed plugin whose overlay matches the
    // queried core_kind but produces ZERO projected rows (no overlay
    // records on disk) must degrade to the unmodified core view --
    // byte-identical to the no-`--plugin` output, never a plugin-framed
    // payload carrying empty `plugin`/`overlays`/`overlay` surfaces.
    let fixture = support::Fixture::new(ROUTING, AGING);
    fixture.plant_scenario_in_git("root", "world.hotdeal.07", "a scenario", Utc::now());
    fixture.write_plugin_manifest("porting", PORTING_YAML);
    // No overlay record planted at all -> project_overlay yields an empty map.

    let no_plugin = fixture.run_canon(&["query", "--kind", "scenario", "--json"]);
    assert!(no_plugin.status.success());
    let with_plugin = fixture.run_canon(&["query", "--kind", "scenario", "--plugin", "porting", "--json"]);
    assert!(with_plugin.status.success(), "stderr: {}", support::stderr(&with_plugin));

    assert_eq!(
        support::stdout(&no_plugin),
        support::stdout(&with_plugin),
        "an all-empty projection must fall back byte-identical to the no-`--plugin` output"
    );

    let payload: Value = serde_json::from_str(&support::stdout(&with_plugin)).unwrap();
    assert!(payload.get("plugin").is_none(), "no plugin metadata may leak when nothing projected: {payload}");
    assert!(payload.get("overlays").is_none());
    let record = scenario_record(payload["records"].as_array().unwrap(), "world.hotdeal.07");
    assert!(record.get("overlay").is_none());
}

/// A `shared`-namespace overlay declaration for the F2 ownership test --
/// two plugins declare DIFFERENT kinds under the SAME namespace.
fn shared_decl(kind: &str, field: &str) -> OverlayDecl {
    OverlayDecl {
        namespace: "shared".to_string(),
        kind: kind.to_string(),
        identity: format!("shared.{kind}"),
        core_kind: "scenario".to_string(),
        join_key: vec!["project_id".to_string(), "scenario_id".to_string()],
        fields: vec![FieldDecl { name: field.to_string(), ty: Type::Bool }],
    }
}

fn plant_shared_overlay(fixture: &support::Fixture, decl: &OverlayDecl, project_id: &str, scenario_id: &str, field: &str, val: bool) {
    let git = GitTier::new(fixture.git_root());
    let envelope = OverlayEnvelope::new(1, &decl.identity, Utc::now(), actor());
    let mut fields = serde_json::Map::new();
    fields.insert("project_id".to_string(), json!(project_id));
    fields.insert("scenario_id".to_string(), json!(scenario_id));
    fields.insert(field.to_string(), json!(val));
    let body = compose_overlay_body(&envelope, fields);
    write_overlay(&git, decl, body).expect("plant a shared-namespace overlay record");
}

#[test]
fn plugin_projects_only_its_own_overlays_never_a_namespace_sibling_plugins() {
    // ReviewS16P3 F2: two installed plugins may share a namespace with
    // DIFFERENT overlay kinds. `--plugin cov-a` must project ONLY the
    // overlays cov-a itself declared, never cov-b's records that merely
    // share the `shared` namespace.
    const COV_A: &str = "id: cov-a\nnamespace: shared\noverlays:\n  - kind: alpha\n    attaches_to:\n      core_kind: scenario\n      join_key: [project_id, scenario_id]\n    fields:\n      - name: covered\n        type: bool\n";
    const COV_B: &str = "id: cov-b\nnamespace: shared\noverlays:\n  - kind: beta\n    attaches_to:\n      core_kind: scenario\n      join_key: [project_id, scenario_id]\n    fields:\n      - name: flagged\n        type: bool\n";

    let fixture = support::Fixture::new(ROUTING, AGING);
    fixture.plant_scenario_in_git("root", "world.hotdeal.08", "a scenario", Utc::now());
    fixture.write_plugin_manifest("cov-a", COV_A);
    fixture.write_plugin_manifest("cov-b", COV_B);

    let decl_alpha = shared_decl("alpha", "covered");
    let decl_beta = shared_decl("beta", "flagged");
    plant_shared_overlay(&fixture, &decl_alpha, "root", "world.hotdeal.08", "covered", true);
    plant_shared_overlay(&fixture, &decl_beta, "root", "world.hotdeal.08", "flagged", true);

    let output = fixture.run_canon(&["query", "--kind", "scenario", "--plugin", "cov-a", "--json"]);
    assert!(output.status.success(), "stderr: {}", support::stderr(&output));

    let payload: Value = serde_json::from_str(&support::stdout(&output)).unwrap();
    let record = scenario_record(payload["records"].as_array().unwrap(), "world.hotdeal.08");
    assert_eq!(record["overlay"]["shared.alpha"]["covered"], true, "cov-a's own overlay must project: {record}");
    assert!(
        record["overlay"].get("shared.beta").is_none(),
        "cov-b's namespace-sibling overlay must NEVER leak into a `--plugin cov-a` query: {record}"
    );
}
