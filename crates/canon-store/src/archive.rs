//! The archive of superseded record versions (0.23 D2, "latest only").
//!
//! The ledger (`.canon/ledger/kind=<k>/…`) is the working set: what the
//! gate, `canon status` and every guard read. Appending a new version of
//! a natural-keyed record moves the versions it supersedes to the archive
//! (`.canon/archive/kind=<k>/…`, the same layout and file names), so the
//! ledger holds the current decision and a read no longer scans history.
//! The archive is append-only and is read only where a rule or an
//! operator names it: [`GitTier::read_history`] (`canon query --history`),
//! the explicit history reads in `canon-gate`, and the report's staging
//! views.
//!
//! # What "superseded" means
//!
//! A version is superseded when it is not the current record of any
//! group a reader folds it into. Most kinds fold by one key (the file's
//! natural key); the evidence ledger is read through several folds at
//! once (per cell, per project and scenario, per task, per run-seq
//! partition, …), so a record stays in the ledger while it wins any of
//! them. [`superseded`] is that one definition, and it is deliberately
//! conservative: a record it cannot judge (does not parse, carries no
//! join key) stays.
//!
//! A record also stays while it carries a claim a rule reads without
//! folding: a `flagged` overlay or a `reviewed` lifecycle tag (the trust
//! ladder), a brief override (listed by `status`), and an approval bound
//! to the current evidence generation (the risk tiers count distinct
//! approvers across the records of one generation).
//!
//! # Where the archive lives
//!
//! Beside the ledger directory: `<…>/ledger` has `<…>/archive`
//! ([`archive_root_of`]). A tier root that is not named `ledger`
//! (staging under `ledger/_staging`, a test fixture, a custom
//! `tiers.local.root`) has no archive and keeps every version, as before.
//! The archive is local; it is not routed through `tiers:`.
//!
//! # Crash safety
//!
//! A write lands first (atomic file replace); the move follows. A crash
//! between the two leaves the older version in the ledger beside the new
//! one, which every fold already reads correctly; the next append or
//! `canon ledger compact` moves it. A move is a rename (atomic), or copy
//! then remove across filesystems; a file present in both places is
//! verified equal and the ledger copy removed.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use canon_model::envelope::RecordKind;
use canon_model::{EvidenceRecord, RawRecord, ScenarioId, TrustLifecycle};
use serde_json::Value;

use crate::git_tier::GitTier;
use crate::partition::{content_digest12, resolve_partition};
use crate::tier::{raw_record_at, raw_record_schema, StoreError};

/// The directory beside the ledger holding the archive.
pub const ARCHIVE_DIR_NAME: &str = "archive";

/// The name a tier root must have for the archive to apply.
const LEDGER_DIR_NAME: &str = "ledger";

/// The kinds whose superseded versions are archived: the git-routed,
/// authored kinds whose readers fold to one current record. Execution
/// kinds (`task`, `handoff`, `session`, `run`, `event`, `trajectory`)
/// route to the hot and cold rungs by default and are not archived;
/// `strategy_item` has no folding reader.
pub const ARCHIVED_KINDS: [RecordKind; 7] = [
    RecordKind::Change,
    RecordKind::Subject,
    RecordKind::Scenario,
    RecordKind::Finding,
    RecordKind::EvidenceRecord,
    RecordKind::Review,
    RecordKind::Divergence,
];

/// Whether versions of `kind` move to the archive.
pub fn is_archived_kind(kind: RecordKind) -> bool {
    ARCHIVED_KINDS.contains(&kind)
}

/// The archive directory for a ledger root named `ledger`, else `None`.
pub fn archive_root_of(ledger_root: &Path) -> Option<PathBuf> {
    if ledger_root.file_name()? != LEDGER_DIR_NAME {
        return None;
    }
    Some(ledger_root.parent().map_or_else(|| PathBuf::from(ARCHIVE_DIR_NAME), |parent| parent.join(ARCHIVE_DIR_NAME)))
}

/// The order a group's winner is picked by: the first rung compared
/// first. `(at, schema, 0, digest)` for the `fold_latest_by_key` order,
/// `(run_seq, round, 0, "")` for the divergence fold.
type Rank = (i128, i128, i128, String);

/// What [`classify`] decides about one record.
struct Judged {
    /// Each group the record competes in, with its rank there.
    groups: Vec<(String, Rank)>,
    /// A record that stays whatever its groups say.
    pinned: bool,
    /// An approval bound to an evidence generation: the record stays
    /// while that generation is still the current one.
    approval: Option<ApprovalBinding>,
}

struct ApprovalBinding {
    /// The `(project, subject)` groups the approval's record answers;
    /// the current record of each is what the generation is read from.
    group_keys: Vec<String>,
    project: Option<String>,
    sha: Option<String>,
    run: Option<String>,
}

fn nanos(raw: &RawRecord) -> i128 {
    raw_record_at(raw).timestamp_nanos_opt().map_or(0, i128::from)
}

