//! `canon migrate`'s ledger renaming (0.30 `ledger-naming`, design migration steps 1–9).
//!
//! A 0.29 repo names its records by the 0.29 rule: an `area=` level, a 12-hex
//! digest, `kind=evidence_record/`. This module computes, applies and (if a step
//! fails, or on request of a test) reverts the move to the one 0.30 rule of
//! `canon_model::layout`:
//!
//! 1. **Refuses** while a moved path has uncommitted changes (the caller checks
//!    [`Plan::moved_paths`]), while a destination exists or two files would take
//!    one name with different bytes, and while `_staging` holds the trace of an
//!    interrupted promote (a committed record whose `staging_id` is the 12-hex
//!    digest of a staged body). Other staged records move like the ledger.
//! 2. **Records the old world first.** Every evidence file's 0.29 key (task id,
//!    then scenario id, then run id, `unscoped` otherwise; no project, no tag),
//!    its 12-hex file digest, the 12-hex digest of its typed body and its old
//!    body's sha256 are an effect of the plan, so the plan digest covers them,
//!    and [`OldEvidence`] is what the pre-0.23 review pin conversion judges legacy
//!    pins against, through the frozen 0.29 functions of [`v029`], which restore
//!    `kind: "evidence_record"` before hashing.
//! 3. The renaming runs **before every other migrate step**.
//! 4. `canon.yaml`'s routing/aging keys are rewritten textually ([`rewrite_config`]).
//! 5. Ledger, archive, staging and overlay files move (an evidence body's kind
//!    value changes and its digest is recomputed).
//! 6. sqlite rows, and Postgres rows and R2 objects of a repo that routes kinds
//!    there, are re-keyed through the same plan.
//! 7. `.gitignore` negations naming `kind=evidence_record` are rewritten.
//! 8. Every rewritten file and `hot.db` are backed up under
//!    `.canon/migrate-backup/`, with a journal ([`JOURNAL`]) of the moves and of
//!    the old keys; [`revert`] restores the backup and reverts the moves, and
//!    the old Postgres/R2 keys come from that journal.
//! 9. A second run finds nothing to move: [`plan`] is empty.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};
use std::process::Command;

use canon_model::layout;
use canon_model::{EvidenceRecord, RecordKind};
use canon_store::policy::{BackendConfig, TierPolicy, LEGACY_EVIDENCE_KEY};
use canon_store::r2_tier::{ObjectRekey, R2Tier};
use canon_store::sqlite_tier::SqliteTier;
use canon_store::tier::{RowRekey, StoredRow};
use serde::{Deserialize, Serialize};
use serde_json::Value;
use sha2::{Digest, Sha256};

use crate::safety::Effect;

/// The file in the backup directory that lets [`revert`] undo the renaming.
pub const JOURNAL: &str = "names-journal.json";

/// The manifest of the whole-tree snapshot `canon migrate --revert` restores (see [`snapshot`]).
pub const MANIFEST: &str = "tree-manifest.json";

/// The directory of the backup that holds the snapshot's copies, by repo-relative path.
pub const SNAPSHOT_DIR: &str = ".snapshot";

/// The frozen 0.29 name functions (design: "a frozen copy"). They exist only to
/// judge pre-0.23 review pins, which hold the 0.29 key and 12-hex digest of the
/// evidence files they saw; the live functions of `canon-store` compute the 0.30
/// rule and the live kind value is `evidence`.
pub mod v029 {
    use super::{Digest, EvidenceRecord, Sha256, Value};

    /// The kind value 0.29 wrote into evidence bodies and directory names.
    pub const EVIDENCE_KIND: &str = "evidence_record";

    /// First 12 lowercase hex digits of SHA-256 over the canonical JSON.
    pub fn digest12(json: &Value) -> String {
        let bytes = serde_json::to_vec(json).expect("serde_json::Value always serializes");
        Sha256::digest(bytes).iter().take(6).map(|byte| format!("{byte:02x}")).collect()
    }

    /// The natural key 0.29 filed an evidence record under: its task id, else its
    /// scenario id, else its run id, else `unscoped`.
    pub fn evidence_key(json: &Value) -> String {
        ["task_id", "scenario_id", "run_id"].into_iter().find_map(|field| json.get(field).and_then(Value::as_str)).unwrap_or("unscoped").to_string()
    }

    /// `record` as 0.29 hashed it: the typed body, its `kind` back to the 0.29 value.
    pub fn typed_body(record: &EvidenceRecord) -> Value {
        let mut body = serde_json::to_value(record).unwrap_or_default();
        if let Some(kind) = body.get_mut("kind") {
            *kind = Value::String(EVIDENCE_KIND.to_string());
        }
        body
    }

    /// The 12-hex digest 0.29 gave the typed record (the key the file-digest map is built on).
    pub fn typed_digest12(record: &EvidenceRecord) -> String {
        digest12(&typed_body(record))
    }
}

/// What the plan recorded of one evidence file before it was renamed.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct OldEvidence {
    /// The 0.29 natural key.
    pub key: String,
    /// The 12-hex digest of the old body as written (the old file name's suffix).
    pub file_digest: String,
    /// The 12-hex digest of the typed record with the 0.29 kind value: what a
    /// review pin's digest is looked up by.
    pub typed_digest: String,
    /// sha256 of the old file's bytes.
    pub body_sha256: String,
}

/// One record file to move.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileMove {
    /// Repo-relative, `/`-separated.
    pub from: String,
    pub to: String,
    /// The body changes (an evidence record's kind value), so the file is written anew.
    pub rewritten: bool,
    /// Git tracked the old path.
    pub tracked: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub evidence: Option<OldEvidence>,
    /// The new bytes of a rewritten file.
    #[serde(skip)]
    pub new_bytes: Vec<u8>,
}

/// The re-keyed rows of one live database, with the table they live in.
#[derive(Debug, Clone, Default)]
pub struct RowPlan {
    pub rekeys: Vec<RowRekey>,
    /// The old bodies of those rows, for [`revert`].
    pub old_bodies: Vec<Value>,
}

/// What a backend of the repo could not be read for, said once.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Skipped {
    pub backend: &'static str,
    pub why: String,
}

/// Everything the renaming would do.
#[derive(Debug, Default)]
pub struct Plan {
    pub moves: Vec<FileMove>,
    /// `.gitignore` lines (1-based) naming `kind=evidence_record`.
    pub gitignore_lines: Vec<usize>,
    /// The `canon.yaml` keys spelled the 0.29 way.
    pub config_keys: Vec<String>,
    /// The sqlite file and its re-keyed rows.
    pub sqlite: Option<(PathBuf, RowPlan)>,
    pub postgres: Option<(String, String, RowPlan)>,
    pub s3: Option<Vec<ObjectRekey>>,
    /// The old objects' keys and bodies, for the journal.
    pub s3_old: Vec<canon_store::r2_tier::StoredObject>,
    pub skipped: Vec<Skipped>,
}

impl Plan {
    /// Nothing to move, rewrite or re-key (step 9).
    pub fn is_empty(&self) -> bool {
        self.moves.is_empty()
            && self.gitignore_lines.is_empty()
            && self.config_keys.is_empty()
            && self.sqlite.is_none()
            && self.postgres.is_none()
            && self.s3.is_none()
    }

    /// Whether any record file or row moves (the part that must run before every other step).
    pub fn renames(&self) -> bool {
        !self.moves.is_empty() || self.sqlite.is_some() || self.postgres.is_some() || self.s3.is_some()
    }

    /// The directories the moves leave or fill, repo-relative: checked for uncommitted changes.
    pub fn moved_paths(&self) -> Vec<String> {
        let mut dirs: BTreeSet<String> = BTreeSet::new();
        for file in &self.moves {
            for path in [&file.from, &file.to] {
                if let Some(index) = path.find("/kind=") {
                    let rest = &path[index + 1..];
                    let kind_dir = rest.split('/').next().unwrap_or(rest);
                    dirs.insert(format!("{}/{kind_dir}", &path[..index]));
                }
            }
        }
        dirs.into_iter().collect()
    }

