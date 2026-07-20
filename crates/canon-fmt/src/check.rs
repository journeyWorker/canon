//! `canon fmt --check <corpus-root>` (S11 task group 2): walks a
//! corpus root (a consumer repo's `spec/` directory, or an equivalent
//! fixture root) and validates every file against its kind's
//! [`canon_model::family::LayoutDescriptor`] plus the audited
//! field-level gaps (task 2.2). Every check here is READ-ONLY — this
//! crate only ever validates, it never writes (the `canon migrate`
//! rewrite tool is out of scope per operator directive 2026-07-10).

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use canon_model::family::{layout_problem, FamilyKind, LedgerKind};

use crate::gherkin;
use crate::report::{FmtFailureClass, FmtReport, Violation};
use crate::resolve::{self, ResolveError};
use crate::schema_registry::{self, SchemaCheck};
use crate::sha::is_full_sha;
use crate::util::{relative, walk_files};

pub fn check(root: &Path) -> FmtReport {
    let mut report = FmtReport::default();
    let ledger_backrefs = check_ledger(root, &mut report);
    check_divergences(root, &mut report, &ledger_backrefs);
    check_features(root, &mut report);
    check_inventory(root, &mut report);
    check_policy(root, &mut report);
    check_missing_join_identity(root, &mut report);
    report
}

/// Corpus-wide, ONE-TIME note (not per-record — S11 design Non-Goal:
/// backfilling historical `change_id`/`task_id` is explicitly out of
/// scope, so flagging every one of thousands of legacy records
/// individually would be noise, not signal): does ANY ledger record in
/// this corpus carry a join to `change_id`/`task_id` at all?
fn check_missing_join_identity(root: &Path, report: &mut FmtReport) {
    let mut any_file = false;
    let mut any_join_field = false;
    for path in walk_files(root, "ledger") {
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        any_file = true;
        let Ok(bytes) = std::fs::read(&path) else { continue };
        let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) else { continue };
        if json.get("change_id").is_some() || json.get("task_id").is_some() {
            any_join_field = true;
            break;
        }
    }
    if any_file && !any_join_field {
        report.violations.push(Violation::new(
            FmtFailureClass::MissingJoinIdentity,
            PathBuf::from("<corpus>"),
            "no ledger record family-wide carries change_id/task_id — new-only fields per S11 design Non-Goal (historical backfill out of scope, only records ingested after S4 populate them)",
        ));
    }
}

/// What a divergence review event needs from the ledger side to check
/// `one-way-backref` — the `ledger_ref` path (normalized relative to
/// `root`) it names, and this divergence event's own file path.
struct BackrefExpectation {
    ledger_relative: PathBuf,
    divergence_relative: PathBuf,
}

/// Ledger pass: returns every ledger record's relative path mapped to
/// whether its OWN content already carries a `divergence_refs` array —
/// `check_divergences` uses this to report `one-way-backref` once per
/// ledger record still missing its reciprocal, after collecting every
/// divergence-side expectation.
fn check_ledger(root: &Path, report: &mut FmtReport) -> BTreeMap<PathBuf, BTreeSet<String>> {
    let mut divergence_refs_present: BTreeMap<PathBuf, BTreeSet<String>> = BTreeMap::new();
    for path in walk_files(root, "ledger") {
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        let rel = relative(root, &path);
        report.files_checked += 1;
        let Ok(bytes) = std::fs::read(&path) else { continue };
        let Ok(json) = serde_json::from_slice::<serde_json::Value>(&bytes) else { continue };

        let kind_str = json.get("kind").and_then(|v| v.as_str()).unwrap_or("run");
        check_schema_conformance(kind_str, &json, &rel, report);
        let Some(kind) = LedgerKind::parse(kind_str) else {
            report.violations.push(Violation::new(FmtFailureClass::LayoutGrammar, &rel, format!("unrecognized ledger `kind`: `{kind_str}`")));
            continue;
        };
        let descriptor = FamilyKind::Ledger(kind).layout_descriptor();

        match resolve::resolve_ledger(kind, &json) {
            Ok(resolved) => {
                if let Some(v) = layout_problem(&descriptor, &rel, &resolved) {
                    report.violations.push(Violation::new(FmtFailureClass::LayoutGrammar, &rel, format!("{}: {}", v.expected, v.detail)));
                }
            }
            Err(ResolveError(detail)) => {
                report.violations.push(Violation::new(FmtFailureClass::LayoutGrammar, &rel, detail));
            }
        }

        check_actor_present(&json, &rel, report);

        if kind.is_run_shaped() {
            let evidence_is_empty = json.get("evidence").and_then(|v| v.as_array()).map(|a| a.is_empty()).unwrap_or(true);
            if evidence_is_empty {
                report.violations.push(Violation::new(FmtFailureClass::UnspecifiedEvidence, &rel, "`evidence` is absent or an empty, untyped array"));
            }
            check_sha_fields(&json, &["app_sha", "harness_sha"], &rel, report);
        } else {
            for field in resolve::ref_fields(kind) {
                check_ref_field(&json, field, &rel, report);
            }
            check_sha_fields(&json, &["app_sha"], &rel, report);
            let present: BTreeSet<String> =
                json.get("divergence_refs").and_then(|v| v.as_array()).map(|a| a.iter().filter_map(|v| v.as_str().map(str::to_string)).collect()).unwrap_or_default();
            divergence_refs_present.insert(rel, present);
        }
    }
    divergence_refs_present
}