fn latest_rank(raw: &RawRecord) -> Rank {
    (nanos(raw), i128::from(raw_record_schema(raw)), 0, content_digest12(&raw.0))
}

fn text<'a>(json: &'a Value, field: &str) -> Option<&'a str> {
    json.get(field).and_then(Value::as_str)
}

fn unjudged() -> Judged {
    Judged { groups: Vec::new(), pinned: true, approval: None }
}

fn classify(kind: RecordKind, raw: &RawRecord) -> Judged {
    match kind {
        RecordKind::Change | RecordKind::Subject | RecordKind::Scenario | RecordKind::Finding => match resolve_partition(kind, &raw.0) {
            Ok(key) => Judged { groups: vec![(format!("key|{}", key.natural_key), latest_rank(raw))], pinned: false, approval: None },
            Err(_) => unjudged(),
        },
        RecordKind::Review => {
            // Grouped like the gate's `in_effect_reviews`: per (project,
            // scenario or change, reviewer). A change-level review (a
            // chore's, 0.23 D5) has no scenario and is keyed by its change.
            let target = text(&raw.0, "scenario_id").map(str::to_string).or_else(|| text(&raw.0, "change_id").map(|c| format!("change-{c}")));
            let (Some(project), Some(target), Some(reviewer)) = (text(&raw.0, "project_id"), target, text(&raw.0, "reviewer")) else {
                return unjudged();
            };
            Judged { groups: vec![(format!("review|{project}|{target}|{reviewer}"), latest_rank(raw))], pinned: false, approval: None }
        }
        RecordKind::Divergence => classify_divergence(raw),
        RecordKind::EvidenceRecord => classify_evidence(raw),
        _ => unjudged(),
    }
}

fn classify_divergence(raw: &RawRecord) -> Judged {
    let json = &raw.0;
    let (Some(project), Some(scenario), Some(run_seq), Some(round)) =
        (text(json, "project_id"), text(json, "scenario_id"), json.get("run_seq").and_then(Value::as_u64), json.get("round").and_then(Value::as_u64))
    else {
        return unjudged();
    };
    let (run_seq, round) = (i128::from(run_seq), i128::from(round));
    // The fold winner per `(project, scenario)`: `(run_seq, round)`.
    let mut groups = vec![(format!("div|{project}|{scenario}"), (run_seq, round, 0, String::new()))];
    // `canon_gate::acceptance::held` reads the latest ACCEPTED divergence
    // by `(at, run_seq, round)`, which a later non-accepted one does not
    // replace.
    if json.get("status").is_some_and(|status| status.get("accepted").is_some()) {
        groups.push((format!("div-accepted|{project}|{scenario}"), (nanos(raw), run_seq, round, String::new())));
    }
    // The promotion's next `run_seq` is `1 + max(run_seq)` per `(project,
    // role, surface)`; the record holding the maximum must stay.
    if let (Some(role), Ok(scenario_id)) = (json.get("actor").and_then(|a| text(a, "role")), ScenarioId::parse(scenario)) {
        groups.push((format!("div-seq|{project}|{role}|{}", scenario_id.surface_key()), (run_seq, 0, 0, String::new())));
    }
    Judged { groups, pinned: false, approval: None }
}