    /// The repo-relative files [`apply`] rewrites or replaces, which the backup copies first.
    pub fn backed_up(&self, repo: &Path) -> Vec<String> {
        let mut paths: Vec<String> = self.moves.iter().filter(|m| m.rewritten).map(|m| m.from.clone()).collect();
        if !self.gitignore_lines.is_empty() {
            paths.push(".gitignore".to_string());
        }
        if let Some((db, _)) = &self.sqlite {
            if let Ok(relative) = db.strip_prefix(repo) {
                paths.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
        paths
    }

    /// The old world of every evidence file: what the pre-0.23 pin conversion judges legacy pins against.
    pub fn old_evidence(&self) -> OldWorld {
        OldWorld { files: self.moves.iter().filter_map(|m| m.evidence.clone()).map(|e| (e.typed_digest.clone(), e)).collect() }
    }

    /// The plan as effects, in the order [`apply`] acts (the record of the old world included).
    pub fn effects(&self) -> Vec<Effect> {
        let mut out = Vec::new();
        for file in &self.moves {
            let mut detail = format!("to {}", file.to);
            if file.rewritten {
                detail.push_str("; kind value becomes `evidence`, digest recomputed");
            }
            if let Some(old) = &file.evidence {
                detail.push_str(&format!("; was key {}, digest {}, body sha256:{}", old.key, old.file_digest, old.body_sha256));
            }
            out.push(Effect::path("move", file.from.clone()).detail(detail));
        }
        if !self.gitignore_lines.is_empty() {
            out.push(Effect::path("rewrite", ".gitignore").detail("`kind=evidence_record` becomes `kind=evidence`"));
        }
        if let Some((db, rows)) = &self.sqlite {
            out.extend(row_effects("sqlite", &db.file_name().map_or_else(String::new, |n| n.to_string_lossy().into_owned()), rows));
        }
        if let Some((_, schema, rows)) = &self.postgres {
            out.extend(row_effects("postgres", schema, rows));
        }
        if let Some(objects) = &self.s3 {
            for object in objects {
                out.push(Effect::record("rekey", format!("s3:{}", object.from)).detail(format!("to {}", object.to)));
            }
        }
        out
    }

    /// What the preview says about what it did not read.
    pub fn notes(&self) -> Vec<String> {
        self.skipped.iter().map(|s| format!("{} was not read ({}): its keys are not part of this plan; run `canon migrate` again where it is reachable", s.backend, s.why)).collect()
    }
}

fn row_effects(backend: &str, table: &str, rows: &RowPlan) -> Vec<Effect> {
    rows.rekeys
        .iter()
        .map(|r| Effect::record("rekey", format!("{backend}:{table}:{}:{}:{}", r.from.0, r.from.1, r.from.2)).detail(format!("to {}:{}:{}", r.to.0, r.to.1, r.to.2)))
        .collect()
}

/// The recorded old world of the evidence files, by the typed digest 0.29 gave their records.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct OldWorld {
    files: BTreeMap<String, OldEvidence>,
}

impl OldWorld {
    /// The 12-hex file digest of the evidence file whose typed record had `typed_digest`.
    pub fn file_digest(&self, typed_digest: &str) -> Option<&str> {
        self.files.get(typed_digest).map(|e| e.file_digest.as_str())
    }

