//! Privacy lifecycle commands: purge, export, and append-only access audit.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use canon_learn::{LearnConfig, ParquetTrajectoryStore, TrajectoryStore};
use canon_model::envelope::RecordKind;
use canon_store::registry::{PurgeOutcome, RetentionService, TierRegistry};
use canon_store::tier::{raw_record_at, TierQuery};
use chrono::{DateTime, Utc};
use serde_json::json;

use crate::cli_error::{CliError, CliErrorExt as _};
use crate::context::resolve_canon_yaml;
use crate::json_out::Report;
use crate::safety::{path_digest, Effect, Mode, Plan, Preview, Reads};
use schemars::JsonSchema;
use serde::Serialize;
use crate::tiers;

#[derive(Debug, thiserror::Error)]
pub enum RetentionError {
    #[error("{0}")]
    Message(String),
    #[error("store: {0}")]
    Store(#[from] canon_store::tier::StoreError),
    #[error("learn: {0}")]
    Learn(#[from] canon_learn::LearnError),
    #[error("partial purge: {matched} record(s) matched; at least {deleted} deleted: {reason}; purge backends are non-transactional")]
    PartialPurge { matched: usize, deleted: usize, reason: String },
    #[error("{}", .0.message)]
    Refusal(Box<CliError>),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("yaml: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("tier: {0}")]
    Tiers(#[from] tiers::TierCliError),
}

/// What to do about a refusal of [`tiers_code`].
fn tiers_hint(code: &str) -> &'static str {
    match code {
        "RETENTION_TIERS_REQUIRED" => "set the variables the message names for the tier (or add its `tiers:` entry to canon.yaml)",
        "RETENTION_TIERS_CORRUPT" => "restore the damaged object the message names from a copy, or remove it",
        "RETENTION_TIERS_UNAVAILABLE" => "check that the tier backends are reachable (wait a moment if the system failed the read)",
        "RETENTION_TIERS_DENIED" => "make the file the message names readable by this user",
        _ => "fix the tier or policy problem the message names in canon.yaml",
    }
}

/// The code of a purge or export that could not open or read the tiers before it began, by what is wrong:
/// - a tier that is not attached or not configured (its endpoint or credentials are unset, or canon.yaml has no entry for it) is
///   `RETENTION_TIERS_REQUIRED`: waiting does not set them;
/// - a stored object that does not decode (a Parquet file canon wrote) is `RETENTION_TIERS_CORRUPT`;
/// - a backend that is down or does not answer (a SQL or object-store error) is `RETENTION_TIERS_UNAVAILABLE`, which can be tried again;
/// - a policy, routing entry or config the caller wrote that cannot be used is `RETENTION_TIERS_INVALID`.
///
/// An operating-system error is told by its cause (`read_cause`): a read the system refuses is `RETENTION_TIERS_DENIED`, a path that is
/// not there or is no file or directory (the caller's wrong path) is `RETENTION_TIERS_INVALID`, and only an error a repeat can pass
/// (`EIO`, an interrupted or timed-out call) is `RETENTION_TIERS_UNAVAILABLE`, like a backend that is down.
fn tiers_code(error: &RetentionError) -> &'static str {
    use crate::read_cause::{of_io, Cause};
    use canon_store::tier::StoreError;
    let by_cause = |io: &std::io::Error| match of_io(io) {
        Cause::Denied => "RETENTION_TIERS_DENIED",
        Cause::Unavailable => "RETENTION_TIERS_UNAVAILABLE",
        Cause::NotFound | Cause::WrongPath | Cause::Other => "RETENTION_TIERS_INVALID",
    };
    let by_store = |store: &StoreError| match store {
        StoreError::Io(io) => by_cause(io),
        StoreError::TierUnavailable { .. } | StoreError::BackendUnattached { .. } => "RETENTION_TIERS_REQUIRED",
        StoreError::Parquet(_) => "RETENTION_TIERS_CORRUPT",
        StoreError::Sql(_) | StoreError::ObjectStore(_) => "RETENTION_TIERS_UNAVAILABLE",
        _ => "RETENTION_TIERS_INVALID",
    };
    match error {
        RetentionError::Io(io) | RetentionError::Tiers(tiers::TierCliError::ReadCanonYaml { source: io, .. }) => by_cause(io),
        RetentionError::Store(store) | RetentionError::Tiers(tiers::TierCliError::Store(store)) => by_store(store),
        RetentionError::Learn(canon_learn::LearnError::Parquet(_)) => "RETENTION_TIERS_CORRUPT",
        RetentionError::Learn(canon_learn::LearnError::Io(io)) => by_cause(io),
        _ => "RETENTION_TIERS_INVALID",
    }
}

fn actor() -> String {
    std::env::var("CANON_ACTOR")
        .ok()
        .filter(|s| !s.trim().is_empty())
        .or_else(|| std::env::var("USER").ok())
        .or_else(|| std::env::var("USERNAME").ok())
        .unwrap_or_else(|| "unknown".to_string())
}

/// Where an access event is appended: beside the `canon.yaml` the
/// command read, or nowhere when that file does not exist.
///
/// `canon.yaml` is the only marker that a directory is a canon repo. A
/// read run where canon was never set up, or against another repository
/// through `--repo`/`--canon-yaml`, used to create `.canon/audit/` in the
/// current directory — an untracked write in a repository the operator
/// had only read (0.15 D6, dogfood eval 2 G14). The event now goes to
/// the repository whose `canon.yaml` was read, and to no directory at
/// all when there is none.
fn audit_path(repo: &Path) -> Option<PathBuf> {
    let yaml = resolve_canon_yaml(repo);
    if !yaml.is_file() {
        return None;
    }
    let root = yaml.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or(Path::new("."));
    Some(root.join(canon_model::paths::ACCESS_AUDIT_FILE))
}

/// Append one access event without recording query contents, credentials, or
/// record bodies. New audit files are created mode 0600. Silently writes
/// nothing when there is no `canon.yaml` to scope the event to
/// ([`audit_path`]).
pub fn append_access_audit(
    repo: &Path,
    operation: &str,
    kind: Option<RecordKind>,
    scope: &str,
    sensitive: bool,
    matched: usize,
    deleted: usize,
) -> Result<(), RetentionError> {
    let Some(path) = audit_path(repo) else {
        return Ok(());
    };
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    #[cfg(unix)]
    let options = {
        use std::os::unix::fs::OpenOptionsExt;
        let mut options = OpenOptions::new();
        options.create(true).append(true).mode(0o600);
        options
    };
    #[cfg(not(unix))]
    let mut options = {
        let mut options = OpenOptions::new();
        options.create(true).append(true);
        options
    };
    let line = json!({
        "actor": actor(),
        "operation": operation,
        "kind": kind.map(RecordKind::as_str),
        "scope": scope,
        "sensitive": sensitive,
        "result_counts": {"matched": matched, "deleted": deleted},
        "at": Utc::now().to_rfc3339(),
    });
    writeln!(options.open(path)?, "{}", serde_json::to_string(&line)?)?;
    Ok(())
}

fn sensitive_allowed(repo: &Path) -> Result<bool, RetentionError> {
    let path = repo.join(".canon/policy.yaml");
    if !path.exists() {
        return Ok(false);
    }
    let value: serde_yaml::Value = serde_yaml::from_str(&fs::read_to_string(path)?)?;
    Ok(value.get("query").and_then(|q| q.get("allow_sensitive")).and_then(serde_yaml::Value::as_bool).unwrap_or(false))
}

pub fn validate_sensitive(repo: &Path, requested: bool) -> Result<(), RetentionError> {
    ensure_sensitive(repo, requested)
}

fn ensure_sensitive(repo: &Path, requested: bool) -> Result<(), RetentionError> {
    if requested && !sensitive_allowed(repo)? {
        return Err(RetentionError::Message("--include-sensitive requires policy query.allow_sensitive=true".to_string()));
    }
    Ok(())
}

/// The registry of the tiers `kind` lives in. `read_only`: an absent sqlite hot db is read as empty and not created.
fn loaded_registry(repo: &Path, kind: RecordKind, read_only: bool) -> Result<(PathBuf, TierRegistry), RetentionError> {
    let yaml = resolve_canon_yaml(repo);
    let loaded = if read_only { tiers::build_lenient_tiers_for_kind_read(&yaml, kind)? } else { tiers::build_lenient_tiers_for_kind(&yaml, kind)? };
    let registry = TierRegistry::new(loaded.policy, loaded.git, loaded.pg, loaded.r2, loaded.sqlite);
    Ok((yaml, registry))
}

/// Open the operator-local trajectory store used by `canon-learn`. A
/// retention command must not turn a missing/unreadable configured learn root
/// into a successful zero-row purge: that would leave the raw learning tier
/// untouched while claiming the kind was fully retained.
fn loaded_trajectory_store(repo: &Path, yaml: &Path) -> Result<ParquetTrajectoryStore, RetentionError> {
    let manifest = fs::read_to_string(yaml)?;
    let config = LearnConfig::from_manifest(&manifest).map_err(|err| RetentionError::Message(format!("cannot load learn retention config from {}: {err}", yaml.display())))?;
    let project_root = yaml.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or(repo);
    let learn_root = project_root.join(&config.root);
    let metadata = fs::metadata(&learn_root).map_err(|err| {
        RetentionError::Message(format!("trajectory purge requires configured learn root {}: {err}", learn_root.display()))
    })?;
    if !metadata.is_dir() {
        return Err(RetentionError::Message(format!("trajectory purge requires learn root {} to be a directory", learn_root.display())));
    }
    fs::read_dir(&learn_root).map_err(|err| RetentionError::Message(format!("trajectory purge cannot read learn root {}: {err}", learn_root.display())))?;
    // Opening the store is side-effect free. Its `trajectories` child may be
    // absent in a newly initialized but otherwise usable learn root; the
    // store treats that as an empty tier.
    Ok(ParquetTrajectoryStore::open(learn_root.join("trajectories")))
}

/// One rung's counts in a purge report.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PurgeRung {
    pub rung: String,
    pub backend: String,
    pub matched: usize,
    pub deleted: usize,
}

/// The learn store's counts in a `trajectory` purge report.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PurgeLearnStore {
    pub backend: &'static str,
    pub matched: usize,
    pub deleted: usize,
}

