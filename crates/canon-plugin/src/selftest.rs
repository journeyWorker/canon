//! canon-plugin's own shared-contract selftest entry point (s16 P6,
//! `openspec/changes/s16-plugin-extensibility/`, tasks.md 6.1): the
//! 10th `canon selftest` suite (registered as `"plugin-overlays"` in
//! `canon-cli`'s `crate::selftest::suites`). A SYNTHETIC fixture corpus
//! -- a `.canon/plugins/<id>/plugin.yaml` manifest plus overlay records,
//! built entirely inside a scratch directory at RUN time (no checked-in
//! fixture files; this module never reads or writes anything under this
//! repo's own `.canon/plugins/porting/`) -- exercises the FULL P1-P3
//! machinery this crate ships in one pipeline: resolve a manifest
//! snapshot ([`resolve_plugin_snapshot`]), validate a well-formed body
//! AND three independently-malformed candidate bodies
//! ([`validate_overlay_body`], reached through [`write_overlay`]'s own
//! validate-before-write ordering, design.md D4), write two well-formed
//! overlay records through the REAL store path ([`write_overlay`] ->
//! `canon_store::GitTier::write_namespaced`), and project a
//! covered/uncovered/unmatched/schema-drifted-skipped set of core
//! `Scenario` records ([`project_overlay`]).
//!
//! # Two-sided exact-set diagnostic oracle
//!
//! Mirrors `canon_store::selftest`'s and canon-cli's
//! `crate::inventory_selftest`'s two-sided (missing AND extra both
//! fail) exact-set discipline, generalized from a `(tag, subject)`
//! violation pair to this crate's own `(code, message)` diagnostic pair
//! -- every `subject` a P2/P3 finding carries here is the SAME constant
//! overlay `identity` string, so it never discriminates between
//! fixtures; `message` does. [`EXPECTED_DIAGNOSTICS`] names exactly the
//! four diagnostics this fixture's whole run must produce: three from
//! [`validate_overlay_body`] independently rejecting each of the three
//! malformed candidate bodies before write, one from
//! [`project_overlay`]'s fail-soft skip of a schema-drifted record
//! injected directly onto disk (design.md R7 -- bypassing
//! `write_overlay`'s own validation, simulating a manifest that changed
//! shape after the record was written). [`check_diagnostic_oracle`]
//! diffs the fixture's ACTUAL diagnostic set against this exact
//! expectation both ways: a diagnostic this fixture no longer produces
//! (under-detection) and a diagnostic it produces beyond what's
//! expected (over-triggering / a new, unaccounted-for finding) both
//! fail the suite.
//!
//! # Rebindable scratch root, no `tempfile` dependency
//!
//! [`ScratchDir`] is a minimal `std`-only equivalent of
//! `tempfile::TempDir` (mirrors `canon_vocab::selftest::ScratchDir` /
//! canon-cli's `crate::inventory_selftest::ScratchDir` verbatim) --
//! `tempfile` is this crate's `[dev-dependencies]` only, and this
//! module compiles into the release `canon` binary via `canon
//! selftest`, not only under `cargo test`. Side-effect-free against the
//! real repo: every read/write is scoped to a fresh scratch directory
//! under `std::env::temp_dir()`, `Drop`-cleaned, mirroring production's
//! own `<repo>/.canon/plugins/<id>/plugin.yaml` +
//! `<repo>/.canon/ledger/kind=<x>/...` layout (`GateCtx::from_repo`'s
//! default `ledger_root`) so the fixture stays realistic.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::sync::atomic::{AtomicU64, Ordering};

use canon_model::records::Scenario;
use canon_model::{Actor, Envelope, ProjectId, RawRecord, RecordKind, RoleId, ScenarioId, SpecDigest};
use canon_store::git_tier::GitTier;
use chrono::{DateTime, Utc};
use serde_json::json;