fn check_actor_present(json: &serde_json::Value, rel: &Path, report: &mut FmtReport) {
    let has_structured_actor = json.get("actor").and_then(|v| v.as_object()).is_some();
    if !has_structured_actor {
        let detail = if json.get("by").and_then(|v| v.as_str()).is_some() {
            "bare `by` string, not the structured envelope actor"
        } else {
            "no actor/attribution field at all"
        };
        report.violations.push(Violation::new(FmtFailureClass::MissingActor, rel, detail));
    }
}

/// Validate `value` against the family schema REGISTERED for
/// `kind_str` (`canon_model::schema_export::family_schemas`, via
/// [`schema_registry`]) — the S11 review's Critical finding: a
/// per-kind schema pass, not just the hand-written field checks around
/// it. Every schema failure becomes a [`FmtFailureClass::SchemaViolation`]
/// violation; a `kind_str` with no registered schema at all gets its
/// own detail message so "unknown kind" is never silently indistinguishable
/// from "conforms".
fn check_schema_conformance(kind_str: &str, value: &serde_json::Value, rel: &Path, report: &mut FmtReport) {
    match schema_registry::check(kind_str, value) {
        None => {}
        Some(SchemaCheck::NoRegisteredSchema) => {
            report.violations.push(Violation::new(FmtFailureClass::SchemaViolation, rel, format!("no registered schema for kind `{kind_str}`")));
        }
        Some(SchemaCheck::Violations(errors)) => {
            for error in errors {
                report.violations.push(Violation::new(FmtFailureClass::SchemaViolation, rel, error));
            }
        }
    }
}

fn check_sha_fields(json: &serde_json::Value, fields: &[&str], rel: &Path, report: &mut FmtReport) {
    for field in fields {
        if let Some(value) = json.get(*field).and_then(|v| v.as_str()) {
            if !is_full_sha(value) {
                report.violations.push(Violation::new(FmtFailureClass::AbbreviatedSha, rel, format!("`{field}` is `{value}` ({} chars, expected 40)", value.len())));
            }
        }
    }
}

fn check_ref_field(json: &serde_json::Value, field: &str, rel: &Path, report: &mut FmtReport) {
    // A record that already carries a non-empty `refs` array is
    // already structured; skip re-checking its raw ref field.
    if json.get("refs").and_then(|v| v.as_array()).is_some_and(|a| !a.is_empty()) {
        return;
    }
    let Some(raw) = json.get(field).and_then(|v| v.as_str()) else { return };
    if raw.trim().is_empty() {
        return;
    }
    let (refs, unparsed) = resolve::parse_ref_field(raw);
    if crate::refparse::is_joined(raw) {
        report.violations.push(Violation::new(FmtFailureClass::JoinedRef, rel, format!("`{field}` is a joined ref string, not yet a structured `refs` array: `{raw}`")));
    }
    if refs.is_empty() || !unparsed.is_empty() {
        report.violations.push(Violation::new(
            FmtFailureClass::FreeTextRef,
            rel,
            format!("`{field}` has {} segment(s) that do not match `<file>#<symbol>`: {unparsed:?}", unparsed.len()),
        ));
    }
}