/// `canon purge`'s `data`: the plan (`planDigest`, `effects[]`, `committed`)
/// and what each rung matched and deleted.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PurgeReport {
    pub kind: String,
    pub before: String,
    pub source_scope: &'static str,
    pub rungs: Vec<PurgeRung>,
    /// Rungs the plan could not reach.
    pub unavailable: Vec<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub learn_store: Option<PurgeLearnStore>,
    #[serde(flatten)]
    pub plan: Preview,
}

impl Report for PurgeReport {
    /// 1.1: the plan is `atip::plan::Preview`, whose `effects` may carry an `append` of the audit line and whose `planDigest` is `sha256:` and 64 hex digits.
    const SCHEMA_VERSION: &'static str = "1.1";
}

/// The `data` of a purge that stopped partway: tiers are not transactional, so
/// some records may be gone.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PartialPurgeReport {
    pub kind: String,
    pub before: String,
    pub status: &'static str,
    pub matched: usize,
    pub deleted: usize,
    pub error: String,
    pub atomic: bool,
}

impl Report for PartialPurgeReport {}

fn purge_report(outcome: &PurgeOutcome, learn_counts: Option<(usize, usize)>, plan: Preview) -> PurgeReport {
    PurgeReport {
        kind: outcome.kind.as_str().to_string(),
        before: outcome.before.to_rfc3339(),
        source_scope: "routed_and_aging_destinations",
        rungs: outcome.rungs.iter().map(|r| PurgeRung { rung: r.rung.as_str().to_string(), backend: r.backend.as_str().to_string(), matched: r.matched, deleted: r.deleted }).collect(),
        unavailable: outcome.unavailable.clone(),
        learn_store: learn_counts.map(|(matched, deleted)| PurgeLearnStore { backend: "parquet", matched, deleted }),
        plan,
    }
}

