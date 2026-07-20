//! Staging → promote (design decision 5, O13 `cmd_promote`; extended by
//! s15 P3b / design D10 to `RecordKind::Divergence`). Reviewers write
//! unordered records under `_staging/` (no `run_seq`); [`promote`]
//! assigns a monotonic per-`(role, surface)` `run_seq` to `EvidenceRecord`
//! candidates, re-validates each candidate with the SAME structural check
//! the gate itself applies, writes the committed file, and deletes the
//! staging source. Generalizes `tools/parity.py`'s `_next_run_seq` +
//! `cmd_promote` (the donor parity-harness audit's divergence-log notes
//! §3.2/§3.5) over this crate's `EvidenceRecord`
//! corpus, all through `canon-store`'s [`GitTier`] (S2) — never a
//! hand-rolled filesystem writer.
//!
//! # `(role, surface)`, not parity.py's `(lane, surface)`
//! Design decision 5 picks canon's own role-namespacing (S6) over the
//! donor's `lane` axis (design doc's Open Questions: this choice is
//! "decided when S11 wires the donor consumer repo's actual corpus through
//! `canon-gate`" for THAT consumer; this crate's own generic
//! implementation commits to `(role, surface)` now). Both halves are
//! already-typed fields, no companion type needed: `role` is the
//! writing actor's own `envelope.actor.role`; `surface` is
//! `ScenarioId::surface_key()` (`canon-model/src/ids.rs`, parity.py's
//! own `_surface_key_of`: `<area>-<surface>`) when the candidate
//! carries a `scenario_id`, falling back to `TaskId::change_id()`
//! (the owning change, a coarser but still stable grouping)
//! when it carries only a `task_id`. A candidate with neither, or with
//! no `actor.role` at all, has no derivable partition key and is
//! refused (`malformed-evidence`) — never silently assigned an
//! arbitrary one.
//!
//! # Why `staging`/`committed` are two separate [`GitTier`] roots
//! `GitTier::read`'s scan walks `<root>/kind={kind}/` RECURSIVELY
//! (`walkdir::WalkDir`) — unlike parity.py's own fixed-depth
//! `Path.glob`, which naturally excludes a `_staging/` directory one
//! path segment deeper than its glob pattern by construction. Nesting
//! staging inside the SAME `kind=evidence_record/` subtree a committed
//! [`GitTier`] scans would make staging candidates visible to
//! `committed.read()` by accident. The caller instead roots `staging`
//! at a SEPARATE directory — by convention
//! `GitTier::new(ledger_root.join("_staging"))` — keeping the two
//! subtrees disjoint under any recursive walk, while both still
//! resolve every kind's Hive layout identically
//! (`canon_store::partition::expected_relative_path` is a pure
//! function of `(kind, json)`, independent of which root it is
//! interpreted under).
//!
//! # Re-validation: the SAME check the gate applies, literally
//! `staging.read()` already runs every well-formedness check
//! `crate::context::GateContext::load` itself runs on the committed
//! ledger (layout self-consistency, then `canon_model::validate_evidence`
//! — the identical function, not a re-implementation): a structurally
//! malformed staging candidate lands in `staged.violations`, never
//! `staged.records`, and this module refuses it directly from that
//! list — it is IMPOSSIBLE for `promote` to accept a candidate the
//! gate's own read path would reject (mirrors parity.py's own
//! `_run_problems` re-validation guarantee, divergence-log.md §3.5:
//! "ONE validator guarantees `promote` never emits a committed run the
//! gate would reject, and never refuses one the gate would accept").
//!
//! # A present-malformed native field is refused UPSTREAM, at `staging.read()`
//! `lifecycle`/`flagged`/`evidence_sha`/`surface_ref`/`run_seq` are now
//! `EvidenceRecord`'s own native, typed fields (s15 P1/D9) — a
//! present-malformed one fails `canon_model::validate_evidence`'s full
//! `EvidenceRecord::deserialize` (the SAME check `staging.read()` already
//! runs, paragraph above), so it lands in `staged.violations`, never
//! `staged.records`, and is refused by THIS module's very first loop
//! (over `staged.violations`) before candidate processing even begins —
//! never a second, redundant re-check inside the per-candidate loop (an
//! earlier revision's `trust_ladder_tag_of` re-check is gone: it is now
//! structurally unreachable, since no present-malformed native field can
//! ever survive into `candidates` at all). The gate's own
//! `crate::trust::TrustLadderCheck` never even sees such a record
//! (`ctx.evidence` excludes it too) — the SAME "promote never emits a
//! run the gate would reject" guarantee two paragraphs up already states,
//! now enforced at ONE validation point instead of two.
//!
//! # Extended to `Divergence` (s15 P3b, design D10)
//! [`promote`] stays hardcoded to `EvidenceRecord` — its `(role, surface)`
//! axis is UNCHANGED. `Divergence` gets its OWN, separate promote path
//! ([`promote_divergence`]/[`commit_divergence`]) rather than a shared
//! generic function, because its staging shape is NOT a full `Divergence`
//! record: `Divergence.run_seq: TotalOrder` stays REQUIRED (the committed-
//! record invariant [`crate::fold`]'s ordering depends on), so a staged,
//! run_seq-less candidate cannot even deserialize as one. [`DivergenceCandidate`]
//! is the run_seq-less staging shape ("a staging JSONL-equivalent, NOT a
//! committed Divergence record" — design D10's own framing); staging
//! candidates live as flat, content-digest-named JSON files under
//! `<ledger_root>/_staging_divergence/` ([`stage_divergence`]) — NEVER
//! through [`GitTier`]'s kind=/area= Hive layout, since
//! `canon_store::partition::resolve_partition`'s `Divergence` arm
//! requires `run_seq` to compute even the natural key, and a staged
//! candidate has none yet. [`promote_divergence`] partitions run_seq by
//! `(project_id, role, surface)` — `project_id` from the REQUIRED field,
//! `role` from the writing actor, `surface` from `scenario_id.surface_key()`
//! — batch-promoting every staged candidate, mirroring [`promote`]'s own
//! shape. `canon divergence resolve`/`defer` (native-verdict-lifecycle
//! spec) go through [`commit_divergence`] instead — a single-candidate
//! direct commit that never touches the batch staging directory at all,
//! so a routine resolve/defer can never accidentally promote an unrelated
//! candidate a reviewer is still mid-`stage`ing.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use canon_model::{Divergence, DivergenceStatus, EvidenceRecord, Envelope, ProjectId, RawRecord, RecordKind, RoleId, ScenarioId, Sha, TotalOrder};
use canon_store::git_tier::GitTier;
use canon_store::partition::{content_digest12, expected_relative_path};
use canon_store::tier::{RawWrite, StoreError, Tier, TierQuery};
use serde::{Deserialize, Serialize};