fn check_divergences(root: &Path, report: &mut FmtReport, ledger_backrefs: &BTreeMap<PathBuf, BTreeSet<String>>) {
    let mut expectations: Vec<BackrefExpectation> = Vec::new();
    for path in walk_files(root, "divergences") {
        if path.extension().and_then(|e| e.to_str()) != Some("jsonl") {
            continue;
        }
        let rel = relative(root, &path);
        report.files_checked += 1;
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        for line in text.lines() {
            let line = line.trim();
            if line.is_empty() {
                continue;
            }
            let Ok(json) = serde_json::from_str::<serde_json::Value>(line) else { continue };
            check_schema_conformance("divergence", &json, &rel, report);
            match resolve::resolve_divergence(&json) {
                Ok(resolved) => {
                    let descriptor = FamilyKind::Divergence.layout_descriptor();
                    if let Some(v) = layout_problem(&descriptor, &rel, &resolved) {
                        report.violations.push(Violation::new(FmtFailureClass::LayoutGrammar, &rel, format!("{}: {}", v.expected, v.detail)));
                    }
                }
                Err(ResolveError(detail)) => {
                    report.violations.push(Violation::new(FmtFailureClass::LayoutGrammar, &rel, detail));
                }
            }

            let event_type = json.get("type").and_then(|v| v.as_str()).unwrap_or("");
            if json.get("actor").and_then(|v| v.as_object()).is_none() {
                report.violations.push(Violation::new(FmtFailureClass::MissingActor, &rel, format!("`{event_type}` event has no `actor` field")));
            }
            if let Some(sha) = json.get("app_sha").and_then(|v| v.as_str()) {
                if !is_full_sha(sha) {
                    report.violations.push(Violation::new(FmtFailureClass::AbbreviatedSha, &rel, format!("`app_sha` is `{sha}` ({} chars, expected 40)", sha.len())));
                }
            }
            if event_type == "review" {
                if let Some(port_ref) = json.get("port_ref").and_then(|v| v.as_str()) {
                    check_ref_field(&json, "port_ref", &rel, report);
                    let _ = port_ref;
                }
                if let Some(ledger_ref) = json.get("ledger_ref").and_then(|v| v.as_str()) {
                    let normalized = ledger_ref.strip_prefix("spec/").unwrap_or(ledger_ref);
                    expectations.push(BackrefExpectation { ledger_relative: PathBuf::from(normalized), divergence_relative: rel.clone() });
                }
            }
        }
    }

    let mut missing_by_ledger_path: BTreeMap<PathBuf, usize> = BTreeMap::new();
    for expectation in &expectations {
        let already_has = ledger_backrefs
            .get(&expectation.ledger_relative)
            .is_some_and(|refs| refs.iter().any(|r| r == expectation.divergence_relative.to_string_lossy().as_ref()));
        if !already_has {
            *missing_by_ledger_path.entry(expectation.ledger_relative.clone()).or_insert(0) += 1;
        }
    }
    for (ledger_path, count) in missing_by_ledger_path {
        report.violations.push(Violation::new(
            FmtFailureClass::OneWayBackref,
            ledger_path,
            format!("{count} divergence event(s) point at this ledger record via `ledger_ref` with no reciprocal `divergence_refs` entry"),
        ));
    }
}

fn check_features(root: &Path, report: &mut FmtReport) {
    for path in walk_files(root, "features") {
        if path.extension().and_then(|e| e.to_str()) != Some("feature") {
            continue;
        }
        let rel = relative(root, &path);
        report.files_checked += 1;
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        let scan = gherkin::scan(&text);

        match resolve::resolve_feature(&scan.scenario_ids) {
            Ok(resolved) => {
                let descriptor = FamilyKind::Feature.layout_descriptor();
                if let Some(v) = layout_problem(&descriptor, &rel, &resolved) {
                    report.violations.push(Violation::new(FmtFailureClass::LayoutGrammar, &rel, format!("{}: {}", v.expected, v.detail)));
                }
            }
            Err(ResolveError(detail)) => {
                let message = if is_fresh_feature_stub(&text, &scan) {
                    format!("empty feature stub (not yet a valid corpus entry): {detail}")
                } else {
                    detail
                };
                report.violations.push(Violation::new(FmtFailureClass::LayoutGrammar, &rel, message));
            }
        }

        if scan.missing_provenance_count() > 0 {
            report.violations.push(Violation::new(
                FmtFailureClass::MissingProvenance,
                &rel,
                format!("{} of {} Feature:/Scenario: header(s) lack a `# canon: {{...}}` provenance comment", scan.missing_provenance_count(), scan.headers.len()),
            ));
        }
    }
}