fn partial_purge_report(kind: RecordKind, before: DateTime<Utc>, matched: usize, deleted: usize, error: &str) -> PartialPurgeReport {
    PartialPurgeReport { kind: kind.as_str().to_string(), before: before.to_rfc3339(), status: "partial", matched, deleted, error: error.to_string(), atomic: false }
}

/// The plan of a purge: one `delete` per record the removal set holds, in
/// every rung and in the learn store, and one `skip` per rung that could not
/// be reached (a plan that differs there is a different plan).
fn purge_effects(outcome: &PurgeOutcome, keys: &[String], learn_keys: Option<&[String]>) -> Vec<Effect> {
    let mut effects: Vec<Effect> = keys.iter().map(|key| Effect::record("delete", key.as_str())).collect();
    effects.extend(learn_keys.unwrap_or_default().iter().map(|key| Effect::record("delete", format!("learn:parquet:{key}"))));
    effects.extend(outcome.unavailable.iter().map(|rung| Effect::record("skip", rung.as_str()).detail("unreachable")));
    effects
}

/// The record a purge's audit line is made of, as far as it is known when the plan is made: everything but `actor` and `at`.
fn audit_effect(repo: &Path, kind: RecordKind, matched: usize) -> Option<Effect> {
    let path = audit_path(repo)?;
    let label = path.strip_prefix(repo).unwrap_or(&path).display().to_string();
    let line = json!({"operation": "purge", "kind": kind.as_str(), "scope": PURGE_SCOPE, "sensitive": false, "result_counts": {"matched": matched, "deleted": matched}});
    Some(Effect::path("append", label).detail(format!("one access audit line: purge of {}, {matched} matched and deleted", kind.as_str())).with("line", line))
}

/// The scope a purge's audit line names.
const PURGE_SCOPE: &str = "routed_and_aging_destinations";

/// A time after every record: the removal set of a purge before it is the whole kind, which is what the plan read.
fn after_every_record() -> DateTime<Utc> {
    DateTime::parse_from_rfc3339("9999-12-31T23:59:59Z").expect("a fixed time parses").with_timezone(&Utc)
}