use crate::failure_class::{FailureClass, Violation};

/// One candidate successfully promoted from `_staging/` to the
/// committed ledger — the `run_seq` this call assigned it, and the
/// `(role, surface)` partition key that `run_seq` is monotonic within
/// (module doc).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Promoted {
    pub role: RoleId,
    pub surface: String,
    pub run_seq: u64,
    /// The committed-tier-relative path the record now lives at —
    /// content-derived, resolved AFTER `run_seq` is stamped onto the
    /// body (stamping changes the content-digest suffix, so this is
    /// never the same path the staging copy resolved to).
    pub target: PathBuf,
}

/// One staging candidate refused promotion — no `run_seq` was ever
/// consumed for it (design decision 5: "refuses without consuming a
/// `run_seq`"). The underlying staging file is left in place (never
/// deleted), so a reviewer can see and fix it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Refused {
    pub violation: Violation,
}

/// One [`promote`] call's outcome — every candidate lands in exactly
/// one of `promoted`/`refused`, never both, and refusal never shrinks
/// or reorders another candidate's assigned `run_seq`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PromoteReport {
    pub promoted: Vec<Promoted>,
    pub refused: Vec<Refused>,
}

impl PromoteReport {
    pub fn is_clean(&self) -> bool {
        self.refused.is_empty()
    }
}

/// This candidate's `(role, surface)` run_seq-partition key (module
/// doc). `None` when neither a role nor a derivable surface exists —
/// such a candidate cannot be promoted under this scheme.
fn partition_key(record: &EvidenceRecord) -> Option<(RoleId, String)> {
    let role = record.envelope.actor.role.clone()?;
    let surface = record
        .scenario_id
        .as_ref()
        .map(|scenario_id| scenario_id.surface_key())
        .or_else(|| record.task_id.as_ref().map(|task_id| task_id.change_id().to_string()))?;
    Some((role, surface))
}

/// This record's `Violation::subject` — `task_id` preferred, then
/// `scenario_id`, then `run_id`, matching `crate::trust::subject_of`'s
/// own preference order.
fn subject_of(record: &EvidenceRecord) -> String {
    if let Some(task_id) = &record.task_id {
        task_id.to_string()
    } else if let Some(scenario_id) = &record.scenario_id {
        scenario_id.to_string()
    } else if let Some(run_id) = &record.run_id {
        run_id.to_string()
    } else {
        "<unscoped>".to_string()
    }
}

fn refuse(subject: impl Into<String>, detail: impl Into<String>) -> Refused {
    Refused { violation: Violation::new(FailureClass::MalformedEvidence, subject, detail) }
}