    /// Every recorded file digest, by typed digest.
    pub fn file_digests(&self) -> BTreeMap<String, String> {
        self.files.iter().map(|(typed, e)| (typed.clone(), e.file_digest.clone())).collect()
    }
}

// ---------------------------------------------------------------- planning

/// The tier roots a 0.29 ledger can sit in, with the ledger first.
fn roots(ledger: &Path) -> Vec<PathBuf> {
    let mut roots = vec![ledger.to_path_buf()];
    roots.extend(canon_store::archive::archive_root_of(ledger));
    roots.push(ledger.join("_staging"));
    roots
}

fn rel(repo: &Path, path: &Path) -> String {
    path.strip_prefix(repo).unwrap_or(path).components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/")
}

fn files_under(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = entries.flatten().collect();
    entries.sort_by_key(std::fs::DirEntry::file_name);
    for entry in entries {
        match entry.file_type() {
            Ok(t) if t.is_dir() && entry.file_name() != "_staging" => files_under(&entry.path(), out),
            Ok(t) if t.is_file() => out.push(entry.path()),
            _ => {}
        }
    }
}

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// The core kind a `kind=<dir>` directory holds, spelled the 0.29 or the 0.30 way.
fn core_kind(dir: &str) -> Option<RecordKind> {
    if dir == "evidence_record" {
        return Some(RecordKind::Evidence);
    }
    RecordKind::ALL.into_iter().find(|k| k.as_str() == dir)
}

/// The plan for `repo` (module doc). `text` is its `canon.yaml`.
pub fn plan(repo: &Path, text: &str) -> Result<Plan, String> {
    let policy = TierPolicy::from_yaml_at(text, repo).map_err(|e| format!("canon.yaml cannot be read: {e}"))?;
    let ledger = canon_gate::GateCtx::from_repo(repo).map_err(|e| e.to_string())?.ledger_root;
    let mut plan = Plan { config_keys: policy.legacy_kind_keys.clone(), ..Plan::default() };
    plan_files(repo, &ledger, &mut plan)?;
    plan.gitignore_lines = std::fs::read_to_string(repo.join(".gitignore"))
        .map(|t| t.lines().enumerate().filter(|(_, l)| !l.trim_start().starts_with('#') && l.contains(canon_store::partition::LEGACY_KIND_DIR)).map(|(i, _)| i + 1).collect())
        .unwrap_or_default();
    plan_databases(repo, &policy, &mut plan)?;
    Ok(plan)
}

/// A record file's new name and, for evidence, its rewritten body.
struct Renamed {
    to: PathBuf,
    bytes: Option<Vec<u8>>,
    evidence: Option<OldEvidence>,
}

fn rename_one(root: &Path, relative: &Path, kind_dir: &str, old_bytes: &[u8]) -> Result<Renamed, String> {
    let shown = relative.display();
    let body: Value = serde_json::from_slice(old_bytes).map_err(|e| format!("`{shown}` is not JSON ({e}); fix or remove it, then run `canon migrate` again"))?;
    if let Some(kind) = core_kind(kind_dir) {
        let old_kind = if kind == RecordKind::Evidence { v029::EVIDENCE_KIND } else { kind.as_str() };
        if body.get("kind").and_then(Value::as_str) != Some(old_kind) {
            return Err(format!("`{shown}` sits under `kind={kind_dir}/` but its body's `kind` is {:?}; fix or remove it, then run `canon migrate` again", body.get("kind")));
        }
        let mut new_body = body.clone();
        let mut evidence = None;
        let mut bytes = None;
        if kind == RecordKind::Evidence {
            new_body["kind"] = Value::String(RecordKind::Evidence.as_str().to_string());
            let typed: EvidenceRecord = serde_json::from_value(new_body.clone()).map_err(|e| format!("`{shown}` is not an evidence record canon can read ({e}); fix or remove it, then run `canon migrate` again"))?;
            evidence = Some(OldEvidence { key: v029::evidence_key(&body), file_digest: v029::digest12(&body), typed_digest: v029::typed_digest12(&typed), body_sha256: sha256_hex(old_bytes) });
            bytes = Some(serde_json::to_vec_pretty(&new_body).map_err(|e| e.to_string())?);
        }
        let name = canon_store::partition::resolve_name(kind, &new_body)
            .map_err(|e| format!("`{shown}` has no name under the 0.30 rule: {}; fix or remove it, then run `canon migrate` again", e.detail))?;
        return Ok(Renamed { to: root.join(name.relative_path(kind.as_str(), "json")), bytes, evidence });
    }
    // A plugin overlay kind: `kind=<namespace>.<kind>/<join-key values joined by __>__<digest12>.json`.
    let stem = relative.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
    let Some((key, _)) = stem.rsplit_once("__") else { return Err(format!("`{shown}` has no `__<digest12>` suffix")) };
    let parts: Vec<String> = key.split("__").map(str::to_string).collect();
    canon_store::partition::overlay_key_matches_body(&parts, &body).map_err(|e| format!("`{shown}`: {}; fix or remove it, then run `canon migrate` again", e.detail))?;
    let components = canon_store::partition::overlay_components(&parts).map_err(|e| e.detail)?;
    let name = layout::name(&components, &canon_store::partition::content_digest(&body));
    Ok(Renamed { to: root.join(name.relative_path(kind_dir, "json")), bytes: None, evidence: None })
}

fn plan_files(repo: &Path, ledger: &Path, plan: &mut Plan) -> Result<(), String> {
    let mut moves: Vec<FileMove> = Vec::new();
    let mut destinations: BTreeMap<PathBuf, Vec<u8>> = BTreeMap::new();
    let mut staged: Vec<(PathBuf, Vec<u8>)> = Vec::new();
    let mut committed_staging_ids: BTreeSet<String> = BTreeSet::new();
    for root in roots(ledger) {
        let Ok(entries) = std::fs::read_dir(&root) else { continue };
        let mut dirs: Vec<_> = entries.flatten().filter(|e| e.file_type().is_ok_and(|t| t.is_dir()) && e.file_name().to_string_lossy().starts_with("kind=")).collect();
        dirs.sort_by_key(std::fs::DirEntry::file_name);
        let is_staging = root.file_name().is_some_and(|n| n == "_staging");
        for dir in dirs {
            let kind_dir = dir.file_name().to_string_lossy().trim_start_matches("kind=").to_string();
            let mut files = Vec::new();
            files_under(&dir.path(), &mut files);
            for file in files {
                let relative = file.strip_prefix(&root).unwrap_or(&file).to_path_buf();
                if file.extension().is_none_or(|e| e != "json") || !canon_store::partition::is_old_layout_path(&relative) {
                    continue;
                }
                let old_bytes = std::fs::read(&file).map_err(|e| format!("cannot read `{}`: {e}", rel(repo, &file)))?;
                let renamed = rename_one(&root, &relative, &kind_dir, &old_bytes)?;
                let new_bytes = renamed.bytes.clone().unwrap_or_else(|| old_bytes.clone());
                match destinations.get(&renamed.to) {
                    Some(held) if *held != new_bytes => {
                        return Err(format!("`{}` and another file would both be named `{}` with different content (an 8-hex digest collision); nothing was moved", rel(repo, &file), rel(repo, &renamed.to)));
                    }
                    _ => {}
                }
                if renamed.to.exists() && std::fs::read(&renamed.to).ok().as_deref() != Some(new_bytes.as_slice()) {
                    return Err(format!("`{}` would move to `{}`, which already exists with other content; nothing was moved", rel(repo, &file), rel(repo, &renamed.to)));
                }
                destinations.insert(renamed.to.clone(), new_bytes.clone());
                if let Ok(body) = serde_json::from_slice::<Value>(&old_bytes) {
                    if is_staging {
                        staged.push((file.clone(), old_bytes.clone()));
                    } else if let Some(id) = body.get("staging_id").and_then(Value::as_str) {
                        committed_staging_ids.insert(id.to_string());
                    }
                }
                moves.push(FileMove {
                    from: rel(repo, &file),
                    to: rel(repo, &renamed.to),
                    rewritten: renamed.bytes.is_some(),
                    tracked: false,
                    evidence: renamed.evidence,
                    new_bytes: renamed.bytes.unwrap_or_default(),
                });
            }
        }
    }
    // An interrupted promote: a committed record whose `staging_id` is the digest of a staged body.
    for (path, bytes) in staged {
        if let Ok(body) = serde_json::from_slice::<Value>(&bytes) {
            let id = v029::digest12(&body);
            if committed_staging_ids.contains(&id) {
                return Err(format!(
                    "`{}` is staged and a committed record names it as its `staging_id` ({id}): a promote was interrupted. Finish it with the 0.29 `canon gate promote` (or remove the staged copy), then run `canon migrate` again",
                    rel(repo, &path)
                ));
            }
        }
    }
    let tracked = git_tracked(repo, &moves.iter().map(|m| m.from.clone()).collect::<Vec<_>>());
    for file in &mut moves {
        file.tracked = tracked.contains(&file.from);
    }
    plan.moves = moves;
    Ok(())
}

/// The repo-relative paths among `paths` that git tracks (none outside a work tree).
fn git_tracked(repo: &Path, dirs_or_files: &[String]) -> BTreeSet<String> {
    if dirs_or_files.is_empty() {
        return BTreeSet::new();
    }
    let kind_dirs: BTreeSet<String> = dirs_or_files.iter().filter_map(|p| p.find("/kind=").map(|i| p[..i + 1 + p[i + 1..].find('/').unwrap_or(p.len() - i - 1)].to_string())).collect();
    let Ok(out) = Command::new("git").args(["ls-files", "-z", "--"]).args(&kind_dirs).current_dir(repo).output() else { return BTreeSet::new() };
    if !out.status.success() {
        return BTreeSet::new();
    }
    String::from_utf8_lossy(&out.stdout).split('\0').filter(|p| !p.is_empty()).map(str::to_string).collect()
}

/// The new key and body of one hot-tier row, or why it has none.
fn rekey_row(row: &StoredRow) -> Result<Option<RowRekey>, String> {
    let (kind, mut body) = (row.kind.clone(), row.body.clone());
    let new_kind = if kind == v029::EVIDENCE_KIND { RecordKind::Evidence.as_str().to_string() } else { kind.clone() };
    if kind == v029::EVIDENCE_KIND {
        body["kind"] = Value::String(new_kind.clone());
    }
    let Some(record_kind) = RecordKind::ALL.into_iter().find(|k| k.as_str() == new_kind) else { return Ok(None) };
    let name = canon_store::partition::resolve_name(record_kind, &body).map_err(|e| format!("row {}:{}:{} has no name under the 0.30 rule: {}", row.kind, row.id, row.digest, e.detail))?;
    let to = (new_kind, name.key, name.digest);
    let from = (row.kind.clone(), row.id.clone(), row.digest.clone());
    Ok((from != to || body != row.body).then_some(RowRekey { from, to, body }))
}

fn plan_rows(rows: Vec<StoredRow>) -> Result<Option<RowPlan>, String> {
    let mut plan = RowPlan::default();
    let mut taken: BTreeMap<(String, String, String), Value> = rows.iter().map(|r| ((r.kind.clone(), r.id.clone(), r.digest.clone()), r.body.clone())).collect();
    for row in &rows {
        let Some(rekey) = rekey_row(row)? else { continue };
        if rekey.from != rekey.to {
            if let Some(held) = taken.get(&rekey.to) {
                if *held != rekey.body {
                    return Err(format!("row {}:{}:{} would take the key of another row with different content", row.kind, row.id, row.digest));
                }
            }
            taken.remove(&rekey.from);
            taken.insert(rekey.to.clone(), rekey.body.clone());
        }
        plan.old_bodies.push(row.body.clone());
        plan.rekeys.push(rekey);
    }
    Ok((!plan.rekeys.is_empty()).then_some(plan))
}

/// Steps 6: the rows and objects of the repo's databases and bucket.
fn plan_databases(repo: &Path, policy: &TierPolicy, plan: &mut Plan) -> Result<(), String> {
    for cfg in policy.tiers.values() {
        match cfg {
            BackendConfig::Git(_) => {}
            BackendConfig::Sqlite(cfg) => {
                if !cfg.path.is_file() {
                    continue;
                }
                let tier = SqliteTier::connect_read_only(&cfg.path).map_err(|e| format!("cannot open {}: {e}", rel(repo, &cfg.path)))?;
                let rows = tier.all_rows().map_err(|e| e.to_string())?;
                if let Some(rows) = plan_rows(rows)? {
                    plan.sqlite = Some((cfg.path.clone(), rows));
                }
            }
            BackendConfig::Postgres(cfg) => match crate::tiers::attach_postgres(cfg) {
                Ok((Some(tier), _)) => {
                    if let Some(rows) = plan_rows(tier.all_rows().map_err(|e| e.to_string())?)? {
                        plan.postgres = Some((cfg.dsn_env.clone(), cfg.schema.clone(), rows));
                    }
                }
                Ok((None, why)) => plan.skipped.push(Skipped { backend: "postgres", why: why.unwrap_or_default() }),
                Err(e) => return Err(e.to_string()),
            },
            BackendConfig::S3(cfg) => match crate::tiers::attach_s3(cfg) {
                Ok((Some(tier), _)) => {
                    let mut moves = Vec::new();
                    for object in tier.all_objects().map_err(|e| e.to_string())? {
                        let kind_dir = object.kind.clone();
                        let new_kind = if kind_dir == v029::EVIDENCE_KIND { RecordKind::Evidence.as_str() } else { kind_dir.as_str() };
                        let mut body = object.body.clone();
                        if kind_dir == v029::EVIDENCE_KIND {
                            body["kind"] = Value::String(new_kind.to_string());
                        }
                        let Some(record_kind) = core_kind(&kind_dir) else { continue };
                        let name = canon_store::partition::resolve_name(record_kind, &body).map_err(|e| format!("object {} has no name under the 0.30 rule: {}", object.key, e.detail))?;
                        let to = name.relative_path(new_kind, "parquet").to_string_lossy().replace('\\', "/");
                        if to != object.key || body != object.body {
                            plan.s3_old.push(object.clone());
                            moves.push(ObjectRekey { from: object.key.clone(), to, kind: new_kind.to_string(), natural_key: name.key, digest: name.digest, at: object.at.clone(), body });
                        }
                    }
                    if !moves.is_empty() {
                        plan.s3 = Some(moves);
                    }
                }
                Ok((None, why)) => plan.skipped.push(Skipped { backend: "s3", why: why.unwrap_or_default() }),
                Err(e) => return Err(e.to_string()),
            },
        }
    }
    Ok(())
}

// -------------------------------------------------------- canon.yaml / .gitignore

/// `canon.yaml` text with its `routing`/`aging` keys spelled the 0.30 way. Only the key names change:
/// comments and layout stay, and the result is refused unless it parses to the old document with those
/// keys renamed and nothing else different.
pub fn rewrite_config(text: &str) -> Result<String, String> {
    if !text.contains(LEGACY_EVIDENCE_KEY) {
        return Ok(text.to_string());
    }
    let mut section = String::new();
    let mut out = String::with_capacity(text.len());
    for line in text.split_inclusive('\n') {
        let trimmed = line.trim_start();
        if !line.starts_with([' ', '\t', '#', '\n', '\r']) && !trimmed.is_empty() {
            section = trimmed.split(':').next().unwrap_or_default().trim().to_string();
        }
        if matches!(section.as_str(), "routing" | "aging") && !trimmed.starts_with('#') {
            out.push_str(&replace_key(line));
        } else {
            out.push_str(line);
        }
    }
    let before: serde_yaml::Value = serde_yaml::from_str(text).map_err(|e| format!("canon.yaml is not valid YAML: {e}"))?;
    let after: serde_yaml::Value = serde_yaml::from_str(&out).map_err(|e| format!("the renamed canon.yaml would not parse: {e}"))?;
    let mut expected = before;
    for name in ["routing", "aging"] {
        let Some(map) = expected.get_mut(name).and_then(serde_yaml::Value::as_mapping_mut) else { continue };
        if let Some(value) = map.remove(LEGACY_EVIDENCE_KEY) {
            if map.contains_key("evidence") {
                return Err(format!("canon.yaml `{name}:` names both `evidence` and `{LEGACY_EVIDENCE_KEY}`; keep `evidence` and delete the other line, then run `canon migrate` again"));
            }
            map.insert("evidence".into(), value);
        }
    }
    if after != expected {
        return Err("canon.yaml spells the `evidence_record` key in a form canon cannot rewrite safely (a flow mapping or an anchor); rename `routing.evidence_record` and `aging.evidence_record` to `evidence` by hand, then run `canon migrate` again".to_string());
    }
    Ok(out)
}

/// `line` with a mapping key `evidence_record` renamed.
fn replace_key(line: &str) -> String {
    let mut out = String::with_capacity(line.len());
    let mut rest = line;
    while let Some(at) = rest.find(LEGACY_EVIDENCE_KEY) {
        let (head, tail) = rest.split_at(at);
        let after = &tail[LEGACY_EVIDENCE_KEY.len()..];
        let before_ok = head.chars().next_back().is_none_or(|c| !(c.is_alphanumeric() || c == '_' || c == '-'));
        let key = before_ok && after.trim_start().starts_with(':');
        out.push_str(head);
        out.push_str(if key { "evidence" } else { LEGACY_EVIDENCE_KEY });
        rest = after;
    }
    out.push_str(rest);
    out
}

/// `.gitignore` text with the negations that name the renamed kind directory rewritten.
pub fn rewrite_gitignore(text: &str) -> String {
    text.split_inclusive('\n')
        .map(|line| if !line.trim_start().starts_with('#') && line.contains(canon_store::partition::LEGACY_KIND_DIR) { line.replace(canon_store::partition::LEGACY_KIND_DIR, "kind=evidence") } else { line.to_string() })
        .collect()
}

// ------------------------------------------------------------------ applying

/// The journal [`revert`] reads.
#[derive(Debug, Serialize, Deserialize)]
struct Journal {
    moves: Vec<FileMove>,
    sqlite: Option<JournalRows>,
    postgres: Option<(String, String, JournalRows)>,
    /// `(old key, old body)` of every re-keyed R2 object, with the new key and its columns.
    s3: Vec<JournalObject>,
    /// The old `.gitignore` line numbers rewritten.
    gitignore: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct JournalRows {
    path: String,
    rows: Vec<JournalRow>,
}

#[derive(Debug, Serialize, Deserialize)]
struct JournalRow {
    from: (String, String, String),
    to: (String, String, String),
    old_body: Value,
    new_body: Value,
}

#[derive(Debug, Serialize, Deserialize)]
struct JournalObject {
    from: String,
    to: String,
    old_body: Value,
    at: String,
    old_kind: String,
    old_natural_key: String,
    old_digest: String,
}

fn journal_rows(path: String, rows: &RowPlan) -> JournalRows {
    JournalRows {
        path,
        rows: rows.rekeys.iter().zip(&rows.old_bodies).map(|(r, old)| JournalRow { from: r.from.clone(), to: r.to.clone(), old_body: old.clone(), new_body: r.body.clone() }).collect(),
    }
}

fn git(repo: &Path, args: &[&str], stdin: Option<&str>) -> Result<(), String> {
    use std::io::Write;
    let mut child = Command::new("git").args(args).current_dir(repo).stdin(if stdin.is_some() { std::process::Stdio::piped() } else { std::process::Stdio::null() }).stdout(std::process::Stdio::null()).stderr(std::process::Stdio::piped()).spawn().map_err(|e| format!("could not run git: {e}"))?;
    if let (Some(text), Some(mut pipe)) = (stdin, child.stdin.take()) {
        pipe.write_all(text.as_bytes()).map_err(|e| format!("could not feed git: {e}"))?;
    }
    let out = child.wait_with_output().map_err(|e| format!("could not run git: {e}"))?;
    if out.status.success() { Ok(()) } else { Err(format!("`git {}` failed: {}", args.join(" "), String::from_utf8_lossy(&out.stderr).trim())) }
}

/// Stage the renames of tracked files the way `git mv` does: the old path leaves the index, the new one joins it.
fn stage_moves(repo: &Path, pairs: &[(&str, &str)]) -> Result<(), String> {
    if pairs.is_empty() {
        return Ok(());
    }
    let old: String = pairs.iter().map(|(from, _)| format!("{from}\0")).collect();
    let new: String = pairs.iter().map(|(_, to)| format!("{to}\0")).collect();
    git(repo, &["rm", "--cached", "-q", "--ignore-unmatch", "--pathspec-from-file=-", "--pathspec-file-nul"], Some(&old))?;
    git(repo, &["add", "-f", "--pathspec-from-file=-", "--pathspec-file-nul"], Some(&new))
}

/// Where the journal and the copies of what [`apply`] rewrites go.
pub fn backup_dir(repo: &Path, existing: Option<PathBuf>) -> Result<PathBuf, String> {
    if let Some(dir) = existing {
        return Ok(dir);
    }
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let base = repo.join(canon_model::paths::MIGRATE_BACKUP_DIR);
    let mut dir = base.join(&stamp);
    let mut n = 1;
    while dir.exists() {
        dir = base.join(format!("{stamp}-{n}"));
        n += 1;
    }
    crate::legacy::ensure_backup_ignored(&base).map_err(|e| format!("could not create {}: {e}", base.display()))?;
    std::fs::create_dir_all(&dir).map_err(|e| format!("could not create {}: {e}", dir.display()))?;
    Ok(dir)
}

/// Folds the sqlite write-ahead log into its file, so the backup that copies the file holds every row.
pub fn prepare_backup(plan: &Plan) -> Result<(), String> {
    if let Some((db, _)) = &plan.sqlite {
        SqliteTier::connect(db).and_then(|tier| tier.checkpoint()).map_err(|e| format!("could not checkpoint {}: {e}", db.display()))?;
    }
    Ok(())
}

/// Apply the renaming (module doc, steps 3–8). `backup` already holds the copies of [`Plan::backed_up`]. A
/// step that fails reverts the ones before it, so the repo is as it was (the error says so).
pub fn apply(repo: &Path, plan: &Plan, backup: &Path, git_tree: bool) -> Result<(), String> {
    let journal = Journal {
        moves: plan.moves.clone(),
        sqlite: plan.sqlite.as_ref().map(|(db, rows)| journal_rows(rel(repo, db), rows)),
        postgres: plan.postgres.as_ref().map(|(dsn, schema, rows)| (dsn.clone(), schema.clone(), journal_rows(schema.clone(), rows))),
        s3: plan
            .s3
            .iter()
            .flatten()
            .zip(&plan.s3_old)
            .map(|(m, old)| JournalObject { from: old.key.clone(), to: m.to.clone(), old_body: old.body.clone(), at: m.at.clone(), old_kind: old.kind.clone(), old_natural_key: old.natural_key.clone(), old_digest: old.digest.clone() })
            .collect(),
        gitignore: !plan.gitignore_lines.is_empty(),
    };
    let text = serde_json::to_string_pretty(&journal).map_err(|e| e.to_string())?;
    std::fs::write(backup.join(JOURNAL), text).map_err(|e| format!("could not write the journal: {e}"))?;
    match apply_steps(repo, plan, git_tree) {
        Ok(()) => Ok(()),
        Err(e) => match revert(repo, backup) {
            Ok(()) => Err(format!("{e}; the renaming was reverted and nothing changed")),
            Err(undo) => Err(format!("{e}; reverting it failed too ({undo}): restore from {}", backup.display())),
        },
    }
}

fn apply_steps(repo: &Path, plan: &Plan, git_tree: bool) -> Result<(), String> {
    for file in &plan.moves {
        let (from, to) = (repo.join(&file.from), repo.join(&file.to));
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("could not create {}: {e}", rel(repo, parent)))?;
        }
        if file.rewritten {
            canon_store::write_atomic(&to, &file.new_bytes).map_err(|e| format!("could not write {}: {e}", file.to))?;
            std::fs::remove_file(&from).map_err(|e| format!("could not remove {}: {e}", file.from))?;
        } else {
            std::fs::rename(&from, &to).map_err(|e| format!("could not move {} to {}: {e}", file.from, file.to))?;
        }
    }
    remove_empty_dirs(repo, plan.moves.iter().map(|m| m.from.as_str()));
    if git_tree {
        let pairs: Vec<(&str, &str)> = plan.moves.iter().filter(|m| m.tracked).map(|m| (m.from.as_str(), m.to.as_str())).collect();
        stage_moves(repo, &pairs)?;
    }
    if !plan.gitignore_lines.is_empty() {
        let path = repo.join(".gitignore");
        let text = std::fs::read_to_string(&path).map_err(|e| format!("could not read .gitignore: {e}"))?;
        std::fs::write(&path, rewrite_gitignore(&text)).map_err(|e| format!("could not write .gitignore: {e}"))?;
    }
    if let Some((db, rows)) = &plan.sqlite {
        let tier = SqliteTier::connect(db).map_err(|e| e.to_string())?;
        tier.rekey(&rows.rekeys).map_err(|e| format!("could not re-key {}: {e}", rel(repo, db)))?;
        tier.checkpoint().map_err(|e| e.to_string())?;
    }
    if let Some((dsn_env, schema, rows)) = &plan.postgres {
        let dsn = std::env::var(dsn_env).ok().ok_or_else(|| format!("`{dsn_env}` is unset"))?;
        canon_store::pg_tier::PgTier::connect(&dsn, schema).and_then(|tier| tier.rekey(&rows.rekeys)).map_err(|e| format!("could not re-key postgres rows: {e}"))?;
    }
    if let Some(objects) = &plan.s3 {
        attached_s3(repo)?.rekey_objects(objects).map_err(|e| format!("could not re-key s3 objects: {e}"))?;
    }
    Ok(())
}