/// `canon purge --kind K --before T [--commit --plan-digest D]`. A bare run
/// previews and writes nothing, not even the access-audit line; `--commit`
/// deletes only when the plan recomputed now has the digest given. The plan lists the audit line a commit
/// appends (when it deletes anything) as its last effect, and reads the whole kind in every rung it lives in.
pub fn run_purge(repo: &Path, kind: RecordKind, before: DateTime<Utc>, mode: &Mode, json_output: bool) -> u8 {
    let base = crate::safety::preview_command("purge");
    let deleting = std::cell::Cell::new(false);
    let result = (|| -> Result<(), RetentionError> {
        let (yaml, registry) = loaded_registry(repo, kind, true)?;
        let learn_store = if kind == RecordKind::Trajectory { Some(loaded_trajectory_store(repo, &yaml)?) } else { None };
        let learn_preflight = if let Some(store) = &learn_store { Some(store.purge_before(before, true)?) } else { None };
        let preflight = RetentionService::new(&registry).purge(kind, before, true)?;
        let keys = RetentionService::new(&registry).purge_plan_keys(kind, before)?;
        let learn_keys = if let Some(store) = &learn_store { Some(store.purge_keys_before(before)?) } else { None };
        // What the plan read to pick its removal set: every record of the kind in every rung, with its content digest.
        let every = after_every_record();
        let scanned = RetentionService::new(&registry).purge_plan_keys(kind, every)?;
        let learn_scanned = if let Some(store) = &learn_store { store.purge_keys_before(every)? } else { Vec::new() };
        let root = yaml.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or(repo);
        let reads = Reads::new().digest("canon.yaml", path_digest(root, &yaml)).set("records", &scanned).set("learn-records", &learn_scanned);
        let will_match: usize = preflight.rungs.iter().map(|rung| rung.matched).sum::<usize>() + learn_preflight.unwrap_or(0);
        let mut effects = purge_effects(&preflight, &keys, learn_keys.as_deref());
        if will_match > 0 {
            effects.extend(audit_effect(repo, kind, will_match));
        }
        let plan = Plan::new("purge", effects, reads).map_err(|e| RetentionError::Refusal(Box::new(e)))?;
        let commit = plan.gate(mode).map_err(|e| RetentionError::Refusal(Box::new(e)))?;
        // Deleting touches every rung the kind lives in, and a committed run appends an audit line: each must take the change before the first removal. A plan that removes nothing writes nothing and is not refused.
        if commit && (preflight.rungs.iter().any(|rung| rung.matched > 0) || learn_preflight.unwrap_or(0) > 0) {
            let mut targets = crate::write_mode::resident_targets(&yaml, kind);
            targets.extend(audit_path(repo));
            if learn_store.is_some() {
                let learn = LearnConfig::from_manifest(&fs::read_to_string(&yaml).unwrap_or_default()).ok();
                targets.extend(learn.map(|config| yaml.parent().filter(|parent| !parent.as_os_str().is_empty()).unwrap_or(repo).join(&config.root).join("trajectories")));
            }
            for target in &targets {
                crate::write_mode::check_writable(target).map_err(|cause| RetentionError::Refusal(Box::new(crate::write_mode::unwritable(target, &cause))))?;
            }
        }
        deleting.set(commit);
        // `purge-before-delete` (a debug build's test seam, `incomplete::injected_stop`): the purge fails before it deleted any record.
        if let Some(stop) = crate::incomplete::injected_stop("purge-before-delete").filter(|_| commit) {
            return Err(RetentionError::Store(canon_store::tier::StoreError::Io(stop)));
        }
        // Deleting writes the hot db; the plan above only read it, so a preview or a refused digest leaves no db behind.
        let outcome = if commit { RetentionService::new(&loaded_registry(repo, kind, false)?.1).purge(kind, before, false)? } else { preflight };
        if let Some(stop) = crate::incomplete::injected_stop("purge-after-delete").filter(|_| commit) {
            // The deletion in the tiers is done, and the step after it stops (the test seam stands for the learn store, which a trajectory purge empties next).
            return Err(RetentionError::PartialPurge { matched: outcome.rungs.iter().map(|r| r.matched).sum(), deleted: outcome.rungs.iter().map(|r| r.deleted).sum(), reason: stop.to_string() });
        }
        let registry_matched: usize = outcome.rungs.iter().map(|r| r.matched).sum();
        let registry_deleted: usize = outcome.rungs.iter().map(|r| r.deleted).sum();
        let learn_counts = if let Some(store) = learn_store {
            if !commit {
                Some((learn_preflight.expect("trajectory preflight count"), 0))
            } else {
                let count = store.purge_before(before, false).map_err(|err| RetentionError::PartialPurge {
                    matched: registry_matched + learn_preflight.unwrap_or(0),
                    deleted: registry_deleted,
                    reason: format!("learn parquet store failed after tier deletion: {err}"),
                })?;
                Some((learn_preflight.unwrap_or(count), count))
            }
        } else {
            None
        };
        let matched = registry_matched + learn_counts.map_or(0, |(matched, _)| matched);
        let deleted = registry_deleted + learn_counts.map_or(0, |(_, deleted)| deleted);
        if commit && matched == 0 && deleted == 0 {
            // Nothing was in the removal set, so nothing was read or deleted: no audit line, an unchanged run.
            crate::json_out::mark_unchanged();
        } else if commit {
            append_access_audit(repo, "purge", Some(kind), PURGE_SCOPE, false, matched, deleted)?;
        }
        let report = purge_report(&outcome, learn_counts, if commit { plan.committed() } else { plan.preview() });
        if commit {
            if !json_output {
                println!("canon purge --kind {}: {matched} matched, {deleted} deleted", kind.as_str());
            }
            crate::next::finish(repo, "purge", &crate::next::Scope::Repo, false, json_output, &report);
        } else {
            plan.finish_preview("purge", "the plan only lists what would be removed; deleting takes its planDigest", json_output, &report);
        }
        Ok(())
    })();
    match result {
        Ok(()) => 0,
        Err(RetentionError::Refusal(error)) => error.emit("canon purge", json_output),
        Err(err) => {
            let (matched, deleted) = match &err {
                RetentionError::Store(canon_store::tier::StoreError::PurgePartial { matched, deleted, .. }) => (*matched, *deleted),
                RetentionError::PartialPurge { matched, deleted, .. } => (*matched, *deleted),
                _ => (0, 0),
            };
            // A refusal that read and deleted nothing leaves no audit line (O7).
            if matched > 0 || deleted > 0 {
                let _ = append_access_audit(repo, "purge", Some(kind), PURGE_SCOPE, false, matched, deleted);
            }
            // The tiers are opened and the plan is read and gated before anything is deleted (exit 2). A commit that stops after it deleted something is
            // `RETENTION_PURGE_INCOMPLETE` (exit 1, what was deleted stays deleted); one that stopped before the first record was deleted (the error carries the
            // counts) is answered by its cause like the refusals before it.
            let incomplete = deleting.get() && deleted > 0;
            let code = if incomplete { "RETENTION_PURGE_INCOMPLETE" } else { tiers_code(&err) };
            if incomplete {
                crate::incomplete::read_status("read what the purge deleted before it stopped (`data` names the counts): what was deleted stays deleted");
            }
            let hint = if incomplete {
                format!("run `{base}` again to see what remains, then commit its plan with `--commit --plan-digest <planDigest>`")
            } else {
                format!("{}, then run `{base}` again and commit its plan with `--commit --plan-digest <planDigest>`", tiers_hint(code))
            };
            let error = CliError::refused(code, err.to_string()).hint(hint);
            let error = if incomplete { error.reason("WRITE_FAILED") } else { error };
            let partial = match &err {
                RetentionError::Store(canon_store::tier::StoreError::PurgePartial { matched, deleted, .. }) | RetentionError::PartialPurge { matched, deleted, .. } => {
                    Some(partial_purge_report(kind, before, *matched, *deleted, &err.to_string()))
                }
                _ => None,
            };
            match partial {
                // One document: the partial-purge manifest is `data` beside the shared `error`.
                Some(manifest) if json_output => {
                    crate::human_err!("{}", error.human_text());
                    error.emit_with_data("canon purge", true, &manifest)
                }
                _ => error.emit("canon purge", json_output),
            }
        }
    }
}