fn classify_evidence(raw: &RawRecord) -> Judged {
    let Ok(record) = serde_json::from_value::<EvidenceRecord>(raw.0.clone()) else { return unjudged() };
    let task = record.task_id.as_ref().map(ToString::to_string);
    let scenario = record.scenario_id.as_ref().map(ToString::to_string);
    if task.is_none() && scenario.is_none() {
        // No cell identity: no fold groups it, so nothing supersedes it.
        return unjudged();
    }
    let role = record.envelope.actor.role.as_ref().map_or("-".to_string(), |r| r.as_str().to_string());
    let project = record.project_id.as_ref().map_or("-".to_string(), ToString::to_string);
    let audit = if record.is_audit() { "audit" } else { "declared" };
    // The gate folds by the typed record's digest, not the file's.
    let typed_digest = content_digest12(&serde_json::to_value(&record).unwrap_or_default());
    let rank: Rank = (i128::from(record.envelope.at.timestamp_nanos_opt().unwrap_or(0)), i128::from(record.envelope.schema), 0, typed_digest);

    let subjects: Vec<String> = task.iter().map(|t| format!("task:{t}")).chain(scenario.iter().map(|s| format!("scenario:{s}"))).collect();
    let mut groups = Vec::new();
    let mut approval_keys = Vec::new();
    for subject in &subjects {
        // `latest_verdicts`, staleness: the latest per (subject, role).
        groups.push((format!("ev|{audit}|cell|{subject}|{role}"), rank.clone()));
        // Review pins: the latest per role among one project's records.
        groups.push((format!("ev|{audit}|project-cell|{project}|{subject}|{role}"), rank.clone()));
        // Risk tiers, the artifact check, evidence binding, coverage:
        // the latest per (project, subject) and per subject.
        let project_group = format!("ev|{audit}|project|{project}|{subject}");
        approval_keys.push(project_group.clone());
        groups.push((project_group, rank.clone()));
        groups.push((format!("ev|{audit}|subject|{subject}"), rank.clone()));
    }
    // `live_bindings_of` and the acceptance's `sha`: the latest record
    // that carries an `evidence_sha`, which may not be the latest.
    if let (Some(scenario), Some(_)) = (&scenario, &record.evidence_sha) {
        groups.push((format!("ev|{audit}|sha|{project}|{scenario}"), rank.clone()));
    }
    // Promotion's next `run_seq`: `1 + max(run_seq)` per (role, surface).
    if let (Some(run_seq), Some(role)) = (raw.0.get("run_seq").and_then(Value::as_u64), record.envelope.actor.role.as_ref()) {
        let surface = record.scenario_id.as_ref().map(ScenarioId::surface_key).or_else(|| record.task_id.as_ref().map(|t| t.change_id().to_string()));
        if let Some(surface) = surface {
            groups.push((format!("ev|{audit}|seq|{}|{surface}", role.as_str()), (i128::from(run_seq), 0, 0, String::new())));
        }
    }
    let flagged = record.flagged.as_ref().is_some_and(|f| f.flagged);
    let reviewed = record.lifecycle == Some(TrustLifecycle::Reviewed);
    let pinned = flagged || reviewed || record.brief_override.is_some();
    let approval = record.approval.as_ref().map(|_| ApprovalBinding {
        group_keys: approval_keys,
        project: record.project_id.as_ref().map(ToString::to_string),
        sha: record.evidence_sha.as_ref().map(ToString::to_string),
        run: record.run_id.as_ref().map(ToString::to_string),
    });
    Judged { groups, pinned, approval }
}

/// The paths of `rows` that are superseded: not the current record of
/// any group they compete in, and not pinned. Pure; the result for a set
/// of rows does not depend on their order.
pub fn superseded(kind: RecordKind, rows: &[(PathBuf, RawRecord)]) -> BTreeSet<PathBuf> {
    let judged: Vec<Judged> = rows.iter().map(|(_, raw)| classify(kind, raw)).collect();

    // The current record of each group: the greatest rank, the smaller
    // path on an exact tie (identical content).
    let mut winners: BTreeMap<&str, (&Rank, &Path, usize)> = BTreeMap::new();
    for (index, (judged, (path, _))) in judged.iter().zip(rows).enumerate() {
        for (group, rank) in &judged.groups {
            let beats = winners.get(group.as_str()).is_none_or(|(held, held_path, _)| rank > *held || (rank == *held && path.as_path() < *held_path));
            if beats {
                winners.insert(group.as_str(), (rank, path.as_path(), index));
            }
        }
    }

    let mut out = BTreeSet::new();
    for (index, (judged, (path, _))) in judged.iter().zip(rows).enumerate() {
        if judged.pinned || judged.groups.is_empty() {
            continue;
        }
        let wins_a_group = judged.groups.iter().any(|(group, _)| winners.get(group.as_str()).is_some_and(|(_, _, winner)| *winner == index));
        if wins_a_group {
            continue;
        }
        if let Some(approval) = &judged.approval {
            // An approval stays while its generation is the current one.
            let current = approval.group_keys.iter().any(|key| {
                let Some((_, _, winner)) = winners.get(key.as_str()) else { return false };
                same_generation(approval, &rows[*winner].1)
            });
            if current {
                continue;
            }
        }
        out.insert(path.clone());
    }
    out
}

/// Whether `winner` (the current record of an approval's group) is the
/// generation the approval is bound to: the same project, `evidence_sha`
/// and `run_id`.
fn same_generation(approval: &ApprovalBinding, winner: &RawRecord) -> bool {
    let sha = text(&winner.0, "evidence_sha");
    sha.is_some()
        && sha == approval.sha.as_deref()
        && text(&winner.0, "run_id") == approval.run.as_deref()
        && text(&winner.0, "project_id") == approval.project.as_deref()
}

/// One kind's compaction: what the ledger held and what moved.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KindCompaction {
    pub kind: RecordKind,
    /// Files in the ledger before the move.
    pub files: usize,
    /// Files that stay: the current record of a key, and pinned records.
    pub live: usize,
    /// Files the ledger holds that are superseded.
    pub superseded: usize,
    /// Of those, the files moved (`0` under `dry_run`).
    pub moved: usize,
    /// Total bytes of the superseded files.
    pub bytes: u64,
    /// The superseded files, relative to the ledger root, sorted (the same under `dry_run`).
    pub paths: Vec<PathBuf>,
}

impl GitTier {
    /// The archive directory beside this tier's root, when its root is a
    /// ledger directory ([`archive_root_of`]).
    pub fn archive_root(&self) -> Option<PathBuf> {
        archive_root_of(self.root())
    }