/// The repo's R2 tier, attached the way the plan read it.
fn attached_s3(repo: &Path) -> Result<R2Tier, String> {
    let text = std::fs::read_to_string(repo.join("canon.yaml")).map_err(|e| e.to_string())?;
    let policy = TierPolicy::from_yaml_at(&text, repo).map_err(|e| e.to_string())?;
    let cfg = policy.tiers.values().find_map(|c| if let BackendConfig::S3(cfg) = c { Some(cfg) } else { None }).ok_or("canon.yaml has no s3 tier")?;
    R2Tier::attach(cfg).map_err(|e| e.to_string())
}

/// Removes the directories `paths` leave empty, innermost first, up to (not including) a `kind=` directory's parent.
fn remove_empty_dirs<'a>(repo: &Path, paths: impl Iterator<Item = &'a str>) {
    let mut dirs: BTreeSet<PathBuf> = BTreeSet::new();
    for path in paths {
        let mut dir = repo.join(path);
        while dir.pop() {
            if dir.file_name().is_some_and(|n| n.to_string_lossy().starts_with("kind=")) || dir.file_name().is_some_and(|n| n.to_string_lossy().starts_with("area=")) {
                dirs.insert(dir.clone());
            } else {
                break;
            }
        }
    }
    let mut dirs: Vec<PathBuf> = dirs.into_iter().collect();
    dirs.sort_by_key(|d| std::cmp::Reverse(d.components().count()));
    for dir in dirs {
        let _ = std::fs::remove_dir(&dir);
    }
}