/// One exported record: identity and digest, never the content.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExportRecord {
    pub kind: &'static str,
    pub at: String,
    pub id: Option<String>,
    pub digest: String,
}

impl ExportRecord {
    fn of(kind: RecordKind, raw: &canon_model::evidence::RawRecord) -> Self {
        let digest = canon_store::partition::content_digest(&raw.0);
        let at = raw_record_at(raw).to_rfc3339();
        let id = canon_store::partition::resolve_partition(kind, &raw.0).ok().map(|p| p.natural_key);
        Self { kind: kind.as_str(), at, id, digest }
    }
}

/// The `canon.retention.export` manifest: what `export` writes to stdout or
/// `--output`, and what `--json` envelopes (`ExportReport`). The page state is `meta.page` of the envelope; the file and
/// the stdout document hold it beside the manifest ([`ExportDocument`]).
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExportManifest {
    pub schema: &'static str,
    pub version: u32,
    pub kind: &'static str,
    pub source_scope: &'static str,
    pub before: Option<String>,
    pub after: Option<String>,
    /// The records this answer covers: report content, not paging state (`meta.page` is the paging).
    pub record_count: usize,
    pub content_digests: Vec<String>,
    pub records: Vec<ExportRecord>,
    pub sensitive: bool,
    /// Present only with `--include-sensitive`: the full records, verbatim.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub sensitive_records: Option<Vec<crate::json_out::LedgerRecord>>,
}

impl crate::json_out::Report for ExportManifest {
    /// 2.0: the three answers of `export` are three outputs, so this one is the manifest alone, with no page state of
    /// its own, and its records are one shape that `--fields` cuts.
    const SCHEMA_VERSION: &'static str = "2.0";
}

/// What `export` writes to a file or to stdout in text mode: the manifest and the page it holds.
#[derive(Debug, serde::Serialize)]
struct ExportDocument<'a> {
    #[serde(flatten)]
    manifest: &'a ExportManifest,
    meta: &'a crate::paging::PageMeta,
}

/// What `--json --output` returns: the file holds the manifest, this names it.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExportWritten {
    pub output: String,
    pub kind: &'static str,
    pub record_count: usize,
}