/// Promote every well-formed `_staging/` candidate to the committed
/// ledger (module doc). `dry_run` computes and returns the FULL plan
/// (assigned `run_seq`, target path) WITHOUT touching disk —
/// `canon gate promote --dry-run`'s printer (task 2.3, CLI wave, not
/// implemented here) is the intended caller of that mode; this
/// function only guarantees the plan itself is side-effect free.
pub fn promote(staging: &GitTier, committed: &GitTier, dry_run: bool) -> Result<PromoteReport, StoreError> {
    let mut report = PromoteReport::default();

    let staged = staging.read(&TierQuery::kind(RecordKind::EvidenceRecord))?;

    // Malformed/misfiled staging candidates never reach run_seq
    // assignment at all (module doc's re-validation guarantee).
    for violation in staged.violations {
        report.refused.push(refuse(violation.subject, violation.detail));
    }

    // `1 + max(run_seq)` per (role, surface), scanning only the
    // committed tier's own already-landed records (parity.py
    // `_next_run_seq`) — `next_seq` holds the HIGHEST run_seq assigned
    // so far per key; each successful promotion below bumps it by
    // exactly one, so N candidates for the same key in one call get N
    // distinct sequential values without re-scanning disk per
    // candidate.
    let committed_read = committed.read(&TierQuery::kind(RecordKind::EvidenceRecord))?;
    let mut next_seq: HashMap<(RoleId, String), u64> = HashMap::new();
    for raw in &committed_read.records {
        let Ok(record) = serde_json::from_value::<EvidenceRecord>(raw.0.clone()) else { continue };
        let Some(key) = partition_key(&record) else { continue };
        if let Some(seq) = raw.0.get("run_seq").and_then(serde_json::Value::as_u64) {
            let slot = next_seq.entry(key).or_insert(0);
            *slot = (*slot).max(seq);
        }
    }

    // Parse every well-formed staging candidate up front so processing
    // order is deterministic (same staging set -> same run_seq
    // assignment every run) rather than filesystem-walk order.
    let mut candidates: Vec<(EvidenceRecord, RawRecord)> = Vec::new();
    for raw in staged.records {
        match serde_json::from_value::<EvidenceRecord>(raw.0.clone()) {
            Ok(record) => candidates.push((record, raw)),
            Err(e) => {
                // Unreachable in practice — `staging.read()` already
                // validated this exact JSON as an EvidenceRecord a few
                // lines above — but §7 forbids assuming that instead
                // of handling it.
                report.refused.push(refuse("<staging>", format!("candidate re-parse failed after passing staging.read()'s own validation: {e}")));
            }
        }
    }
    candidates.sort_by_key(|(a, _)| subject_of(a));

    for (record, raw) in candidates {
        let subject = subject_of(&record);

        let Some((role, surface)) = partition_key(&record) else {
            report.refused.push(refuse(
                subject,
                "no derivable (role, surface) run_seq partition key: record carries no `actor.role`, or neither a `scenario_id` nor a `task_id`",
            ));
            continue;
        };

        let seq = {
            let slot = next_seq.entry((role.clone(), surface.clone())).or_insert(0);
            *slot += 1;
            *slot
        };

        let mut body = raw.0.clone();
        body.as_object_mut().expect("an EvidenceRecord's raw body is always a JSON object").insert("run_seq".to_string(), serde_json::json!(seq));

        let target = expected_relative_path(RecordKind::EvidenceRecord, &body).map_err(StoreError::Layout)?;

        if !dry_run {
            committed.write(&RawWrite(RawRecord(body)))?;
            let staging_relative = expected_relative_path(RecordKind::EvidenceRecord, &raw.0).map_err(StoreError::Layout)?;
            std::fs::remove_file(staging.root().join(&staging_relative))?;
        }

        report.promoted.push(Promoted { role, surface, run_seq: seq, target });
    }

    Ok(report)
}

/// A staged `Divergence` candidate — every `Divergence` field except
/// `run_seq` (module doc: `Divergence.run_seq` stays REQUIRED, so a
/// run_seq-less candidate cannot deserialize as one). [`stage_divergence`]
/// writes one of these; [`promote_divergence`]/[`commit_divergence`]
/// assign the monotonic `run_seq` and construct the committed
/// [`Divergence`] only at that point.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DivergenceCandidate {
    #[serde(flatten)]
    pub envelope: Envelope,
    pub project_id: ProjectId,
    pub scenario_id: ScenarioId,
    pub sha: Sha,
    pub status: DivergenceStatus,
    pub round: u32,
    pub reviewer: String,
    #[serde(default, skip_serializing_if = "String::is_empty")]
    pub detail: String,
}

/// `<ledger_root>/_staging_divergence/` — a FLAT staging directory for
/// [`DivergenceCandidate`]s (module doc: never `GitTier`'s kind=/area=
/// Hive layout, since `resolve_partition`'s `Divergence` arm requires a
/// `run_seq` a staged candidate has none of yet).
pub fn divergence_staging_dir(ledger_root: &Path) -> PathBuf {
    ledger_root.join("_staging_divergence")
}