    /// The archive as a tier of its own, or `None` without an archive.
    /// Its root is named `archive`, so writes to it never archive.
    pub fn archive_tier(&self) -> Option<GitTier> {
        self.archive_root().map(GitTier::new)
    }

    /// Every version of `kind`: the ledger's records, then the archive's.
    /// The one history read (`canon query --history`, the rules that
    /// name history). Violations from both sides are returned.
    pub fn read_history(&self, kind: RecordKind) -> Result<crate::tier::TierReadResult, StoreError> {
        self.read_history_shared(kind).map(crate::git_tier::SharedRead::into_owned)
    }

    /// [`Self::read_history`] with the records shared with the per-process
    /// file cache rather than copied: for a reader that only borrows each
    /// body (`T::deserialize(&raw.0)`), as promotion's committed-history
    /// index does.
    pub fn read_history_shared(&self, kind: RecordKind) -> Result<crate::git_tier::SharedRead, StoreError> {
        use crate::tier::TierQuery;
        let mut read = self.read_shared(&TierQuery::kind(kind))?;
        if let Some(archive) = self.archive_tier() {
            let archived = archive.read_shared(&TierQuery::kind(kind))?;
            read.records.extend(archived.records);
            read.violations.extend(archived.violations);
        }
        Ok(read)
    }

    /// Move every superseded version of `kind` out of the ledger.
    /// Idempotent: a second call finds nothing superseded. Without an
    /// archive ([`archive_root_of`]) it does nothing.
    pub fn compact_kind(&self, kind: RecordKind, dry_run: bool) -> Result<KindCompaction, StoreError> {
        let mut report = KindCompaction { kind, files: 0, live: 0, superseded: 0, moved: 0, bytes: 0, paths: Vec::new() };
        if !is_archived_kind(kind) || self.archive_root().is_none() {
            return Ok(report);
        }
        let (rows, _violations) = self.scan_kind(kind)?;
        self.archive_rows(kind, rows, dry_run, &mut report)?;
        Ok(report)
    }

    /// Compact every archived kind ([`ARCHIVED_KINDS`]).
    pub fn compact(&self, dry_run: bool) -> Result<Vec<KindCompaction>, StoreError> {
        ARCHIVED_KINDS.iter().map(|kind| self.compact_kind(*kind, dry_run)).collect()
    }

    /// After a write of `written` (a record of `kind`): move the versions
    /// it supersedes. Scans only the files that can share a group with it
    /// (the same key's files; the whole kind for evidence, whose groups
    /// span keys). A failure leaves the ledger readable and is returned
    /// for the caller to report; the next append or `canon ledger
    /// compact` finishes the move.
    pub fn archive_after_write(&self, kind: RecordKind, written: &RawRecord) -> Result<usize, StoreError> {
        if !is_archived_kind(kind) || self.archive_root().is_none() {
            return Ok(0);
        }
        let rows = match scope_of(kind, written) {
            Some((dir, belongs)) => self.scan_dir(kind, &dir, &belongs)?.0,
            None => self.scan_kind(kind)?.0,
        };
        let mut report = KindCompaction { kind, files: 0, live: 0, superseded: 0, moved: 0, bytes: 0, paths: Vec::new() };
        self.archive_rows(kind, rows, false, &mut report)?;
        Ok(report.moved)
    }

    fn scan_kind(&self, kind: RecordKind) -> Result<crate::git_tier::ScanResult, StoreError> {
        self.scan_all(kind)
    }

    fn archive_rows(&self, kind: RecordKind, rows: Vec<(PathBuf, RawRecord)>, dry_run: bool, report: &mut KindCompaction) -> Result<(), StoreError> {
        let Some(archive) = self.archive_root() else { return Ok(()) };
        let moving = superseded(kind, &rows);
        report.files = rows.len();
        report.superseded = moving.len();
        report.live = rows.len() - moving.len();
        for path in &moving {
            report.bytes += std::fs::metadata(self.root().join(path)).map_or(0, |m| m.len());
        }
        report.paths = moving.iter().cloned().collect();
        if dry_run {
            return Ok(());
        }
        for path in &moving {
            if move_between(self.root(), &archive, path)? {
                report.moved += 1;
            }
        }
        Ok(())
    }
}

/// The directory and file-name test bounding which files can share a
/// group with a freshly written `raw`. `None`: the whole kind.
type Scope = (PathBuf, Box<dyn Fn(&str) -> bool>);

