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
//! # Retry after an INTERRUPTED promotion is recovery, never a duplicate
//! Each candidate's two side effects — `committed.write` then
//! `remove_file(<staging source>)` — are two syscalls with no
//! transaction around them, and no filesystem gives us one. So the
//! window is real: the committed record lands, the removal fails (or the
//! process dies), and the staging file survives. Before s42
//! (`close-the-open-loops`)'s review found this, the retry that
//! operators are explicitly told to run then re-scanned `committed`,
//! saw the higher partition maximum, and appended the SAME attestation
//! again at the next `run_seq` — permanently, into an append-only
//! ledger, once per retry. A best-effort "try harder to delete" would
//! not have fixed it; the duplicate came from the retry being unable to
//! RECOGNIZE its own earlier work.
//!
//! So a candidate carries a stable identity across the window:
//! `staging_candidate_id`, the content digest of its staged body — the
//! same digest its staging filename already ends in, promoted from an
//! implicit path detail to an explicit `staging_id` field
//! (`committed_body` stamps it alongside `run_seq`). [`promote`] indexes
//! `committed` by that field, and a candidate whose identity is already
//! committed is treated as COMPLETE: no second write, no `run_seq`
//! consumed, the staging source drained, and the outcome reported in
//! [`PromoteReport::recovered`] rather than [`PromoteReport::promoted`],
//! carrying the `run_seq` the interrupted call already assigned. Retrying
//! until it succeeds is therefore safe by construction, and the number of
//! committed records equals the number of distinct staged bodies no matter
//! how many times the removal failed.
//!
//! Identity is the STAGED body, before `run_seq`/`staging_id` are stamped
//! on, so it is computable from the surviving staging file alone — the
//! only artifact a retry still has. Two byte-identical staged bodies
//! share one identity because they already share one staging FILE (the
//! digest is that filename); distinct attestations differ in `at` and so
//! never collide (`canon evidence add`'s own "not idempotent,
//! deliberately" contract).
//!
//! # WHICH kinds are drained is DATA ([`STAGED_KINDS`]), not a match arm
//! [`promote`] drains every kind registered in [`STAGED_KINDS`], and
//! branches only on what promotion has to ASSIGN that kind
//! ([`StagedAssignment`]) — the one axis on which its members actually
//! differ. A kind absent from that list is INVISIBLE to promotion: an
//! authoring command staging it would print "run `canon gate promote`"
//! while promote printed "nothing staged", and the record would sit
//! under `_staging/` forever, unreadable by every committed-tier query
//! — a surface asserting something no query computes, which is the
//! defect class s43 exists to stop. Registration is therefore the
//! whole cost of adding a staged kind.
//!
//! # `Finding` stages for RE-VALIDATION and batch atomicity, not identity
//! s43 registers `RecordKind::Finding` under
//! [`StagedAssignment::Nothing`], and the distinction matters to anyone
//! reading this after the `EvidenceRecord` section above.
//! `EvidenceRecord` MUST be staged: its `run_seq` does not exist until
//! promotion assigns it, so a record that skipped promotion would be
//! incomplete. `Finding`'s natural key is
//! `{change_id}__{round}__{seq}` — complete the moment it is authored,
//! with nothing for promotion to compute. So staging buys it two
//! different things: the re-validation guarantee two sections up (for
//! a `Finding`, `staging.read()` runs
//! `canon_store::partition::validate_body`, whose `Finding` arm is
//! `Finding::check_coherence` — the identical call the committed read
//! path makes), and batch atomicity for an author staging a whole
//! review round before committing any of it. Do NOT read
//! [`StagedAssignment::Nothing`] as a weaker `RunSeqPerRoleSurface`;
//! it is a kind that needs no identity assigned, not one whose
//! identity is skipped.
//!
//! # NOT extended to `Divergence` (s15 P3b, design D10)
//! `Divergence` is deliberately absent from [`STAGED_KINDS`] and keeps
//! its OWN, separate promote path
//! ([`promote_divergence`]/[`commit_divergence`]) rather than joining
//! the registry, because its staging shape is NOT a full `Divergence`
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
//!
//! That a third promotion path exists at all is a known wart, and s43
//! did not untangle it: folding a divergence candidate in would mean
//! reconciling a DIFFERENT staging representation, a different staging
//! directory, and a different CLI surface, which is its own change.
//! [`STAGED_KINDS`] is the seam that stops a FOURTH one appearing.

use std::collections::HashMap;
use std::path::{Path, PathBuf};

use canon_model::{Divergence, DivergenceStatus, EvidenceRecord, Envelope, ProjectId, RawRecord, RecordKind, RoleId, ScenarioId, Sha, TotalOrder};
use canon_store::git_tier::GitTier;
use canon_store::partition::{content_digest12, expected_relative_path};
use canon_store::tier::{RawWrite, StoreError, Tier, TierQuery};
use serde::{Deserialize, Serialize};

use crate::failure_class::{FailureClass, Violation};