/// Stage one [`DivergenceCandidate`] — a content-digest-named JSON file
/// under `staging_dir` (module doc's digest-suffixed-uniqueness
/// convention, `canon_store::partition`'s own precedent), never through
/// `GitTier` (module doc). Returns the file's path.
pub fn stage_divergence(staging_dir: &Path, candidate: &DivergenceCandidate) -> std::io::Result<PathBuf> {
    std::fs::create_dir_all(staging_dir)?;
    let body = serde_json::to_value(candidate).expect("DivergenceCandidate always serializes");
    let digest = content_digest12(&body);
    let path = staging_dir.join(format!("{digest}.json"));
    std::fs::write(&path, serde_json::to_string_pretty(&body).expect("serde_json::Value always serializes"))?;
    Ok(path)
}

/// Every staged [`DivergenceCandidate`] under `staging_dir`, alongside
/// its file path (so a successful promotion can delete exactly that
/// file) — a malformed/unparseable staging file is reported as a
/// [`Refused`] directly (module doc's soft-skip / fail-loud split,
/// mirrored from `staging.read()`'s own `violations` for `EvidenceRecord`,
/// even though there is no `Tier` here to do it for us). A missing
/// `staging_dir` (nothing ever staged) is "zero candidates", never an
/// error.
fn read_staged_divergence(staging_dir: &Path) -> (Vec<(PathBuf, DivergenceCandidate)>, Vec<Refused>) {
    let mut candidates = Vec::new();
    let mut refused = Vec::new();

    let Ok(entries) = std::fs::read_dir(staging_dir) else {
        return (candidates, refused);
    };
    for entry in entries.filter_map(Result::ok) {
        let path = entry.path();
        if path.extension().and_then(|e| e.to_str()) != Some("json") {
            continue;
        }
        match std::fs::read_to_string(&path).ok().and_then(|s| serde_json::from_str::<DivergenceCandidate>(&s).ok()) {
            Some(candidate) => candidates.push((path, candidate)),
            None => refused.push(refuse("<staging-divergence>", format!("{}: malformed staged Divergence candidate", path.display()))),
        }
    }
    candidates.sort_by_key(|(_, c)| (c.project_id.clone(), c.scenario_id.clone(), c.round));
    (candidates, refused)
}

/// This candidate's `(project_id, role, surface)` run_seq-partition key
/// (module doc, design D10) — `project_id` from the REQUIRED field,
/// `role` from the writing actor, `surface` from
/// `scenario_id.surface_key()`. `None` when the candidate carries no
/// `actor.role` at all — such a candidate cannot be promoted under this
/// scheme.
fn divergence_partition_key(project_id: &ProjectId, role: Option<&RoleId>, scenario_id: &ScenarioId) -> Option<(ProjectId, RoleId, String)> {
    Some((project_id.clone(), role?.clone(), scenario_id.surface_key()))
}

/// `1 + max(run_seq)` per `(project_id, role, surface)`, scanning only
/// `committed`'s own already-landed `Divergence` records (mirrors
/// [`promote`]'s own `next_seq` scan).
fn scan_divergence_next_seq(committed: &GitTier) -> Result<HashMap<(ProjectId, RoleId, String), u64>, StoreError> {
    let committed_read = committed.read(&TierQuery::kind(RecordKind::Divergence))?;
    let mut next_seq: HashMap<(ProjectId, RoleId, String), u64> = HashMap::new();
    for raw in &committed_read.records {
        let Ok(record) = serde_json::from_value::<Divergence>(raw.0.clone()) else { continue };
        let Some(key) = divergence_partition_key(&record.project_id, record.envelope.actor.role.as_ref(), &record.scenario_id) else { continue };
        if let Some(seq) = raw.0.get("run_seq").and_then(serde_json::Value::as_u64) {
            let slot = next_seq.entry(key).or_insert(0);
            *slot = (*slot).max(seq);
        }
    }
    Ok(next_seq)
}