fn scope_of(kind: RecordKind, raw: &RawRecord) -> Option<Scope> {
    let relative = crate::partition::expected_relative_path(kind, &raw.0).ok()?;
    let dir = relative.parent()?.to_path_buf();
    match kind {
        RecordKind::Change | RecordKind::Subject | RecordKind::Scenario | RecordKind::Finding => {
            let key = resolve_partition(kind, &raw.0).ok()?.natural_key;
            Some((dir, Box::new(move |name| stem_key(name) == Some(key.as_str()))))
        }
        RecordKind::Review | RecordKind::Divergence => {
            let target = text(&raw.0, "scenario_id").map(str::to_string).or_else(|| text(&raw.0, "change_id").map(|c| format!("change-{c}")))?;
            let prefix = format!("{}__{}__", text(&raw.0, "project_id")?, target);
            Some((dir, Box::new(move |name| name.starts_with(&prefix))))
        }
        // `EvidenceRecord` has no narrower scope, on purpose: the ledger
        // is one flat directory keyed by scenario or task, but its fold
        // groups span keys (the `run_seq` partition covers every scenario
        // of one surface, the project and subject groups several), so a
        // fresh record can supersede a file under any other key, and a
        // pass has always moved every superseded version it finds.
        // `canon evidence add` therefore writes a batch unarchived and
        // calls `archive_after_write` once, after the last record.
        _ => None,
    }
}

/// The natural-key part of a record file name `<key>__<digest12>.json`.
fn stem_key(name: &str) -> Option<&str> {
    name.strip_suffix(".json")?.rsplit_once("__").map(|(key, _)| key)
}

/// Move `<from>/<relative>` to `<to>/<relative>`. `Ok(false)` when
/// the source is already gone (another process moved it).
fn move_between(from: &Path, to: &Path, relative: &Path) -> Result<bool, StoreError> {
    let source = from.join(relative);
    let target = to.join(relative);
    if let Some(parent) = target.parent() {
        std::fs::create_dir_all(parent)?;
    }
    if target.exists() {
        // A crash after the copy, or the same version archived twice.
        let incoming = match std::fs::read(&source) {
            Ok(bytes) => bytes,
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(false),
            Err(e) => return Err(e.into()),
        };
        if std::fs::read(&target)? != incoming {
            return Err(StoreError::Policy(format!(
                "`{}` already exists at `{}` with different bytes; ledger and archive never overwrite each other, so neither copy was touched",
                relative.display(),
                to.display()
            )));
        }
        std::fs::remove_file(&source)?;
        return Ok(true);
    }
    match std::fs::rename(&source, &target) {
        Ok(()) => Ok(true),
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => Ok(false),
        Err(_) => {
            let bytes = std::fs::read(&source)?;
            crate::atomic::write_atomic(&target, &bytes)?;
            std::fs::remove_file(&source)?;
            Ok(true)
        }
    }
}

impl GitTier {
    /// Move back to the ledger every archived version that is no longer
    /// superseded: the inverse of [`GitTier::compact_kind`], for a writer
    /// that removed a record it had just written (a batch that failed
    /// partway) and so left the archived version it replaced as the
    /// current one of its key. Judges ledger and archive together, so a
    /// version that something still supersedes stays archived.
    pub fn restore_unsuperseded(&self, kind: RecordKind) -> Result<usize, StoreError> {
        let (Some(archive), true) = (self.archive_tier(), is_archived_kind(kind)) else { return Ok(0) };
        let (ledger_rows, _) = self.scan_all(kind)?;
        let (archived_rows, _) = archive.scan_all(kind)?;
        let in_ledger: BTreeSet<PathBuf> = ledger_rows.iter().map(|(path, _)| path.clone()).collect();
        let archived_paths: Vec<PathBuf> = archived_rows.iter().map(|(path, _)| path.clone()).filter(|path| !in_ledger.contains(path)).collect();
        let rows: Vec<(PathBuf, RawRecord)> = ledger_rows.into_iter().chain(archived_rows).collect();
        let moving = superseded(kind, &rows);
        let mut restored = 0;
        for path in archived_paths.iter().filter(|path| !moving.contains(*path)) {
            if move_between(archive.root(), self.root(), path)? {
                restored += 1;
            }
        }
        Ok(restored)
    }
}

#[cfg(test)]
mod tests {
    use canon_model::envelope::{Actor, Envelope};
    use canon_model::ids::{ChangeId, ProjectId, RoleId, Sha, TotalOrder};
    use canon_model::records::{Change, ChangeStatus, Divergence, DivergenceStatus};
    use canon_model::{EvidenceVerdict, FlaggedOverlay, Review, ProvenanceRef};
    use chrono::{Duration, TimeZone, Utc};

    use super::*;
    use crate::tier::{RawWrite, StoredRecord, Tier, TierQuery};

    fn base() -> chrono::DateTime<Utc> {
        Utc.with_ymd_and_hms(2026, 3, 1, 12, 0, 0).unwrap()
    }

    fn actor(role: &str) -> Actor {
        Actor::new(format!("agent-{role}"), RoleId::parse(role).unwrap())
    }

    /// `<tmp>/.canon/ledger`: a ledger root with an archive beside it.
    fn ledger(dir: &tempfile::TempDir) -> GitTier {
        GitTier::new(dir.path().join(".canon").join("ledger"))
    }

