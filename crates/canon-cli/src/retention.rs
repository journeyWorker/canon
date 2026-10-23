//! Privacy lifecycle commands: purge, export, and append-only access audit.

use std::fs::{self, OpenOptions};
use std::io::Write;
use std::path::{Path, PathBuf};

use canon_learn::{LearnConfig, ParquetTrajectoryStore, TrajectoryStore};
use canon_model::envelope::RecordKind;
use canon_store::registry::{PurgeOutcome, RetentionService, TierRegistry};
use canon_store::tier::{raw_record_at, TierQuery};
use chrono::{DateTime, Utc};
use serde_json::{json, Value};

use crate::cli_error::CliError;
use crate::context::resolve_canon_yaml;
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
    #[error("plan digest {expected} no longer matches the data (now {actual})")]
    PlanChanged { expected: String, actual: String },
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("json: {0}")]
    Json(#[from] serde_json::Error),
    #[error("yaml: {0}")]
    Yaml(#[from] serde_yaml::Error),
    #[error("tier: {0}")]
    Tiers(#[from] tiers::TierCliError),
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
fn audit_path(repo: &Path, canon_yaml: Option<&Path>) -> Option<PathBuf> {
    let yaml = resolve_canon_yaml(repo, canon_yaml);
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
#[allow(clippy::too_many_arguments)]
pub fn append_access_audit(
    repo: &Path,
    canon_yaml: Option<&Path>,
    operation: &str,
    kind: Option<RecordKind>,
    scope: &str,
    sensitive: bool,
    matched: usize,
    deleted: usize,
) -> Result<(), RetentionError> {
    let Some(path) = audit_path(repo, canon_yaml) else {
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

fn loaded_registry(repo: &Path, kind: RecordKind) -> Result<(PathBuf, TierRegistry), RetentionError> {
    let yaml = resolve_canon_yaml(repo, None);
    let loaded = tiers::build_lenient_tiers_for_kind(&yaml, kind)?;
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

fn print_human(value: &Value, summary: &str) {
    println!("{summary}");
    println!("{}", serde_json::to_string_pretty(value).expect("manifest is serializable"));
}

fn purge_value(outcome: &PurgeOutcome, learn_counts: Option<(usize, usize)>) -> Value {
    let mut value = json!({
        "schema": "canon.retention.purge",
        "version": 1,
        "kind": outcome.kind.as_str(),
        "before": outcome.before.to_rfc3339(),
        "dry_run": outcome.dry_run,
        "source_scope": "routed_and_aging_destinations",
        "rungs": outcome.rungs.iter().map(|r| json!({"rung": r.rung.as_str(), "backend": r.backend.as_str(), "matched": r.matched, "deleted": r.deleted})).collect::<Vec<_>>(),
        "unavailable": outcome.unavailable,
    });
    if let Some((matched, deleted)) = learn_counts {
        value.as_object_mut().expect("purge manifest object").insert("learn_store".to_string(), json!({"backend": "parquet", "matched": matched, "deleted": deleted}));
    }
    value
}

fn partial_purge_value(kind: RecordKind, before: DateTime<Utc>, matched: usize, deleted: usize, error: &str) -> Value {
    json!({
        "schema": "canon.retention.purge",
        "version": 1,
        "kind": kind.as_str(),
        "before": before.to_rfc3339(),
        "dry_run": false,
        "status": "partial",
        "matched": matched,
        "deleted": deleted,
        "error": error,
        "atomic": false,
    })
}

/// What `canon purge` was asked to do. A bare `purge` is refused before
/// this is built: deleting needs a reviewed plan.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum PurgeMode {
    /// `--plan`: report what would be removed and its `planDigest`; delete nothing.
    Plan,
    /// `--confirm <digest>`: delete only if the plan recomputed now still has this digest.
    Confirm(String),
}

/// The digest of a purge plan: the kind, the cutoff, what the preflight
/// counted at every rung and in the learn store, and the identity of every
/// record it would remove (the sorted keys, streamed into the hash). A
/// record added, removed or swapped for another between `--plan` and
/// `--confirm` changes the digest even when the count does not.
fn plan_digest(kind: RecordKind, before: DateTime<Utc>, outcome: &PurgeOutcome, learn_matched: Option<usize>, keys: &[String], learn_keys: Option<&[String]>) -> String {
    use sha2::{Digest, Sha256};
    let header = json!({
        "kind": kind.as_str(),
        "before": before.to_rfc3339(),
        "rungs": outcome.rungs.iter().map(|r| json!([r.rung.as_str(), r.backend.as_str(), r.matched])).collect::<Vec<_>>(),
        "unavailable": outcome.unavailable,
        "learn_matched": learn_matched,
    });
    let mut hasher = Sha256::new();
    hasher.update(header.to_string().as_bytes());
    for (label, set) in [("registry", Some(keys)), ("learn", learn_keys)] {
        hasher.update(format!("\n{label}:{}\n", set.map_or(0, <[String]>::len)).as_bytes());
        for key in set.unwrap_or_default() {
            hasher.update(key.as_bytes());
            hasher.update(b"\n");
        }
    }
    hasher.finalize().iter().map(|b| format!("{b:02x}")).collect()
}

pub fn run_purge(repo: &Path, kind: RecordKind, before: DateTime<Utc>, mode: Option<PurgeMode>, json_output: bool) -> u8 {
    let Some(mode) = mode else {
        return CliError::refused("PURGE_PLAN_REQUIRED", "`canon purge` deletes only a reviewed plan, and none was given")
            .hint(format!("run `canon purge --kind {} --before {} --plan`, review it, then rerun with `--confirm <planDigest>`", crate::query::kind_cli_name(kind), before.to_rfc3339()))
            .emit("canon purge", json_output) as u8;
    };
    let result = (|| -> Result<(), RetentionError> {
        let (yaml, registry) = loaded_registry(repo, kind)?;
        let learn_store = if kind == RecordKind::Trajectory { Some(loaded_trajectory_store(repo, &yaml)?) } else { None };
        let learn_preflight = if let Some(store) = &learn_store { Some(store.purge_before(before, true)?) } else { None };
        let plan = RetentionService::new(&registry).purge(kind, before, true)?;
        let keys = RetentionService::new(&registry).purge_plan_keys(kind, before)?;
        let learn_keys = if let Some(store) = &learn_store { Some(store.purge_keys_before(before)?) } else { None };
        let digest = plan_digest(kind, before, &plan, learn_preflight, &keys, learn_keys.as_deref());
        let confirmed = match &mode {
            PurgeMode::Plan => false,
            PurgeMode::Confirm(expected) if *expected == digest => true,
            PurgeMode::Confirm(expected) => return Err(RetentionError::PlanChanged { expected: expected.clone(), actual: digest }),
        };
        let outcome = if confirmed { RetentionService::new(&registry).purge(kind, before, false)? } else { plan };
        let registry_matched: usize = outcome.rungs.iter().map(|r| r.matched).sum();
        let registry_deleted: usize = outcome.rungs.iter().map(|r| r.deleted).sum();
        let learn_counts = if let Some(store) = learn_store {
            if !confirmed {
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
        let learn_matched = learn_counts.map(|(matched, _)| matched).unwrap_or(0);
        let learn_deleted = learn_counts.map(|(_, deleted)| deleted).unwrap_or(0);
        let mut value = purge_value(&outcome, learn_counts);
        value.as_object_mut().expect("purge manifest object").insert("planDigest".to_string(), json!(digest));
        let matched = registry_matched + learn_matched;
        let deleted = registry_deleted + learn_deleted;
        append_access_audit(repo, None, "purge", Some(kind), "routed_and_aging_destinations", false, matched, deleted)?;
        let summary = if confirmed {
            format!("canon purge --kind {}: {} matched, {} deleted", crate::query::kind_cli_name(kind), matched, deleted)
        } else {
            format!(
                "canon purge --kind {} --plan: {matched} record(s) would be removed (planDigest {digest}); delete them with `canon purge --kind {} --before {} --confirm {digest}`",
                crate::query::kind_cli_name(kind),
                crate::query::kind_cli_name(kind),
                before.to_rfc3339()
            )
        };
        if !json_output {
            print_human(&value, &summary);
        } else if confirmed {
            crate::next::finish(repo, "purge", &crate::next::Scope::Repo, false, true, &crate::json_out::untyped(value));
        } else {
            // A plan deletes nothing: the one step is the human's review, then the confirm.
            let step = crate::status::NextStep {
                command: format!("review the plan, then run `canon purge --kind {} --before {} --confirm {digest}`", crate::query::kind_cli_name(kind), before.to_rfc3339()),
                why: "the plan only lists what would be removed; deleting takes a reviewed plan digest".into(),
                actor: crate::status::StepActor::Human,
                role: None,
            };
            crate::json_out::data_with_next("purge", &crate::json_out::untyped(value), &crate::next::After::fixed(vec![step]));
        }
        Ok(())
    })();
    match result {
        Ok(()) => 0,
        Err(RetentionError::PlanChanged { expected, actual }) => {
            let _ = append_access_audit(repo, None, "purge", Some(kind), "routed_and_aging_destinations", false, 0, 0);
            CliError::refused("PLAN_CHANGED", format!("the plan digest {expected} no longer matches the data (now {actual}); nothing was deleted"))
                .hint(format!("run `canon purge --kind {} --before {} --plan` again and review it", crate::query::kind_cli_name(kind), before.to_rfc3339()))
                .emit("canon purge", json_output) as u8
        }
        Err(err) => {
            let (matched, deleted) = match &err {
                RetentionError::Store(canon_store::tier::StoreError::PurgePartial { matched, deleted, .. }) => (*matched, *deleted),
                RetentionError::PartialPurge { matched, deleted, .. } => (*matched, *deleted),
                _ => (0, 0),
            };
            let _ = append_access_audit(repo, None, "purge", Some(kind), "routed_and_aging_destinations", false, matched, deleted);
            let error = CliError::refused("RETENTION_PURGE_FAILED", err.to_string()).hint("run `canon purge --plan` again to see what remains, then confirm with `--confirm <planDigest>`");
            let partial = match &err {
                RetentionError::Store(canon_store::tier::StoreError::PurgePartial { matched, deleted, .. }) | RetentionError::PartialPurge { matched, deleted, .. } => {
                    Some(partial_purge_value(kind, before, *matched, *deleted, &err.to_string()))
                }
                _ => None,
            };
            match partial {
                // One document: the partial-purge manifest is `data` beside the shared `error`.
                Some(manifest) if json_output => {
                    crate::human_err!("{}", error.human("canon purge"));
                    error.emit_with_data("canon purge", true, &crate::json_out::untyped(manifest)) as u8
                }
                _ => error.emit("canon purge", json_output) as u8,
            }
        }
    }
}

fn redacted_metadata(kind: RecordKind, raw: &canon_model::evidence::RawRecord) -> Value {
    let digest = canon_store::partition::content_digest12(&raw.0);
    let at = raw_record_at(raw).to_rfc3339();
    let id = canon_store::partition::resolve_partition(kind, &raw.0).ok().map(|p| p.natural_key);
    json!({"kind": kind.as_str(), "at": at, "id": id, "digest": digest})
}

/// The fields of one exported record, which `--fields` may name.
const RECORD_FIELDS: [&str; 4] = ["kind", "at", "id", "digest"];

pub fn run_export(
    repo: &Path,
    kind: RecordKind,
    before: Option<DateTime<Utc>>,
    after: Option<DateTime<Utc>>,
    out: Option<&Path>,
    json_output: bool,
    include_sensitive: bool,
    page: &crate::paging::Page,
    fields: Option<&[String]>,
) -> u8 {
    let mut next_cursor_line = None;
    let result = (|| -> Result<(), ExportFailure> {
        if let Some(requested) = fields {
            crate::paging::check_known("canon export", requested, &RECORD_FIELDS).map_err(ExportFailure::Cursor)?;
        }
        ensure_sensitive(repo, include_sensitive)?;
        let (_, registry) = loaded_registry(repo, kind)?;
        let mut query = TierQuery::kind(kind);
        if let Some(after) = after { query = query.since(after); }
        let rows = registry.query(&query)?.records;
        let rows: Vec<_> = rows.into_iter().filter(|raw| before.is_none_or(|cutoff| raw_record_at(raw) < cutoff)).collect();
        let (rows, meta) = crate::paging::paginate(
            &format!("export:{}", kind.as_str()),
            rows,
            |raw| {
                let metadata = redacted_metadata(kind, raw);
                format!("{}|{}|{}", metadata["at"].as_str().unwrap_or_default(), metadata["id"].as_str().unwrap_or_default(), metadata["digest"].as_str().unwrap_or_default())
            },
            page,
        )
        .map_err(ExportFailure::Cursor)?;
        let metadata: Vec<Value> = rows.iter().map(|raw| redacted_metadata(kind, raw)).collect();
        let mut manifest = json!({
            "schema": "canon.retention.export",
            "version": 1,
            "kind": kind.as_str(),
            "source_scope": "routed_and_aging_destinations",
            "before": before.map(|t| t.to_rfc3339()),
            "after": after.map(|t| t.to_rfc3339()),
            "record_count": rows.len(),
            "content_digests": metadata.iter().filter_map(|m| m.get("digest").cloned()).collect::<Vec<_>>(),
            "records": match fields {
                Some(requested) => metadata.iter().map(|m| crate::paging::project(m, requested)).collect::<Vec<_>>(),
                None => metadata,
            },
            "sensitive": include_sensitive,
            "meta": meta.to_json(),
        });
        if include_sensitive {
            manifest.as_object_mut().expect("manifest object").insert("sensitive_records".to_string(), Value::Array(rows.iter().map(|r| r.0.clone()).collect()));
        }
        let encoded = serde_json::to_vec_pretty(&manifest)?;
        if let Some(path) = out {
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
        append_access_audit(repo, None, "export", Some(kind), "routed_and_aging_destinations", include_sensitive, rows.len(), 0)?;
        if json_output {
            // The manifest is the result; with `--out` it went to the file, and the result names it.
            let (manifest, page_meta) = crate::json_out::split_meta(manifest);
            let data = match out {
                Some(path) => json!({ "out": path.display().to_string(), "kind": kind.as_str(), "recordCount": rows.len() }),
                None => manifest,
            };
            crate::next::finish_with_meta(repo, "export", &crate::next::Scope::Repo, false, true, &crate::json_out::untyped(data), page_meta);
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
        Err(ExportFailure::Cursor(err)) => {
            let _ = append_access_audit(repo, None, "export", Some(kind), "routed_and_aging_destinations", include_sensitive, 0, 0);
            err.emit("canon export", json_output) as u8
        }
        Err(ExportFailure::Retention(err)) => {
            let _ = append_access_audit(repo, None, "export", Some(kind), "routed_and_aging_destinations", include_sensitive, 0, 0);
            CliError::refused("RETENTION_EXPORT_FAILED", err.to_string()).hint("check that the destination is writable and the ledger is readable, then run `canon export` again").emit("canon export", json_output) as u8
        }
    }
}

/// Why an export stopped: a bad `--cursor`, or the retention failures
/// `RETENTION_EXPORT_FAILED` has always covered.
enum ExportFailure {
    Cursor(CliError),
    Retention(RetentionError),
}

impl<E: Into<RetentionError>> From<E> for ExportFailure {
    fn from(err: E) -> Self {
        ExportFailure::Retention(err.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

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
        let metadata = redacted_metadata(RecordKind::Event, &raw);
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
        let value = purge_value(&outcome, Some((3, 2)));
        assert_eq!(value["learn_store"]["backend"], "parquet");
        assert_eq!(value["learn_store"]["matched"], 3);
        assert_eq!(value["learn_store"]["deleted"], 2);
    }

    #[test]
    fn partial_purge_manifest_preserves_non_atomic_counts() {
        let value = partial_purge_value(RecordKind::Event, Utc::now(), 8, 3, "cold failed");
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
        append_access_audit(dir.path(), None, "query", Some(RecordKind::Event), "all", false, 0, 0).unwrap();
        use std::os::unix::fs::PermissionsExt;
        let mode = fs::metadata(audit_path(dir.path(), None).unwrap()).unwrap().permissions().mode() & 0o777;
        assert_eq!(mode, 0o600);
    }

    #[test]
    fn no_canon_yaml_means_no_audit_write() {
        let dir = tempfile::tempdir().unwrap();
        append_access_audit(dir.path(), None, "query", Some(RecordKind::Event), "all", false, 0, 0).unwrap();
        assert!(fs::read_dir(dir.path()).unwrap().next().is_none(), "a directory without canon.yaml must be left untouched");
    }
}

/// The access event for one `canon query`, scoped like every other
/// event ([`audit_path`]): `canon_yaml` is the query's own
/// `--canon-yaml` override, when it has one.
pub fn audit_query(repo: &Path, canon_yaml: Option<&Path>, kind: RecordKind, since: Option<DateTime<Utc>>, sensitive: bool, matched: usize) -> Result<(), RetentionError> {
    append_access_audit(repo, canon_yaml, "query", Some(kind), if since.is_some() { "since" } else { "all" }, sensitive, matched, 0)
}