impl crate::json_out::Report for ExportWritten {}

/// What `--json --dry-run` returns: what the run would write, nothing written.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ExportPreview {
    /// Always true.
    pub dry_run: bool,
    /// The file `--output` names, when given.
    pub output: Option<String>,
    pub kind: &'static str,
    pub record_count: usize,
}

impl crate::json_out::Report for ExportPreview {}

/// One answer of `export`, as a batch item carries it: the manifest, the name of the file it was written to, or the preview.
#[derive(Debug, serde::Serialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum ExportAnswer {
    Manifest(ExportManifest),
    Written(ExportWritten),
    Preview(ExportPreview),
}

impl crate::json_out::Report for ExportAnswer {}

/// `canon export`: the window must not be empty.
pub(crate) fn check_window(args: &crate::cli::top::ExportArgs) -> Result<(), CliError> {
    if let (Some(after), Some(before)) = (args.after, args.before) {
        if after > before {
            return Err(CliError::refused("USAGE", format!("`--after {}` is later than `--before {}`; the window would be empty", after.to_rfc3339(), before.to_rfc3339()))
                .field("--after")
                .hint("give `--after` an earlier time than `--before` (`--after` is inclusive, `--before` exclusive)"));
        }
    }
    Ok(())
}

pub fn run_export(args: crate::cli::top::ExportArgs) -> u8 {
    let crate::cli::top::ExportArgs { kind, before, after, output, repo, json: json_output, include_sensitive, page, fields, dry_run, .. } = args;
    let repo = repo.as_path();
    let out = output.as_deref();
    let page = &page.page();
    let fields = fields.as_deref();
    let dry_run = dry_run.dry_run;
    let mut next_cursor_line = None;
    let mut writing = false;
    let result = (|| -> Result<(), ExportFailure> {
        // `--fields` is read against the output this call selects (spec 6.10): the preview; with `--json` and `--output`, the answer that
        // names the file; otherwise the manifest, which is what a text-mode run prints or writes to the file.
        let answer = if dry_run { "ExportPreviewReport" } else if json_output && out.is_some() { "ExportWrittenReport" } else { "ExportReport" };
        let chosen = crate::fields::choose_output("export", Some(answer), fields, None).map_err(ExportFailure::Refused)?;
        ensure_sensitive(repo, include_sensitive).map_err(|error| match error {
            RetentionError::Message(message) => ExportFailure::Refused(CliError::refused("RETENTION_EXPORT_DENIED", message).hint("ask the human to set `query.allow_sensitive: true` in .canon/policy.yaml, or export without --include-sensitive")),
            other => ExportFailure::from(other),
        })?;
        let (_, registry) = loaded_registry(repo, kind, true)?;
        let mut query = TierQuery::kind(kind);
        if let Some(after) = after { query = query.since(after); }
        let rows = registry.query(&query)?.records;
        let rows: Vec<_> = rows.into_iter().filter(|raw| before.is_none_or(|cutoff| raw_record_at(raw) < cutoff)).collect();
        let (rows, meta) = crate::paging::paginate(
            "export",
            rows,
            |raw| {
                let record = ExportRecord::of(kind, raw);
                format!("{}|{}|{}", record.at, record.id.as_deref().unwrap_or_default(), record.digest)
            },
            page,
        )
        .map_err(ExportFailure::Refused)?;
        let records: Vec<ExportRecord> = rows.iter().map(|raw| ExportRecord::of(kind, raw)).collect();
        let content_digests = records.iter().map(|m| m.digest.clone()).collect();
        let manifest = ExportManifest {
            schema: "canon.retention.export",
            version: 1,
            kind: kind.as_str(),
            source_scope: "routed_and_aging_destinations",
            before: before.map(|t| t.to_rfc3339()),
            after: after.map(|t| t.to_rfc3339()),
            record_count: rows.len(),
            content_digests,
            records,
            sensitive: include_sensitive,
            sensitive_records: include_sensitive.then(|| rows.iter().map(|r| crate::json_out::LedgerRecord(r.0.clone())).collect()),
        };
        let document = ExportDocument { manifest: &manifest, meta: &meta };
        // The bytes of the manifest (the file, or stdout in text mode) are cut only when the manifest is the output this call selects.
        let encoded = match &chosen {
            Some(chosen) if answer == "ExportReport" => {
                let mut value = serde_json::to_value(&document)?;
                chosen.apply(&mut value);
                serde_json::to_vec_pretty(&value)?
            }
            _ => serde_json::to_vec_pretty(&document)?,
        };
        // The destination is checked before anything is written, for a preview as for a run: an output that cannot be created refuses with nothing written and no audit line.
        if let Some(path) = out.filter(|path| !fs::read(path).is_ok_and(|held| held == encoded)) {
            crate::write_mode::check_creatable(path, false).and_then(|()| crate::write_mode::check_writable(path)).map_err(|e| ExportFailure::Refused(crate::write_mode::unwritable(path, &e)))?;
        }
        // A real run appends one audit line; a repo whose audit file cannot take it is refused the same way. A preview appends none.
        if let Some(path) = audit_path(repo).filter(|_| !dry_run) {
            crate::write_mode::check_writable(&path).map_err(|e| ExportFailure::Refused(crate::write_mode::unwritable(&path, &e)))?;
        }
        if dry_run {
            if json_output {
                let data = ExportPreview { dry_run: true, output: out.map(|p| p.display().to_string()), kind: kind.as_str(), record_count: rows.len() };
                if let Some(chosen) = &chosen {
                    chosen.activate();
                }
                crate::next::finish_with_meta(repo, "export", &crate::next::Scope::Repo, false, true, &data, meta.to_json());
            } else {
                crate::human_err!("canon export: would write {} record(s){}", rows.len(), out.map(|p| format!(" to {}", p.display())).unwrap_or_default());
            }
            next_cursor_line = meta.human_line();
            return Ok(());
        }
        writing = true;
        if out.is_some_and(|path| fs::read(path).is_ok_and(|held| held == encoded)) {
            // The file already holds exactly this manifest: left alone (the access audit
            // line is still appended, O7), and the run is unchanged.
            crate::json_out::mark_unchanged();
        } else if let Some(path) = out {
            #[cfg(unix)]
            {
                use std::os::unix::fs::OpenOptionsExt;
                let mut options = OpenOptions::new();
                options.write(true).create(true).truncate(true).mode(0o600);
                options.open(path)?.write_all(&encoded)?;
            }
            #[cfg(not(unix))]
            fs::write(path, &encoded)?;
        } else if !json_output {
            println!("{}", String::from_utf8_lossy(&encoded));
        }
        append_access_audit(repo, "export", Some(kind), "routed_and_aging_destinations", include_sensitive, rows.len(), 0)?;
        if json_output {
            // The manifest is the result; with `--output` it went to the file, and the result names it.
            if let Some(chosen) = &chosen {
                chosen.activate();
            }
            match out {
                Some(path) => crate::next::finish_with_meta(repo, "export", &crate::next::Scope::Repo, false, true, &ExportWritten { output: path.display().to_string(), kind: kind.as_str(), record_count: rows.len() }, meta.to_json()),
                None => crate::next::finish_with_meta(repo, "export", &crate::next::Scope::Repo, false, true, &manifest, meta.to_json()),
            }
        }
        if out.is_some() && !json_output { crate::human_err!("canon export: wrote {} record(s)", rows.len()); }
        // The manifest itself is JSON on stdout, so the human cursor line
        // goes to stderr and never breaks a pipe into a JSON parser.
        next_cursor_line = meta.human_line();
        Ok(())
    })();
    match result {
        Ok(()) => {
            if let Some(line) = next_cursor_line {
                crate::human_err!("{line}");
            }
            0
        }
        // A refused export read and wrote nothing: no audit line (O7).
        Err(ExportFailure::Refused(err)) => {
            err.emit("canon export", json_output)
        }
        Err(ExportFailure::Retention(err)) => {
            // The ledger is read and the destination is checked before anything is written (exit 2); once the manifest is being written the same errors are a failure during the work (exit 1).
            let code = if writing { "RETENTION_EXPORT_FAILED" } else { tiers_code(&err) };
            let hint = if writing { "check that the destination is writable and the ledger is readable, then run `canon export` again".to_string() } else { format!("{}, then run `canon export` again", tiers_hint(code)) };
            CliError::refused(code, err.to_string()).hint(hint).emit("canon export", json_output)
        }
    }
}