/// Commit one [`DivergenceCandidate`] to `committed`, assigning it the
/// next `run_seq` within its `(project_id, role, surface)` partition
/// (`next_seq` tracks the running max across a whole batch — module
/// doc's [`promote`] precedent). `Ok(Err(Refused))` for a candidate with
/// a mismatched `kind` or no derivable partition key, never a panic;
/// refusal never consumes a `run_seq` (design D10).
fn commit_divergence_candidate(
    candidate: &DivergenceCandidate,
    committed: &GitTier,
    next_seq: &mut HashMap<(ProjectId, RoleId, String), u64>,
    dry_run: bool,
) -> Result<Result<Promoted, Refused>, StoreError> {
    let subject = candidate.scenario_id.to_string();

    if candidate.envelope.kind != RecordKind::Divergence {
        return Ok(Err(refuse(subject, format!("staged candidate carries `kind={}`, not `divergence`", candidate.envelope.kind.as_str()))));
    }

    let Some(key) = divergence_partition_key(&candidate.project_id, candidate.envelope.actor.role.as_ref(), &candidate.scenario_id) else {
        return Ok(Err(refuse(subject, "no derivable (project_id, role, surface) run_seq partition key: candidate carries no `actor.role`")));
    };

    let seq = {
        let slot = next_seq.entry(key.clone()).or_insert(0);
        *slot += 1;
        *slot
    };

    let divergence = Divergence::new(
        candidate.envelope.clone(),
        candidate.project_id.clone(),
        candidate.scenario_id.clone(),
        candidate.sha.clone(),
        candidate.status.clone(),
        TotalOrder::new(seq),
        candidate.round,
        candidate.reviewer.clone(),
        candidate.detail.clone(),
    );

    let target = expected_relative_path(RecordKind::Divergence, &serde_json::to_value(&divergence).expect("Divergence always serializes")).map_err(StoreError::Layout)?;

    if !dry_run {
        committed.write(&divergence)?;
    }

    Ok(Ok(Promoted { role: key.1, surface: key.2, run_seq: seq, target }))
}

/// Batch-promote every staged [`DivergenceCandidate`] under `staging_dir`
/// to `committed` (module doc, design D10) — mirrors [`promote`]'s own
/// shape, over the Divergence-specific staging representation.
pub fn promote_divergence(staging_dir: &Path, committed: &GitTier, dry_run: bool) -> Result<PromoteReport, StoreError> {
    let mut report = PromoteReport::default();

    let (candidates, malformed) = read_staged_divergence(staging_dir);
    report.refused.extend(malformed);

    let mut next_seq = scan_divergence_next_seq(committed)?;

    for (path, candidate) in candidates {
        match commit_divergence_candidate(&candidate, committed, &mut next_seq, dry_run)? {
            Ok(promoted) => {
                if !dry_run {
                    std::fs::remove_file(&path)?;
                }
                report.promoted.push(promoted);
            }
            Err(refused) => report.refused.push(refused),
        }
    }

    Ok(report)
}

/// `canon divergence resolve`/`defer`'s own direct-commit convenience
/// path (module doc): assign the next run_seq and commit ONE candidate
/// directly, without touching the batch `_staging_divergence/` directory
/// at all — a routine resolve/defer never risks promoting a DIFFERENT,
/// unrelated candidate a reviewer is still mid-`stage`ing.
pub fn commit_divergence(candidate: &DivergenceCandidate, committed: &GitTier) -> Result<Result<Promoted, Refused>, StoreError> {
    let mut next_seq = scan_divergence_next_seq(committed)?;
    commit_divergence_candidate(candidate, committed, &mut next_seq, false)
}

#[cfg(test)]
mod tests {
    use canon_model::{Actor, Envelope, EvidenceVerdict, ScenarioId};
    use tempfile::TempDir;

    use super::*;

    fn tiers(dir: &TempDir) -> (GitTier, GitTier) {
        let ledger_root = dir.path().join("canon").join("ledger");
        (GitTier::new(ledger_root.join("_staging")), GitTier::new(ledger_root))
    }

    fn evidence(role: &str, scenario_id: Option<&str>) -> EvidenceRecord {
        EvidenceRecord::new(
            Envelope::new(1, RecordKind::EvidenceRecord, chrono::Utc::now(), Actor::new("test-agent", RoleId::parse(role).unwrap())),
            None,
            scenario_id.map(|s| ScenarioId::parse(s).unwrap()),
            None,
            EvidenceVerdict::Faithful,
        )
    }

    #[test]
    fn promote_assigns_monotonic_gap_free_run_seq_within_one_invocation() {
        let dir = TempDir::new().unwrap();
        let (staging, committed) = tiers(&dir);

        // Same (area, surface) -> same surface_key(), different `nn`.
        let a = ScenarioId::parse("world.firstbuy-hotdeal.14").unwrap();
        let b = ScenarioId::parse("world.firstbuy-hotdeal.26").unwrap();
        assert_eq!(a.surface_key(), b.surface_key());

        staging.write(&evidence("implementer", Some("world.firstbuy-hotdeal.14"))).unwrap();
        staging.write(&evidence("implementer", Some("world.firstbuy-hotdeal.26"))).unwrap();

        let report = promote(&staging, &committed, false).unwrap();
        assert!(report.is_clean(), "refused: {:?}", report.refused);
        assert_eq!(report.promoted.len(), 2);
        let mut seqs: Vec<u64> = report.promoted.iter().map(|p| p.run_seq).collect();
        seqs.sort_unstable();
        assert_eq!(seqs, vec![1, 2], "no gaps, strictly increasing");

        // Staging is now empty (both candidates landed + were deleted).
        assert!(staging.read(&TierQuery::kind(RecordKind::EvidenceRecord)).unwrap().records.is_empty());
        // Both committed files exist, each carrying its own run_seq.
        let landed = committed.read(&TierQuery::kind(RecordKind::EvidenceRecord)).unwrap();
        assert_eq!(landed.records.len(), 2);

        // A THIRD candidate for the SAME (role, surface), promoted in a
        // SEPARATE invocation, continues from the committed max — never
        // restarts at 1.
        staging.write(&evidence("implementer", Some("world.firstbuy-hotdeal.33"))).unwrap();
        let report2 = promote(&staging, &committed, false).unwrap();
        assert_eq!(report2.promoted.len(), 1);
        assert_eq!(report2.promoted[0].run_seq, 3);
    }