/// One candidate that reached the committed ledger.
///
/// Reported under [`PromoteReport::promoted`] when THIS call wrote it,
/// and under [`PromoteReport::recovered`] when an earlier, interrupted
/// call already had (module doc's recovery section) — same facts either
/// way, so the two lists share one type.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Promoted {
    /// Which staged kind this candidate is. Reported so a batch that
    /// drained several kinds at once is readable PER KIND: "promoted
    /// evidence_record=1, finding=6" is a claim an operator can check
    /// against the ledger, where a bare "promoted 7" is not.
    pub kind: RecordKind,
    /// What promotion assigned this candidate — `None` for a kind it
    /// assigns nothing to ([`StagedAssignment::Nothing`]).
    ///
    /// `None` is not "unknown" and not "failed to derive": it is a kind
    /// that HAS no `run_seq` partition, and rendering a fabricated one
    /// would report a value promotion never computed.
    pub assigned: Option<RunSeqAssignment>,
    /// The committed-tier-relative path the record lives at —
    /// content-derived. Under [`StagedAssignment::RunSeqPerRoleSurface`]
    /// it is resolved AFTER `run_seq`/`staging_id` are stamped onto the
    /// body (stamping changes the content-digest suffix, so it is never
    /// the path the staging copy resolved to); under
    /// [`StagedAssignment::Nothing`] the body is unchanged, so it is the
    /// SAME relative path under both roots. For a RECOVERED candidate it
    /// is the path of the committed record actually found on disk.
    pub target: PathBuf,
}

/// The `run_seq` promotion assigned a candidate, together with the
/// `(role, surface)` partition that `run_seq` is monotonic within
/// (module doc's first section).
///
/// One struct rather than three [`Promoted`] fields because the three
/// co-vary absolutely: a `run_seq` without the partition it is
/// monotonic within is a meaningless number, and this makes that pair
/// unrepresentable rather than merely discouraged.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RunSeqAssignment {
    pub role: RoleId,
    pub surface: String,
    pub run_seq: u64,
}

impl Promoted {
    /// The `run_seq` promotion assigned, or `None` for a kind it
    /// assigns nothing to.
    pub fn run_seq(&self) -> Option<u64> {
        self.assigned.as_ref().map(|assigned| assigned.run_seq)
    }

    /// The `(role, surface)` partition's surface half, or `None` as
    /// above.
    pub fn surface(&self) -> Option<&str> {
        self.assigned.as_ref().map(|assigned| assigned.surface.as_str())
    }

    /// This candidate's operator-facing label: the kind, then the
    /// partition and `run_seq` only when promotion actually assigned
    /// one.
    ///
    /// One function so every printer renders an unassigned kind
    /// identically, instead of each inventing a placeholder
    /// (`run_seq=0`, `-/-`) for a value that does not exist — a
    /// placeholder in a promote line is indistinguishable from a real
    /// assignment to the operator reading it.
    pub fn label(&self) -> String {
        match &self.assigned {
            Some(assigned) => format!("{} {}/{} run_seq={}", self.kind.as_str(), assigned.role.as_str(), assigned.surface, assigned.run_seq),
            None => self.kind.as_str().to_string(),
        }
    }
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
/// one of `promoted`/`recovered`/`refused`, never two, and refusal never
/// shrinks or reorders another candidate's assigned `run_seq`.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct PromoteReport {
    /// Candidates THIS call wrote into the committed ledger.
    pub promoted: Vec<Promoted>,
    /// Candidates already committed under their own `staging_id` by an
    /// earlier call whose staging removal did not land (module doc's
    /// recovery section) — no second write, no `run_seq` consumed, the
    /// staging source drained. Not a failure: it is the retry doing
    /// exactly what it was run for, so it does not affect
    /// [`PromoteReport::is_clean`].
    ///
    /// Only [`promote`] populates this. [`promote_divergence`] has its own
    /// staging representation and its own (still unaddressed) window;
    /// this list stays empty there rather than pretending otherwise.
    pub recovered: Vec<Promoted>,
    pub refused: Vec<Refused>,
}

impl PromoteReport {
    /// Whether every candidate this call saw ended up committed —
    /// `refused` empty. A `recovered` candidate IS committed, so it never
    /// makes a report unclean: `canon gate promote`'s retry after an
    /// interrupted run has to be able to exit `0`, or the recovery is
    /// unusable.
    pub fn is_clean(&self) -> bool {
        self.refused.is_empty()
    }
}

/// The committed body's companion key carrying a promoted record's
/// originating staging identity (module doc's recovery section) — read
/// back by [`promote`] to recognize its own interrupted work.
///
/// A companion key, exactly like `evidence`/`evidence_note`:
/// `EvidenceRecord`'s own `Deserialize` is not
/// `deny_unknown_fields`, so it survives every read that re-parses a
/// committed body while staying invisible to the typed model — the
/// established "re-read the raw ledger JSON for a companion key" pattern
/// `crate::markers::evidence_note_of` and `crate::trust`'s
/// `trust_ladder`/`evidence_sha` readers already use.
const STAGING_ID_KEY: &str = "staging_id";

/// One staging candidate's stable identity: the content digest of its
/// STAGED body, before `run_seq`/`staging_id` are stamped on (module
/// doc's recovery section).
///
/// Deliberately the same `content_digest12` the candidate's own staging
/// FILENAME already ends in (`canon_store::partition`'s Hive object key)
/// — the identity is not a new invented token, it is the one the staging
/// layout already assigned, made explicit so a committed record can carry
/// it forward.
fn staging_candidate_id(staged_body: &serde_json::Value) -> String {
    content_digest12(staged_body)
}