/// Why an export stopped: an error that is already a [`CliError`] (a bad
/// `--cursor`, an output that cannot be written), or a retention failure that
/// is refused before the output is touched and fails after.
enum ExportFailure {
    Refused(CliError),
    Retention(RetentionError),
}

impl<E: Into<RetentionError>> From<E> for ExportFailure {
    fn from(err: E) -> Self {
        ExportFailure::Retention(err.into())
    }
}

/// The access event for one `canon query`, scoped like every other
/// event ([`audit_path`]). Written only for a query that reached the store: a
/// refused one leaves no line (O7).
pub fn audit_query(repo: &Path, kind: RecordKind, since: Option<DateTime<Utc>>, sensitive: bool, matched: usize) -> Result<(), RetentionError> {
    append_access_audit(repo, "query", Some(kind), if since.is_some() { "since" } else { "all" }, sensitive, matched, 0)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// atip.codestored.13: the code of a refusal to open the tiers is told by what is wrong, and only a backend that does not answer or an error a
    /// repeat can pass is retryable (`UNAVAILABLE`).
    #[test]
    fn atip_codestored_13_the_tiers_code_is_told_by_what_is_wrong() {
        use canon_store::policy::{Backend, Rung};
        use canon_store::tier::StoreError;
        let store = |error: StoreError| RetentionError::Store(error);
        for (error, code) in [
            (store(StoreError::BackendUnattached { backend: Backend::Postgres, reason: "no live DSN".into() }), "RETENTION_TIERS_REQUIRED"),
            (store(StoreError::tier_unavailable(Rung::Hot, Some(Backend::Postgres), "no live DSN")), "RETENTION_TIERS_REQUIRED"),
            (store(StoreError::tier_unavailable(Rung::Cold, None, "no tiers.cold entry")), "RETENTION_TIERS_REQUIRED"),
            (store(StoreError::Parquet("Invalid Parquet file. Corrupt footer".into())), "RETENTION_TIERS_CORRUPT"),
            (RetentionError::Learn(canon_learn::LearnError::Parquet("Invalid Parquet file. Corrupt footer".into())), "RETENTION_TIERS_CORRUPT"),
            (store(StoreError::Sql("connection refused".into())), "RETENTION_TIERS_UNAVAILABLE"),
            (store(StoreError::ObjectStore("timed out".into())), "RETENTION_TIERS_UNAVAILABLE"),
            (RetentionError::Io(std::io::Error::from_raw_os_error(5)), "RETENTION_TIERS_UNAVAILABLE"),
            (store(StoreError::Io(std::io::Error::from_raw_os_error(13))), "RETENTION_TIERS_DENIED"),
            (RetentionError::Learn(canon_learn::LearnError::Io(std::io::Error::from_raw_os_error(13))), "RETENTION_TIERS_DENIED"),
            (RetentionError::Io(std::io::Error::from_raw_os_error(2)), "RETENTION_TIERS_INVALID"),
            (RetentionError::Io(std::io::Error::from_raw_os_error(21)), "RETENTION_TIERS_INVALID"),
            (store(StoreError::Policy("bad tier".into())), "RETENTION_TIERS_INVALID"),
        ] {
            assert_eq!(tiers_code(&error), code, "{error}");
            assert_eq!(crate::cli_error::code_info(code).map(|info| info.retryable), Some(code.ends_with("_UNAVAILABLE")), "{code}");
        }
    }

    #[test]
    fn sensitive_export_requires_explicit_policy() {
        let dir = tempfile::tempdir().unwrap();
        assert!(validate_sensitive(dir.path(), true).is_err());
    }

    #[test]
    fn export_metadata_never_contains_record_body() {
        let raw = canon_model::evidence::RawRecord(json!({
            "kind": "event",
            "schema": 1,
            "at": "2026-01-01T00:00:00Z",
            "event_id": "evt-1",
            "detail": {"text": "secret"},
        }));
        let metadata = serde_json::to_value(ExportRecord::of(RecordKind::Event, &raw)).unwrap();
        assert!(metadata.get("detail").is_none());
        assert!(metadata.get("digest").is_some());
    }

    #[test]
    fn trajectory_purge_manifest_reports_parquet_learn_store_counts() {
        let outcome = PurgeOutcome {
            kind: RecordKind::Trajectory,
            before: Utc::now(),
            dry_run: false,
            rungs: vec![],
            unavailable: vec![],
        };
        let value = serde_json::to_value(purge_report(&outcome, Some((3, 2)), Plan::new("purge", Vec::new(), Reads::new()).unwrap().preview())).unwrap();
        assert_eq!(value["learnStore"]["backend"], "parquet");
        assert_eq!(value["learnStore"]["matched"], 3);
        assert_eq!(value["learnStore"]["deleted"], 2);
        assert_eq!(value["committed"], false);
    }

    #[test]
    fn partial_purge_manifest_preserves_non_atomic_counts() {
        let value = serde_json::to_value(partial_purge_report(RecordKind::Event, Utc::now(), 8, 3, "cold failed")).unwrap();
        assert_eq!(value["status"], "partial");
        assert_eq!(value["matched"], 8);
        assert_eq!(value["deleted"], 3);
        assert_eq!(value["atomic"], false);
    }

    #[test]
    fn trajectory_purge_rejects_missing_learn_root() {
        let dir = tempfile::tempdir().unwrap();
        let yaml = dir.path().join("canon.yaml");
        fs::write(&yaml, "learn:\n  root: missing-learn\n").unwrap();
        let err = loaded_trajectory_store(dir.path(), &yaml).unwrap_err();
        assert!(err.to_string().contains("requires configured learn root"));
    }

    #[cfg(unix)]
    #[test]
    fn audit_file_is_created_restrictively() {
        let dir = tempfile::tempdir().unwrap();
        fs::write(dir.path().join("canon.yaml"), "").unwrap();
        append_access_audit(dir.path(), "query", Some(RecordKind::Event), "all", false, 0, 0).unwrap();
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(audit_path(dir.path()).unwrap()).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn no_canon_yaml_means_no_audit_write() {
        let dir = tempfile::tempdir().unwrap();
        append_access_audit(dir.path(), "query", Some(RecordKind::Event), "all", false, 0, 0).unwrap();
        assert!(fs::read_dir(dir.path()).unwrap().next().is_none(), "a directory without canon.yaml must be left untouched");
    }
}