    fn change(at_offset: i64, status: ChangeStatus) -> Change {
        Change::new(Envelope::new(1, RecordKind::Change, base() + Duration::seconds(at_offset), actor("implementer")), ChangeId::parse("add-login").unwrap(), "L", "login", status)
    }

    fn evidence(role: &str, scenario: &str, at_offset: i64, verdict: EvidenceVerdict) -> EvidenceRecord {
        EvidenceRecord::new(Envelope::new(1, RecordKind::EvidenceRecord, base() + Duration::seconds(at_offset), actor(role)), None, Some(ScenarioId::parse(scenario).unwrap()), None, verdict)
            .with_project_id(ProjectId::parse("root").unwrap())
    }

    fn names(tier: &GitTier, kind: RecordKind) -> Vec<String> {
        let mut out: Vec<String> = tier.scan_all(kind).unwrap().0.into_iter().map(|(path, _)| path.file_name().unwrap().to_string_lossy().into_owned()).collect();
        out.sort();
        out
    }

    fn archived(tier: &GitTier, kind: RecordKind) -> usize {
        tier.archive_tier().unwrap().read(&TierQuery::kind(kind)).unwrap().records.len()
    }

    #[test]
    fn an_append_moves_the_versions_it_supersedes_to_the_archive() {
        let dir = tempfile::tempdir().unwrap();
        let tier = ledger(&dir);
        tier.write(&change(0, ChangeStatus::Proposed)).unwrap();
        tier.write(&change(10, ChangeStatus::InProgress)).unwrap();
        let latest = tier.write(&change(20, ChangeStatus::Completed)).unwrap();

        let held = tier.read(&TierQuery::kind(RecordKind::Change)).unwrap().records;
        assert_eq!(held.len(), 1, "the ledger holds one file per key");
        assert_eq!(held[0].0["status"], "completed");
        assert!(tier.root().join(&latest.location).exists());
        assert_eq!(archived(&tier, RecordKind::Change), 2, "the two earlier versions are in the archive");
        assert_eq!(tier.read_history(RecordKind::Change).unwrap().records.len(), 3, "history reads both");
        assert!(dir.path().join(".canon/archive/kind=change").is_dir(), "the archive sits beside the ledger, same layout");
    }

    #[test]
    fn a_late_older_version_is_archived_not_promoted_over_the_current_one() {
        let dir = tempfile::tempdir().unwrap();
        let tier = ledger(&dir);
        tier.write(&change(20, ChangeStatus::Completed)).unwrap();
        tier.write(&change(0, ChangeStatus::Proposed)).unwrap();

        let held = tier.read(&TierQuery::kind(RecordKind::Change)).unwrap().records;
        assert_eq!(held.len(), 1);
        assert_eq!(held[0].0["status"], "completed", "the fold would pick this one, so the ledger keeps it");
    }

    #[test]
    fn evidence_of_another_role_or_project_is_not_superseded_by_a_newer_record() {
        let dir = tempfile::tempdir().unwrap();
        let tier = ledger(&dir);
        let scenario = "world.firstbuy-hotdeal.26";
        tier.write(&evidence("implementer", scenario, 0, EvidenceVerdict::Faithful)).unwrap();
        tier.write(&evidence("reviewer", scenario, 5, EvidenceVerdict::Faithful)).unwrap();
        assert_eq!(names(&tier, RecordKind::EvidenceRecord).len(), 2, "each role's latest is a different cell: both stay");

        tier.write(&evidence("implementer", scenario, 10, EvidenceVerdict::Divergent)).unwrap();
        let held = tier.read(&TierQuery::kind(RecordKind::EvidenceRecord)).unwrap().records;
        assert_eq!(held.len(), 2, "the implementer's newer record replaced only the implementer's older one");
        assert_eq!(archived(&tier, RecordKind::EvidenceRecord), 1);

        let other_project = evidence("implementer", scenario, 12, EvidenceVerdict::Faithful).with_project_id(ProjectId::parse("other").unwrap());
        tier.write(&other_project).unwrap();
        assert_eq!(names(&tier, RecordKind::EvidenceRecord).len(), 3, "a record of another project does not supersede this project's");
    }

    #[test]
    fn standing_claims_stay_in_the_ledger_while_a_rule_reads_them_unfolded() {
        let dir = tempfile::tempdir().unwrap();
        let tier = ledger(&dir);
        let scenario = "world.firstbuy-hotdeal.26";
        let flagged = evidence("implementer", scenario, 0, EvidenceVerdict::Faithful).with_flagged(FlaggedOverlay::set(actor("human"), base()));
        tier.write(&flagged).unwrap();
        tier.write(&evidence("implementer", scenario, 10, EvidenceVerdict::Faithful)).unwrap();
        assert_eq!(names(&tier, RecordKind::EvidenceRecord).len(), 2, "a flagged record is never green and is read unfolded: it stays");
        assert_eq!(archived(&tier, RecordKind::EvidenceRecord), 0);
    }