    #[test]
    fn promote_refuses_a_malformed_candidate_without_consuming_a_run_seq() {
        let dir = TempDir::new().unwrap();
        let (staging, committed) = tiers(&dir);

        // Malformed: missing the required `verdict` field.
        let malformed = serde_json::json!({
            "schema": 1,
            "kind": "evidence_record",
            "at": chrono::Utc::now().to_rfc3339(),
            "actor": {"agent_id": "agent-x", "role": "implementer"},
            "scenario_id": "world.firstbuy-hotdeal.40",
        });
        staging.write(&RawWrite(RawRecord(malformed))).unwrap();

        // A well-formed sibling for the SAME (role, surface).
        staging.write(&evidence("implementer", Some("world.firstbuy-hotdeal.41"))).unwrap();

        let report = promote(&staging, &committed, false).unwrap();
        assert_eq!(report.promoted.len(), 1);
        assert_eq!(report.promoted[0].run_seq, 1, "the malformed sibling must not have consumed run_seq 1");
        assert_eq!(report.refused.len(), 1);
        assert_eq!(report.refused[0].violation.class, FailureClass::MalformedEvidence);

        // The malformed staging file is left in place, never committed;
        // the well-formed one is gone (promoted + deleted from staging).
        let staged_after = staging.read(&TierQuery::kind(RecordKind::EvidenceRecord)).unwrap();
        assert!(staged_after.records.is_empty());
        assert_eq!(staged_after.violations.len(), 1);
        assert_eq!(committed.read(&TierQuery::kind(RecordKind::EvidenceRecord)).unwrap().records.len(), 1);
    }

    #[test]
    fn promote_refuses_a_staged_record_with_a_present_malformed_native_field() {
        let dir = TempDir::new().unwrap();
        let (staging, committed) = tiers(&dir);

        // A present-malformed native `lifecycle` field fails
        // `EvidenceRecord`'s own `Deserialize` — `staging.read()`
        // catches it at the SAME point `GateContext::load` would,
        // landing it in `staged.violations`, never `staged.records`
        // (module doc: no more per-candidate re-check needed).
        let malformed = serde_json::json!({
            "schema": 1,
            "kind": "evidence_record",
            "at": chrono::Utc::now().to_rfc3339(),
            "actor": {"agent_id": "agent-x", "role": "implementer"},
            "scenario_id": "world.firstbuy-hotdeal.80",
            "verdict": "faithful",
            "lifecycle": "bogus-lifecycle",
        });
        staging.write(&RawWrite(RawRecord(malformed))).unwrap();

        // A well-formed sibling for the SAME (role, surface).
        staging.write(&evidence("implementer", Some("world.firstbuy-hotdeal.81"))).unwrap();

        let report = promote(&staging, &committed, false).unwrap();
        assert_eq!(report.promoted.len(), 1, "promoted: {:?}", report.promoted);
        assert_eq!(report.promoted[0].run_seq, 1, "the malformed sibling must not have consumed run_seq 1");
        assert_eq!(report.refused.len(), 1);
        assert_eq!(report.refused[0].violation.class, FailureClass::MalformedEvidence);

        // The malformed candidate is caught at `staging.read()` time —
        // it lands in `staged.violations`, so it never even reaches
        // `staged.records`, and stays on disk (never deleted, never
        // committed).
        let staged_after = staging.read(&TierQuery::kind(RecordKind::EvidenceRecord)).unwrap();
        assert!(staged_after.records.is_empty());
        assert_eq!(staged_after.violations.len(), 1);
        assert_eq!(committed.read(&TierQuery::kind(RecordKind::EvidenceRecord)).unwrap().records.len(), 1);
    }