/// Postgres rows and R2 objects get their old keys back (their old keys and bodies are in the journal).
fn revert_remote(repo: &Path, journal: &Journal) -> Result<(), String> {
    if let Some((dsn_env, schema, rows)) = &journal.postgres {
        let undo: Vec<RowRekey> = rows.rows.iter().map(|r| RowRekey { from: r.to.clone(), to: r.from.clone(), body: r.old_body.clone() }).collect();
        if let Ok(dsn) = std::env::var(dsn_env) {
            canon_store::pg_tier::PgTier::connect(&dsn, schema).and_then(|tier| tier.rekey(&undo)).map_err(|e| format!("could not restore postgres keys: {e}"))?;
        }
    }
    if !journal.s3.is_empty() {
        let undo: Vec<ObjectRekey> = journal
            .s3
            .iter()
            .map(|o| ObjectRekey { from: o.to.clone(), to: o.from.clone(), kind: o.old_kind.clone(), natural_key: o.old_natural_key.clone(), digest: o.old_digest.clone(), at: o.at.clone(), body: o.old_body.clone() })
            .collect();
        attached_s3(repo)?.rekey_objects(&undo).map_err(|e| format!("could not restore s3 keys: {e}"))?;
    }
    Ok(())
}

/// Undo [`apply`] from the journal in `backup`: the sqlite file, `canon.yaml`, `.gitignore` and every rewritten
/// file are restored from their copies, the moves are reverted, and Postgres rows and R2 objects get their old
/// keys back (their old keys and bodies are in the journal).
pub fn revert(repo: &Path, backup: &Path) -> Result<(), String> {
    let text = std::fs::read_to_string(backup.join(JOURNAL)).map_err(|e| format!("no renaming journal in {}: {e}", backup.display()))?;
    let journal: Journal = serde_json::from_str(&text).map_err(|e| format!("the journal in {} is unreadable: {e}", backup.display()))?;
    revert_remote(repo, &journal)?;
    // Files: reverse each move; a rewritten file comes back from its copy.
    for file in journal.moves.iter().rev() {
        let (from, to) = (repo.join(&file.from), repo.join(&file.to));
        if file.rewritten {
            let _ = std::fs::remove_file(&to);
        } else if to.exists() {
            if let Some(parent) = from.parent() {
                std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
            }
            std::fs::rename(&to, &from).map_err(|e| format!("could not move {} back to {}: {e}", file.to, file.from))?;
        }
    }
    // Every copy in the backup goes back where it was (rewritten evidence files, `.gitignore`, `canon.yaml`, `hot.db`).
    let mut copies = Vec::new();
    files_under(backup, &mut copies);
    for copy in copies {
        let relative = copy.strip_prefix(backup).unwrap_or(&copy);
        if relative == Path::new(JOURNAL) || relative == Path::new(MANIFEST) || relative.starts_with(SNAPSHOT_DIR) {
            continue;
        }
        let target = repo.join(relative);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        if relative.file_name().is_some_and(|n| n.to_string_lossy().starts_with("hot.db")) {
            for sidecar in ["-wal", "-shm"] {
                let mut name = target.clone().into_os_string();
                name.push(sidecar);
                let _ = std::fs::remove_file(PathBuf::from(name));
            }
        }
        std::fs::copy(&copy, &target).map_err(|e| format!("could not restore {}: {e}", relative.display()))?;
    }
    remove_empty_dirs(repo, journal.moves.iter().map(|m| m.to.as_str()));
    if is_git_tree(repo) {
        let tracked: Vec<&FileMove> = journal.moves.iter().filter(|m| m.tracked).collect();
        let pairs: Vec<(&str, &str)> = tracked.iter().map(|m| (m.to.as_str(), m.from.as_str())).collect();
        stage_moves(repo, &pairs)?;
    }
    Ok(())
}