    #[test]
    fn compaction_moves_everything_superseded_once_and_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        let tier = ledger(&dir);
        for (offset, status) in [(0, ChangeStatus::Proposed), (10, ChangeStatus::InProgress), (20, ChangeStatus::Completed)] {
            tier.write_unarchived(&change(offset, status)).unwrap();
        }
        assert_eq!(names(&tier, RecordKind::Change).len(), 3, "written without the archive step");

        let preview = tier.compact(true).unwrap();
        let change_row = preview.iter().find(|k| k.kind == RecordKind::Change).unwrap();
        assert_eq!((change_row.files, change_row.live, change_row.superseded, change_row.moved), (3, 1, 2, 0), "a dry run moves nothing");
        assert_eq!(names(&tier, RecordKind::Change).len(), 3);

        let first = tier.compact(false).unwrap();
        let change_row = first.iter().find(|k| k.kind == RecordKind::Change).unwrap();
        assert_eq!((change_row.superseded, change_row.moved), (2, 2));
        assert!(change_row.bytes > 0);
        assert_eq!((names(&tier, RecordKind::Change).len(), archived(&tier, RecordKind::Change)), (1, 2));

        let second = tier.compact(false).unwrap();
        assert!(second.iter().all(|k| k.superseded == 0 && k.moved == 0), "a second run finds nothing: {second:?}");
        assert_eq!((names(&tier, RecordKind::Change).len(), archived(&tier, RecordKind::Change)), (1, 2));
    }

    #[test]
    fn a_crash_between_the_write_and_the_move_leaves_a_readable_ledger_that_compaction_finishes() {
        let dir = tempfile::tempdir().unwrap();
        let tier = ledger(&dir);
        let old = tier.write_unarchived(&change(0, ChangeStatus::Proposed)).unwrap();
        tier.write_unarchived(&change(10, ChangeStatus::Completed)).unwrap();
        // The crash: the old version was copied to the archive, not yet removed.
        let archive = tier.archive_root().unwrap();
        std::fs::create_dir_all(archive.join("kind=change")).unwrap();
        std::fs::copy(tier.root().join(&old.location), archive.join(&old.location)).unwrap();

        let read = tier.read(&TierQuery::kind(RecordKind::Change)).unwrap();
        assert_eq!(read.records.len(), 2, "both versions are readable; a fold picks the latest");

        tier.compact(false).unwrap();
        assert_eq!(names(&tier, RecordKind::Change).len(), 1);
        assert_eq!(archived(&tier, RecordKind::Change), 1, "no duplicate in the archive");
    }

    #[test]
    fn a_conflicting_archive_copy_is_an_error_and_neither_file_is_touched() {
        let dir = tempfile::tempdir().unwrap();
        let tier = ledger(&dir);
        let old = tier.write_unarchived(&change(0, ChangeStatus::Proposed)).unwrap();
        tier.write_unarchived(&change(10, ChangeStatus::Completed)).unwrap();
        let archive = tier.archive_root().unwrap();
        std::fs::create_dir_all(archive.join("kind=change")).unwrap();
        std::fs::write(archive.join(&old.location), b"{}").unwrap();

        let err = tier.compact_kind(RecordKind::Change, false).unwrap_err();
        assert!(err.to_string().contains("different bytes"), "{err}");
        assert!(tier.root().join(&old.location).exists());
        assert_eq!(std::fs::read(archive.join(&old.location)).unwrap(), b"{}");
    }

    #[test]
    fn a_tier_root_that_is_not_a_ledger_directory_keeps_every_version() {
        let dir = tempfile::tempdir().unwrap();
        let tier = GitTier::new(dir.path().join("records"));
        tier.write(&change(0, ChangeStatus::Proposed)).unwrap();
        tier.write(&change(10, ChangeStatus::Completed)).unwrap();
        assert_eq!(names(&tier, RecordKind::Change).len(), 2);
        assert!(tier.archive_root().is_none());
        assert!(tier.compact(false).unwrap().iter().all(|k| k.moved == 0));

        let staging = GitTier::new(dir.path().join(".canon").join("ledger").join("_staging"));
        assert!(staging.archive_root().is_none(), "staging never archives");
    }

    #[test]
    fn restoring_after_a_rollback_puts_the_replaced_version_back() {
        let dir = tempfile::tempdir().unwrap();
        let tier = ledger(&dir);
        tier.write(&change(0, ChangeStatus::Proposed)).unwrap();
        let newer = tier.write(&change(10, ChangeStatus::Completed)).unwrap();
        assert_eq!(archived(&tier, RecordKind::Change), 1);

        std::fs::remove_file(tier.root().join(&newer.location)).unwrap();
        assert_eq!(names(&tier, RecordKind::Change).len(), 0, "the rolled-back batch left the ledger without the key");
        assert_eq!(tier.restore_unsuperseded(RecordKind::Change).unwrap(), 1);
        let held = tier.read(&TierQuery::kind(RecordKind::Change)).unwrap().records;
        assert_eq!((held.len(), archived(&tier, RecordKind::Change)), (1, 0));
        assert_eq!(held[0].0["status"], "proposed");
        assert_eq!(tier.restore_unsuperseded(RecordKind::Change).unwrap(), 0, "idempotent");
    }

    fn divergence(run_seq: u64, status: DivergenceStatus, at_offset: i64) -> Divergence {
        Divergence::new(
            Envelope::new(1, RecordKind::Divergence, base() + Duration::seconds(at_offset), actor("reviewer")),
            ProjectId::parse("root").unwrap(),
            ScenarioId::parse("world.firstbuy-hotdeal.26").unwrap(),
            Sha::parse("a".repeat(40)).unwrap(),
            status,
            TotalOrder::new(run_seq),
            1,
            "alice",
            "detail",
        )
    }

    #[test]
    fn divergences_keep_the_fold_winner_and_the_latest_acceptance() {
        let dir = tempfile::tempdir().unwrap();
        let tier = ledger(&dir);
        let accepted = DivergenceStatus::Accepted { reason: "known".into(), tracking: "ISSUE-1".into(), expiry: None, evidence: Vec::new(), checks: None };
        tier.write(&divergence(1, DivergenceStatus::Open, 0)).unwrap();
        tier.write(&divergence(2, accepted, 10)).unwrap();
        tier.write(&divergence(3, DivergenceStatus::StillDivergent, 20)).unwrap();

        let held: Vec<u64> = {
            let mut seqs: Vec<u64> = tier.read(&TierQuery::kind(RecordKind::Divergence)).unwrap().records.iter().map(|r| r.0["run_seq"].as_u64().unwrap()).collect();
            seqs.sort();
            seqs
        };
        assert_eq!(held, vec![2, 3], "run_seq 3 wins the fold; run_seq 2 is the latest acceptance `acceptance::held` reads; run_seq 1 is history");
        assert_eq!(archived(&tier, RecordKind::Divergence), 1);
    }

    #[test]
    fn reviews_supersede_per_reviewer_not_per_pin() {
        let dir = tempfile::tempdir().unwrap();
        let tier = ledger(&dir);
        let review = |reviewer: &str, pin: &str, at: i64| {
            Review::new(
                Envelope::new(1, RecordKind::Review, base() + Duration::seconds(at), actor("reviewer")),
                ProjectId::parse("root").unwrap(),
                ScenarioId::parse("world.firstbuy-hotdeal.26").unwrap(),
                reviewer,
                pin,
                ProvenanceRef::OriginalSpecRef("docs/prd.md".into()),
            )
        };
        tier.write(&review("alice", "pin-a", 0)).unwrap();
        tier.write(&review("bob", "pin-a", 5)).unwrap();
        assert_eq!(names(&tier, RecordKind::Review).len(), 2, "two reviewers at one pin: both are in effect");
        tier.write(&review("alice", "pin-b", 10)).unwrap();
        assert_eq!(names(&tier, RecordKind::Review).len(), 2, "alice's re-review replaced her earlier one, at another pin");
        assert_eq!(archived(&tier, RecordKind::Review), 1);
    }

    /// A chore's change-level review (0.23 D5) has no scenario: it is
    /// grouped by its change, so a re-review supersedes the earlier one
    /// instead of both staying in the ledger forever.
    #[test]
    fn change_level_reviews_supersede_per_change_and_reviewer() {
        let dir = tempfile::tempdir().unwrap();
        let tier = ledger(&dir);
        let review = |pin: &str, at: i64| {
            Review::for_change(
                Envelope::new(1, RecordKind::Review, base() + Duration::seconds(at), actor("reviewer")),
                ProjectId::parse("root").unwrap(),
                ChangeId::parse("split-tables").unwrap(),
                "alice",
                pin,
                ProvenanceRef::OriginalSpecRef("docs/prd.md".into()),
            )
        };
        tier.write(&review("brief-a", 0)).unwrap();
        tier.write(&review("brief-b", 10)).unwrap();
        assert_eq!(names(&tier, RecordKind::Review).len(), 1, "the re-review is the one in effect");
        assert_eq!(archived(&tier, RecordKind::Review), 1);
    }

    #[test]
    fn a_raw_write_of_an_archived_kind_archives_too() {
        let dir = tempfile::tempdir().unwrap();
        let tier = ledger(&dir);
        for record in [change(0, ChangeStatus::Proposed), change(10, ChangeStatus::Completed)] {
            tier.write(&RawWrite(record.to_raw())).unwrap();
        }
        assert_eq!((names(&tier, RecordKind::Change).len(), archived(&tier, RecordKind::Change)), (1, 1));
    }
}