/// The EXACT committed body a staged candidate promotes to: its staged
/// body plus `run_seq` and [`STAGING_ID_KEY`].
///
/// One function so the write path and the identity it records can never
/// disagree — the whole recovery guarantee rests on a later call being
/// able to find, by that field, the record an earlier call wrote here.
fn committed_body(staged_body: &serde_json::Value, run_seq: u64, staging_id: &str) -> serde_json::Value {
    let mut body = staged_body.clone();
    let object = body.as_object_mut().expect("an EvidenceRecord's raw body is always a JSON object");
    object.insert("run_seq".to_string(), serde_json::json!(run_seq));
    object.insert(STAGING_ID_KEY.to_string(), serde_json::Value::String(staging_id.to_string()));
    body
}

/// Where one already-committed `staging_id` landed — the `run_seq` the
/// interrupted call assigned it and the committed path it wrote.
///
/// A named struct rather than a `(u64, PathBuf)` pair: the index it lives
/// in is read back a hundred lines from where it is built, and both halves
/// are copied verbatim into a [`Promoted`] an operator reads as
/// `run_seq=N -> <path>`.
#[derive(Debug)]
struct RecoveredCommit {
    run_seq: u64,
    target: PathBuf,
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

/// What promotion has to ASSIGN a staged candidate before its body can
/// be committed — the only axis on which [`STAGED_KINDS`]'s members
/// differ, and therefore the only thing [`promote`] branches on.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StagedAssignment {
    /// A monotonic `run_seq` per `(role, surface)` (module doc's first
    /// section). This is WHY `EvidenceRecord` must be staged: the value
    /// does not exist until promotion computes it, so a record that
    /// skipped promotion would carry none.
    ///
    /// The scheme is `EvidenceRecord`-specific by construction — it
    /// reads `scenario_id`/`task_id`/`actor.role` off that concrete
    /// type — so this variant has exactly one member and adding a
    /// second would mean generalizing the partition key first.
    RunSeqPerRoleSurface,
    /// Nothing at all: the kind's natural key is complete the moment it
    /// is authored, so the committed body is the staged body byte for
    /// byte.
    ///
    /// Not a weaker `RunSeqPerRoleSurface` — a kind with nothing to
    /// assign, which still gets the two things staging buys regardless
    /// (module doc's `Finding` section): re-validation through the tier
    /// read path, and batch atomicity.
    Nothing,
}

/// One staged record kind [`promote`] drains, and what promotion owes
/// it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct StagedKind {
    pub kind: RecordKind,
    pub assignment: StagedAssignment,
}

/// Every kind `canon gate promote` drains from `<ledger_root>/_staging`
/// (module doc). DATA, so registering a kind is the whole cost of
/// making it promotable — and so the set of promotable kinds is
/// enumerable from one place by an operator, a diagnostic, and the
/// per-kind tally `canon gate promote` prints.
pub const STAGED_KINDS: [StagedKind; 2] = [
    StagedKind { kind: RecordKind::EvidenceRecord, assignment: StagedAssignment::RunSeqPerRoleSurface },
    // s43: a review finding. Natural key `{change_id}__{round}__{seq}`,
    // complete at authoring time — nothing for promotion to assign.
    StagedKind { kind: RecordKind::Finding, assignment: StagedAssignment::Nothing },
];

/// Promote every well-formed `_staging/` candidate of every kind in
/// [`STAGED_KINDS`] to the committed ledger (module doc). `dry_run`
/// computes and returns the FULL plan (assigned `run_seq`, target path)
/// WITHOUT touching disk — `canon gate promote --dry-run`'s printer is
/// the intended caller of that mode; this function only guarantees the
/// plan itself is side-effect free.
///
/// IDEMPOTENT across an interrupted call (module doc's recovery
/// section), under BOTH assignment strategies: a candidate already
/// committed by an earlier call is drained from staging and reported
/// under [`PromoteReport::recovered`], never written twice and never
/// consuming a `run_seq`. So `canon gate promote` may be retried until
/// it reports success, and the committed record count equals the number
/// of distinct staged bodies regardless of how many retries that took.
///
/// Kinds are drained in [`STAGED_KINDS`] order and each accumulates
/// into the SAME report, so one call's outcome is one report an
/// operator reads per kind ([`Promoted::kind`]) — never one report per
/// kind the caller has to remember to ask for.
pub fn promote(staging: &GitTier, committed: &GitTier, dry_run: bool) -> Result<PromoteReport, StoreError> {
    let mut report = PromoteReport::default();
    for staged_kind in STAGED_KINDS {
        match staged_kind.assignment {
            StagedAssignment::RunSeqPerRoleSurface => promote_with_run_seq(staging, committed, dry_run, &mut report)?,
            StagedAssignment::Nothing => promote_verbatim(staged_kind.kind, staging, committed, dry_run, &mut report)?,
        }
    }
    Ok(report)
}