fn is_git_tree(repo: &Path) -> bool {
    Command::new("git").args(["rev-parse", "--is-inside-work-tree"]).current_dir(repo).output().is_ok_and(|out| out.status.success() && String::from_utf8_lossy(&out.stdout).trim() == "true")
}

// ------------------------------------------------------------------- preview

/// A scratch repo holding the ledger and archive under their new names (what the downstream steps of a preview
/// judge): `canon.yaml`, the renamed copies, and nothing else. Removed on drop.
#[derive(Debug)]
pub struct Scratch {
    dir: PathBuf,
    ledger: PathBuf,
}

impl Scratch {
    /// The scratch repo root.
    pub fn path(&self) -> &Path {
        &self.dir
    }

    /// The renamed copy of the ledger root.
    pub fn ledger_root(&self) -> &Path {
        &self.ledger
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

/// Builds the [`Scratch`] of `plan` over `repo`: every file of the ledger and its archive, at the name `plan` gives it.
pub fn scratch(repo: &Path, config: &str, plan: &Plan) -> Result<Scratch, String> {
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map_or(0, |d| d.as_nanos());
    let dir = std::env::temp_dir().join(format!("canon-migrate-preview-{}-{nanos}", std::process::id()));
    std::fs::create_dir_all(&dir).map_err(|e| format!("could not create {}: {e}", dir.display()))?;
    let made = (|| -> Result<PathBuf, String> {
        std::fs::write(dir.join("canon.yaml"), config).map_err(|e| e.to_string())?;
        let real_ledger = canon_gate::GateCtx::from_repo(repo).map_err(|e| e.to_string())?.ledger_root;
        let moved: BTreeMap<String, &FileMove> = plan.moves.iter().map(|m| (m.from.clone(), m)).collect();
        let mut copied = BTreeSet::new();
        for root in roots(&real_ledger).into_iter().take(2) {
            let mut files = Vec::new();
            files_under(&root, &mut files);
            for file in files {
                let from = rel(repo, &file);
                let (to, bytes) = match moved.get(&from) {
                    Some(m) if m.rewritten => (m.to.clone(), m.new_bytes.clone()),
                    Some(m) => (m.to.clone(), std::fs::read(&file).map_err(|e| e.to_string())?),
                    None => (from, std::fs::read(&file).map_err(|e| e.to_string())?),
                };
                let target = dir.join(&to);
                if copied.insert(to) {
                    if let Some(parent) = target.parent() {
                        std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
                    }
                    std::fs::write(&target, bytes).map_err(|e| e.to_string())?;
                }
            }
        }
        Ok(dir.join(rel(repo, &real_ledger)))
    })();
    match made {
        Ok(ledger) => Ok(Scratch { dir, ledger }),
        Err(e) => {
            let _ = std::fs::remove_dir_all(&dir);
            Err(format!("could not build the preview of the renamed ledger: {e}"))
        }
    }
}

// ------------------------------------------------------- the revert of the whole migrate

/// One file of the snapshot: what [`revert_plan`] compares the tree against.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
struct Entry {
    sha256: String,
    /// Unix permission bits (0 where the platform has none).
    mode: u32,
    /// The target of a symlink (then `sha256` is of the target text).
    #[serde(skip_serializing_if = "Option::is_none")]
    link: Option<String>,
}

/// The state of everything `canon migrate` may touch, taken before it acts ([`snapshot`]).
#[derive(Debug, Serialize, Deserialize)]
struct Manifest {
    /// Repo-relative files and directories the snapshot covers.
    scope: Vec<String>,
    files: BTreeMap<String, Entry>,
    dirs: BTreeSet<String>,
    /// `git ls-files -s` entries of the scope (absent outside a git work tree).
    index: Option<Vec<String>>,
}

/// What [`scan`] found.
#[derive(Debug, Default)]
struct Tree {
    files: BTreeMap<String, Entry>,
    dirs: BTreeSet<String>,
}

/// Paths the snapshot leaves out: the backups themselves, the content-addressed artifact store (migrate does not
/// touch it) and the sqlite sidecars (transient; the snapshot folds them into `hot.db` first).
fn skipped(relative: &str) -> bool {
    let under = |dir: &str| relative == dir || relative.strip_prefix(dir).is_some_and(|rest| rest.starts_with('/'));
    under(canon_model::paths::MIGRATE_BACKUP_DIR)
        || under(canon_model::paths::ARTIFACTS_SHA256_DIR)
        || relative == format!("{}-wal", canon_model::paths::HOT_DB_FILE)
        || relative == format!("{}-shm", canon_model::paths::HOT_DB_FILE)
}

fn in_scope(scope: &[String], relative: &str) -> bool {
    !skipped(relative) && scope.iter().any(|s| relative == s || relative.strip_prefix(s.as_str()).is_some_and(|rest| rest.starts_with('/')))
}

/// The scope of the snapshot: `canon.yaml`, `.gitignore`, `AGENTS.md`, `.canon/` and every path in `extra` (the
/// places the other migrate steps read and write: spec roots, openspec changes, hooks, the skill install), repo-relative
/// and without the paths another entry holds.
pub fn scope(repo: &Path, extra: impl IntoIterator<Item = PathBuf>) -> Vec<String> {
    let mut all: BTreeSet<String> = [".canon", "canon.yaml", ".gitignore", "AGENTS.md"].into_iter().map(str::to_string).collect();
    for path in extra {
        let relative = if path.is_absolute() { path.strip_prefix(repo).map(Path::to_path_buf).ok() } else { Some(path) };
        if let Some(relative) = relative {
            let text = relative.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/");
            if !text.is_empty() && !text.starts_with("..") {
                all.insert(text);
            }
        }
    }
    let kept: Vec<String> = all.iter().filter(|p| !all.iter().any(|other| other != *p && p.strip_prefix(other.as_str()).is_some_and(|rest| rest.starts_with('/')))).cloned().collect();
    kept
}

#[cfg(unix)]
fn mode_of(meta: &std::fs::Metadata) -> u32 {
    use std::os::unix::fs::PermissionsExt;
    meta.permissions().mode() & 0o7777
}

#[cfg(not(unix))]
fn mode_of(_: &std::fs::Metadata) -> u32 {
    0
}

fn walk(repo: &Path, relative: &str, tree: &mut Tree) -> Result<(), String> {
    if skipped(relative) {
        return Ok(());
    }
    let path = repo.join(relative);
    let Ok(meta) = std::fs::symlink_metadata(&path) else { return Ok(()) };
    if meta.is_dir() {
        tree.dirs.insert(relative.to_string());
        let mut names: Vec<String> = std::fs::read_dir(&path).map_err(|e| format!("could not read {relative}: {e}"))?.filter_map(Result::ok).map(|e| e.file_name().to_string_lossy().into_owned()).collect();
        names.sort();
        for name in names {
            walk(repo, &format!("{relative}/{name}"), tree)?;
        }
    } else if meta.file_type().is_symlink() {
        let target = std::fs::read_link(&path).map_err(|e| format!("could not read the link {relative}: {e}"))?.to_string_lossy().into_owned();
        tree.files.insert(relative.to_string(), Entry { sha256: sha256_hex(target.as_bytes()), mode: 0, link: Some(target) });
    } else if meta.is_file() {
        let bytes = std::fs::read(&path).map_err(|e| format!("could not read {relative}: {e}"))?;
        tree.files.insert(relative.to_string(), Entry { sha256: sha256_hex(&bytes), mode: mode_of(&meta), link: None });
    }
    Ok(())
}

fn scan(repo: &Path, scope: &[String]) -> Result<Tree, String> {
    let mut tree = Tree::default();
    for entry in scope {
        walk(repo, entry, &mut tree)?;
    }
    Ok(tree)
}

/// `git ls-files -s` entries (`<mode> <sha> <stage>\t<path>`) of the scope.
fn index_entries(repo: &Path, scope: &[String]) -> Result<Vec<String>, String> {
    let out = Command::new("git").args(["ls-files", "-s", "-z"]).current_dir(repo).output().map_err(|e| format!("could not run git: {e}"))?;
    if !out.status.success() {
        return Err(format!("`git ls-files` failed: {}", String::from_utf8_lossy(&out.stderr).trim()));
    }
    Ok(String::from_utf8_lossy(&out.stdout).split('\0').filter(|e| e.split_once('\t').is_some_and(|(_, path)| in_scope(scope, path))).map(str::to_string).collect())
}

/// Copy everything `canon migrate` may touch (`scope`), and the git index entries of it, under `backup`, before the
/// first step acts: [`revert_plan`] and [`revert_apply`] restore exactly this state, whatever the steps did (the
/// renaming, spec and change moves, hooks and skill, the ledger compaction, the review pins, the index rebuild).
pub fn snapshot(repo: &Path, backup: &Path, scope: &[String]) -> Result<(), String> {
    let hot = repo.join(canon_model::paths::HOT_DB_FILE);
    if hot.is_file() {
        SqliteTier::connect(&hot).and_then(|tier| tier.checkpoint()).map_err(|e| format!("could not checkpoint {}: {e}", hot.display()))?;
    }
    let tree = scan(repo, scope)?;
    let copies = backup.join(SNAPSHOT_DIR);
    for (relative, entry) in &tree.files {
        if entry.link.is_some() {
            continue;
        }
        let to = copies.join(relative);
        if let Some(parent) = to.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("could not create {}: {e}", parent.display()))?;
        }
        std::fs::copy(repo.join(relative), &to).map_err(|e| format!("could not copy {relative} into the backup: {e}"))?;
    }
    let index = if is_git_tree(repo) { Some(index_entries(repo, scope)?) } else { None };
    let manifest = Manifest { scope: scope.to_vec(), files: tree.files, dirs: tree.dirs, index };
    let text = serde_json::to_string(&manifest).map_err(|e| e.to_string())?;
    std::fs::write(backup.join(MANIFEST), text).map_err(|e| format!("could not write the tree manifest: {e}"))
}