/// s19 `wip-feature-stub-class` design D4/R2: the EXACT shape a fresh
/// `canon feature new` writes — a single `Feature:` header carrying its
/// own paired `# canon: {...}` provenance comment, and ZERO
/// `@`-tagged scenarios anywhere in the file. Reuses the SAME
/// [`gherkin::scan`] result [`check_features`] already computed (never
/// a second scan, never a second file-read pass) plus the one text
/// line the scan's own header already points at (`HeaderScan::line_no`)
/// to confirm that lone header is a `Feature:` — not a bare, malformed
/// `Scenario:` header with no preceding tag, a distinct (and much
/// rarer) shape this detector must never claim as "empty feature
/// stub" WIP.
fn is_fresh_feature_stub(text: &str, scan: &gherkin::FeatureScan) -> bool {
    let [only] = scan.headers.as_slice() else { return false };
    if !scan.scenario_ids.is_empty() || !only.has_provenance {
        return false;
    }
    text.lines().nth(only.line_no.saturating_sub(1)).is_some_and(|line| line.trim_start().starts_with("Feature:"))
}

fn check_inventory(root: &Path, report: &mut FmtReport) {
    for path in walk_files(root, "inventory") {
        let rel = relative(root, &path);
        let file_name = path.file_name().and_then(|f| f.to_str()).unwrap_or("");
        let is_yaml = path.extension().and_then(|e| e.to_str()) == Some("yaml");

        if file_name == "assets.lock" {
            report.files_checked += 1;
            report.violations.push(Violation::new(FmtFailureClass::LayoutGrammar, &rel, "assets.lock is a fourth ad-hoc format, not `kind=inventory-lock/assets.lock.yaml`"));
            continue;
        }
        if !is_yaml {
            continue;
        }
        report.files_checked += 1;
        let Ok(text) = std::fs::read_to_string(&path) else { continue };
        let Ok(value) = serde_yaml::from_str::<serde_json::Value>(&text) else { continue };
        let Some(obj) = value.as_object() else { continue };

        if file_name == "assets.lock.yaml" {
            check_schema_conformance("inventory-lock", &value, &rel, report);
            check_envelope_keys(obj, &rel, report);
            continue;
        }

        check_schema_conformance("inventory", &value, &rel, report);
        let entry_keys: Vec<String> = obj.keys().filter(|k| !["schema", "kind", "at", "actor"].contains(&k.as_str())).cloned().collect();
        match resolve::resolve_inventory(&entry_keys) {
            Ok(resolved) => {
                let descriptor = FamilyKind::Inventory.layout_descriptor();
                if let Some(v) = layout_problem(&descriptor, &rel, &resolved) {
                    report.violations.push(Violation::new(FmtFailureClass::LayoutGrammar, &rel, format!("{}: {}", v.expected, v.detail)));
                }
            }
            Err(ResolveError(detail)) => {
                report.violations.push(Violation::new(FmtFailureClass::LayoutGrammar, &rel, detail));
            }
        }
        check_envelope_keys(obj, &rel, report);
    }
}

fn check_envelope_keys(obj: &serde_json::Map<String, serde_json::Value>, rel: &Path, report: &mut FmtReport) {
    let missing: Vec<&str> = ["schema", "kind", "at", "actor"].into_iter().filter(|k| !obj.contains_key(*k)).collect();
    if !missing.is_empty() {
        report.violations.push(Violation::new(FmtFailureClass::MissingEnvelope, rel, format!("missing envelope key(s): {missing:?}")));
    }
}

fn check_policy(root: &Path, report: &mut FmtReport) {
    let path = root.join("policy.yaml");
    if !path.exists() {
        return;
    }
    report.files_checked += 1;
    let rel = relative(root, &path);
    let Ok(text) = std::fs::read_to_string(&path) else { return };
    let Ok(value) = serde_yaml::from_str::<serde_json::Value>(&text) else { return };
    check_schema_conformance("policy", &value, &rel, report);
    let Some(obj) = value.as_object() else { return };
    check_envelope_keys(obj, &rel, report);
}