/// Drain every staged candidate of a kind promotion assigns NOTHING to
/// ([`StagedAssignment::Nothing`]): re-validate, commit the staged body
/// UNCHANGED, delete the staging source.
///
/// No `run_seq`, no `staging_id` companion, and no recovery index —
/// none of which is a simplification of [`promote_with_run_seq`]'s
/// scheme so much as a consequence of not needing one. Because the
/// committed body IS the staged body, `expected_relative_path` (a pure
/// function of `(kind, json)`) resolves the SAME tier-relative path
/// under both roots, so an interrupted call's own work is recognizable
/// by that path alone. `staging_id` exists precisely because promotion
/// REWRITES an `EvidenceRecord` body and so cannot find it that way;
/// here there is nothing to rewrite.
///
/// The existence check is load-bearing rather than an optimization:
/// `GitTier::write` refuses an already-occupied path with
/// `StoreError::DuplicatePath`, so without it a retry after a failed
/// staging removal would abort the whole batch instead of completing it.
fn promote_verbatim(kind: RecordKind, staging: &GitTier, committed: &GitTier, dry_run: bool, report: &mut PromoteReport) -> Result<(), StoreError> {
    let staged = staging.read(&TierQuery::kind(kind))?;

    // The same re-validation guarantee [`promote_with_run_seq`] states:
    // `staging.read()` has already run
    // `canon_store::partition::validate_body` over every candidate —
    // for `Finding` that arm IS `Finding::check_coherence`, the
    // identical call the committed read path makes — so a candidate the
    // gate's own read would reject cannot reach the write below. A
    // malformed candidate is refused with its staging file left in
    // place, never deleted, so it can be fixed and re-promoted.
    for violation in staged.violations {
        report.refused.push(refuse(violation.subject, violation.detail));
    }

    // Paths resolved up front and sorted, so the same staging set drains
    // in the same order every run rather than in filesystem-walk order —
    // the determinism [`promote_with_run_seq`] gets from sorting by
    // subject, over the only total order this path has.
    let mut candidates: Vec<(PathBuf, RawRecord)> = Vec::with_capacity(staged.records.len());
    for raw in staged.records {
        candidates.push((expected_relative_path(kind, &raw.0).map_err(StoreError::Layout)?, raw));
    }
    candidates.sort_by(|(a, _), (b, _)| a.cmp(b));

    for (relative, raw) in candidates {
        let recovered = committed.root().join(&relative).exists();
        if !dry_run {
            if !recovered {
                committed.write(&RawWrite(raw))?;
            }
            // The same unprotected window as the run_seq path: if this
            // removal does not land, the record above is committed and
            // the staging file survives, and the next call takes the
            // recovery branch above rather than writing a second copy.
            std::fs::remove_file(staging.root().join(&relative))?;
        }
        let landed = Promoted { kind, assigned: None, target: relative };
        if recovered {
            report.recovered.push(landed);
        } else {
            report.promoted.push(landed);
        }
    }
    Ok(())
}

