//! `canon query --kind <k> [--since <t>] [--plugin <id>] [--json]` (S2
//! task 4.1; s16 P3, tasks.md 3.3, adds `--plugin`): the CLI surface
//! over `canon_store::registry::TierRegistry::query`'s
//! fan-out-across-tiers-and-merge-by-`at` (unified-query spec, design
//! D4) — no cross-tier JOIN happens here or in the library it calls.
//!
//! `--plugin <id>` (s16, `plugin-overlay-projection` spec) is a
//! SEPARATE, ADDITIVE layer: [`run`]/[`format_human`]/[`format_json`]
//! are the pre-s16 functions task 3.4's own "no `--plugin` ⇒
//! byte-identical" hard test pins, and this module never edits them to
//! thread `--plugin` through — [`run_with_plugin`] is a superset
//! function calling the identical query steps [`run`] does, plus
//! [`resolve_and_project`]'s own resolution; [`format_human_with_overlay`]/
//! [`format_json_with_overlay`] are new functions, never a branch
//! bolted onto [`format_human`]/[`format_json`]. `main.rs::run_query`
//! only reaches for the `_with_overlay` formatters when a projection
//! actually resolved (`PluginQueryOutcome::projections` non-empty);
//! otherwise it calls [`format_human`]/[`format_json`] verbatim, the
//! exact same call the no-`--plugin` path makes.

use std::collections::BTreeMap;
use std::path::Path;

use canon_model::envelope::RecordKind;
use canon_model::evidence::RawRecord;
use canon_model::ids::{ChangeId, ProjectId, ScenarioId, TaskId};
use canon_model::records::Scenario;
use canon_plugin::manifest::snapshot::OverlayDecl;
use canon_plugin::{Diagnostic as PluginDiagnostic, project_overlay, resolve_plugin_snapshot};
use canon_store::fold_latest_by_key;
use canon_store::git_tier::GitTier;
use canon_store::registry::TierRegistry;
use canon_store::tier::{StoreError, TierQuery};
use chrono::{DateTime, Utc};
use serde_json::Value;

use crate::context::resolve_canon_yaml;
use crate::tiers::{self, TierCliError};