    #[test]
    fn promote_refuses_a_candidate_with_no_derivable_partition_key() {
        let dir = TempDir::new().unwrap();
        let (staging, committed) = tiers(&dir);

        // Well-formed EvidenceRecord, but no `actor.role` at all.
        let record = EvidenceRecord::new(
            Envelope::new(1, RecordKind::EvidenceRecord, chrono::Utc::now(), Actor::new_unattributed("legacy-writer")),
            None,
            Some(ScenarioId::parse("world.firstbuy-hotdeal.50").unwrap()),
            None,
            EvidenceVerdict::Faithful,
        );
        staging.write(&record).unwrap();

        let report = promote(&staging, &committed, false).unwrap();
        assert!(report.promoted.is_empty());
        assert_eq!(report.refused.len(), 1);
        assert_eq!(report.refused[0].violation.class, FailureClass::MalformedEvidence);
        assert!(committed.read(&TierQuery::kind(RecordKind::EvidenceRecord)).unwrap().records.is_empty());
    }

    #[test]
    fn dry_run_computes_the_plan_without_writing_or_deleting() {
        let dir = TempDir::new().unwrap();
        let (staging, committed) = tiers(&dir);
        staging.write(&evidence("implementer", Some("world.firstbuy-hotdeal.60"))).unwrap();

        let report = promote(&staging, &committed, true).unwrap();
        assert_eq!(report.promoted.len(), 1);
        assert_eq!(report.promoted[0].run_seq, 1);

        assert!(committed.read(&TierQuery::kind(RecordKind::EvidenceRecord)).unwrap().records.is_empty(), "dry-run must not write");
        assert_eq!(staging.read(&TierQuery::kind(RecordKind::EvidenceRecord)).unwrap().records.len(), 1, "dry-run must not delete");
    }

    #[test]
    fn distinct_surfaces_get_independent_run_seq_sequences() {
        let dir = TempDir::new().unwrap();
        let (staging, committed) = tiers(&dir);
        staging.write(&evidence("implementer", Some("world.firstbuy-hotdeal.70"))).unwrap();
        staging.write(&evidence("implementer", Some("place.lock.71"))).unwrap();

        let report = promote(&staging, &committed, false).unwrap();
        assert!(report.is_clean(), "refused: {:?}", report.refused);
        assert_eq!(report.promoted.len(), 2);
        // Different surfaces -> both independently start at run_seq 1.
        assert!(report.promoted.iter().all(|p| p.run_seq == 1));
        let surfaces: std::collections::HashSet<&str> = report.promoted.iter().map(|p| p.surface.as_str()).collect();
        assert_eq!(surfaces.len(), 2);
    }

    fn divergence_candidate(project_id: &str, scenario_id: &str, role: &str, status: DivergenceStatus, round: u32) -> DivergenceCandidate {
        DivergenceCandidate {
            envelope: Envelope::new(1, RecordKind::Divergence, chrono::Utc::now(), Actor::new("reviewer-1", RoleId::parse(role).unwrap())),
            project_id: ProjectId::parse(project_id).unwrap(),
            scenario_id: ScenarioId::parse(scenario_id).unwrap(),
            sha: Sha::parse("a".repeat(40)).unwrap(),
            status,
            round,
            reviewer: "reviewer-1".to_string(),
            detail: String::new(),
        }
    }

    #[test]
    fn divergence_stage_then_promote_assigns_a_monotonic_run_seq() {
        let dir = TempDir::new().unwrap();
        let ledger_root = dir.path().join("canon").join("ledger");
        let staging_dir = divergence_staging_dir(&ledger_root);
        let committed = GitTier::new(&ledger_root);

        stage_divergence(&staging_dir, &divergence_candidate("app-a", "world.firstbuy-hotdeal.14", "reviewer", DivergenceStatus::Open, 1)).unwrap();
        stage_divergence(&staging_dir, &divergence_candidate("app-a", "world.firstbuy-hotdeal.26", "reviewer", DivergenceStatus::Open, 1)).unwrap();

        let report = promote_divergence(&staging_dir, &committed, false).unwrap();
        assert!(report.is_clean(), "refused: {:?}", report.refused);
        assert_eq!(report.promoted.len(), 2);
        let mut seqs: Vec<u64> = report.promoted.iter().map(|p| p.run_seq).collect();
        seqs.sort_unstable();
        assert_eq!(seqs, vec![1, 2], "no gaps, strictly increasing within one (project_id, role, surface) partition");

        // Staging is empty afterward; both committed Divergence records exist.
        assert!(std::fs::read_dir(&staging_dir).unwrap().next().is_none());
        assert_eq!(committed.read(&TierQuery::kind(RecordKind::Divergence)).unwrap().records.len(), 2);

        // A THIRD candidate in the SAME partition, staged+promoted in a
        // SEPARATE call, continues from the committed max — never restarts.
        stage_divergence(&staging_dir, &divergence_candidate("app-a", "world.firstbuy-hotdeal.33", "reviewer", DivergenceStatus::Open, 1)).unwrap();
        let report2 = promote_divergence(&staging_dir, &committed, false).unwrap();
        assert_eq!(report2.promoted.len(), 1);
        assert_eq!(report2.promoted[0].run_seq, 3);
    }