/// Drain every staged `EvidenceRecord`, assigning each the monotonic
/// per-`(role, surface)` `run_seq` it has none of until now
/// ([`StagedAssignment::RunSeqPerRoleSurface`]).
///
/// Hardcoded to `EvidenceRecord`, and honestly so: the partition key it
/// assigns within is read off that concrete type's own
/// `scenario_id`/`task_id`/`actor.role` fields, so this is not a
/// generic function with one caller — it is the one kind whose identity
/// promotion mints.
fn promote_with_run_seq(staging: &GitTier, committed: &GitTier, dry_run: bool, report: &mut PromoteReport) -> Result<(), StoreError> {
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
    // The SAME scan also indexes every committed record by the
    // `staging_id` it was promoted under (module doc's recovery section),
    // so the per-candidate loop below can recognize an attestation an
    // interrupted earlier call already committed instead of appending it a
    // second time. One pass, not two: the recovery index and the
    // partition maxima are both facts about the committed corpus.
    let mut already_committed: HashMap<String, RecoveredCommit> = HashMap::new();
    for raw in &committed_read.records {
        let Ok(record) = serde_json::from_value::<EvidenceRecord>(raw.0.clone()) else { continue };
        let Some(key) = partition_key(&record) else { continue };
        let Some(seq) = raw.0.get("run_seq").and_then(serde_json::Value::as_u64) else { continue };
        let slot = next_seq.entry(key).or_insert(0);
        *slot = (*slot).max(seq);

        let Some(staging_id) = raw.0.get(STAGING_ID_KEY).and_then(serde_json::Value::as_str) else { continue };
        // LOWEST `run_seq` wins when one identity somehow appears twice
        // (a corpus that already carries duplicates from before this
        // recovery existed): the earliest commit is the original, and
        // "earliest" is a total order over the data rather than an
        // artifact of directory-walk order.
        if already_committed.get(staging_id).is_some_and(|existing| existing.run_seq <= seq) {
            continue;
        }
        let target = expected_relative_path(RecordKind::EvidenceRecord, &raw.0).map_err(StoreError::Layout)?;
        already_committed.insert(staging_id.to_string(), RecoveredCommit { run_seq: seq, target });
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

        // The identity this candidate would be (or already was) committed
        // under — computed from the STAGED body, the only artifact a retry
        // after an interrupted call still has (module doc).
        let staging_id = staging_candidate_id(&raw.0);

        if let Some(existing) = already_committed.get(&staging_id) {
            // This exact attestation is already in the append-only
            // committed ledger: an earlier call wrote it and did not get
            // to remove the staging source. Completing that call means
            // draining staging — NOT writing a second copy, and NOT
            // consuming a `run_seq`, which would also shift every later
            // candidate in this batch.
            if !dry_run {
                let staging_relative = expected_relative_path(RecordKind::EvidenceRecord, &raw.0).map_err(StoreError::Layout)?;
                std::fs::remove_file(staging.root().join(&staging_relative))?;
            }
            report.recovered.push(Promoted {
                kind: RecordKind::EvidenceRecord,
                assigned: Some(RunSeqAssignment { role, surface, run_seq: existing.run_seq }),
                target: existing.target.clone(),
            });
            continue;
        }

        let seq = {
            let slot = next_seq.entry((role.clone(), surface.clone())).or_insert(0);
            *slot += 1;
            *slot
        };

        let body = committed_body(&raw.0, seq, &staging_id);
        let target = expected_relative_path(RecordKind::EvidenceRecord, &body).map_err(StoreError::Layout)?;

        if !dry_run {
            committed.write(&RawWrite(RawRecord(body)))?;
            // The window (module doc): if this removal does not land, the
            // record above is committed and the staging file survives. A
            // retry then finds `staging_id` in `already_committed` and
            // takes the recovery branch instead of promoting again.
            let staging_relative = expected_relative_path(RecordKind::EvidenceRecord, &raw.0).map_err(StoreError::Layout)?;
            std::fs::remove_file(staging.root().join(&staging_relative))?;
        }

        report.promoted.push(Promoted {
            kind: RecordKind::EvidenceRecord,
            assigned: Some(RunSeqAssignment { role, surface, run_seq: seq }),
            target,
        });
    }

    Ok(())
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

    Ok(Ok(Promoted { kind: RecordKind::Divergence, assigned: Some(RunSeqAssignment { role: key.1, surface: key.2, run_seq: seq }), target }))
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
        let ledger_root = dir.path().join(".canon").join("ledger");
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

    fn finding(change_id: &str, round: u32, seq: u32) -> canon_model::Finding {
        canon_model::Finding::new(
            Envelope::new(1, RecordKind::Finding, chrono::Utc::now(), Actor::new("reviewer-1", RoleId::parse("reviewer").unwrap())),
            canon_model::ChangeId::parse(change_id).unwrap(),
            round,
            seq,
            canon_model::FindingSeverity::Blocker,
            "reviewer-1",
            "the count was typed from memory",
        )
    }

    /// s43: a kind registered under [`StagedAssignment::Nothing`] is
    /// drained by the SAME `canon gate promote` call, in the same
    /// report. The regression this pins is the one that made the whole
    /// registry necessary: before it, `promote` read one hardcoded
    /// kind, so a staged `Finding` was invisible — `canon finding add`
    /// printed "run `canon gate promote`", promote printed "nothing
    /// staged", and the record sat in `_staging/` unreadable by every
    /// committed-tier query, forever.
    #[test]
    fn promote_drains_a_registered_kind_it_assigns_nothing_to() {
        let dir = TempDir::new().unwrap();
        let (staging, committed) = tiers(&dir);

        staging.write(&finding("s43-findings-are-records", 1, 1)).unwrap();
        staging.write(&finding("s43-findings-are-records", 1, 2)).unwrap();
        staging.write(&evidence("implementer", Some("world.firstbuy-hotdeal.14"))).unwrap();

        let report = promote(&staging, &committed, false).unwrap();
        assert!(report.is_clean(), "refused: {:?}", report.refused);

        // One report, readable per kind — the property `canon gate
        // promote`'s tally line is printed from.
        let findings: Vec<&Promoted> = report.promoted.iter().filter(|p| p.kind == RecordKind::Finding).collect();
        assert_eq!(findings.len(), 2, "both findings promoted: {:?}", report.promoted);
        assert_eq!(report.promoted.iter().filter(|p| p.kind == RecordKind::EvidenceRecord).count(), 1);

        // Nothing was assigned, and nothing is CLAIMED to have been —
        // a fabricated `run_seq=0` here would be indistinguishable from
        // a real assignment in the promote output.
        assert!(findings.iter().all(|p| p.assigned.is_none()), "a finding has no run_seq partition to report");
        assert!(findings.iter().all(|p| p.run_seq().is_none()));
        assert_eq!(findings[0].label(), "finding", "an unassigned kind's label is the kind alone, never a placeholder");

        assert!(staging.read(&TierQuery::kind(RecordKind::Finding)).unwrap().records.is_empty(), "promotion drains finding staging");
        assert_eq!(committed.read(&TierQuery::kind(RecordKind::Finding)).unwrap().records.len(), 2);
    }

    /// The committed body of an unassigned kind is the staged body
    /// BYTE FOR BYTE — nothing is stamped onto it, which is exactly why
    /// it needs no `staging_id` to be recognizable later.
    #[test]
    fn promoting_an_unassigned_kind_rewrites_nothing_in_the_body() {
        let dir = TempDir::new().unwrap();
        let (staging, committed) = tiers(&dir);

        let record = finding("s43-findings-are-records", 2, 1).fixed_by(Sha::parse("b".repeat(40)).unwrap());
        let staged_path = staging.root().join(staging.write(&record).unwrap().location);
        let staged_bytes = std::fs::read(&staged_path).unwrap();

        let report = promote(&staging, &committed, false).unwrap();
        let target = &report.promoted.iter().find(|p| p.kind == RecordKind::Finding).expect("the finding promoted").target;
        assert_eq!(std::fs::read(committed.root().join(target)).unwrap(), staged_bytes, "the committed body must be the staged body, unmodified");
        assert!(!staged_path.exists(), "the staging source is drained");
    }

    /// The re-validation guarantee, over the arm that is `Finding`'s:
    /// `validate_body` runs `Finding::check_coherence`, so an incoherent
    /// candidate (`fixed` naming no closing commit) is refused at
    /// `staging.read()` and never reaches the committed ledger — with
    /// its staging file left in place to be fixed, and a well-formed
    /// sibling promoted anyway.
    #[test]
    fn promote_refuses_an_incoherent_finding_and_leaves_it_staged() {
        let dir = TempDir::new().unwrap();
        let (staging, committed) = tiers(&dir);

        let mut incoherent = serde_json::to_value(finding("s43-findings-are-records", 3, 1)).unwrap();
        incoherent.as_object_mut().unwrap().insert("disposition".to_string(), serde_json::json!("fixed"));
        staging.write(&RawWrite(RawRecord(incoherent))).unwrap();
        staging.write(&finding("s43-findings-are-records", 3, 2)).unwrap();

        let report = promote(&staging, &committed, false).unwrap();
        assert_eq!(report.promoted.len(), 1, "only the coherent sibling promotes: {:?}", report.promoted);
        assert_eq!(report.refused.len(), 1);
        assert_eq!(report.refused[0].violation.class, FailureClass::MalformedEvidence);

        let staged_after = staging.read(&TierQuery::kind(RecordKind::Finding)).unwrap();
        assert!(staged_after.records.is_empty(), "the coherent one is drained");
        assert_eq!(staged_after.violations.len(), 1, "the incoherent one stays on disk to be fixed");
        assert_eq!(committed.read(&TierQuery::kind(RecordKind::Finding)).unwrap().records.len(), 1);
    }

    /// A retry after an interrupted promote of an unassigned kind
    /// RECOVERS rather than aborting. Without the existence check,
    /// `GitTier::write` would refuse the already-occupied path with
    /// `DuplicatePath` and take the whole batch down with it.
    #[test]
    fn a_retried_promote_of_an_unassigned_kind_recovers_instead_of_erroring() {
        let dir = TempDir::new().unwrap();
        let (staging, committed) = tiers(&dir);

        let record = finding("s43-findings-are-records", 4, 1);
        staging.write(&record).unwrap();
        let first = promote(&staging, &committed, false).unwrap();
        assert_eq!(first.promoted.len(), 1);
        assert!(first.recovered.is_empty());

        // The interruption: the committed write landed, the staging
        // removal did not.
        staging.write(&record).unwrap();

        let retry = promote(&staging, &committed, false).unwrap();
        assert!(retry.promoted.is_empty(), "the retry must not write a second record: {:?}", retry.promoted);
        assert_eq!(retry.recovered.len(), 1);
        assert_eq!(retry.recovered[0].target, first.promoted[0].target);
        assert_eq!(committed.read(&TierQuery::kind(RecordKind::Finding)).unwrap().records.len(), 1, "exactly one committed record, however many retries");
        assert!(staging.read(&TierQuery::kind(RecordKind::Finding)).unwrap().records.is_empty());
    }

    /// `--dry-run` is side-effect free for an unassigned kind too.
    #[test]
    fn dry_run_over_an_unassigned_kind_writes_and_deletes_nothing() {
        let dir = TempDir::new().unwrap();
        let (staging, committed) = tiers(&dir);
        staging.write(&finding("s43-findings-are-records", 5, 1)).unwrap();

        let report = promote(&staging, &committed, true).unwrap();
        assert_eq!(report.promoted.len(), 1, "the plan still names the candidate");
        assert!(committed.read(&TierQuery::kind(RecordKind::Finding)).unwrap().records.is_empty(), "dry-run must not write");
        assert_eq!(staging.read(&TierQuery::kind(RecordKind::Finding)).unwrap().records.len(), 1, "dry-run must not delete");
    }

    /// Every kind an authoring command can stage MUST be registered, or
    /// its records are stranded (module doc). `Divergence` is the one
    /// deliberate exclusion — it has its own staging directory and
    /// promote path — so pinning the exact set makes adding a staging
    /// writer without registering its kind a test failure rather than a
    /// silent hole.
    #[test]
    fn the_staged_kind_registry_is_exactly_the_kinds_promote_drains() {
        let kinds: Vec<RecordKind> = STAGED_KINDS.iter().map(|staged| staged.kind).collect();
        assert_eq!(kinds, vec![RecordKind::EvidenceRecord, RecordKind::Finding]);
        assert!(!kinds.contains(&RecordKind::Divergence), "Divergence promotes through `promote_divergence`, not this registry");
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
        let mut seqs: Vec<Option<u64>> = report.promoted.iter().map(|p| p.run_seq()).collect();
        seqs.sort_unstable();
        assert_eq!(seqs, vec![Some(1), Some(2)], "no gaps, strictly increasing");

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
        assert_eq!(report2.promoted[0].run_seq(), Some(3));
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
        assert_eq!(report.promoted[0].run_seq(), Some(1), "the malformed sibling must not have consumed run_seq 1");
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
        assert_eq!(report.promoted[0].run_seq(), Some(1), "the malformed sibling must not have consumed run_seq 1");
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
        assert_eq!(report.promoted[0].run_seq(), Some(1));

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
        assert!(report.promoted.iter().all(|p| p.run_seq() == Some(1)));
        let surfaces: std::collections::HashSet<&str> = report.promoted.iter().filter_map(|p| p.surface()).collect();
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
        let ledger_root = dir.path().join(".canon").join("ledger");
        let staging_dir = divergence_staging_dir(&ledger_root);
        let committed = GitTier::new(&ledger_root);

        stage_divergence(&staging_dir, &divergence_candidate("app-a", "world.firstbuy-hotdeal.14", "reviewer", DivergenceStatus::Open, 1)).unwrap();
        stage_divergence(&staging_dir, &divergence_candidate("app-a", "world.firstbuy-hotdeal.26", "reviewer", DivergenceStatus::Open, 1)).unwrap();

        let report = promote_divergence(&staging_dir, &committed, false).unwrap();
        assert!(report.is_clean(), "refused: {:?}", report.refused);
        assert_eq!(report.promoted.len(), 2);
        let mut seqs: Vec<Option<u64>> = report.promoted.iter().map(|p| p.run_seq()).collect();
        seqs.sort_unstable();
        assert_eq!(seqs, vec![Some(1), Some(2)], "no gaps, strictly increasing within one (project_id, role, surface) partition");

        // Staging is empty afterward; both committed Divergence records exist.
        assert!(std::fs::read_dir(&staging_dir).unwrap().next().is_none());
        assert_eq!(committed.read(&TierQuery::kind(RecordKind::Divergence)).unwrap().records.len(), 2);

        // A THIRD candidate in the SAME partition, staged+promoted in a
        // SEPARATE call, continues from the committed max — never restarts.
        stage_divergence(&staging_dir, &divergence_candidate("app-a", "world.firstbuy-hotdeal.33", "reviewer", DivergenceStatus::Open, 1)).unwrap();
        let report2 = promote_divergence(&staging_dir, &committed, false).unwrap();
        assert_eq!(report2.promoted.len(), 1);
        assert_eq!(report2.promoted[0].run_seq(), Some(3));
    }

    #[test]
    fn divergence_promote_refuses_a_malformed_candidate_without_consuming_a_run_seq() {
        let dir = TempDir::new().unwrap();
        let ledger_root = dir.path().join(".canon").join("ledger");
        let staging_dir = divergence_staging_dir(&ledger_root);
        let committed = GitTier::new(&ledger_root);
        std::fs::create_dir_all(&staging_dir).unwrap();

        // Malformed: not even valid JSON for a DivergenceCandidate (missing required fields).
        std::fs::write(staging_dir.join("malformed.json"), serde_json::to_string(&serde_json::json!({"not": "a candidate"})).unwrap()).unwrap();

        stage_divergence(&staging_dir, &divergence_candidate("app-a", "world.firstbuy-hotdeal.41", "reviewer", DivergenceStatus::Open, 1)).unwrap();

        let report = promote_divergence(&staging_dir, &committed, false).unwrap();
        assert_eq!(report.promoted.len(), 1, "promoted: {:?}", report.promoted);
        assert_eq!(report.promoted[0].run_seq(), Some(1), "the malformed sibling must not have consumed run_seq 1");
        assert_eq!(report.refused.len(), 1);
        assert_eq!(report.refused[0].violation.class, FailureClass::MalformedEvidence);
    }

    #[test]
    fn divergence_promote_refuses_a_candidate_with_no_derivable_partition_key() {
        let dir = TempDir::new().unwrap();
        let ledger_root = dir.path().join(".canon").join("ledger");
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
        let ledger_root = dir.path().join(".canon").join("ledger");
        let staging_dir = divergence_staging_dir(&ledger_root);
        let committed = GitTier::new(&ledger_root);

        // Same (role, surface_key), DIFFERENT project_id — independent sequences.
        stage_divergence(&staging_dir, &divergence_candidate("app-a", "world.firstbuy-hotdeal.14", "reviewer", DivergenceStatus::Open, 1)).unwrap();
        stage_divergence(&staging_dir, &divergence_candidate("app-b", "world.firstbuy-hotdeal.26", "reviewer", DivergenceStatus::Open, 1)).unwrap();

        let report = promote_divergence(&staging_dir, &committed, false).unwrap();
        assert!(report.is_clean(), "refused: {:?}", report.refused);
        assert_eq!(report.promoted.len(), 2);
        assert!(report.promoted.iter().all(|p| p.run_seq() == Some(1)), "different project_id partitions both independently start at run_seq 1");
    }

    #[test]
    fn commit_divergence_direct_commit_never_touches_the_staging_directory() {
        let dir = TempDir::new().unwrap();
        let ledger_root = dir.path().join(".canon").join("ledger");
        let staging_dir = divergence_staging_dir(&ledger_root);
        let committed = GitTier::new(&ledger_root);

        // A candidate a reviewer is still mid-`stage`ing, untouched by
        // a SEPARATE `resolve`/`defer` direct commit below.
        stage_divergence(&staging_dir, &divergence_candidate("app-a", "world.firstbuy-hotdeal.14", "reviewer", DivergenceStatus::Open, 1)).unwrap();

        let resolved = divergence_candidate("app-a", "world.firstbuy-hotdeal.99", "reviewer", DivergenceStatus::Resolved, 1);
        let outcome = commit_divergence(&resolved, &committed).unwrap();
        let promoted = outcome.expect("resolve candidate should promote cleanly");
        assert_eq!(promoted.run_seq(), Some(1));

        // The unrelated staged candidate is still sitting there, untouched.
        assert_eq!(std::fs::read_dir(&staging_dir).unwrap().count(), 1);
        assert_eq!(committed.read(&TierQuery::kind(RecordKind::Divergence)).unwrap().records.len(), 1);
    }

    /// The `ReviewEvidence` BLOCKER (module doc's recovery section): the
    /// committed write lands, the staging removal does not, and the retry
    /// operators are told to run used to append the SAME attestation
    /// again at the next `run_seq` — permanently, into an append-only
    /// ledger.
    ///
    /// Re-writing the identical record reproduces the surviving staging
    /// file byte-for-byte (its path AND its contents are content-derived),
    /// which is precisely the on-disk state a killed process or a failed
    /// `remove_file` leaves behind — so this exercises the real recovery
    /// input, not a stand-in for it.
    #[test]
    fn a_retried_promote_after_an_interrupted_one_recovers_instead_of_duplicating() {
        let dir = TempDir::new().unwrap();
        let (staging, committed) = tiers(&dir);
        let record = evidence("implementer", Some("world.firstbuy-hotdeal.14"));

        staging.write(&record).unwrap();
        let first = promote(&staging, &committed, false).unwrap();
        assert_eq!(first.promoted.len(), 1);
        assert!(first.recovered.is_empty(), "nothing to recover on a first, uninterrupted promote");
        assert_eq!(first.promoted[0].run_seq(), Some(1));

        // The interruption.
        staging.write(&record).unwrap();
        assert_eq!(staging.read(&TierQuery::kind(RecordKind::EvidenceRecord)).unwrap().records.len(), 1);

        let retry = promote(&staging, &committed, false).unwrap();
        assert!(retry.is_clean(), "a recovery must exit clean or the retry is unusable: {:?}", retry.refused);
        assert!(retry.promoted.is_empty(), "the retry must not write a second record: {:?}", retry.promoted);
        assert_eq!(retry.recovered.len(), 1);
        assert_eq!(retry.recovered[0].run_seq(), Some(1), "the recovery reports the run_seq the interrupted call already assigned");
        assert_eq!(retry.recovered[0].target, first.promoted[0].target, "and the committed path that call actually wrote");

        assert_eq!(
            committed.read(&TierQuery::kind(RecordKind::EvidenceRecord)).unwrap().records.len(),
            1,
            "exactly one committed record survives the retry"
        );
        assert!(staging.read(&TierQuery::kind(RecordKind::EvidenceRecord)).unwrap().records.is_empty(), "the retry drains staging");

        // The identity is per-CANDIDATE, not per-(role, surface): a
        // different attestation in the SAME run_seq partition
        // (`world.firstbuy-hotdeal.26` shares `surface_key()` with `.14`)
        // still promotes, at the next run_seq. Without this the recovery
        // would silently swallow every later record for the surface.
        // Distinguished by scenario rather than by `at`, so the assertion
        // does not depend on two `Utc::now()` calls landing in different
        // serialized timestamps.
        staging.write(&evidence("implementer", Some("world.firstbuy-hotdeal.26"))).unwrap();
        let third = promote(&staging, &committed, false).unwrap();
        assert_eq!(third.promoted.len(), 1, "a distinct attestation is not a recovery: {third:?}");
        assert_eq!(third.promoted[0].run_seq(), Some(2));
        assert_eq!(committed.read(&TierQuery::kind(RecordKind::EvidenceRecord)).unwrap().records.len(), 2);
    }

    /// `--dry-run` stays side-effect free on the recovery path too: it
    /// reports what the retry WOULD drain without removing the staging
    /// file, so an operator can inspect the situation before acting.
    #[test]
    fn a_dry_run_retry_reports_the_recovery_without_draining_staging() {
        let dir = TempDir::new().unwrap();
        let (staging, committed) = tiers(&dir);
        let record = evidence("implementer", Some("world.firstbuy-hotdeal.14"));

        staging.write(&record).unwrap();
        promote(&staging, &committed, false).unwrap();
        staging.write(&record).unwrap();

        let dry = promote(&staging, &committed, true).unwrap();
        assert_eq!(dry.recovered.len(), 1);
        assert!(dry.promoted.is_empty());
        assert_eq!(staging.read(&TierQuery::kind(RecordKind::EvidenceRecord)).unwrap().records.len(), 1, "--dry-run removes nothing");
        assert_eq!(committed.read(&TierQuery::kind(RecordKind::EvidenceRecord)).unwrap().records.len(), 1, "--dry-run writes nothing");
    }

    /// The identity is carried by the committed record itself, so it
    /// survives a fresh process: `promote` recovers by READING
    /// `staging_id` back off the committed ledger, never from in-memory
    /// state a crash would have taken with it.
    #[test]
    fn the_committed_record_carries_the_staging_identity_it_was_promoted_under() {
        let dir = TempDir::new().unwrap();
        let (staging, committed) = tiers(&dir);
        let record = evidence("implementer", Some("world.firstbuy-hotdeal.14"));
        let staged_body = serde_json::to_value(&record).unwrap();

        staging.write(&record).unwrap();
        promote(&staging, &committed, false).unwrap();

        let landed = committed.read(&TierQuery::kind(RecordKind::EvidenceRecord)).unwrap();
        assert_eq!(landed.records.len(), 1);
        assert_eq!(
            landed.records[0].0.get(STAGING_ID_KEY).and_then(serde_json::Value::as_str),
            Some(staging_candidate_id(&staged_body).as_str()),
            "the committed record must carry the digest of the staged body it came from: {:?}",
            landed.records[0].0
        );
    }
}