use crate::diagnostic::{Diagnostic, E_PLUGIN_BODY_MISSING, E_PLUGIN_BODY_TYPE, E_PLUGIN_BODY_UNDECLARED};
use crate::manifest::snapshot::OverlayDecl;
use crate::overlay::{OverlayEnvelope, OverlayWriteError, compose_overlay_body, write_overlay};
use crate::project::{ProjectedOverlay, project_overlay};
use crate::resolve_plugin_snapshot::resolve_plugin_snapshot;

const PLUGIN_ID: &str = "selftest-coverage";
const NAMESPACE: &str = "selftest";
/// `<namespace>.<kind>` -- kept in sync with `PLUGIN_MANIFEST_YAML` by
/// hand (mirrors `crate::overlay::tests::coverage_decl`'s own
/// hand-paired `identity` literal).
const OVERLAY_IDENTITY: &str = "selftest.coverage";

const PLUGIN_MANIFEST_YAML: &str = "id: selftest-coverage\nnamespace: selftest\noverlays:\n  - kind: coverage\n    attaches_to:\n      core_kind: scenario\n      join_key: [project_id, scenario_id]\n    fields:\n      - name: covered\n        type: bool\n      - name: surface_ref\n        type: { list: string }\n";

/// A `std`-only, `Drop`-cleaned scratch directory -- see module doc.
struct ScratchDir(PathBuf);