/// What the tree differs from the snapshot by.
#[derive(Debug, Default, PartialEq, Eq)]
struct Diff {
    /// Files changed or gone since the snapshot, and directories gone (trailing `/`).
    restore: Vec<String>,
    /// Files (and directories, trailing `/`) the snapshot did not have.
    remove: Vec<String>,
    /// The git index entries of the scope differ.
    index: bool,
}

impl Diff {
    fn is_empty(&self) -> bool {
        *self == Diff::default()
    }
}

fn diff(repo: &Path, manifest: &Manifest) -> Result<Diff, String> {
    let now = scan(repo, &manifest.scope)?;
    let mut out = Diff::default();
    for (path, entry) in &manifest.files {
        if now.files.get(path) != Some(entry) {
            out.restore.push(path.clone());
        }
    }
    out.restore.extend(manifest.dirs.iter().filter(|d| !now.dirs.contains(*d)).map(|d| format!("{d}/")));
    out.remove.extend(now.files.keys().filter(|p| !manifest.files.contains_key(*p)).cloned());
    out.remove.extend(now.dirs.iter().filter(|d| !manifest.dirs.contains(*d)).map(|d| format!("{d}/")));
    if let Some(saved) = &manifest.index {
        out.index = is_git_tree(repo) && index_entries(repo, &manifest.scope)? != *saved;
    }
    Ok(out)
}

/// What `canon migrate --revert` would do: restore the tree and the git index to the snapshot and give Postgres rows
/// and R2 objects their old keys back from the journal.
#[derive(Debug)]
pub struct RevertPlan {
    backup: PathBuf,
    manifest: Manifest,
    diff: Diff,
    journal: Option<Journal>,
}

impl RevertPlan {
    /// The backup directory the plan restores from.
    pub fn backup(&self) -> &Path {
        &self.backup
    }

    /// Nothing differs: the repo already is as the snapshot recorded it.
    pub fn is_empty(&self) -> bool {
        self.diff.is_empty()
    }

    /// The plan as effects, in the order [`revert_apply`] acts.
    pub fn effects(&self) -> Vec<Effect> {
        let mut out = Vec::new();
        if let Some(journal) = &self.journal {
            if let Some((_, schema, rows)) = &journal.postgres {
                for row in &rows.rows {
                    out.push(Effect::record("rekey", format!("postgres:{schema}:{}:{}:{}", row.to.0, row.to.1, row.to.2)).detail(format!("back to {}:{}:{}", row.from.0, row.from.1, row.from.2)));
                }
            }
            for object in &journal.s3 {
                out.push(Effect::record("rekey", format!("s3:{}", object.to)).detail(format!("back to {}", object.from)));
            }
        }
        out.extend(self.diff.remove.iter().map(|p| Effect::path("remove", p.clone()).detail("not there before the migrate")));
        out.extend(self.diff.restore.iter().map(|p| Effect::path("restore", p.clone()).detail("from the backup")));
        if self.diff.index {
            out.push(Effect::record("restore", "git index").detail("the staged paths of the migrate's scope, as before (the `git mv`s undone)"));
        }
        out
    }
}

/// The backup `--revert` restores: `given` (repo-relative or absolute), else the newest one holding a tree manifest.
pub fn find_backup(repo: &Path, given: Option<&Path>) -> Result<PathBuf, String> {
    if let Some(dir) = given {
        let dir = if dir.is_absolute() { dir.to_path_buf() } else { repo.join(dir) };
        return if dir.join(MANIFEST).is_file() { Ok(dir) } else { Err(format!("{} holds no {MANIFEST}: it is not a backup `canon migrate` 0.30 took", dir.display())) };
    }
    let base = repo.join(canon_model::paths::MIGRATE_BACKUP_DIR);
    let mut found: Vec<(String, u32, PathBuf)> = std::fs::read_dir(&base)
        .into_iter()
        .flatten()
        .filter_map(Result::ok)
        .filter(|e| e.path().join(MANIFEST).is_file())
        .map(|e| {
            let name = e.file_name().to_string_lossy().into_owned();
            let (stamp, n) = name.split_once('-').map_or((name.clone(), 0), |(s, n)| (s.to_string(), n.parse().unwrap_or(0)));
            (stamp, n, e.path())
        })
        .collect();
    found.sort();
    found.pop().map(|(_, _, dir)| dir).ok_or_else(|| format!("no backup under {}/ holds a tree manifest: `canon migrate --commit` has not run here (or ran before 0.30)", canon_model::paths::MIGRATE_BACKUP_DIR))
}