    #[test]
    fn divergence_promote_refuses_a_malformed_candidate_without_consuming_a_run_seq() {
        let dir = TempDir::new().unwrap();
        let ledger_root = dir.path().join("canon").join("ledger");
        let staging_dir = divergence_staging_dir(&ledger_root);
        let committed = GitTier::new(&ledger_root);
        std::fs::create_dir_all(&staging_dir).unwrap();

        // Malformed: not even valid JSON for a DivergenceCandidate (missing required fields).
        std::fs::write(staging_dir.join("malformed.json"), serde_json::to_string(&serde_json::json!({"not": "a candidate"})).unwrap()).unwrap();

        stage_divergence(&staging_dir, &divergence_candidate("app-a", "world.firstbuy-hotdeal.41", "reviewer", DivergenceStatus::Open, 1)).unwrap();

        let report = promote_divergence(&staging_dir, &committed, false).unwrap();
        assert_eq!(report.promoted.len(), 1, "promoted: {:?}", report.promoted);
        assert_eq!(report.promoted[0].run_seq, 1, "the malformed sibling must not have consumed run_seq 1");
        assert_eq!(report.refused.len(), 1);
        assert_eq!(report.refused[0].violation.class, FailureClass::MalformedEvidence);
    }

    #[test]
    fn divergence_promote_refuses_a_candidate_with_no_derivable_partition_key() {
        let dir = TempDir::new().unwrap();
        let ledger_root = dir.path().join("canon").join("ledger");
        let staging_dir = divergence_staging_dir(&ledger_root);
        let committed = GitTier::new(&ledger_root);

        let mut candidate = divergence_candidate("app-a", "world.firstbuy-hotdeal.50", "reviewer", DivergenceStatus::Open, 1);
        candidate.envelope.actor = Actor::new_unattributed("legacy-writer");
        stage_divergence(&staging_dir, &candidate).unwrap();

        let report = promote_divergence(&staging_dir, &committed, false).unwrap();
        assert!(report.promoted.is_empty());
        assert_eq!(report.refused.len(), 1);
        assert_eq!(report.refused[0].violation.class, FailureClass::MalformedEvidence);
        assert!(committed.read(&TierQuery::kind(RecordKind::Divergence)).unwrap().records.is_empty());
    }

    #[test]
    fn divergence_promotion_partitions_run_seq_by_project_id_role_surface() {
        let dir = TempDir::new().unwrap();
        let ledger_root = dir.path().join("canon").join("ledger");
        let staging_dir = divergence_staging_dir(&ledger_root);
        let committed = GitTier::new(&ledger_root);

        // Same (role, surface_key), DIFFERENT project_id — independent sequences.
        stage_divergence(&staging_dir, &divergence_candidate("app-a", "world.firstbuy-hotdeal.14", "reviewer", DivergenceStatus::Open, 1)).unwrap();
        stage_divergence(&staging_dir, &divergence_candidate("app-b", "world.firstbuy-hotdeal.26", "reviewer", DivergenceStatus::Open, 1)).unwrap();

        let report = promote_divergence(&staging_dir, &committed, false).unwrap();
        assert!(report.is_clean(), "refused: {:?}", report.refused);
        assert_eq!(report.promoted.len(), 2);
        assert!(report.promoted.iter().all(|p| p.run_seq == 1), "different project_id partitions both independently start at run_seq 1");
    }

    #[test]
    fn commit_divergence_direct_commit_never_touches_the_staging_directory() {
        let dir = TempDir::new().unwrap();
        let ledger_root = dir.path().join("canon").join("ledger");
        let staging_dir = divergence_staging_dir(&ledger_root);
        let committed = GitTier::new(&ledger_root);

        // A candidate a reviewer is still mid-`stage`ing, untouched by
        // a SEPARATE `resolve`/`defer` direct commit below.
        stage_divergence(&staging_dir, &divergence_candidate("app-a", "world.firstbuy-hotdeal.14", "reviewer", DivergenceStatus::Open, 1)).unwrap();

        let resolved = divergence_candidate("app-a", "world.firstbuy-hotdeal.99", "reviewer", DivergenceStatus::Resolved, 1);
        let outcome = commit_divergence(&resolved, &committed).unwrap();
        let promoted = outcome.expect("resolve candidate should promote cleanly");
        assert_eq!(promoted.run_seq, 1);

        // The unrelated staged candidate is still sitting there, untouched.
        assert_eq!(std::fs::read_dir(&staging_dir).unwrap().count(), 1);
        assert_eq!(committed.read(&TierQuery::kind(RecordKind::Divergence)).unwrap().records.len(), 1);
    }
}