#[derive(Debug, thiserror::Error)]
pub enum QueryError {
    #[error(transparent)]
    Tiers(#[from] TierCliError),
    #[error(transparent)]
    Store(#[from] StoreError),
}

pub struct QueryOutcome {
    pub kind: RecordKind,
    pub since: Option<DateTime<Utc>>,
    pub records: Vec<RawRecord>,
    pub violation_count: usize,
    /// s19 `query-scope-filters` (design D6): `Some((done, total))` for
    /// `--kind task` only, computed over `records` (i.e. AFTER any
    /// `--change-id`/`--status` filtering) — every other kind's
    /// `QueryOutcome` always carries `None`.
    pub rollup: Option<(usize, usize)>,
}

/// `--kind`'s `clap` value parser — `<k>` is a [`RecordKind`] wire
/// string (`RecordKind::as_str()`, e.g. `handoff`/`strategy_item`),
/// the same snake_case vocabulary `canon.yaml`'s own `routing`/`aging`
/// keys use (`canon-store`'s `policy` module doc), never a second
/// casing convention.
pub fn parse_kind(s: &str) -> Result<RecordKind, String> {
    RecordKind::ALL.into_iter().find(|k| k.as_str() == s).ok_or_else(|| {
        let known = RecordKind::ALL.iter().map(|k| k.as_str()).collect::<Vec<_>>().join(", ");
        format!("unknown record kind `{s}` (expected one of: {known})")
    })
}

/// `--since`'s `clap` value parser — an RFC3339/ISO-8601 timestamp.
pub fn parse_since(s: &str) -> Result<DateTime<Utc>, String> {
    DateTime::parse_from_rfc3339(s).map(|dt| dt.with_timezone(&Utc)).map_err(|e| format!("invalid --since timestamp `{s}`: {e}"))
}

/// `--change-id`'s `clap` value parser — grammar-level `ChangeId::parse`
/// only; KIND-gating (whether `--change-id` even applies to the queried
/// `--kind`) happens in [`validate_scope`], not here, so a malformed
/// grammar and an inapplicable kind produce two distinctly-worded
/// errors (design D5).
pub fn parse_change_id(s: &str) -> Result<ChangeId, String> {
    ChangeId::parse(s).map_err(|e| e.to_string())
}

const TASK_STATUSES: [&str; 2] = ["open", "done"];
const CHANGE_STATUSES: [&str; 4] = ["proposed", "in_progress", "completed", "archived"];
/// s36 `subject-domain-loop`: the closed [`canon_model::SubjectStatus`]
/// vocabulary — `--status`'s valid domain for `--kind subject`, the
/// same snake_case wire strings the model serializes.
const SUBJECT_STATUSES: [&str; 6] = ["proposed", "specced", "building", "verifying", "shipped", "retired"];

fn status_domain(kind: RecordKind) -> &'static [&'static str] {
    match kind {
        RecordKind::Task => &TASK_STATUSES,
        RecordKind::Change => &CHANGE_STATUSES,
        RecordKind::Subject => &SUBJECT_STATUSES,
        _ => &[],
    }
}

/// s19 `query-scope-filters` design D5, extended by s36
/// `subject-domain-loop`: `--change-id`/`--status`/`--domain` usage
/// faults — a flag on a `--kind` that does not support it, or a
/// `--status` value outside the QUERIED kind's own status domain.
#[derive(Debug, thiserror::Error)]
pub enum ScopeError {
    #[error("`{flag}` applies only to {supported} (got `--kind {}`)", .kind.as_str())]
    UnsupportedKind { flag: &'static str, supported: &'static str, kind: RecordKind },
    #[error("`--status {value}` is not valid for `--kind {}` (expected one of: {})", .kind.as_str(), .valid.join(", "))]
    InvalidStatus { kind: RecordKind, value: String, valid: Vec<String> },
}

/// Validate `--change-id`/`--status`/`--domain` BEFORE any tier read
/// (task 3.2): per-flag kind-gating (design D5) first, then — only
/// once gating passes — `--status`'s value against the queried kind's
/// own domain (design D5, task 3.3). `--change-id` applies to
/// `change`/`task`; `--status` to `change`/`task`/`subject` (s36);
/// `--domain` to `subject` only (s36).
pub fn validate_scope(kind: RecordKind, change_id: Option<&ChangeId>, status: Option<&str>, domain: Option<&str>) -> Result<(), ScopeError> {
    if change_id.is_some() && !matches!(kind, RecordKind::Change | RecordKind::Task) {
        return Err(ScopeError::UnsupportedKind { flag: "--change-id", supported: "`--kind change`/`--kind task`", kind });
    }
    if status.is_some() && !matches!(kind, RecordKind::Change | RecordKind::Task | RecordKind::Subject) {
        return Err(ScopeError::UnsupportedKind { flag: "--status", supported: "`--kind change`/`--kind task`/`--kind subject`", kind });
    }
    if domain.is_some() && kind != RecordKind::Subject {
        return Err(ScopeError::UnsupportedKind { flag: "--domain", supported: "`--kind subject`", kind });
    }
    if let Some(value) = status {
        let allowed = status_domain(kind);
        if !allowed.contains(&value) {
            return Err(ScopeError::InvalidStatus { kind, value: value.to_string(), valid: allowed.iter().map(|s| s.to_string()).collect() });
        }
    }
    Ok(())
}

fn record_change_id(raw: &RawRecord) -> Option<ChangeId> {
    raw.0.get("change_id").and_then(Value::as_str).and_then(|s| ChangeId::parse(s).ok())
}

fn record_task_id(raw: &RawRecord) -> Option<TaskId> {
    raw.0.get("task_id").and_then(Value::as_str).and_then(|s| TaskId::parse(s).ok())
}

/// `--change-id`'s per-record match (design D5/spec Requirement 2):
/// `Change` records by `change_id` equality, `Task` records by
/// `TaskId::change_id()`'s owning change — the SAME derivation
/// `canon-ingest`'s plan/verdict adapters already use, never a second
/// parsing of the task id string.
fn matches_change_id(kind: RecordKind, raw: &RawRecord, change_id: &ChangeId) -> bool {
    match kind {
        RecordKind::Change => record_change_id(raw).as_ref() == Some(change_id),
        RecordKind::Task => record_task_id(raw).map(|t| t.change_id()).as_ref() == Some(change_id),
        _ => true,
    }
}

fn matches_status(raw: &RawRecord, status: &str) -> bool {
    raw.0.get("status").and_then(Value::as_str) == Some(status)
}

/// `--domain <d>`'s per-record match (s36 `subject-domain-loop`):
/// `Subject` records by their own `domain` field equality. Only
/// `--kind subject` ever carries `--domain` ([`validate_scope`]), so
/// this is only reached for subject rows.
fn matches_domain(raw: &RawRecord, domain: &str) -> bool {
    raw.0.get("domain").and_then(Value::as_str) == Some(domain)
}

/// `--kind change`/`--kind task`'s deterministic sort key (design D6):
/// `(change_id, task-number-segments)`, reusing the SAME natural key
/// [`canon_store::partition::resolve_partition`] already derives for
/// these two kinds (`format_human`'s own per-row call) — `Task`'s
/// natural key IS its `task_id` string, split once on `#` into the
/// owning change plus its dot-separated task number, parsed as
/// integers so `1.2` sorts before `1.10` (never a lexicographic string
/// compare, which would order them the other way).
fn scope_sort_key(kind: RecordKind, raw: &RawRecord) -> (String, Vec<u64>) {
    let natural_key = canon_store::partition::resolve_partition(kind, &raw.0).map(|p| p.natural_key).unwrap_or_default();
    match kind {
        RecordKind::Task => {
            let (change, number) = natural_key.split_once('#').unwrap_or((natural_key.as_str(), ""));
            (change.to_string(), number.split('.').filter_map(|s| s.parse().ok()).collect())
        }
        _ => (natural_key, Vec::new()),
    }
}

/// s21 P4 (design.md D5, spec `cross-tier-supersession` "Every reader of
/// a PgTier-routed kind resolves current state through the shared
/// fold, with no reader exempted"): `task`/`handoff`/`session`/`run`/
/// `event` are routed to the hot rung (backed by `PgTier`, per
/// `canon.yaml`'s `routing`), whose `read` now returns every retained
/// historical version (s21 P3) rather than one row per key — a raw
/// `TierRegistry::query` result for one of these kinds is no longer
/// safe to hand straight to [`apply_scope`]/[`rollup_for`]. Folds via
/// the SAME shared `fold_latest_by_key` every other multi-version
/// reader already applies (`canon-gate::ledger::latest_verdicts`,
/// `canon-report::divergence`), keyed by the SAME natural key
/// [`scope_sort_key`]/[`format_human`] already derive
/// (`canon_store::partition::resolve_partition`), winner = greatest
/// `(at, content_digest12)`. A no-op for every kind routed to a
/// local/cold (GitTier/R2Tier-backed) rung (returned untouched)
/// and for a corpus with no supersession (row-count parity, design.md
/// R3's own mitigation).
///
/// MAINTAINER NOTE (s21 review, ReviewS21 important-finding): this list is
/// KIND-gated, not routing-derived, on purpose — the R3 acceptance tests
/// prove supersession on a git-routed fixture (no live Postgres), and the
/// fold is correct by KIND identity regardless of the routed rung. The
/// trade-off: it must stay in sync with `canon.yaml`'s hot-rung routing —
/// a kind NEWLY routed to `hot` (whose `PgTier::read` no longer pre-folds)
/// MUST be added here or its `canon query` result regresses to
/// N-independent-versions. The hot-routed set is asserted to equal this
/// list by `fold_list_matches_pg_routing` (tests/query.rs).
fn fold_pg_routed_kind(kind: RecordKind, records: Vec<RawRecord>) -> Vec<RawRecord> {
    if !matches!(kind, RecordKind::Task | RecordKind::Handoff | RecordKind::Session | RecordKind::Run | RecordKind::Event) {
        return records;
    }
    fold_latest_by_natural_key(kind, records)
}

/// s36 `subject-domain-loop`: `subject` is git-routed (local rung) but,
/// unlike an authored-once `Review`, is RE-WRITTEN by `canon subject
/// adopt`/`canon subject status` — each appends links / flips state at
/// a bumped envelope `at`, a genuine new git-tier append at a new path
/// (`canon_store::partition` module doc). A bare `TierRegistry::query`
/// for `subject` therefore returns EVERY historical version; this
/// folds them to one latest row per `subject_id`, so `canon query
/// --kind subject` reads an adopt/status re-write as ONE current
/// record. A no-op for every other kind.
fn fold_subject_kind(kind: RecordKind, records: Vec<RawRecord>) -> Vec<RawRecord> {
    if kind != RecordKind::Subject {
        return records;
    }
    fold_latest_by_natural_key(kind, records)
}

/// The shared fold-to-latest-per-natural-key body (design D11/s21 D3):
/// winner per natural key ([`canon_store::partition::resolve_partition`])
/// is the greatest `(at, content_digest12)` pair, via the SAME
/// [`fold_latest_by_key`] `canon-gate::ledger` and `canon-report`
/// already use. Called by both [`fold_pg_routed_kind`] (hot-routed
/// multi-version kinds) and [`fold_subject_kind`] (git-routed but
/// re-written), so the two can never drift in fold rule.
fn fold_latest_by_natural_key(kind: RecordKind, records: Vec<RawRecord>) -> Vec<RawRecord> {
    struct Candidate {
        key: String,
        at: DateTime<Utc>,
        digest: String,
        record: RawRecord,
    }
    let candidates = records.into_iter().map(|record| {
        let key = canon_store::partition::resolve_partition(kind, &record.0).map(|p| p.natural_key).unwrap_or_default();
        let at = canon_store::tier::raw_record_at(&record);
        let digest = canon_store::partition::content_digest12(&record.0);
        Candidate { key, at, digest, record }
    });
    fold_latest_by_key(candidates, |c| c.key.clone(), |c| c.at, |c| c.digest.as_str()).into_values().map(|c| c.record).collect()
}

/// Post-tier-merge scope application (design D5/D6, tasks 3.4/3.6;
/// `--domain` added by s36): filters `records` by
/// `change_id`/`status`/`domain` when given, then — for `--kind
/// change`/`--kind task` UNCONDITIONALLY, filtered or not — sorts
/// ascending by [`scope_sort_key`]. Every other kind's records pass
/// through completely untouched (order and all), preserving
/// `TierRegistry::query`'s native `at`-merge order (design D6).
fn apply_scope(kind: RecordKind, records: Vec<RawRecord>, change_id: Option<&ChangeId>, status: Option<&str>, domain: Option<&str>) -> Vec<RawRecord> {
    let mut records: Vec<RawRecord> = records
        .into_iter()
        .filter(|r| change_id.is_none_or(|c| matches_change_id(kind, r, c)))
        .filter(|r| status.is_none_or(|s| matches_status(r, s)))
        .filter(|r| domain.is_none_or(|d| matches_domain(r, d)))
        .collect();
    if matches!(kind, RecordKind::Change | RecordKind::Task) {
        records.sort_by_key(|r| scope_sort_key(kind, r));
    }
    records
}

/// `--kind task`'s `done`/`total` rollup (design D6, task 3.5), computed
/// over the (possibly `--change-id`/`--status`-filtered) result set —
/// `None` for every other kind.
fn rollup_for(kind: RecordKind, records: &[RawRecord]) -> Option<(usize, usize)> {
    if kind != RecordKind::Task {
        return None;
    }
    let done = records.iter().filter(|r| matches_status(r, "done")).count();
    Some((done, records.len()))
}

pub fn run(
    repo: &Path,
    canon_yaml: Option<&Path>,
    kind: RecordKind,
    since: Option<DateTime<Utc>>,
    change_id: Option<&ChangeId>,
    status: Option<&str>,
    domain: Option<&str>,
) -> Result<QueryOutcome, QueryError> {
    let canon_yaml_path = resolve_canon_yaml(repo, canon_yaml);
    let loaded = tiers::build_lenient_tiers_for_kind(&canon_yaml_path, kind)?;
    let registry = TierRegistry::new(loaded.policy, loaded.git, loaded.pg, loaded.r2, loaded.sqlite);

    let mut query = TierQuery::kind(kind);
    if let Some(since) = since {
        query = query.since(since);
    }

    let result = registry.query(&query)?;
    let violation_count = result.violations.len();
    let records = fold_pg_routed_kind(kind, result.records);
    let records = fold_subject_kind(kind, records);
    let records = apply_scope(kind, records, change_id, status, domain);
    let rollup = rollup_for(kind, &records);
    Ok(QueryOutcome { kind, since, records, violation_count, rollup })
}

/// Default human-table report: a header line (`--kind`, `--since`,
/// record count) plus one row per merged record — natural key (via
/// [`canon_store::partition::resolve_partition`]) and content digest,
/// ordered by `at` exactly as [`TierRegistry::query`] returns them.
pub fn format_human(outcome: &QueryOutcome) -> String {
    let since_desc = outcome.since.map(|s| s.to_rfc3339()).unwrap_or_else(|| "none".to_string());
    let mut out = format!("canon query --kind {} --since {}: {} record(s)", outcome.kind.as_str(), since_desc, outcome.records.len());
    if outcome.violation_count > 0 {
        out.push_str(&format!("  ({} violation(s) reported, excluded)", outcome.violation_count));
    }
    out.push('\n');
    if let Some((done, total)) = outcome.rollup {
        out.push_str(&format!("{done}/{total} done\n"));
    }

    if outcome.records.is_empty() {
        return out;
    }

    out.push_str("\nAT                             ID                              DIGEST\n");
    for raw in &outcome.records {
        let at = canon_store::tier::raw_record_at(raw);
        let id = canon_store::partition::resolve_partition(outcome.kind, &raw.0)
            .map(|p| match p.area {
                Some(area) => format!("area={area}/{}", p.natural_key),
                None => p.natural_key,
            })
            .unwrap_or_else(|_| "-".to_string());
        let digest = canon_store::partition::content_digest12(&raw.0);
        out.push_str(&format!("{:<30} {:<30} {}\n", at.to_rfc3339(), id, digest));
    }
    out
}

/// `--json`: machine-readable output — the full merged record bodies,
/// never a human-table-shaped projection.
pub fn format_json(outcome: &QueryOutcome) -> String {
    let mut payload = serde_json::json!({
        "kind": outcome.kind.as_str(),
        "since": outcome.since.map(|s| s.to_rfc3339()),
        "count": outcome.records.len(),
        "violations": outcome.violation_count,
        "records": outcome.records.iter().map(|r| r.0.clone()).collect::<Vec<_>>(),
    });
    if let Some((done, total)) = outcome.rollup {
        payload.as_object_mut().expect("payload is always a JSON object").insert("rollup".to_string(), serde_json::json!({"done": done, "total": total}));
    }
    serde_json::to_string_pretty(&payload).expect("serde_json::Value always serializes")
}

/// One resolved overlay's projected view (task 3.3): the `<namespace>.
/// <kind>` identity it came from, plus [`project_overlay`]'s own
/// folded output.
pub struct OverlayProjection {
    pub identity: String,
    pub projected: BTreeMap<(ProjectId, ScenarioId), serde_json::Map<String, Value>>,
}

/// `--plugin <id>`'s full resolution result (task 3.3, design.md D3,
/// `plugin-overlay-projection` spec's fail-soft requirement):
/// `projections` is EMPTY whenever the named plugin has no installed
/// manifest, the plugin declares no overlay at all, OR (parent-agent
/// steer, `--kind`/`--plugin` `core_kind` mismatch pin) the plugin's
/// overlay(s) all declare a DIFFERENT `core_kind` than the queried
/// `--kind` — every one of these degrades to the unmodified core view,
/// never a panic, never a process error. `diagnostics` is
/// human-readable text, always safe to print on stderr, carrying every
/// reason along the way (resolution diagnostics, the `core_kind`
/// mismatch message, or a per-record [`project_overlay`] skip) —
/// non-empty whenever `projections` is empty, and possibly non-empty
/// even when `projections` isn't (a malformed sibling overlay record
/// is still worth reporting).
#[derive(Default)]
pub struct PluginQueryOutcome {
    pub projections: Vec<OverlayProjection>,
    pub diagnostics: Vec<String>,
}

fn format_plugin_diag(d: &PluginDiagnostic) -> String {
    format!("[{}] {}: {}", d.code, d.subject, d.message)
}

/// Resolve `plugin_id`'s overlay declaration(s) for `kind` under
/// `project_dir` and project them onto `core` (task 3.3): fail-soft at
/// every step (spec.md "Projection is fail-soft when a plugin or an
/// overlay record is absent"). An unresolved plugin, a plugin
/// declaring no overlay at all, or — the exact scenario a
/// `--kind`/`--plugin` mismatch produces — every resolved overlay's
/// OWN `core_kind` disagreeing with `kind`, all degrade to an EMPTY
/// `projections` list plus an explanatory diagnostic; this function
/// NEVER deserializes `core` as `Scenario`, and NEVER scans a single
/// overlay record, unless at least one resolved overlay's `core_kind`
/// actually equals `kind.as_str()` — so a mismatched `--kind` never
/// even reaches [`project_overlay`], and the core view is guaranteed
/// untouched.
fn resolve_and_project(project_dir: &Path, git: Option<&GitTier>, kind: RecordKind, plugin_id: &str, core: &[RawRecord]) -> PluginQueryOutcome {
    let mut diagnostics = Vec::new();
    let (snapshot, resolve_diags) = resolve_plugin_snapshot(project_dir);
    diagnostics.extend(resolve_diags.iter().map(format_plugin_diag));

    let Some(resolved) = snapshot.plugins.get(plugin_id) else {
        diagnostics.push(format!("plugin `{plugin_id}` has no installed manifest under `canon/plugins/` -- unmodified core view"));
        return PluginQueryOutcome { projections: Vec::new(), diagnostics };
    };

    // Select overlays this plugin OWNS (by identity), never every overlay
    // sharing its namespace -- two installed plugins may share a namespace
    // with different kinds, and `--plugin a` must never scan/project `b`'s
    // records (ReviewS16P3 F2).
    let mut owned_overlays: Vec<&OverlayDecl> = resolved.overlays.iter().filter_map(|id| snapshot.overlays.get(id)).collect();
    owned_overlays.sort_by(|a, b| a.identity.cmp(&b.identity));

    if owned_overlays.is_empty() {
        diagnostics.push(format!("plugin `{plugin_id}` declares no overlay -- unmodified core view"));
        return PluginQueryOutcome { projections: Vec::new(), diagnostics };
    }

    let matching: Vec<&OverlayDecl> = owned_overlays.iter().filter(|d| d.core_kind == kind.as_str()).copied().collect();
    if matching.is_empty() {
        let declared = owned_overlays[0].core_kind.clone();
        diagnostics.push(format!("plugin `{plugin_id}` overlays core_kind=`{declared}`, not `{}`; no projection applied", kind.as_str()));
        return PluginQueryOutcome { projections: Vec::new(), diagnostics };
    }

    let scenarios: Vec<Scenario> = core
        .iter()
        .filter_map(|raw| match serde_json::from_value::<Scenario>(raw.0.clone()) {
            Ok(s) => Some(s),
            Err(e) => {
                diagnostics.push(format!("a queried record failed to deserialize as Scenario, excluded from projection: {e}"));
                None
            }
        })
        .collect();

    let mut projections = Vec::new();
    for decl in matching {
        let (overlay_raw, scan_violations) = match git {
            Some(git) => match git.scan_namespaced_kind(&decl.identity) {
                Ok((records, violations)) => (records.into_iter().map(|(_, r)| r).collect::<Vec<_>>(), violations),
                Err(e) => {
                    diagnostics.push(format!("scanning overlay records for `{}`: {e}", decl.identity));
                    (Vec::new(), Vec::new())
                }
            },
            None => {
                diagnostics.push("no git tier configured -- unmodified core view".to_string());
                (Vec::new(), Vec::new())
            }
        };
        diagnostics.extend(scan_violations.iter().map(|v| format!("overlay scan violation ({}): {v}", decl.identity)));

        let (projected, project_diags) = project_overlay(&scenarios, &overlay_raw, decl);
        diagnostics.extend(project_diags.iter().map(format_plugin_diag));
        // Only a projection that actually produced rows carries the plugin
        // framing forward; an empty projected map (no git tier, scan error,
        // zero overlay records, or all malformed) degrades to the unmodified
        // core view the diagnostics above promise (ReviewS16P3 F1) -- the
        // per-record no-overlay case is unaffected (its projected map is
        // non-empty for the keys that DID match).
        if !projected.is_empty() {
            projections.push(OverlayProjection { identity: decl.identity.clone(), projected });
        }
    }

    PluginQueryOutcome { projections, diagnostics }
}

/// `--plugin <id>`'s `(project_id, scenario_id)` key, extracted from a
/// queried [`RawRecord`]'s own JSON body — `None` for a record that
/// isn't a well-formed `Scenario` shape.
fn scenario_key(raw: &RawRecord) -> Option<(ProjectId, ScenarioId)> {
    let obj = raw.0.as_object()?;
    let project_id = obj.get("project_id").and_then(Value::as_str).and_then(|s| ProjectId::parse(s).ok())?;
    let scenario_id = obj.get("scenario_id").and_then(Value::as_str).and_then(|s| ScenarioId::parse(s).ok())?;
    Some((project_id, scenario_id))
}

/// `--plugin <id>` variant of [`run`] (task 3.3): the identical
/// `tiers::build_lenient_tiers_for_kind`/`TierRegistry::query` steps
/// [`run`] itself performs (s22 `query-tier-degradation`), PLUS
/// `--plugin <id>`'s resolved projection layered on
/// top. [`run`] is left completely untouched (never calls this
/// function, never shares a body with it) — task 3.4's byte-identical
/// hard test pins [`run`]'s own behavior, so this function duplicates
/// rather than refactors [`run`]'s few query-construction lines,
/// eliminating any risk of the two silently drifting into each other.
pub fn run_with_plugin(
    repo: &Path,
    canon_yaml: Option<&Path>,
    kind: RecordKind,
    since: Option<DateTime<Utc>>,
    plugin_id: &str,
    change_id: Option<&ChangeId>,
    status: Option<&str>,
    domain: Option<&str>,
) -> Result<(QueryOutcome, PluginQueryOutcome), QueryError> {
    let canon_yaml_path = resolve_canon_yaml(repo, canon_yaml);
    let loaded = tiers::build_lenient_tiers_for_kind(&canon_yaml_path, kind)?;
    let project_dir = tiers::project_dir(&canon_yaml_path).to_path_buf();
    let git_root = loaded.git.as_ref().map(|g| g.root().to_path_buf());

    let registry = TierRegistry::new(loaded.policy, loaded.git, loaded.pg, loaded.r2, loaded.sqlite);

    let mut query = TierQuery::kind(kind);
    if let Some(since) = since {
        query = query.since(since);
    }

    let result = registry.query(&query)?;
    let violation_count = result.violations.len();
    let records = fold_pg_routed_kind(kind, result.records);
    let records = fold_subject_kind(kind, records);
    let records = apply_scope(kind, records, change_id, status, domain);
    let rollup = rollup_for(kind, &records);
    let outcome = QueryOutcome { kind, since, records, violation_count, rollup };

    let git = git_root.map(GitTier::new);
    let plugin_outcome = resolve_and_project(&project_dir, git.as_ref(), kind, plugin_id, &outcome.records);

    Ok((outcome, plugin_outcome))
}

/// `--json`, `--plugin <id>` variant of [`format_json`] (task 3.3):
/// identical `kind`/`since`/`count`/`violations` fields PLUS
/// `"plugin"` (the resolved plugin id) and `"overlays"` (the overlay
/// identities actually projected); each record in `"records"` gains an
/// `"overlay"` object — keyed by overlay identity, carrying ONLY that
/// overlay's declared fields — for every projection with a matching
/// key, and NO `"overlay"` key at all when no projection matched
/// (spec.md: "no default or guessed value is invented"). Never called
/// by `main.rs::run_query` unless `projections` is non-empty — the
/// empty case calls [`format_json`] verbatim instead.
pub fn format_json_with_overlay(outcome: &QueryOutcome, plugin_id: &str, projections: &[OverlayProjection]) -> String {
    let records: Vec<Value> = outcome.records.iter().map(|raw| merge_overlay_fields(raw, projections)).collect();
    let mut payload = serde_json::json!({
        "kind": outcome.kind.as_str(),
        "since": outcome.since.map(|s| s.to_rfc3339()),
        "count": outcome.records.len(),
        "violations": outcome.violation_count,
        "plugin": plugin_id,
        "overlays": projections.iter().map(|p| p.identity.clone()).collect::<Vec<_>>(),
        "records": records,
    });
    if let Some((done, total)) = outcome.rollup {
        payload.as_object_mut().expect("payload is always a JSON object").insert("rollup".to_string(), serde_json::json!({"done": done, "total": total}));
    }
    serde_json::to_string_pretty(&payload).expect("serde_json::Value always serializes")
}

fn merge_overlay_fields(raw: &RawRecord, projections: &[OverlayProjection]) -> Value {
    let Some(key) = scenario_key(raw) else { return raw.0.clone() };
    let mut overlay_obj = serde_json::Map::new();
    for projection in projections {
        if let Some(fields) = projection.projected.get(&key) {
            overlay_obj.insert(projection.identity.clone(), Value::Object(fields.clone()));
        }
    }
    if overlay_obj.is_empty() {
        return raw.0.clone();
    }
    let mut merged = raw.0.clone();
    if let Some(obj) = merged.as_object_mut() {
        obj.insert("overlay".to_string(), Value::Object(overlay_obj));
    }
    merged
}

/// `--plugin <id>` variant of [`format_human`] (task 3.3): the same
/// header line plus `--plugin <id>`, and a 4th `OVERLAY` table column
/// — a compact JSON object per matched record (identical shape to
/// [`format_json_with_overlay`]'s per-record `"overlay"` key), `-`
/// when no projection matched. `AT`/`ID`/`DIGEST` are computed from
/// each record's OWN untouched [`RawRecord`] (never a merged copy), so
/// the digest column always reflects the record's REAL on-disk content
/// — merging overlay fields into a record before hashing it would make
/// the digest column lie about what is actually stored (design.md D3).
/// Never called by `main.rs::run_query` unless `projections` is
/// non-empty — the empty case calls [`format_human`] verbatim instead.
pub fn format_human_with_overlay(outcome: &QueryOutcome, plugin_id: &str, projections: &[OverlayProjection]) -> String {
    let since_desc = outcome.since.map(|s| s.to_rfc3339()).unwrap_or_else(|| "none".to_string());
    let mut out =
        format!("canon query --kind {} --since {} --plugin {}: {} record(s)", outcome.kind.as_str(), since_desc, plugin_id, outcome.records.len());
    if outcome.violation_count > 0 {
        out.push_str(&format!("  ({} violation(s) reported, excluded)", outcome.violation_count));
    }
    out.push('\n');
    if let Some((done, total)) = outcome.rollup {
        out.push_str(&format!("{done}/{total} done\n"));
    }

    if outcome.records.is_empty() {
        return out;
    }

    out.push_str("\nAT                             ID                              DIGEST           OVERLAY\n");
    for raw in &outcome.records {
        let at = canon_store::tier::raw_record_at(raw);
        let id = canon_store::partition::resolve_partition(outcome.kind, &raw.0)
            .map(|p| match p.area {
                Some(area) => format!("area={area}/{}", p.natural_key),
                None => p.natural_key,
            })
            .unwrap_or_else(|_| "-".to_string());
        let digest = canon_store::partition::content_digest12(&raw.0);
        let overlay_desc = scenario_key(raw)
            .map(|key| {
                let mut obj = serde_json::Map::new();
                for projection in projections {
                    if let Some(fields) = projection.projected.get(&key) {
                        obj.insert(projection.identity.clone(), Value::Object(fields.clone()));
                    }
                }
                if obj.is_empty() { "-".to_string() } else { Value::Object(obj).to_string() }
            })
            .unwrap_or_else(|| "-".to_string());
        out.push_str(&format!("{:<30} {:<30} {:<16} {}\n", at.to_rfc3339(), id, digest, overlay_desc));
    }
    out
}