impl ScratchDir {
    fn new() -> Result<Self, String> {
        static COUNTER: AtomicU64 = AtomicU64::new(0);
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let unique = COUNTER.fetch_add(1, Ordering::Relaxed);
        let path = std::env::temp_dir().join(format!("canon-plugin-selftest-{}-{nanos}-{unique}", std::process::id()));
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

fn actor() -> Actor {
    Actor::new("canon-plugin-selftest", RoleId::parse("implementer").expect("literal RoleId"))
}

fn at(offset_secs: i64) -> DateTime<Utc> {
    DateTime::UNIX_EPOCH + chrono::Duration::seconds(offset_secs)
}

fn project_id() -> ProjectId {
    ProjectId::parse("root").expect("literal ProjectId")
}

fn scenario_id(id: &str) -> ScenarioId {
    ScenarioId::parse(id).expect("literal ScenarioId")
}

fn scenario(id: &str) -> Scenario {
    Scenario::new(
        Envelope::new(1, RecordKind::Scenario, at(0), actor()),
        project_id(),
        scenario_id(id),
        "a canon-plugin selftest scenario",
        "",
        SpecDigest::parse("a".repeat(64)).expect("literal SpecDigest"),
    )
}

fn well_formed_body(id: &str, covered: bool, surface_ref: &[&str]) -> RawRecord {
    let envelope = OverlayEnvelope::new(1, OVERLAY_IDENTITY, at(0), actor());
    let mut fields = serde_json::Map::new();
    fields.insert("project_id".to_string(), json!("root"));
    fields.insert("scenario_id".to_string(), json!(id));
    fields.insert("covered".to_string(), json!(covered));
    fields.insert("surface_ref".to_string(), json!(surface_ref));
    compose_overlay_body(&envelope, fields)
}

/// Omits the declared `surface_ref` field entirely -- `E_PLUGIN_BODY_MISSING`.
fn body_missing_declared_field(id: &str) -> RawRecord {
    let envelope = OverlayEnvelope::new(1, OVERLAY_IDENTITY, at(0), actor());
    let mut fields = serde_json::Map::new();
    fields.insert("project_id".to_string(), json!("root"));
    fields.insert("scenario_id".to_string(), json!(id));
    fields.insert("covered".to_string(), json!(true));
    compose_overlay_body(&envelope, fields)
}

/// Carries a field outside (envelope ∪ join-key ∪ declared) -- `E_PLUGIN_BODY_UNDECLARED`.
fn body_with_undeclared_field(id: &str) -> RawRecord {
    let envelope = OverlayEnvelope::new(1, OVERLAY_IDENTITY, at(0), actor());
    let mut fields = serde_json::Map::new();
    fields.insert("project_id".to_string(), json!("root"));
    fields.insert("scenario_id".to_string(), json!(id));
    fields.insert("covered".to_string(), json!(true));
    fields.insert("surface_ref".to_string(), json!([]));
    fields.insert("bogus_field".to_string(), json!(42));
    compose_overlay_body(&envelope, fields)
}

/// `covered` is a JSON string, not a bool -- `E_PLUGIN_BODY_TYPE`.
fn body_with_wrong_type(id: &str) -> RawRecord {
    let envelope = OverlayEnvelope::new(1, OVERLAY_IDENTITY, at(0), actor());
    let mut fields = serde_json::Map::new();
    fields.insert("project_id".to_string(), json!("root"));
    fields.insert("scenario_id".to_string(), json!(id));
    fields.insert("covered".to_string(), json!("yes"));
    fields.insert("surface_ref".to_string(), json!([]));
    compose_overlay_body(&envelope, fields)
}

/// Omits `covered` only (`surface_ref` present) -- the schema-drift
/// record this fixture injects DIRECTLY onto disk, bypassing
/// `write_overlay`'s own validation (module doc, design.md R7).
fn body_missing_covered_only(id: &str) -> RawRecord {
    let envelope = OverlayEnvelope::new(1, OVERLAY_IDENTITY, at(0), actor());
    let mut fields = serde_json::Map::new();
    fields.insert("project_id".to_string(), json!("root"));
    fields.insert("scenario_id".to_string(), json!(id));
    fields.insert("surface_ref".to_string(), json!(["x"]));
    compose_overlay_body(&envelope, fields)
}

/// The two-sided exact-set diagnostic oracle (module doc). `(code,
/// message)` pairs, verbatim against `crate::overlay`'s own `diag(...)`
/// call sites for the three malformed candidate bodies plus
/// `crate::project`'s fail-soft diagnostic for the injected
/// schema-drifted on-disk record.
const EXPECTED_DIAGNOSTICS: &[(&str, &str)] = &[
    (E_PLUGIN_BODY_MISSING, "missing declared field `surface_ref`"),
    (E_PLUGIN_BODY_UNDECLARED, "field `bogus_field` is outside the overlay's declared schema (envelope \u{222A} join-key \u{222A} declared fields)"),
    (E_PLUGIN_BODY_TYPE, "declared field `covered` does not match its manifest type"),
    (E_PLUGIN_BODY_MISSING, "missing declared field `covered`"),
];

/// One complete fixture run -- everything the [`Check`]s below assert
/// against, gathered by exactly one pass through the REAL P1-P3
/// pipeline (no second, independently-computed view of this fixture's
/// own data).
struct FixtureRun {
    resolution_diags: Vec<Diagnostic>,
    decl: OverlayDecl,
    covered_receipt_location: String,
    uncovered_receipt_location: String,
    /// Record/violation counts from `scan_namespaced_kind` taken AFTER
    /// the three malformed `write_overlay` attempts (module doc: they
    /// must never reach disk) but BEFORE the schema-drifted record is
    /// injected.
    pre_injection_record_count: usize,
    pre_injection_violation_count: usize,
    /// The same scan taken AFTER injection -- one more record, still
    /// zero STORE-layer violations (the injected record is
    /// structurally well-formed JSON; only P2's MANIFEST-schema
    /// validation rejects it, which store-layer scanning never runs).
    post_injection_record_count: usize,
    post_injection_violation_count: usize,
    /// Diagnostics from `validate_overlay_body`, surfaced through
    /// `write_overlay`'s `OverlayWriteError::Validation` for each of
    /// the three malformed candidate bodies.
    malformed_validate_diags: Vec<Diagnostic>,
    projected: ProjectedOverlay,
    project_diags: Vec<Diagnostic>,
}

fn run_fixture() -> Result<FixtureRun, String> {
    let scratch = ScratchDir::new()?;
    write_file(scratch.path(), &format!("{}/{PLUGIN_ID}/plugin.yaml", canon_model::paths::PLUGINS_DIR), PLUGIN_MANIFEST_YAML)?;

    let (snapshot, resolution_diags) = resolve_plugin_snapshot(scratch.path());
    let decl = snapshot.overlay(OVERLAY_IDENTITY).cloned().ok_or_else(|| format!("resolved snapshot is missing the `{OVERLAY_IDENTITY}` overlay declaration"))?;

    // Mirrors production's own `<repo>/.canon/ledger` default
    // (`GateCtx::from_repo`) -- nested under the SAME scratch root as
    // `.canon/plugins/`, so this fixture's layout matches a real repo's.
    let tier = GitTier::new(scratch.path().join(canon_model::paths::LEDGER_DIR));

    let covered_receipt =
        write_overlay(&tier, &decl, well_formed_body("world.selftest.01", true, &["world.selftest.01"])).map_err(|e| format!("write covered overlay: {e}"))?;
    let uncovered_receipt =
        write_overlay(&tier, &decl, well_formed_body("world.selftest.02", false, &[])).map_err(|e| format!("write uncovered overlay: {e}"))?;

    let mut malformed_validate_diags = Vec::new();
    for (label, body) in [
        ("missing-field", body_missing_declared_field("world.selftest.90")),
        ("undeclared-field", body_with_undeclared_field("world.selftest.91")),
        ("wrong-type", body_with_wrong_type("world.selftest.92")),
    ] {
        match write_overlay(&tier, &decl, body) {
            Ok(receipt) => return Err(format!("{label} body must be rejected before write, but wrote {}", receipt.location)),
            Err(OverlayWriteError::Validation(diags)) => malformed_validate_diags.extend(diags),
            Err(other) => return Err(format!("{label} body: unexpected write_overlay error: {other}")),
        }
    }

    let (pre_records, pre_violations) = tier.scan_namespaced_kind(OVERLAY_IDENTITY).map_err(|e| format!("scan after malformed-write attempts: {e}"))?;

    // Inject a schema-drifted record DIRECTLY (bypassing write_overlay's
    // own validation) -- design.md R7's scenario: a record already on
    // disk that no longer matches the CURRENT manifest.
    let malformed_on_disk = body_missing_covered_only("world.selftest.03");
    tier.write_namespaced(OVERLAY_IDENTITY, "root__world.selftest.03", malformed_on_disk).map_err(|e| format!("inject schema-drifted on-disk record: {e}"))?;

    let (post_records, post_violations) = tier.scan_namespaced_kind(OVERLAY_IDENTITY).map_err(|e| format!("scan after injecting schema-drifted record: {e}"))?;

    let core = vec![
        scenario("world.selftest.01"), // covered
        scenario("world.selftest.02"), // uncovered
        scenario("world.selftest.03"), // schema-drifted overlay record -- must be skipped
        scenario("world.selftest.04"), // no overlay record at all -- unmatched
    ];
    let overlay_raw: Vec<RawRecord> = post_records.iter().map(|(_, raw)| raw.clone()).collect();
    let (projected, project_diags) = project_overlay(&core, &overlay_raw, &decl);

    Ok(FixtureRun {
        resolution_diags,
        decl,
        covered_receipt_location: covered_receipt.location,
        uncovered_receipt_location: uncovered_receipt.location,
        pre_injection_record_count: pre_records.len(),
        pre_injection_violation_count: pre_violations.len(),
        post_injection_record_count: post_records.len(),
        post_injection_violation_count: post_violations.len(),
        malformed_validate_diags,
        projected,
        project_diags,
    })
}

fn check_resolution(run: &FixtureRun) -> Result<(), String> {
    if !run.resolution_diags.is_empty() {
        return Err(format!("expected zero diagnostics resolving a well-formed manifest, got {:?}", run.resolution_diags));
    }
    Ok(())
}

fn check_overlay_shape(run: &FixtureRun) -> Result<(), String> {
    let decl = &run.decl;
    let join_key_ok = decl.join_key == [String::from("project_id"), String::from("scenario_id")];
    if decl.identity != OVERLAY_IDENTITY || decl.namespace != NAMESPACE || decl.core_kind != "scenario" || !join_key_ok || decl.fields.len() != 2 {
        return Err(format!("resolved overlay decl shape mismatch: {decl:?}"));
    }
    Ok(())
}

fn check_writes(run: &FixtureRun) -> Result<(), String> {
    if !run.covered_receipt_location.starts_with("kind=selftest.coverage/root__world.selftest.01__") {
        return Err(format!("unexpected covered receipt location `{}`", run.covered_receipt_location));
    }
    if !run.uncovered_receipt_location.starts_with("kind=selftest.coverage/root__world.selftest.02__") {
        return Err(format!("unexpected uncovered receipt location `{}`", run.uncovered_receipt_location));
    }
    if run.pre_injection_record_count != 2 || run.pre_injection_violation_count != 0 {
        return Err(format!(
            "expected exactly 2 well-formed records and 0 store violations after 3 malformed write_overlay attempts (which must never reach disk), got {} records / {} violations",
            run.pre_injection_record_count, run.pre_injection_violation_count
        ));
    }
    Ok(())
}

fn check_scan_after_injection(run: &FixtureRun) -> Result<(), String> {
    if run.post_injection_record_count != 3 || run.post_injection_violation_count != 0 {
        return Err(format!(
            "expected exactly 3 records (2 well-formed + 1 injected schema-drifted) and 0 store-layer violations after injection, got {} records / {} violations",
            run.post_injection_record_count, run.post_injection_violation_count
        ));
    }
    Ok(())
}

fn check_projection_covered(run: &FixtureRun) -> Result<(), String> {
    let key = (project_id(), scenario_id("world.selftest.01"));
    let fields = run.projected.get(&key).ok_or("covered scenario projected nothing")?;
    if fields.get("covered") != Some(&json!(true)) || fields.get("surface_ref") != Some(&json!(["world.selftest.01"])) || fields.len() != 2 {
        return Err(format!("covered scenario projected unexpected fields: {fields:?}"));
    }
    Ok(())
}

fn check_projection_uncovered(run: &FixtureRun) -> Result<(), String> {
    let key = (project_id(), scenario_id("world.selftest.02"));
    let fields = run.projected.get(&key).ok_or("uncovered scenario projected nothing")?;
    if fields.get("covered") != Some(&json!(false)) || fields.get("surface_ref") != Some(&json!([])) || fields.len() != 2 {
        return Err(format!("uncovered scenario projected unexpected fields: {fields:?}"));
    }
    Ok(())
}

fn check_projection_malformed_skipped(run: &FixtureRun) -> Result<(), String> {
    let key = (project_id(), scenario_id("world.selftest.03"));
    if let Some(fields) = run.projected.get(&key) {
        return Err(format!("a schema-drifted overlay record must be skipped, not projected: {fields:?}"));
    }
    Ok(())
}

fn check_projection_unmatched_absent(run: &FixtureRun) -> Result<(), String> {
    let key = (project_id(), scenario_id("world.selftest.04"));
    if let Some(fields) = run.projected.get(&key) {
        return Err(format!("a core scenario with no overlay record must project unmodified (absent from the map), got {fields:?}"));
    }
    Ok(())
}

fn check_diagnostic_oracle(run: &FixtureRun) -> Result<(), String> {
    let actual: BTreeSet<(String, String)> =
        run.malformed_validate_diags.iter().chain(run.project_diags.iter()).map(|d| (d.code.clone(), d.message.clone())).collect();
    let expected: BTreeSet<(String, String)> = EXPECTED_DIAGNOSTICS.iter().map(|(code, message)| (code.to_string(), message.to_string())).collect();

    let missing: Vec<_> = expected.difference(&actual).cloned().collect();
    let extra: Vec<_> = actual.difference(&expected).cloned().collect();

    if missing.is_empty() && extra.is_empty() {
        Ok(())
    } else {
        Err(format!("diagnostic set mismatch -- missing (expected, never produced): {missing:?}; extra (produced, never expected): {extra:?}"))
    }
}

/// One named fixture check (mirrors `canon-ingest::selftest::Check`).
type Check = (&'static str, fn(&FixtureRun) -> Result<(), String>);

const CHECKS: &[Check] = &[
    ("resolve-clean", check_resolution),
    ("resolve-overlay-shape", check_overlay_shape),
    ("write-covered-and-uncovered-never-a-malformed-body", check_writes),
    ("scan-after-schema-drift-injection", check_scan_after_injection),
    ("project-covered-true", check_projection_covered),
    ("project-uncovered-false", check_projection_uncovered),
    ("project-schema-drifted-skipped", check_projection_malformed_skipped),
    ("project-unmatched-absent", check_projection_unmatched_absent),
    ("diagnostic-two-sided-exact-set", check_diagnostic_oracle),
];

/// Run canon-plugin's fixture checks (module doc). `Ok(n)` reports how
/// many independent checks passed; `Err(_)` carries one human-readable
/// line per failing check -- never panics.
pub fn selftest() -> Result<usize, Vec<String>> {
    let run = match run_fixture() {
        Ok(run) => run,
        Err(e) => return Err(vec![format!("fixture setup: {e}")]),
    };

    let mut passed = 0usize;
    let mut failures = Vec::new();
    for (name, check) in CHECKS {
        match check(&run) {
            Ok(()) => passed += 1,
            Err(e) => failures.push(format!("{name}: {e}")),
        }
    }

    if failures.is_empty() { Ok(passed) } else { Err(failures) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selftest_is_clean_against_its_own_synthetic_fixture() {
        let result = selftest();
        assert!(result.is_ok(), "canon-plugin selftest failed against its own fixture: {result:?}");
        assert_eq!(result.unwrap(), CHECKS.len());
    }

    #[test]
    fn a_regressed_expected_diagnostic_would_be_reported_as_missing() {
        // Proves the two-sided oracle is actually discriminating (module
        // doc): an EXPECTED set that no longer matches the fixture's
        // actual diagnostics must fail loud, on the MISSING side.
        let run = run_fixture().expect("fixture setup");
        let actual: BTreeSet<(String, String)> =
            run.malformed_validate_diags.iter().chain(run.project_diags.iter()).map(|d| (d.code.clone(), d.message.clone())).collect();
        let bogus_expected: BTreeSet<(String, String)> = std::iter::once(("E-PLUGIN-BODY-MISSING".to_string(), "a diagnostic this fixture never produces".to_string())).collect();
        assert!(!bogus_expected.difference(&actual).collect::<Vec<_>>().is_empty(), "a bogus expected diagnostic must show up as missing");
    }

    #[test]
    fn an_unexpected_extra_diagnostic_would_be_reported() {
        // The EXTRA side: an actual diagnostic beyond EXPECTED_DIAGNOSTICS
        // must not silently pass.
        let run = run_fixture().expect("fixture setup");
        let actual: BTreeSet<(String, String)> =
            run.malformed_validate_diags.iter().chain(run.project_diags.iter()).map(|d| (d.code.clone(), d.message.clone())).collect();
        let empty_expected: BTreeSet<(String, String)> = BTreeSet::new();
        assert!(!actual.difference(&empty_expected).collect::<Vec<_>>().is_empty(), "an empty expected set must surface every actual diagnostic as extra");
    }
}