/// The plan that restores `repo` to the snapshot in `backup`, or why it cannot.
pub fn revert_plan(repo: &Path, backup: &Path) -> Result<RevertPlan, String> {
    let text = std::fs::read_to_string(backup.join(MANIFEST)).map_err(|e| format!("no tree manifest in {}: {e}", backup.display()))?;
    let manifest: Manifest = serde_json::from_str(&text).map_err(|e| format!("the tree manifest in {} is unreadable: {e}", backup.display()))?;
    for (path, entry) in &manifest.files {
        if entry.link.is_none() && !backup.join(SNAPSHOT_DIR).join(path).is_file() {
            return Err(format!("the backup {} lacks its copy of {path}; it cannot restore the repo", backup.display()));
        }
    }
    let diff = diff(repo, &manifest)?;
    let journal = match std::fs::read_to_string(backup.join(JOURNAL)) {
        Ok(text) if !diff.is_empty() => Some(serde_json::from_str::<Journal>(&text).map_err(|e| format!("the journal in {} is unreadable: {e}", backup.display()))?),
        _ => None,
    };
    if let Some((dsn_env, _, rows)) = journal.as_ref().and_then(|j| j.postgres.as_ref()) {
        if !rows.rows.is_empty() && std::env::var(dsn_env).is_err() {
            return Err(format!("the migrate re-keyed Postgres rows and ${dsn_env} is not set, so their old keys cannot be restored; set it and run the revert again"));
        }
    }
    Ok(RevertPlan { backup: backup.to_path_buf(), manifest, diff, journal })
}

#[cfg(unix)]
fn restore_file(target: &Path, copy: &Path, entry: &Entry) -> Result<(), String> {
    use std::os::unix::fs::{symlink, PermissionsExt};
    if let Some(link) = &entry.link {
        symlink(link, target).map_err(|e| format!("could not restore the link {}: {e}", target.display()))
    } else {
        std::fs::copy(copy, target).map_err(|e| format!("could not restore {}: {e}", target.display()))?;
        std::fs::set_permissions(target, std::fs::Permissions::from_mode(entry.mode)).map_err(|e| format!("could not restore the mode of {}: {e}", target.display()))
    }
}

#[cfg(not(unix))]
fn restore_file(target: &Path, copy: &Path, _: &Entry) -> Result<(), String> {
    std::fs::copy(copy, target).map(|_| ()).map_err(|e| format!("could not restore {}: {e}", target.display()))
}

/// Undo every step of `canon migrate`: R2 objects and Postgres rows get their old keys back, files the migrate wrote
/// or moved are removed, every file it rewrote, moved or removed comes back from the backup (`hot.db` included, its
/// sidecars dropped), and the git index returns to its entries (the `git mv`s undone). It then re-reads the tree and
/// refuses to say done unless it equals the snapshot.
pub fn revert_apply(repo: &Path, plan: &RevertPlan) -> Result<(), String> {
    if let Some(journal) = &plan.journal {
        revert_remote(repo, journal)?;
    }
    let (manifest, copies) = (&plan.manifest, plan.backup.join(SNAPSHOT_DIR));
    for path in plan.diff.remove.iter().filter(|p| !p.ends_with('/')) {
        let target = repo.join(path);
        std::fs::remove_file(&target).map_err(|e| format!("could not remove {path}: {e}"))?;
    }
    let mut gone_dirs: Vec<&str> = plan.diff.remove.iter().filter_map(|p| p.strip_suffix('/')).collect();
    gone_dirs.sort_by_key(|d| std::cmp::Reverse(d.matches('/').count()));
    for dir in gone_dirs {
        let _ = std::fs::remove_dir(repo.join(dir));
    }
    for dir in plan.diff.restore.iter().filter_map(|p| p.strip_suffix('/')) {
        std::fs::create_dir_all(repo.join(dir)).map_err(|e| format!("could not recreate {dir}: {e}"))?;
    }
    for path in plan.diff.restore.iter().filter(|p| !p.ends_with('/')) {
        let entry = &manifest.files[path];
        let target = repo.join(path);
        if let Some(parent) = target.parent() {
            std::fs::create_dir_all(parent).map_err(|e| format!("could not create {}: {e}", parent.display()))?;
        }
        match std::fs::symlink_metadata(&target) {
            Ok(meta) if meta.is_dir() => std::fs::remove_dir_all(&target).map_err(|e| format!("could not clear {path}: {e}"))?,
            Ok(_) => std::fs::remove_file(&target).map_err(|e| format!("could not clear {path}: {e}"))?,
            Err(_) => {}
        }
        restore_file(&target, &copies.join(path), entry)?;
        if path == canon_model::paths::HOT_DB_FILE {
            for sidecar in ["-wal", "-shm"] {
                let _ = std::fs::remove_file(repo.join(format!("{path}{sidecar}")));
            }
        }
    }
    if let (true, Some(saved)) = (is_git_tree(repo), &manifest.index) {
        let now = index_entries(repo, &manifest.scope)?;
        let paths: String = now.iter().filter_map(|e| e.split_once('\t')).map(|(_, p)| format!("{p}\0")).collect();
        if !paths.is_empty() {
            git(repo, &["update-index", "--force-remove", "-z", "--stdin"], Some(&paths))?;
        }
        let info: String = saved.iter().map(|e| format!("{e}\0")).collect();
        if !info.is_empty() {
            git(repo, &["update-index", "-z", "--index-info"], Some(&info))?;
        }
    }
    let left = diff(repo, manifest)?;
    if left.is_empty() {
        Ok(())
    } else {
        Err(format!("the revert ran but the repo still differs from the backup in {} file(s), {} unexpected and {} index state; restore from {}", left.restore.len(), left.remove.len(), if left.index { "the" } else { "no" }, plan.backup.display()))
    }
}

/// `canon migrate --revert [--backup <DIR>] [--commit --plan-digest D]`: preview, then act.
pub fn run_revert(repo_args: &crate::cli::RepoArgs, backup: Option<&Path>, mode: &crate::safety::Mode, json: bool) -> u8 {
    use crate::cli_error::CliError;
    use crate::migrate::MigrateReport;
    use crate::safety;
    let mut base = format!("{} --revert", safety::command("migrate", &[], repo_args));
    if let Some(dir) = backup {
        base.push_str(&format!(" --backup {}", safety::quote(&dir.display().to_string())));
    }
    let repo = crate::context::resolve_repo_root(repo_args);
    let refused = |message: String| CliError::refused("MIGRATE_REFUSED", message).hint("run `canon migrate --revert` in a repo `canon migrate --commit` ran in, or name its backup with `--backup .canon/migrate-backup/<timestamp>`").emit("canon migrate", json);
    let dir = match find_backup(&repo, backup) {
        Ok(dir) => dir,
        Err(e) => return refused(e),
    };
    let plan = match revert_plan(&repo, &dir) {
        Ok(plan) => plan,
        Err(e) => return refused(e),
    };
    let relative = rel(&repo, plan.backup());
    let safe_plan = safety::Plan::new(plan.effects());
    let commit = match safe_plan.gate(mode, &base) {
        Ok(commit) => commit,
        Err(e) => return e.emit("canon migrate", json),
    };
    let mut log = crate::json_out::Log::new(json);
    if !commit {
        log.line(format!("canon migrate --revert: restores the repo to the state {relative}/ recorded before `canon migrate --commit`; anything written under those paths since is discarded"));
        for effect in safe_plan.effects.iter().take(50) {
            log.line(safety::effect_line(effect));
        }
        if safe_plan.effects.len() > 50 {
            log.line(format!("  ... and {} more (`--json` lists every effect)", safe_plan.effects.len() - 50));
        }
        if plan.is_empty() {
            log.line("canon migrate --revert: the repo already is as the backup recorded it; nothing to restore");
        }
        log.line(safe_plan.commit_hint(&base));
        let report = MigrateReport { steps: log.into_lines(), backup_dir: Some(relative), index_records_written: None, index: None, plan: safe_plan };
        if json {
            crate::json_out::data_with_next("migrate", &report, &report.plan.next_step(&base, "the plan only lists what the revert would restore; applying it takes its planDigest"));
        }
        return 0;
    }
    let safe_plan = safe_plan.committed();
    if let Err(e) = revert_apply(&repo, &plan) {
        return CliError::refused("MIGRATE_STOPPED", e).hint("the backup is intact: fix what the message names, then run `canon migrate --revert` again").emit("canon migrate", json);
    }
    log.line(format!("canon migrate --revert: restored {} path(s) and removed {} from {relative}/", safe_plan.effects.iter().filter(|e| e.action == "restore").count(), safe_plan.effects.iter().filter(|e| e.action == "remove").count()));
    if json {
        crate::json_out::data("migrate", &MigrateReport { steps: log.into_lines(), backup_dir: Some(relative), index_records_written: None, index: None, plan: safe_plan });
    }
    0
}
