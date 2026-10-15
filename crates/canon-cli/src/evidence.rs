//! `canon evidence add` (s42 `close-the-open-loops`, task 4.1): the
//! MISSING SEAM in canon's own evidence loop.
//!
//! `canon gate task` refuses an `unevidenced-flip` and `canon gate
//! promote` moves staged evidence into the committed ledger — but until
//! this module there was no way to author an `EvidenceRecord` at all.
//! The only production writer was `crate::demo`, which seeds one for a
//! throwaway scenario in a scaffolded demo repo. The measurable
//! consequence: this repo's plan corpus carries hundreds of checked
//! task boxes (**675** when s42 was proposed) and, across the whole
//! committed ledger, **exactly one `EvidenceRecord`** — so every real
//! flip in its history is a hand flip, while the gate that would have
//! refused those flips shipped working and unused. That is the gap this
//! module closes; it is
//! deliberately NOT a backfill (`proposal.md`: fabricating evidence for
//! work whose proof was never captured is the precise failure the gate
//! exists to prevent).
//!
//! # What a record from here IS: an attributed, auditable ATTESTATION
//! Not machine-verified proof. This command never executes, resolves,
//! fetches, or checks `--ref` in any way — it writes down that a named
//! actor, in a named role, at a stamped time, CLAIMED that the named
//! evidence supports the named task. `--kind test-run --ref 'cargo test
//! -p canon-cli'` produces exactly the same record whether that command
//! passed, failed, or was never run. State it the blunt way, because s42
//! (`close-the-open-loops`)'s own review had to:
//!
//! - **An agent that can run this command can authorize its own
//!   checkbox.** There is no separation between the author of an
//!   attestation and its beneficiary, no signature, and no second party.
//!   `canon evidence add --task <id> --kind test-run --ref never-ran
//!   --role implementer` stages, promotes, and satisfies `canon gate
//!   task <id>`.
//! - The value is ATTRIBUTION and AUDIT, not verification: every flip
//!   acquires a permanent, append-only committed record naming who
//!   claimed what, when, in which role, against which reference — which a
//!   reviewer or a later reader can go check. Before this module the same
//!   flips happened by hand, leaving nothing at all. That is the whole
//!   delta, and it is worth having; it is not proof.
//! - A risk approval is a separate authenticated attestation. `--approval-by`
//!   identifies the signer for the policy's allowed-signers file; the gate
//!   counts it only after `ssh-keygen -Y verify` succeeds over a deterministic
//!   `canon-approval-v1` payload bound to subject, project, artifact SHA, run,
//!   normalized surface/effects, actor, and timestamp. `CANON_ACTOR` and the
//!   persisted `verified` display bit are never authentication.
//! - `--surface-ref` records an explicitly bound path or `effect:<slug>`.
//!   Risk approvals additionally require `--artifact-sha`, `--approval-at`,
//!   and an externally produced `--approval-signature-file`; canon never signs
//!   or reads private keys.
//!
//! Making the underlying evidence verifiable is a DIFFERENT change: canon
//! would have to capture evidence through an execution path it controls (a
//! run it launched, a transcript it recorded, a signed artifact) rather than
//! a string a caller typed. Shelling out to `--ref` here would be worse
//! than the honest gap — it would let a caller pick the command whose
//! exit code becomes canon's proof.
//!
//! ## Exactly what `canon gate task` checks on this path
//! Checked: a non-`Divergent` [`EvidenceRecord`] exists for that
//! `task_id`; its `evidence_note` companion (when present) passes
//! `canon_gate::scan_fake_markers` — the three-marker fabrication
//! blocklist, plus a bare `verified` summary with no `command_result`;
//! that note is a single line
//! (`canon_ingest::reject_multi_line_note`); on the typed path, that the
//! record's `evidence: {kind, ref}` companion equals the task atom's
//! declared contract; and that the plan document actually carries an
//! open row for the task.
//!
//! NOT checked, on this path, at all: whether `--ref` names anything
//! real, let alone anything that ran. None of `canon gate check`'s
//! registered checks execute here either — `canon_gate::check_set` is
//! `coverage`, `ledger`, `staleness`, `trust-ladder` and (under
//! `--release`) `release-trust-required`, and `canon gate task` runs NONE
//! of them: an attestation older than the staleness policy allows, one
//! whose trust-ladder tag is below what `policy.yaml` requires for the
//! surface, and one whose surface carries an open `Divergence` all flip
//! the row exactly the same. Those checks guard `canon gate check`'s
//! corpus-wide verdict; they are not a second opinion on this flip.
//!
//! `--command-result` is the one cheap strengthening available without
//! an execution path canon owns: an author who pastes the real captured
//! output makes the claim auditable against something concrete, and it
//! is scanned for fabrication markers exactly like `--summary`. It is
//! still text the author supplied — it raises the cost of a false
//! attestation, it does not verify one.
//!
//! # Staged, never committed
//! [`run_add`] writes to [`crate::gate::evidence_staging_dir`] —
//! `<ledger_root>/_staging`, a `GitTier` sibling of the committed
//! `kind=evidence_record/` tree — exactly the mechanism `canon gate
//! promote` already drains (`canon_gate::promote`), never a parallel
//! one. So the three-command loop is:
//!
//! ```text
//! canon evidence add --task <id> --kind <k> --ref <r> --role <role>
//! canon gate promote
//! canon gate task <id>
//! ```
//!
//! Staging (rather than committing directly, the way `canon review add`
//! does) is not a stylistic choice: `EvidenceRecord.run_seq` is assigned
//! by `canon gate promote`, monotonically per `(role, surface)`, and a
//! record that skipped promotion would carry none. `--role` is therefore
//! REQUIRED, not defaulted — `canon_gate::promote`'s partition key is
//! `(actor.role, scenario_id|task_id.change_id())`, and a record with no
//! `actor.role` is refused at promotion with `malformed-evidence`.
//!
//! # What is authored, and what is a raw companion
//! `task_id`/`scenario_id`/`run_id`/`verdict` are
//! [`EvidenceRecord`]'s own typed fields. `--kind`/`--ref` are NOT: they
//! land as the top-level `evidence: {kind, ref}` companion key
//! `EvidenceRecord`'s strict `Deserialize` silently drops and
//! `crate::gate`'s typed path re-reads off the raw JSON — the shape the
//! single committed record in this repo already carries. `--kind` names
//! the CLASS of evidence being attested to (`test-run`, `review`, …) and
//! `--ref` the reference a reader can go resolve; neither is resolved
//! here (the attestation section above). `--summary`/`--command-result`
//! likewise land as the `evidence_note` companion
//! `canon_gate::evidence_note_of` reads, which is what a flipped row's
//! ` — ✅ ` suffix is built from. A scenario-only record carries the same
//! companion (0.14 D5): the summary is part of the attestation whether or
//! not a task row exists, and only a task-keyed record's flip turns it
//! into a row suffix.
//!
//! # Bound files are stored (0.14 D4)
//! `--artifact`/`--report` bytes are copied into the content-addressed
//! store `.canon/artifacts/sha256/<hex>` (`canon_gate::artifact_store`)
//! after every refusal has passed and before the record is staged, so a
//! staged record never names bytes the store lacks. `canon gate check`
//! re-hashes them. `--max-artifact-mib` raises the per-file limit
//! (`canon_gate::DEFAULT_MAX_ARTIFACT_MIB`); a larger file is refused.
//! [`run_vault`] stores the bytes for records written before the store
//! existed.
//!
//! # The suffix is a DOCUMENT write, so its inputs are refused, not escaped
//! `--summary` becomes that suffix verbatim (`canon_gate::gate_task`
//! returns it as the approved note), and with no `--summary` the
//! fallback suffix `canon_gate`'s `default_evidence_text` builds embeds
//! `--actor-id`. A `tasks.md` row is ONE LINE, so a line separator in
//! either value does not produce a longer row — it produces a second
//! document line. s42's own review found the consequence:
//! `--summary $'ok\n- [x] 9.9 Forged task'` appended a fully CHECKED row
//! backed by no evidence record at all, inside the one command whose
//! purpose is refusing unevidenced completion.
//!
//! [`run_add`] therefore refuses any `canon_ingest::task_rows::
//! ROW_LINE_BREAKS` character in `--summary` or `--actor-id`, as a usage
//! error, before anything is staged — and the write-back refuses a
//! multi-line note again on its own (`canon_ingest::
//! reject_multi_line_note`), so a different author reaching `flip_task`
//! cannot reintroduce it. Refused rather than escaped: there is no
//! escaping that makes a two-line evidence note meaningful in a one-line
//! row, and silently rewriting a caller's attestation text is worse than
//! refusing it.
//!
//! `--kind`/`--ref`/`--command-result` are NOT restricted this way, and
//! that is deliberate rather than an oversight: none of them reaches a
//! plan document (the suffix is built from `summary`/`actor.agent_id`
//! alone), they land only in JSON-escaped ledger bodies, and a captured
//! `--command-result` is legitimately multi-line — being the real output
//! is the entire point of pasting it.
//!
//! # Every refusal is a refusal the GATE would have made
//! A record that stages cleanly but that `canon gate task` then refuses
//! is a worse outcome than no command at all — worse still because
//! `canon gate promote` would already have committed it into the
//! append-only ledger by then. So [`run_add`] pre-flights every
//! condition an authored record can actually fail downstream, through
//! the gate's OWN functions rather than a second copy of their rules:
//!
//! - `canon_gate::scan_note_text` over the `evidence_note` being
//!   authored (the rules `canon_gate::gate_task` applies through
//!   `scan_fake_markers`, which delegates to it).
//! - `canon_ingest::task_rows::first_row_line_break` over the two
//!   document-bound fields (section above) — the identical predicate
//!   `canon_ingest::reject_multi_line_note` refuses the flip with.
//! - [`crate::gate::typed_evidence_contract_for_task`] — when the task
//!   carries a typed vocabulary atom declaring `evidence: {kind, ref}`,
//!   the gate narrows its evidence slice to records matching it exactly,
//!   so a mismatched `--kind`/`--ref` would be reported two commands
//!   later as a bare `unevidenced-flip` with no hint of the cause.
//!
//! Unknown-task admission is shared the same way:
//! [`crate::dispatch::validate_task_binding`], the very decision `canon
//! dispatch begin --task` makes (s41 extracted the D8 ownership rule
//! into one place precisely because a second copy had already drifted).
//!
//! # Nothing is attested under an unapproved brief (0.16 M7)
//! The approval is the one thing an agent never assumes: it writes the
//! brief, stops and reports it. As the backstop for that stop, an add is
//! refused (exit `1`, nothing staged) while a subject it attests for has
//! a brief [`crate::brief::brief_of`] reads as enforced and not approved:
//! the scenario's `@subject` owner, and every subject adopting the
//! task's change ([`crate::brief::attested_briefs`]). Each refusal is an
//! `unreviewed-promotion` naming the subject and its canon `brief.md`
//! (with the `Source:` pointer to write, for a change from an external
//! plan), as the ship guard prints it. `--override-reason` attests anyway and
//! records a [`BriefOverride`] (reason, waived refusals, actor) on the
//! record, which `canon status` and `canon gate check` list. A subject
//! with no brief to answer is unaffected. `canon review add` and `canon
//! finding add` are not refused.
//!
//! # Exit-code contract (`crate::gate`'s, unchanged)
//! `0` staged, `1` the authored record would be gate-red (a fabrication
//! marker, or a typed atom that fails vocabulary validation) or a brief
//! refuses it, `2`
//! usage-or-infra (an unknown/ungrammatical task, no plan corpus, a
//! malformed flag combination, a line separator in a document-bound
//! field, an unparseable typed-atoms file, a tier write failure). Each
//! grade matches what `canon gate task` returns for the SAME condition,
//! so the operator sees one verdict, earlier.

use std::path::{Path, PathBuf};

use canon_gate::review_gate::{in_effect_reviews, review_gap, ReviewGapKind};
use canon_gate::{scan_note_text, verify_risk_approval, FailureClass, GateContext, GateCtx, Violation};
use canon_ingest::task_rows::first_row_line_break;
use canon_model::{
    approval_payload_bytes, Actor, AttachmentDrop, AuditResult, BriefOverride, Envelope, EvidenceApproval, EvidenceAudit, EvidenceRecord, EvidenceVerdict, ProjectId,
    ProvenanceRef, RawRecord, RecordKind, ReportFormat, Review, RoleId, RunId, Scenario, ScenarioId, SessionId, Sha, TaskId, WaivedViolation, APPROVAL_NAMESPACE,
};
use canon_policy::SchemaRegistry;
use canon_store::git_tier::GitTier;
use canon_store::tier::{RawWrite, Tier, TierQuery};
use chrono::{DateTime, Utc};

use crate::cli_error::CliError;
use crate::context::resolve_repo_root;
use crate::gate::{evidence_staging_dir, typed_evidence_contract_for_task};

/// What `canon evidence add` tells the agent when a brief refuses the
/// attestation (0.16 M7): the stop belongs at the brief.
const BRIEF_STOP: &str = "stop, report the brief to the human and wait for their answer: nothing is built or attested for a subject before its brief is approved. Record their answer with `canon brief approve --by <the human> --note \"<their words>\"` only from that answer; to attest anyway, pass --override-reason <text>, which is recorded on the evidence record";

/// The refusal for a note a fabrication-marker scan rejected: one line per
/// violation, exit 1 like every other gate-red refusal of this command
/// (0.18 D5, #32).
fn fabricated(violations: &[Violation]) -> CliError {
    CliError::failed("EVIDENCE_FABRICATED", violations.iter().map(Violation::line).collect::<Vec<_>>().join("\n"))
}

/// A `--report`/`--file` binding refusal, with the exit its kind carries.
fn bind_refusal(error: &crate::evidence_attach::BindError) -> CliError {
    use crate::evidence_attach::BindError;
    match error {
        BindError::Usage(message) => CliError::refused("EVIDENCE_BIND_REFUSED", message.as_str()),
        BindError::Contradiction(message) => CliError::failed("EVIDENCE_REPORT_CONTRADICTION", message.as_str()),
    }
}

/// The gate's view of the repository, as every check here reads it.
fn load_gate_context(repo: &Path) -> Result<GateContext, String> {
    GateCtx::from_repo(repo)
        .map_err(|e| e.to_string())
        .and_then(|ctx| GateContext::load(ctx, &SchemaRegistry::load(), Utc::now()).map_err(|e| e.to_string()))
}

/// The brief refusals for this attestation (module doc, 0.16 M7): one
/// `unreviewed-promotion` per subject it attests for whose brief
/// [`crate::brief::brief_of`] reads as enforced and not approved, the
/// class and subject the ship guard's brief check prints. The subjects
/// come from [`crate::brief::attested_briefs`]: the scenario's
/// `@subject` owner and every subject adopting the task's change.
fn brief_refusals(repo: &Path, ctx: &GateContext, task_id: Option<&TaskId>, scenario: Option<(&ProjectId, &ScenarioId)>) -> Vec<BriefRefusal> {
    // An unreadable `plans:` section finds no external proposal.md;
    // `canon status` warns about it.
    let plan_roots = crate::brief::plan_roots(repo).unwrap_or_default();
    let what = [scenario.map(|(_, s)| s.as_str().to_string()), task_id.map(ToString::to_string)]
        .into_iter()
        .flatten()
        .collect::<Vec<_>>()
        .join(" and ");
    crate::brief::attested_briefs(repo, &plan_roots, ctx, task_id, scenario)
        .into_iter()
        .filter(|(_, brief)| brief.enforced)
        .map(|(subject, brief)| BriefRefusal {
            violation: Violation::new(
                FailureClass::UnreviewedPromotion,
                subject.subject_id.as_str(),
                format!("evidence for {what}: brief not approved — {}, so the human has not answered it; once they do, {}", brief.unanswered(), brief.record()),
            ),
            changed: brief.changed,
        })
        .collect()
}

/// One brief refusal: the `unreviewed-promotion` violation it prints, and
/// whether the brief was approved once and edited since (the code readers
/// branch on is `BRIEF_CHANGED_SINCE_APPROVAL` then, `BRIEF_UNAPPROVED`
/// otherwise). Carried as a field, not read back out of the refusal text.
#[derive(Clone)]
struct BriefRefusal {
    violation: Violation,
    changed: bool,
}

/// `--verdict`'s clap `value_parser`. Kebab-cased on the CLI, matching
/// `crate::divergence::parse_status`'s established spelling for a
/// snake_case-serialized model enum, and exhaustive over
/// [`EvidenceVerdict`] — including `divergent`, which is the one verdict
/// `canon_gate::gate_task` treats as blocking. Authoring it is a real
/// operation ("the evidence says this did NOT hold"), not a mistake to
/// be prevented here; refusing to flip on it is the gate's job.
pub fn parse_verdict(s: &str) -> Result<EvidenceVerdict, String> {
    match s {
        "faithful" => Ok(EvidenceVerdict::Faithful),
        "not-applicable" => Ok(EvidenceVerdict::NotApplicable),
        "divergent" => Ok(EvidenceVerdict::Divergent),
        other => Err(format!("`{other}` is not an evidence verdict — expected one of: faithful, not-applicable, divergent")),
    }
}

/// Validate one explicit risk-surface binding. Effects use the unambiguous
/// `effect:<kebab-slug>` form; all other refs are repository-relative paths.
/// This validates shape only — the CLI still records an attestation, not
/// proof that the caller's ref equals the changed paths.
pub fn parse_surface_ref(s: &str) -> Result<String, String> {
    if s.is_empty() || s.trim() != s || s.contains('\n') || s.contains('\r') || s.contains('\0') {
        return Err("surface refs must be non-empty, trimmed, and contain no line separators or NUL".to_string());
    }
    if let Some(effect) = s.strip_prefix("effect:") {
        if effect.is_empty() || !effect.split('-').all(|part| !part.is_empty() && part.chars().all(|ch| ch.is_ascii_lowercase() || ch.is_ascii_digit())) {
            return Err("effect surface refs must use `effect:<kebab-slug>`".to_string());
        }
        return Ok(s.to_string());
    }
    let path = std::path::Path::new(s);
    if path.is_absolute() || path.components().any(|component| matches!(component, std::path::Component::ParentDir | std::path::Component::RootDir | std::path::Component::Prefix(_))) {
        return Err("path surface refs must be repository-relative and stay inside the repository".to_string());
    }
    Ok(s.to_string())
}

fn normalized_strings(values: &[String]) -> Vec<String> {
    let mut values = values.to_vec();
    values.sort_unstable();
    values.dedup();
    values
}

/// One `canon evidence add` invocation's already-parsed flags.
///
/// A named struct rather than a positional parameter list: thirteen
/// arguments, of which `kind`/`evidence_ref`/`summary`/`command_result`/
/// `actor_id`/`surface_ref`/`approval_by` are plain strings — every adjacent
/// pair of them would silently compile if transposed, and approval fields
/// end up in the permanent ledger body.
pub struct EvidenceArgs {
    /// The plan task this evidence attests to, validated against the
    /// live plan corpus by [`crate::dispatch::validate_task_binding`]
    /// rather than merely grammar-checked. `None` for a SCENARIO-keyed
    /// attestation (s44): the spec corpus is a first-class attestation
    /// subject, not only a secondary join hanging off a task.
    /// At least one of `task_id`/`scenario_id` is required — a record
    /// with neither carries no coverage subject at all
    /// (`canon_gate::coverage::CellSubject::of` returns `None`) and
    /// could never be read back by any gate.
    pub task_id: Option<TaskId>,
    /// The spec corpus this evidence's `scenario_id` belongs to. Canon
    /// keys a scenario by the COMPOSITE `(project_id, scenario_id)`
    /// (`Scenario`'s own `project_id` is required), so a scenario-keyed
    /// record without one cannot close that join — two spec roots may
    /// carry the same scenario id.
    pub project_id: Option<ProjectId>,
    /// The `evidence.kind` companion — what CLASS of evidence is being
    /// attested to (`test-run`, `review`, …). Free text on the untyped
    /// path; on the typed path it must match the task atom's declared
    /// kind, and the vocabulary is what constrains the domain.
    pub kind: String,
    /// The `evidence.ref` companion — the reference a reader can go
    /// resolve (a command line, a commit sha, a report path). NEVER
    /// resolved, executed, or checked by this command (module doc's
    /// attestation section): the record says what was claimed, not what
    /// was observed.
    pub evidence_ref: String,
    /// `None` means `faithful` on a declared record. Refused with
    /// `--kind audit`, whose verdict is derived from `audit_result`
    /// ([`audit_verdict`]).
    pub verdict: Option<EvidenceVerdict>,
    /// The `evidence_note.summary` companion: the one line a flipped
    /// row's ` — ✅ ` suffix is built from. Absent, `canon_gate::
    /// gate_task` falls back to its own `default_evidence_text`. Refused
    /// when it carries a line separator — it is written into a plan
    /// document as one row (module doc).
    pub summary: Option<String>,
    /// The `evidence_note.command_result` companion — a captured
    /// command result. Requires `summary`: the companion's own
    /// deserialize makes `summary` mandatory, so a command-result-only
    /// note is unreadable to the gate rather than partially readable.
    /// Legitimately multi-line (it is pasted output), and never part of
    /// the row suffix, so the line-separator refusal does not apply.
    pub command_result: Option<String>,
    pub scenario_id: Option<ScenarioId>,
    pub run_id: Option<RunId>,
    /// Explicit risk binding, persisted on the EvidenceRecord. This is
    /// caller-supplied attestation data; gate checks fail closed whenever a
    /// configured rule cannot be matched to a current binding.
    pub surface_ref: Vec<String>,
    /// The attesting actor's id — the ATTRIBUTION half of what this
    /// command buys (module doc). Refused when it carries a line
    /// separator: with no `--summary`, `default_evidence_text` embeds it
    /// in the row suffix, making it the second injection vector.
    pub actor_id: String,
    /// REQUIRED (module doc): `canon_gate::promote` derives its
    /// `run_seq` partition key from `actor.role`, and refuses a record
    /// that carries none.
    pub role: RoleId,
    /// The attesting session, recorded as `actor.session_id`. Under
    /// `require_review.distinct_actor` a review from this session does
    /// not count for the scenario.
    pub session_id: Option<SessionId>,
    /// Optional authenticated approval. A complete approval additionally
    /// requires a detached SSH signature and exact artifact SHA binding.
    pub approval_by: Option<String>,
    pub approval_role: Option<RoleId>,
    pub approval_signature_file: Option<PathBuf>,
    /// RFC3339 timestamp covered by the external signature.
    pub approval_at: Option<String>,
    /// Exact Git artifact SHA bound by an approval and persisted on the record.
    pub artifact_sha: Option<Sha>,
    /// Experimental evidence binding: files bound by sha256
    /// ([`crate::evidence_attach`]). Never executed.
    pub artifacts: Vec<PathBuf>,
    /// Experimental evidence binding: parsed test reports.
    pub reports: Vec<(ReportFormat, PathBuf)>,
    /// Report cases to bind IN ADDITION to every case carrying the
    /// scenario id (repeatable; 0.14 D5).
    pub report_cases: Vec<String>,
    /// `--all-scenarios` (0.18 D1): one record per indexed scenario of
    /// `--project-id` that owns a case in the `--report`s
    /// ([`run_add_all`]).
    pub all_scenarios: bool,
    /// The artifact store's per-file size limit, in MiB.
    pub max_artifact_mib: u64,
    /// `--kind audit` only (0.15 D3): the external evaluator's id,
    /// version, and sha256, and what it concluded. Required together with
    /// `--kind audit`, refused on every other kind. The digest and result
    /// are validated here, not by clap, so a `canon status` suggestion
    /// with placeholders still parses as printed.
    pub evaluator_id: Option<String>,
    pub evaluator_version: Option<String>,
    pub evaluator_digest: Option<String>,
    pub audit_result: Option<String>,
    /// 0.15 D2: repository-relative paths the scenario's previous latest
    /// record bound that this record deliberately does not, with the one
    /// `drop_reason` recorded for all of them.
    pub drop_attachments: Vec<String>,
    pub drop_reason: Option<String>,
    /// 0.16 M7: attest anyway while a subject this record attests for has
    /// an enforced, unapproved brief ([`brief_refusals`]); recorded on the
    /// record as its [`BriefOverride`], with the actor.
    pub override_reason: Option<String>,
    /// `--promote`/`--no-promote` (0.16 D2): whether this run promotes
    /// the record it stages ([`crate::write_mode`]).
    pub promote: crate::write_mode::PromoteFlag,
    /// `--json`: one JSON document on stdout, every human line on stderr.
    pub json: bool,
}

/// The `--kind` value that marks an audited outcome (0.15 D3).
pub const AUDIT_KIND: &str = "audit";

/// `--audit-result`'s accepted values.
fn parse_audit_result(s: &str) -> Result<AuditResult, String> {
    match s {
        "confirmed" => Ok(AuditResult::Confirmed),
        "refuted" => Ok(AuditResult::Refuted),
        "indeterminate" => Ok(AuditResult::Indeterminate),
        other => Err(format!("--audit-result `{other}` is not an audit result — expected one of: confirmed, refuted, indeterminate")),
    }
}

/// `--evaluator-digest`: a sha256, 64 lowercase hex characters, as every
/// other digest canon records.
fn parse_sha256(s: &str) -> Result<String, String> {
    if s.len() == 64 && s.bytes().all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b)) {
        Ok(s.to_string())
    } else {
        Err(format!("--evaluator-digest `{s}` is not a sha256 — expected 64 lowercase hex characters"))
    }
}

/// The verdict an audit record carries, derived from its result so a
/// reader that ignores `audit` still fails closed on a refutation:
/// `confirmed` → faithful, `refuted` → divergent, `indeterminate` →
/// not-applicable. The gate never reads it as the scenario's declared
/// verdict — audits are folded apart (`canon_gate::audit`).
pub fn audit_verdict(result: AuditResult) -> EvidenceVerdict {
    match result {
        AuditResult::Confirmed => EvidenceVerdict::Faithful,
        AuditResult::Refuted => EvidenceVerdict::Divergent,
        AuditResult::Indeterminate => EvidenceVerdict::NotApplicable,
    }
}

/// The audit an `--kind audit` invocation records, or `None` for every
/// other kind (0.15 D3). The four evaluator/result flags are required
/// with `--kind audit` and refused elsewhere; the declared-record flags
/// the gate reads only off declared evidence (`--task`, `--verdict`,
/// `--surface-ref`, the approval set) are refused on an audit, so no
/// value is recorded that nothing would ever read.
fn audit_of(args: &EvidenceArgs) -> Result<Option<EvidenceAudit>, String> {
    let audit_flags = [
        ("--evaluator-id", args.evaluator_id.is_some()),
        ("--evaluator-version", args.evaluator_version.is_some()),
        ("--evaluator-digest", args.evaluator_digest.is_some()),
        ("--audit-result", args.audit_result.is_some()),
    ];
    if args.kind != AUDIT_KIND {
        let given: Vec<&str> = audit_flags.iter().filter(|(_, set)| *set).map(|(flag, _)| *flag).collect();
        if given.is_empty() {
            return Ok(None);
        }
        return Err(format!("{} belong(s) to `--kind {AUDIT_KIND}` only; this record is `--kind {}`", given.join(", "), args.kind));
    }
    let missing: Vec<&str> = audit_flags.iter().filter(|(_, set)| !*set).map(|(flag, _)| *flag).collect();
    if !missing.is_empty() {
        return Err(format!(
            "`--kind {AUDIT_KIND}` records an external evaluator's re-audit and needs {}; canon never generates or runs the audit, it records the evaluator's result",
            missing.join(", ")
        ));
    }
    let declared_only = [
        ("--task", args.task_id.is_some()),
        ("--verdict", args.verdict.is_some()),
        ("--surface-ref", !args.surface_ref.is_empty()),
        ("--approval-by", args.approval_by.is_some()),
        ("--approval-role", args.approval_role.is_some()),
        ("--approval-signature-file", args.approval_signature_file.is_some()),
        ("--approval-at", args.approval_at.is_some()),
    ];
    let refused: Vec<&str> = declared_only.iter().filter(|(_, set)| *set).map(|(flag, _)| *flag).collect();
    if !refused.is_empty() {
        return Err(format!(
            "{} do(es) not apply to `--kind {AUDIT_KIND}`: an audit re-checks one scenario (--scenario-id/--project-id), its verdict is its --audit-result, and it never flips a task or approves a risk",
            refused.join(", ")
        ));
    }
    if args.scenario_id.is_none() {
        return Err(format!("`--kind {AUDIT_KIND}` needs --scenario-id and --project-id: an audit re-checks one scenario's declared success"));
    }
    let (Some(evaluator_id), Some(evaluator_version), Some(evaluator_digest), Some(result)) =
        (&args.evaluator_id, &args.evaluator_version, &args.evaluator_digest, &args.audit_result)
    else {
        unreachable!("every audit flag is present, checked above");
    };
    for (flag, value) in [("--evaluator-id", evaluator_id), ("--evaluator-version", evaluator_version)] {
        if value.is_empty() || value.trim() != value || first_row_line_break(value).is_some() {
            return Err(format!("{flag} must be a non-empty, trimmed, single-line value"));
        }
    }
    Ok(Some(EvidenceAudit {
        evaluator_id: evaluator_id.clone(),
        evaluator_version: evaluator_version.clone(),
        evaluator_digest: parse_sha256(evaluator_digest)?,
        result: parse_audit_result(result)?,
    }))
}

/// `--drop-attachment` and `--drop-reason` come together, on scenario
/// evidence only (0.15 D2): task-only evidence has no drop guard.
fn check_drop_flags(args: &EvidenceArgs) -> Result<(), String> {
    match (args.drop_attachments.is_empty(), &args.drop_reason) {
        (true, None) => return Ok(()),
        (true, Some(_)) => return Err("--drop-reason explains a --drop-attachment; none was given".to_string()),
        (false, None) => return Err("--drop-attachment needs --drop-reason <text>: a dropped binding is recorded with why it was dropped".to_string()),
        (false, Some(reason)) => {
            if reason.trim().is_empty() || first_row_line_break(reason).is_some() {
                return Err("--drop-reason must be non-empty, single-line text".to_string());
            }
        }
    }
    if args.scenario_id.is_none() {
        return Err("--drop-attachment applies to scenario evidence (--scenario-id); task-only evidence carries no drop guard".to_string());
    }
    Ok(())
}

/// Every well-formed evidence record in the committed ledger and the
/// staging area together, which the drop guard compares against, so an
/// unpromoted record counts too.
fn ledger_and_staged_evidence(ledger_root: &Path) -> Result<Vec<EvidenceRecord>, String> {
    let mut all = Vec::new();
    for root in [ledger_root.to_path_buf(), evidence_staging_dir(ledger_root)] {
        if !root.is_dir() {
            continue;
        }
        let read = GitTier::new(&root)
            .read(&TierQuery::kind(RecordKind::EvidenceRecord))
            .map_err(|e| format!("cannot read {} to compare this record's bindings with the latest one: {e}", root.display()))?;
        let (records, _malformed) = canon_model::validate_evidence_batch(&read.records);
        all.extend(records);
    }
    Ok(all)
}

/// The drop guard (0.15 D2). The scenario's current latest record of the
/// SAME fold — declared, or audit for an audit — is read from the
/// committed ledger and the staging area together, so an unpromoted
/// record counts too. Every path it binds that the new record does not
/// must be named by `--drop-attachment`, and every `--drop-attachment`
/// must name such a path. Re-binding a path to new bytes is not a drop.
/// `Ok(Some)` is the drop to record on the new record; `Ok(None)` when
/// nothing is dropped. `records` is [`ledger_and_staged_evidence`].
fn drop_guard(
    records: &[EvidenceRecord],
    args: &EvidenceArgs,
    (project_id, scenario_id): (&ProjectId, &ScenarioId),
    is_audit: bool,
    attachments: &[canon_model::EvidenceAttachment],
) -> Result<Option<AttachmentDrop>, String> {
    use std::collections::BTreeSet;

    let same_fold: Vec<EvidenceRecord> = records
        .iter()
        .filter(|r| r.is_audit() == is_audit && r.project_id.as_ref() == Some(project_id) && r.scenario_id.as_ref() == Some(scenario_id))
        .cloned()
        .collect();
    let latest = canon_gate::spec_coverage::latest_by_scenario(&same_fold);
    let previous = latest.get(&(project_id.clone(), scenario_id.clone())).copied();
    let bound: BTreeSet<&str> = attachments.iter().map(|a| a.path.as_str()).collect();
    let dropped: BTreeSet<&str> = previous.map(|p| p.attachments.iter().map(|a| a.path.as_str()).filter(|path| !bound.contains(path)).collect()).unwrap_or_default();
    let requested: BTreeSet<&str> = args.drop_attachments.iter().map(String::as_str).collect();
    let fold = if is_audit { "audit record" } else { "evidence record" };
    let scenario = scenario_id.as_str();

    if let Some(stray) = requested.iter().find(|path| !dropped.contains(*path)) {
        let leaves_out = if dropped.is_empty() { "nothing".to_string() } else { dropped.iter().map(|p| format!("`{p}`")).collect::<Vec<_>>().join(", ") };
        return Err(format!(
            "--drop-attachment `{stray}` is not a file the latest {fold} for {scenario} binds and this record leaves out (it leaves out: {leaves_out})"
        ));
    }
    let unexplained: Vec<String> = dropped.difference(&requested).map(|p| format!("`{p}`")).collect();
    if let (false, Some(previous)) = (unexplained.is_empty(), previous) {
        return Err(format!(
            "the latest {fold} for {scenario} (project {}), by `{}` at {}, binds file(s) this record does not: {}; re-bind them (--artifact <path> or --report <format>:<path>; new bytes at the same path are fine), or record the drop with --drop-attachment <path> (repeatable) --drop-reason <text>",
            project_id.as_str(),
            previous.envelope.actor.agent_id,
            previous.envelope.at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
            unexplained.join(", ")
        ));
    }
    Ok(args.drop_reason.as_ref().filter(|_| !requested.is_empty()).map(|reason| AttachmentDrop {
        paths: requested.iter().map(|path| path.to_string()).collect(),
        reason: reason.clone(),
    }))
}

/// Inputs shared by payload export and authenticated evidence staging.
pub struct ApprovalPayloadArgs {
    pub task_id: Option<TaskId>,
    pub scenario_id: Option<ScenarioId>,
    pub project_id: Option<ProjectId>,
    pub run_id: Option<RunId>,
    pub artifact_sha: Sha,
    pub surface_ref: Vec<String>,
    pub approval_by: String,
    pub approval_at: String,
    pub json: bool,
}

fn approval_binding(repo: &Path, args: &ApprovalPayloadArgs) -> Result<(Vec<u8>, Vec<String>, Vec<String>), String> {
    let subject = args.task_id.as_ref().map(ToString::to_string)
        .or_else(|| args.scenario_id.as_ref().map(ToString::to_string))
        .ok_or("give --task or --scenario-id")?;
    if args.scenario_id.is_some() && args.project_id.is_none() {
        return Err("--scenario-id requires --project-id".into());
    }
    if args.approval_by.is_empty() || args.approval_by.trim() != args.approval_by
        || first_row_line_break(&args.approval_by).is_some() {
        return Err("--approval-by must be a non-empty trimmed single-line identity".into());
    }
    let at = args.approval_at.parse::<DateTime<Utc>>()
        .map_err(|_| "--approval-at must be an RFC3339 timestamp")?;
    if let Some(task) = &args.task_id {
        crate::dispatch::validate_task_binding(repo, task).map_err(|error| error.to_string())?;
    }
    let mut surface = canon_gate::risk::artifact_changed_paths(repo, &args.artifact_sha)?;
    for value in &args.surface_ref {
        parse_surface_ref(value)?;
        if value.starts_with("effect:") {
            surface.push(value.clone());
        }
    }
    let surface = normalized_strings(&surface);
    let effects = normalized_strings(&surface.iter().filter_map(|value| value.strip_prefix("effect:").map(str::to_string)).collect::<Vec<_>>());
    let payload = approval_payload_bytes(
        APPROVAL_NAMESPACE, &subject,
        args.project_id.as_ref().map(ToString::to_string).as_deref(),
        &args.artifact_sha.to_string(),
        args.run_id.as_ref().map(ToString::to_string).as_deref(),
        &surface, &effects, &args.approval_by, &at,
    );
    Ok((payload, surface, effects))
}

/// Emit only the exact signable bytes to stdout; never stage or sign.
pub fn run_approval_payload(repo: &Path, args: &ApprovalPayloadArgs) -> i32 {
    use std::io::Write;
    match approval_binding(&resolve_repo_root(repo), args) {
        Ok((payload, surface, effects)) if args.json => {
            let hex: String = payload.iter().map(|byte| format!("{byte:02x}")).collect();
            crate::json_out::data(
                "evidence approval-payload",
                serde_json::json!({ "payloadHex": hex, "payload": String::from_utf8(payload).ok(), "surface": surface, "effects": effects }),
            );
            0
        }
        Ok((payload, _, _)) => match std::io::stdout().lock().write_all(&payload) {
            Ok(()) => 0,
            Err(error) => {
                CliError::refused("WRITE_FAILED", error.to_string()).emit("canon evidence approval-payload", false)
            }
        },
        Err(error) => {
            CliError::refused("EVIDENCE_REFUSED", error.to_string()).emit("canon evidence approval-payload", args.json)
        }
    }
}

/// `canon evidence add` (module doc). Returns the process exit code.
///
/// Order matters and is not incidental: every refusal below happens
/// BEFORE the single `staging.write` at the end, so a refused
/// invocation leaves the staging directory byte-identical — the same
/// "a refused add writes nothing" property `crate::review::run_add`
/// holds, and the reason `crate::dispatch::begin` validates its
/// `--task` before minting anything.
///
/// Not idempotent, deliberately: each call stamps a fresh `at`, so a
/// second identical invocation stages a SECOND record and `canon gate
/// promote` assigns it the next `run_seq`. That mirrors
/// `canon_gate::stage_divergence`, and it is the honest shape — two
/// attestations made at two times are two pieces of evidence, not one
/// re-stated.
pub fn run_add(repo: &Path, args: &EvidenceArgs) -> i32 {
    // `validate_task_binding` resolves `<repo>/canon.yaml` directly, no
    // ancestor walk of its own — so the walk happens HERE, once, exactly
    // as `crate::gate`'s every subcommand does it.
    let repo = resolve_repo_root(repo);
    let promote = match crate::write_mode::should_promote(&repo, args.promote) {
        Ok(promote) => promote,
        Err(error) => {
            return CliError::refused("CONFIG_UNREADABLE", error.to_string()).emit("canon evidence add", args.json);
        }
    };

    if args.kind.trim().is_empty() || args.evidence_ref.trim().is_empty() {
        return CliError::refused("EVIDENCE_INVALID_ARGUMENT", "--kind and --ref must both be non-empty; an empty companion narrows the gate's typed evidence slice to nothing").emit("canon evidence add", args.json);
    }
    if let Err(error) = check_all_scenarios_flags(args) {
        return CliError::refused("EVIDENCE_INVALID_ARGUMENT", error.to_string()).emit("canon evidence add", args.json);
    }
    let audit = match audit_of(args) {
        Ok(audit) => audit,
        Err(error) => {
            return CliError::refused("EVIDENCE_INVALID_ARGUMENT", error.to_string()).emit("canon evidence add", args.json);
        }
    };
    let verdict = match &audit {
        Some(audit) => audit_verdict(audit.result),
        None => args.verdict.unwrap_or(EvidenceVerdict::Faithful),
    };
    if let Err(error) = check_drop_flags(args) {
        return CliError::refused("EVIDENCE_INVALID_ARGUMENT", error.to_string()).emit("canon evidence add", args.json);
    }
    if let Some(reason) = &args.override_reason {
        if reason.trim().is_empty() || first_row_line_break(reason).is_some() {
            return CliError::refused("EVIDENCE_INVALID_ARGUMENT", "--override-reason must be one non-empty line; it is recorded on the evidence record as the accountable reason").emit("canon evidence add", args.json);
        }
    }
    for surface in &args.surface_ref {
        if let Err(error) = parse_surface_ref(surface) {
            return CliError::refused("EVIDENCE_INVALID_ARGUMENT", format!("--surface-ref `{surface}` is invalid: {error}")).emit("canon evidence add", args.json);
        }
    }
    let approval_requested = args.approval_by.is_some()
        || args.approval_role.is_some()
        || args.approval_signature_file.is_some()
        || args.approval_at.is_some();
    if approval_requested {
        if args.approval_by.is_none()
            || args.approval_role.is_none()
            || args.approval_signature_file.is_none()
            || args.approval_at.is_none()
            || args.artifact_sha.is_none()
        {
            return CliError::refused("EVIDENCE_APPROVAL_INVALID", "authenticated approval requires --approval-by, --approval-role human, --approval-signature-file, --approval-at, and --artifact-sha").emit("canon evidence add", args.json);
        }
        let approver = args.approval_by.as_deref().unwrap_or_default();
        if approver.trim().is_empty() || approver.trim() != approver || approver.chars().any(|ch| matches!(ch, '\n' | '\r')) {
            return CliError::refused("EVIDENCE_APPROVAL_INVALID", "approval identity must be a non-empty trimmed single-line value").emit("canon evidence add", args.json);
        }
        if args.approval_role.as_ref().map_or(true, |role| role.as_str() != "human") {
            return CliError::refused("EVIDENCE_APPROVAL_INVALID", "only the human approval role can satisfy risk approval").emit("canon evidence add", args.json);
        }
        if args.approval_at.as_deref().and_then(|value| value.parse::<DateTime<Utc>>().ok()).is_none() {
            return CliError::refused("EVIDENCE_APPROVAL_INVALID", "--approval-at must be an RFC3339 timestamp").emit("canon evidence add", args.json);
        }
        let signature_path = args.approval_signature_file.as_ref().unwrap();
        if !signature_path.is_file() {
            return CliError::refused("EVIDENCE_APPROVAL_INVALID", format!("detached signature file does not exist: {}", signature_path.display())).emit("canon evidence add", args.json);
        }
    }
    if args.summary.is_none() && args.command_result.is_some() {
        return CliError::refused("EVIDENCE_INVALID_ARGUMENT", "--command-result requires --summary; `canon_gate::evidence_note_of` requires `summary`, so a summary-less note is unparseable to the gate rather than partially read").emit("canon evidence add", args.json);
    }

    // The document-bound fields, refused before anything is staged
    // (module doc's injection section). Both of these — and ONLY these —
    // reach a plan document: `--summary` becomes the row's ` — ✅ `
    // suffix verbatim, and `--actor-id` becomes it via
    // `default_evidence_text` when no summary is given. Checked through
    // the row grammar's OWN separator set rather than a local `contains
    // ('\n')`, so this refusal and the write-back's cannot disagree about
    // which inputs are safe.
    for (flag, value) in [("--summary", args.summary.as_deref()), ("--actor-id", Some(args.actor_id.as_str()))] {
        let Some((offset, separator)) = value.and_then(first_row_line_break) else { continue };
        return CliError::refused("EVIDENCE_INVALID_ARGUMENT", format!("{flag} carries the line separator {separator:?} at byte offset {offset}; it is written into a plan document as ONE checkbox row, so a separator there appends a second row (a `- [x] ` one, if the value says so) that no evidence record backs")).emit("canon evidence add", args.json);
    }
    if args.all_scenarios {
        return run_add_all(&repo, args, promote);
    }

    // At least one coverage subject, checked before anything else
    // touches the corpus. A record with neither key is invisible to
    // every gate (`canon_gate::coverage::CellSubject::of` yields
    // `None`), so staging one would be writing an attestation nothing
    // can ever read back.
    if args.task_id.is_none() && args.scenario_id.is_none() {
        return CliError::refused("EVIDENCE_INVALID_ARGUMENT", "give --task, --scenario-id, or both; a record with neither carries no coverage subject and no gate can read it back").emit("canon evidence add", args.json);
    }

    // A scenario-keyed record needs the COMPOSITE key, because
    // `Scenario`'s own `project_id` is required and two spec roots may
    // carry the same scenario id. Refused rather than defaulted: there
    // is no safe guess when `specs.roots[]` holds more than one entry.
    if args.scenario_id.is_some() && args.project_id.is_none() {
        return CliError::refused("EVIDENCE_INVALID_ARGUMENT", "--scenario-id needs --project-id; canon keys a scenario by the composite (project_id, scenario_id) and two spec roots may carry the same scenario id").emit("canon evidence add", args.json);
    }

    // The SAME plan-corpus admission `canon dispatch begin --task` makes
    // (module doc) — an id no import pass would persist as a `Task`
    // record is not an id evidence may be attested against. Skipped
    // entirely for a scenario-only record: there is no task to admit.
    if let Some(task_id) = &args.task_id {
        if let Err(e) = crate::dispatch::validate_task_binding(&repo, task_id) {
            return e.to_cli_error().emit("canon evidence add", args.json);
        }
    }

    // A summary is allowed with or without a task (0.14 D5, dogfood F8:
    // refusing it on scenario-only evidence pushed an agent into
    // inventing a scenario→task mapping just to keep its notes). Only a
    // task-keyed record's flip turns it into a row suffix.
    if let Some(summary) = &args.summary {
        // The gate's own scan, run here so a fabricated note is refused
        // while it is still a flag value — after `canon gate promote`
        // the record is in the append-only committed ledger and the
        // refusal is no longer fixable by re-running the command.
        let subject = match (&args.task_id, &args.scenario_id) {
            (Some(task_id), _) => task_id.to_string(),
            (None, Some(scenario_id)) => scenario_id.as_str().to_string(),
            (None, None) => unreachable!("one of --task/--scenario-id is required, checked above"),
        };
        // Worded and exited like every other gate-red refusal of this
        // command (0.18 D5, #32): `refused —`, exit 1.
        let violations = scan_note_text(&subject, summary, args.command_result.as_deref());
        if !violations.is_empty() {
            return fabricated(&violations).emit("canon evidence add", args.json);
        }
    }

    // The typed `{kind, ref}` contract, read through the gate's own
    // resolution (module doc). `Ok(None)` = the free path, where the
    // gate accepts any non-`Divergent` record for this task regardless
    // of kind, so there is nothing to check. A scenario-only record has
    // no task atom to bind a contract to, so the whole step is skipped.
    if let Some(task_id) = &args.task_id {
        match typed_evidence_contract_for_task(&repo, task_id) {
            Ok(Some(contract)) if contract.kind != args.kind || contract.evidence_ref != args.evidence_ref => {
                return CliError::failed("EVIDENCE_CONTRACT_MISMATCH", format!("{} declares a typed evidence contract kind=`{}` ref=`{}`, but this record carries kind=`{}` ref=`{}`; `canon gate task` narrows to records matching the atom exactly, so this one would be refused as an unevidenced flip",
                    task_id, contract.kind, contract.evidence_ref, args.kind, args.evidence_ref
                )).emit("canon evidence add", args.json);
            }
            Ok(_) => {}
            Err(e) => {
                return e.to_cli_error().emit("canon evidence add", args.json);
            }
        }
    }
    // 0.16 M7: nothing is attested for a subject whose brief awaits the
    // human's answer (module doc). Before anything is bound or staged.
    let mut gate_context = match load_gate_context(&repo) {
        Ok(ctx) => ctx,
        Err(e) => {
            return CliError::refused("REPO_UNREADABLE", e.to_string()).emit("canon evidence add", args.json);
        }
    };
    let refusals = brief_refusals(&repo, &gate_context, args.task_id.as_ref(), args.project_id.as_ref().zip(args.scenario_id.as_ref()));
    if let Err(error) = report_brief_refusals(args, &refusals) {
        return error.emit("canon evidence add", args.json);
    }
    let brief_override = brief_override_of(args, &refusals);
    // Experimental binding: read (never run) the files the caller names,
    // before anything is staged, so a refused binding leaves no record.
    if !args.report_cases.is_empty() && args.reports.is_empty() {
        return CliError::refused("EVIDENCE_INVALID_ARGUMENT", "--report-case names a case in a --report; none was given").emit("canon evidence add", args.json);
    }
    let bound = match crate::evidence_attach::bind(&crate::evidence_attach::BindRequest {
        repo: &repo,
        artifacts: &args.artifacts,
        reports: &args.reports,
        report_cases: &args.report_cases,
        scenario_id: args.scenario_id.as_ref().map(ScenarioId::as_str),
        // 0.18 D1: a task's report with no scenario to match binds all
        // of it, so a faithful verdict needs every case to pass.
        all_cases: args.task_id.is_some() && args.scenario_id.is_none() && args.report_cases.is_empty(),
        verdict,
        max_bytes: args.max_artifact_mib.saturating_mul(canon_gate::artifact_store::MIB),
    }) {
        Ok(bound) => bound,
        Err(error) => {
            return bind_refusal(&error).emit("canon evidence add", args.json);
        }
    };

    let ledger_root = gate_context.ctx.ledger_root.clone();
    // 0.15 D2: a new latest record never silently drops what the
    // previous one bound. Checked after binding (the new paths are known)
    // and before anything is stored or staged.
    let attachment_drop = match args.project_id.as_ref().zip(args.scenario_id.as_ref()) {
        None => None,
        Some(scenario) => match ledger_and_staged_evidence(&ledger_root).and_then(|records| drop_guard(&records, args, scenario, audit.is_some(), &bound.attachments)) {
            Ok(attachment_drop) => attachment_drop,
            Err(error) => {
                return CliError::refused("EVIDENCE_REFUSED", error.to_string()).emit("canon evidence add", args.json);
            }
        },
    };

    let at = Utc::now();
    let mut actor = Actor::new(args.actor_id.as_str(), args.role.clone());
    if let Some(session_id) = &args.session_id {
        actor = actor.with_session(session_id.clone());
    }
    let mut record = EvidenceRecord::new(
        Envelope::current(RecordKind::EvidenceRecord, at, actor),
        args.task_id.clone(),
        args.scenario_id.clone(),
        args.run_id.clone(),
        verdict,
    )
    .with_attachments(bound.attachments);
    if let Some(attachment_drop) = attachment_drop {
        record = record.with_attachment_drop(attachment_drop);
    }
    if let Some(brief_override) = brief_override {
        record = record.with_brief_override(brief_override);
    }
    if let Some(audit) = audit {
        record = record.with_audit(audit);
    }
    let record_audit_result = record.audit.as_ref().map(|a| a.result);
    if !args.surface_ref.is_empty() {
        record = record.with_surface_ref(normalized_strings(&args.surface_ref));
    }
    if let Some(project_id) = &args.project_id {
        record = record.with_project_id(project_id.clone());
    }
    if let Some(artifact_sha) = &args.artifact_sha {
        record = record.with_evidence_sha(artifact_sha.clone());
    }
    if approval_requested {
        let approver = args.approval_by.as_ref().unwrap();
        let role = args.approval_role.as_ref().unwrap();
        let approval_at = args.approval_at.as_ref().unwrap().parse::<DateTime<Utc>>().unwrap();
        let artifact_sha = args.artifact_sha.as_ref().unwrap();
        let subject = args.task_id.as_ref().map(ToString::to_string).or_else(|| args.scenario_id.as_ref().map(ToString::to_string)).unwrap();
        let (payload, surface, effects) = match approval_binding(&repo, &ApprovalPayloadArgs {
            task_id: args.task_id.clone(),
            scenario_id: args.scenario_id.clone(),
            project_id: args.project_id.clone(),
            run_id: args.run_id.clone(),
            artifact_sha: artifact_sha.clone(),
            surface_ref: args.surface_ref.clone(),
            approval_by: approver.clone(),
            approval_at: args.approval_at.clone().unwrap(),
            json: args.json,
        }) {
            Ok(binding) => binding,
            Err(error) => {
                return CliError::refused("EVIDENCE_REFUSED", error.to_string()).emit("canon evidence add", args.json);
            }
        };
        record = record.with_surface_ref(surface.clone());
        let signature_path = args.approval_signature_file.as_ref().unwrap();
        let signature = match std::fs::read(signature_path) {
            Ok(signature) => signature,
            Err(error) => {
                return CliError::refused("EVIDENCE_APPROVAL_INVALID", format!("read detached signature: {error}")).emit("canon evidence add", args.json);
            }
        };
        let allowed_signers = match canon_gate::policy::allowed_signers_path(&repo) {
            Ok(Some(path)) => path,
            Ok(None) => {
                return CliError::refused("EVIDENCE_APPROVAL_INVALID", "no policy-pinned allowed_signers verifier is configured").emit("canon evidence add", args.json);
            }
            Err(error) => {
                return CliError::refused("EVIDENCE_APPROVAL_INVALID", format!("approval policy is malformed: {error}")).emit("canon evidence add", args.json);
            }
        };
        if let Err(error) = verify_risk_approval(&payload, &signature, approver, &allowed_signers) {
            return CliError::failed("EVIDENCE_APPROVAL_INVALID", error.to_string()).emit("canon evidence add", args.json);
        }
        let signature = match String::from_utf8(signature) {
            Ok(signature) => signature,
            Err(_) => {
                return CliError::refused("EVIDENCE_APPROVAL_INVALID", "detached signature must be UTF-8 armored SSH signature text").emit("canon evidence add", args.json);
            }
        };
        record = record.with_approval(EvidenceApproval {
            approver: approver.clone(),
            role: role.clone(),
            at: approval_at,
            verified: false,
            signature: Some(signature),
            subject: Some(subject),
            project_id: args.project_id.clone(),
            artifact_sha: Some(artifact_sha.clone()),
            run_id: args.run_id.clone(),
            surface,
            effects,
        });
    }

    let body = record_body(&record, args);
    let stale_reviews = stale_review_notices(&mut gate_context, std::slice::from_ref(&record));

    // Store the bound bytes before staging (module doc): every refusal
    // is behind us, and a record must never name a blob the store lacks.
    let stored = match crate::evidence_attach::store_files(&repo, &bound.files) {
        Ok(stored) => stored,
        Err((file, e)) => {
            return CliError::refused("EVIDENCE_STORE_FAILED", format!("could not store {} in the artifact store: {e}", file.resolved.display())).emit("canon evidence add", args.json);
        }
    };
    if !bound.files.is_empty() {
        crate::say!(args.json, "{}", crate::evidence_attach::stored_line("canon evidence add", bound.files.len(), stored));
    }

    let staging = GitTier::new(evidence_staging_dir(&ledger_root));
    match staging.write(&RawWrite(RawRecord(body))) {
        Ok(receipt) => {
            // The next step differs by subject: a task-keyed record
            // feeds a checkbox flip, a scenario-only one feeds
            // `canon gate check`'s spec-coverage pass. Naming the wrong
            // one would send an operator to a command that cannot apply.
            let (subject, next) = match &args.task_id {
                Some(task_id) => (task_id.to_string(), format!("then `canon gate task {task_id}`")),
                None => {
                    let scenario = args.scenario_id.as_ref().expect("one of --task/--scenario-id is required, checked above");
                    (scenario.as_str().to_string(), "then `canon gate check`".to_string())
                }
            };
            let outcome = match &record_audit_result {
                Some(result) => format!("audit {}", result.as_str()),
                None => format!("verdict {}", verdict_slug(verdict)),
            };
            let settled = crate::gate::promote_after_stage("canon evidence add", &ledger_root, &receipt.location, promote, args.json);
            crate::say!(args.json, "canon evidence add: {} for {} ({outcome}) — {}, {}", settled.landed(&receipt.location), subject, settled.suffix(), next);
            let stale = print_stale_reviews(&repo, &stale_reviews, matches!(settled, crate::gate::StagedOutcome::Promoted { .. }), args.json);
            if settled.exit_code() == 0 {
                let mut record = settled.json(&receipt.location);
                record["subject"] = serde_json::json!(subject);
                record["outcome"] = serde_json::json!(outcome);
                record["next"] = serde_json::json!(next);
                let scope = match &args.task_id {
                    Some(_) => crate::next::Scope::Repo,
                    None => crate::next::Scope::Scenarios(vec![subject.clone()]),
                };
                let staged = matches!(settled, crate::gate::StagedOutcome::Staged);
                crate::next::finish(&repo, "evidence add", &scope, staged, args.json, serde_json::json!({ "records": [record], "staleReviews": stale }));
            }
            settled.exit_code()
        }
        Err(e) => {
            CliError::refused("WRITE_FAILED", e.to_string()).emit("canon evidence add", args.json)
        }
    }
}

/// The staged body of `record`: the record, plus the `evidence: {kind,
/// ref}` companion and, with `--summary`, the `evidence_note` companion
/// (module doc).
fn record_body(record: &EvidenceRecord, args: &EvidenceArgs) -> serde_json::Value {
    // `serde_json::to_value` on a record canon just constructed, and
    // `as_object_mut` on the object that produced — the identical pair
    // of canon-originated-data `expect`s `canon_gate::promote` states
    // when it stamps `run_seq` onto a body it just read.
    let mut body = serde_json::to_value(record).expect("an EvidenceRecord always serializes");
    let object = body.as_object_mut().expect("an EvidenceRecord's serialized body is always a JSON object");
    object.insert("evidence".to_string(), serde_json::json!({ "kind": args.kind, "ref": args.evidence_ref }));
    if let Some(summary) = &args.summary {
        // The SAME values that were scanned, so the bytes on disk are the
        // bytes the scan cleared. `task_id` is already the record's own
        // typed field; the companion carries only what
        // `evidence_note_of` reads back.
        let mut companion = serde_json::Map::new();
        companion.insert("summary".to_string(), serde_json::Value::String(summary.clone()));
        if let Some(command_result) = &args.command_result {
            companion.insert("command_result".to_string(), serde_json::Value::String(command_result.clone()));
        }
        object.insert("evidence_note".to_string(), serde_json::Value::Object(companion));
    }
    body
}

/// Prints `refusals` (0.16 M7, module doc). `Err(1)` when they refuse
/// the attestation; with `--override-reason` they are printed as waived.
fn report_brief_refusals(args: &EvidenceArgs, refusals: &[BriefRefusal]) -> Result<(), CliError> {
    if refusals.is_empty() {
        if args.override_reason.is_some() {
            eprintln!("canon evidence add: --override-reason not recorded — no unapproved brief refused this attestation");
        }
        return Ok(());
    }
    let Some(reason) = &args.override_reason else {
        let lines = refusals.iter().map(|r| r.violation.line()).collect::<Vec<_>>().join("\n");
        let code = if refusals.iter().any(|r| r.changed) { crate::brief::CHANGED_CODE } else { "BRIEF_UNAPPROVED" };
        return Err(CliError::failed(code, lines).hint(BRIEF_STOP));
    };
    for refusal in refusals {
        eprintln!("canon evidence add: waived {}", refusal.violation.line());
    }
    eprintln!("canon evidence add: override recorded by `{}` for the {} brief refusal(s) above: {reason}", args.actor_id, refusals.len());
    Ok(())
}

/// The [`BriefOverride`] a record carries for the brief `refusals` it
/// waived, or `None` when nothing was waived.
fn brief_override_of(args: &EvidenceArgs, refusals: &[BriefRefusal]) -> Option<BriefOverride> {
    let reason = args.override_reason.as_ref().filter(|_| !refusals.is_empty())?;
    Some(BriefOverride {
        reason: reason.clone(),
        waived: refusals.iter().map(|r| WaivedViolation { class: r.violation.class.as_str().to_string(), subject: r.violation.subject.clone() }).collect(),
        actor: Actor::new_unattributed(args.actor_id.as_str()),
    })
}

/// `--all-scenarios` (0.18 D1) checks its flag combination before
/// anything else: it needs a `--report` to split and the `--project-id`
/// whose indexed scenarios own the cases, and refuses every flag that
/// names one subject (`--scenario-id`, `--task`, `--report-case`, an
/// audit, a drop, an approval).
fn check_all_scenarios_flags(args: &EvidenceArgs) -> Result<(), String> {
    if !args.all_scenarios {
        return Ok(());
    }
    let one_subject = [
        ("--scenario-id", args.scenario_id.is_some()),
        ("--task", args.task_id.is_some()),
        ("--report-case", !args.report_cases.is_empty()),
        ("--kind audit", args.kind == AUDIT_KIND),
        ("--drop-attachment", !args.drop_attachments.is_empty()),
        ("--drop-reason", args.drop_reason.is_some()),
        ("--approval-by", args.approval_by.is_some()),
        ("--approval-role", args.approval_role.is_some()),
        ("--approval-signature-file", args.approval_signature_file.is_some()),
        ("--approval-at", args.approval_at.is_some()),
    ];
    let refused: Vec<&str> = one_subject.iter().filter(|(_, set)| *set).map(|(flag, _)| *flag).collect();
    if !refused.is_empty() {
        return Err(format!(
            "{} name(s) one subject and do(es) not combine with --all-scenarios, which writes one record per scenario the --report owns; attest one scenario on its own with --scenario-id and without --all-scenarios",
            refused.join(", ")
        ));
    }
    if args.reports.is_empty() {
        return Err("--all-scenarios splits a --report <format>:<path> by the scenario each case belongs to; none was given".to_string());
    }
    if args.project_id.is_none() {
        return Err("--all-scenarios needs --project-id: the cases are matched against that spec root's indexed scenarios".to_string());
    }
    Ok(())
}

/// `canon evidence add --all-scenarios` (0.18 D1): one record per indexed
/// scenario of `--project-id` that owns a case in the reports
/// ([`crate::evidence_attach::bind_all_scenarios`]), each with its own
/// cases and its own verdict (`--verdict` when given, else derived from
/// its cases). All or nothing: the fabrication scan, every record's brief
/// refusals and drop guard, and a claimed `faithful` verdict over any
/// failed case run first, and any refusal writes nothing. The records
/// share the stored report blob and `--artifact` files. Prints one line
/// per record, then each unindexed scenario id it skipped, then any
/// review the batch makes stale.
fn run_add_all(repo: &Path, args: &EvidenceArgs, promote: bool) -> i32 {
    let project_id = args.project_id.as_ref().expect("--all-scenarios requires --project-id, checked by check_all_scenarios_flags");
    let mut gate_context = match load_gate_context(repo) {
        Ok(ctx) => ctx,
        Err(e) => {
            return CliError::refused("REPO_UNREADABLE", e.to_string()).emit("canon evidence add", args.json);
        }
    };
    let mut indexed: Vec<&ScenarioId> =
        canon_gate::spec_coverage::latest_scenarios(&gate_context).into_iter().filter(|s| &s.project_id == project_id).map(|s| &s.scenario_id).collect();
    indexed.sort();
    let indexed: Vec<&str> = indexed.into_iter().map(ScenarioId::as_str).collect();
    let batch = match crate::evidence_attach::bind_all_scenarios(&crate::evidence_attach::BatchRequest {
        repo,
        artifacts: &args.artifacts,
        reports: &args.reports,
        indexed: &indexed,
        verdict: args.verdict,
        max_bytes: args.max_artifact_mib.saturating_mul(canon_gate::artifact_store::MIB),
    }) {
        Ok(batch) => batch,
        Err(error) => {
            return bind_refusal(&error).emit("canon evidence add", args.json);
        }
    };
    let scenario_ids: Vec<ScenarioId> = match batch.scenarios.iter().map(|b| ScenarioId::parse(&b.scenario_id)).collect() {
        Ok(ids) => ids,
        Err(e) => {
            return CliError::refused("EVIDENCE_INVALID_ARGUMENT", e.to_string()).emit("canon evidence add", args.json);
        }
    };

    if let Some(summary) = &args.summary {
        let subject = scenario_ids.iter().map(ScenarioId::as_str).collect::<Vec<_>>().join(", ");
        let violations = scan_note_text(&subject, summary, args.command_result.as_deref());
        if !violations.is_empty() {
            return fabricated(&violations).hint("no record of the batch is written").emit("canon evidence add", args.json);
        }
    }

    let refusals: Vec<Vec<BriefRefusal>> = scenario_ids.iter().map(|id| brief_refusals(repo, &gate_context, None, Some((project_id, id)))).collect();
    if let Err(error) = report_brief_refusals(args, &refusals.concat()) {
        let hint = format!("no record of the batch is written; {}", error.hint.as_deref().unwrap_or_default());
        return error.hint(hint).emit("canon evidence add", args.json);
    }

    let ledger_root = gate_context.ctx.ledger_root.clone();
    let existing = match ledger_and_staged_evidence(&ledger_root) {
        Ok(records) => records,
        Err(error) => {
            return CliError::refused("EVIDENCE_REFUSED", error.to_string()).emit("canon evidence add", args.json);
        }
    };
    let drops: Vec<String> = batch
        .scenarios
        .iter()
        .zip(&scenario_ids)
        .filter_map(|(binding, id)| drop_guard(&existing, args, (project_id, id), false, &binding.attachments).err())
        .collect();
    if !drops.is_empty() {
        return CliError::refused("EVIDENCE_BATCH_REFUSED", drops.join("\n"))
            .hint("no record of the batch is written; attest a scenario whose files changed on its own with --scenario-id (and --drop-attachment if it drops one)")
            .emit("canon evidence add", args.json);
    }

    let at = Utc::now();
    let mut actor = Actor::new(args.actor_id.as_str(), args.role.clone());
    if let Some(session_id) = &args.session_id {
        actor = actor.with_session(session_id.clone());
    }
    let records: Vec<EvidenceRecord> = batch
        .scenarios
        .iter()
        .zip(&scenario_ids)
        .zip(&refusals)
        .map(|((binding, id), refusals)| {
            let mut record = EvidenceRecord::new(Envelope::current(RecordKind::EvidenceRecord, at, actor.clone()), None, Some(id.clone()), args.run_id.clone(), binding.verdict)
                .with_attachments(binding.attachments.clone())
                .with_project_id(project_id.clone());
            if let Some(brief_override) = brief_override_of(args, refusals) {
                record = record.with_brief_override(brief_override);
            }
            if !args.surface_ref.is_empty() {
                record = record.with_surface_ref(normalized_strings(&args.surface_ref));
            }
            if let Some(artifact_sha) = &args.artifact_sha {
                record = record.with_evidence_sha(artifact_sha.clone());
            }
            record
        })
        .collect();
    let stale_reviews = stale_review_notices(&mut gate_context, &records);

    let bodies: Vec<serde_json::Value> = records.iter().map(|record| record_body(record, args)).collect();
    let landed = match land_batch(repo, &ledger_root, &batch.files, bodies, promote, &mut |tier, write| tier.write(write)) {
        Ok(landed) => landed,
        Err((error, code)) => {
            return CliError::failed("EVIDENCE_BATCH_REFUSED", error.to_string())
                .hint("no record of the batch is written: every record and blob this run staged, promoted or stored was removed")
                .with_exit(code)
                .emit("canon evidence add", args.json);
        }
    };
    let json = args.json;
    crate::say!(json, "{}", crate::evidence_attach::stored_line("canon evidence add", batch.files.len(), landed.stored));
    let mut written = Vec::new();
    for ((record, location), id) in records.iter().zip(&landed.locations).zip(&scenario_ids) {
        let settled = match &location.promoted {
            Some(target) => crate::gate::StagedOutcome::Promoted { target: target.clone() },
            None => crate::gate::StagedOutcome::Staged,
        };
        crate::say!(
            json,
            "canon evidence add: {} for {} (verdict {}) — {}, then `canon gate check`",
            settled.landed(&location.staged),
            id.as_str(),
            verdict_slug(record.verdict),
            settled.suffix()
        );
        let mut entry = settled.json(&location.staged);
        entry["subject"] = serde_json::json!(id.as_str());
        entry["outcome"] = serde_json::json!(format!("verdict {}", verdict_slug(record.verdict)));
        written.push(entry);
    }
    let mut skipped = Vec::new();
    for (id, case) in &batch.unindexed {
        crate::say!(json, "canon evidence add: skipped {id} — not an indexed scenario of spec root `{}` (case `{case}`); index it (`canon inventory sync`) or fix the test name", project_id.as_str());
        skipped.push(serde_json::json!({ "scenarioId": id, "case": case, "reason": "not an indexed scenario" }));
    }
    if batch.without_id > 0 {
        crate::say!(json, "canon evidence add: skipped {} case(s) that carry no scenario id", batch.without_id);
    }
    let stale = print_stale_reviews(repo, &stale_reviews, promote, json);
    let scope = crate::next::Scope::Scenarios(scenario_ids.iter().map(|id| id.as_str().to_string()).collect());
    crate::next::finish(repo, "evidence add", &scope, !promote, json, serde_json::json!({ "records": written, "skipped": skipped, "casesWithoutScenarioId": batch.without_id, "staleReviews": stale }));
    0
}

/// Where one record of a batch landed: its staging location, and its
/// committed path when it was promoted.
struct BatchLocation {
    staged: String,
    promoted: Option<String>,
}

/// What [`land_batch`] wrote.
struct LandedBatch {
    /// Blobs newly stored.
    stored: usize,
    locations: Vec<BatchLocation>,
}

/// Write a batch all or nothing (0.18 D1): check that the staging (and,
/// with `promote`, the committed) evidence directory can be written,
/// store every bound file under the store lock, stage every body through
/// `write`, then promote every staged record. A failure at any step
/// removes every record this call staged or promoted and every blob it
/// newly stored (the lock is held throughout, so no other writer's blob
/// is touched), and is returned as the error. `write` is the staging
/// write, `GitTier::write` outside tests.
fn land_batch(
    repo: &Path,
    ledger_root: &Path,
    files: &[crate::evidence_attach::BoundFile],
    bodies: Vec<serde_json::Value>,
    promote: bool,
    write: &mut dyn FnMut(&GitTier, &RawWrite) -> Result<canon_store::tier::WriteReceipt, canon_store::tier::StoreError>,
) -> Result<LandedBatch, (String, i32)> {
    let staging_root = evidence_staging_dir(ledger_root);
    let kind_dir = format!("kind={}", RecordKind::EvidenceRecord.as_str());
    let mut destinations = vec![staging_root.join(&kind_dir)];
    if promote {
        destinations.push(ledger_root.join(&kind_dir));
    }
    for dir in &destinations {
        let writable = std::fs::create_dir_all(dir).and_then(|()| std::fs::metadata(dir)).map(|m| !m.permissions().readonly());
        match writable {
            Ok(true) => {}
            Ok(false) => return Err((format!("{} is read-only", dir.display()), 2)),
            Err(e) => return Err((format!("cannot prepare {}: {e}", dir.display()), 2)),
        }
    }

    let held = crate::evidence_attach::store_files_held(repo, files)
        .map_err(|(file, e)| (format!("could not store {} in the artifact store: {e}", file.resolved.display()), 2))?;
    let staging = GitTier::new(&staging_root);
    let mut locations: Vec<BatchLocation> = Vec::new();
    let rollback = |locations: &[BatchLocation], held: crate::evidence_attach::HeldStore| {
        for location in locations {
            let _ = std::fs::remove_file(staging_root.join(&location.staged));
            if let Some(target) = &location.promoted {
                let _ = std::fs::remove_file(ledger_root.join(target));
            }
        }
        held.rollback();
    };
    for body in bodies {
        match write(&staging, &RawWrite(RawRecord(body))) {
            Ok(receipt) => locations.push(BatchLocation { staged: receipt.location, promoted: None }),
            Err(e) => {
                let error = format!("staging record {} of the batch failed: {e}", locations.len() + 1);
                rollback(&locations, held);
                return Err((error, 2));
            }
        }
    }
    if promote {
        for index in 0..locations.len() {
            match crate::gate::try_promote(ledger_root, &locations[index].staged) {
                Ok(target) => locations[index].promoted = Some(target),
                Err(refusal) => {
                    let why = if refusal.lines.trim().is_empty() { refusal.error.message.clone() } else { refusal.lines.trim_end().to_string() };
                    let error = format!("promoting record {} of the batch ({}) was refused: {why}", index + 1, locations[index].staged);
                    rollback(&locations, held);
                    return Err((error, 1));
                }
            }
        }
    }
    let stored = held.stored();
    Ok(LandedBatch { stored, locations })
}

/// The in-effect pinned reviews that `written` makes stale (0.18 D2),
/// decided by the gate's own [`review_gap`]: a review is listed when,
/// judged alone, it qualifies before the write (signed, its pins
/// current) and is a [`ReviewGapKind::Stale`] gap once `written` is the
/// scenario's latest evidence. `ctx` is restored before returning.
fn stale_review_notices(ctx: &mut GateContext, written: &[EvidenceRecord]) -> Vec<Review> {
    let declared: Vec<EvidenceRecord> = written.iter().filter(|r| !r.is_audit() && r.scenario_id.is_some()).cloned().collect();
    let touched = |s: &Scenario| declared.iter().any(|r| r.scenario_id.as_ref() == Some(&s.scenario_id) && r.project_id.as_ref().is_none_or(|p| *p == s.project_id));
    let scenarios: Vec<Scenario> = canon_gate::spec_coverage::latest_scenarios(ctx).into_iter().filter(|s| touched(s)).cloned().collect();
    let candidates: Vec<(Scenario, Review)> = scenarios
        .iter()
        .flat_map(|s| {
            in_effect_reviews(ctx)
                .into_iter()
                .filter(|r| r.pins.is_some() && r.project_id == s.project_id && r.scenario_id == s.scenario_id)
                .map(|r| (s.clone(), r.clone()))
                .collect::<Vec<_>>()
        })
        .collect();
    if candidates.is_empty() {
        return Vec::new();
    }
    let reviews = std::mem::take(&mut ctx.reviews);
    let mut current_before = Vec::new();
    for (scenario, review) in &candidates {
        ctx.reviews = vec![review.clone()];
        current_before.push(review_gap(ctx, scenario, false).is_none());
    }
    let evidence = ctx.evidence.len();
    ctx.evidence.extend(declared);
    let mut stale = Vec::new();
    for ((scenario, review), was_current) in candidates.into_iter().zip(current_before) {
        ctx.reviews = vec![review.clone()];
        if was_current && review_gap(ctx, &scenario, false).is_some_and(|gap| gap.kind == ReviewGapKind::Stale) {
            stale.push(review);
        }
    }
    ctx.evidence.truncate(evidence);
    ctx.reviews = reviews;
    stale
}

/// One line per review `stale` names (0.18 D2): the reviewer, the
/// scenario, and the `canon review add` that re-pins it — the same
/// reviewer, actor, role, session, provenance ref, runtime labels and
/// artifacts, pinned at
/// `HEAD` (`<sha>` when the repository has no commit). A record still
/// staged makes the review stale once it is promoted.
fn print_stale_reviews(repo: &Path, stale: &[Review], promoted: bool, json: bool) -> Vec<serde_json::Value> {
    if stale.is_empty() {
        return Vec::new();
    }
    let pin = head_sha(repo).unwrap_or_else(|| "<sha>".to_string());
    let when = if promoted { "is now stale" } else { "goes stale once this evidence is promoted" };
    stale
        .iter()
        .map(|review| {
            let repin = repin_command(review, &pin);
            crate::say!(
                json,
                "canon evidence add: review by `{}` of {} {when} (it pinned the evidence this record supersedes); re-pin it: {repin}",
                review.reviewer,
                review.scenario_id.as_str()
            );
            serde_json::json!({ "reviewer": review.reviewer, "scenarioId": review.scenario_id.as_str(), "repin": repin })
        })
        .collect()
}

/// `canon review add` re-pinning `review` at `pin`, as one shell line.
fn repin_command(review: &Review, pin: &str) -> String {
    use canon_gate::audit::shell_quote;

    let role = review.envelope.actor.role.as_ref().map_or("reviewer", |r| r.as_str());
    let mut words = vec![
        "canon review add".to_string(),
        format!("--project-id {}", shell_quote(review.project_id.as_str())),
        format!("--scenario-id {}", shell_quote(review.scenario_id.as_str())),
        format!("--reviewer {}", shell_quote(&review.reviewer)),
        format!("--actor-id {}", shell_quote(&review.envelope.actor.agent_id)),
        format!("--role {}", shell_quote(role)),
        format!("--pin {}", if pin == "<sha>" { pin.to_string() } else { shell_quote(pin) }),
    ];
    if let Some(session) = &review.envelope.actor.session_id {
        words.push(format!("--session-id {}", shell_quote(session.as_str())));
    }
    words.push(match &review.provenance_ref {
        ProvenanceRef::UpstreamRef(r) => format!("--upstream-ref {}", shell_quote(r)),
        ProvenanceRef::OriginalSpecRef(r) => format!("--original-spec-ref {}", shell_quote(r)),
    });
    for (flag, value) in [
        ("--reviewer-provider", &review.reviewer_provider),
        ("--reviewer-model", &review.reviewer_model),
        ("--reviewer-harness", &review.reviewer_harness),
    ] {
        if let Some(value) = value {
            words.push(format!("{flag} {}", shell_quote(value)));
        }
    }
    words.extend(review.artifacts.iter().map(|a| format!("--artifact {}", shell_quote(&a.path))));
    words.join(" ")
}

/// The full sha of `HEAD`, or `None` when the repository has no commit.
fn head_sha(repo: &Path) -> Option<String> {
    let out = std::process::Command::new("git").arg("-C").arg(repo).args(["rev-parse", "--verify", "--quiet", "HEAD"]).output().ok()?;
    let sha = String::from_utf8(out.stdout).ok()?.trim().to_string();
    (out.status.success() && !sha.is_empty()).then_some(sha)
}

/// `canon evidence vault` (0.14 D4): store the bytes for committed
/// evidence records written before the artifact store existed, and for
/// the files of current reviews whose blob was lost (0.18 D5).
///
/// Every distinct `(path, sha256)` attachment across the committed
/// ledger's evidence records, and every artifact of a current in-effect
/// review (`canon_gate::review_gate::current_reviews`, the set `canon
/// gate check` re-checks; a superseded or stale review's files are
/// neither checked nor vaulted): already stored intact → counted;
/// otherwise the working-tree file is stored when it still hashes to the
/// recorded digest and fits the size limit. Everything else is listed as
/// not stored, with why — those bytes are gone, and `canon gate check`
/// reports a LATEST record, or a current review, bound to them as
/// `stale-evidence` (re-attest to fix). Exit `0` when the pass ran,
/// whatever it could store: whether an unstored file matters is the
/// gate's question, and a superseded record's lost file never does. `2`
/// when the ledger cannot be read or a write fails.
pub fn run_vault(repo: &Path, max_artifact_mib: u64, json: bool) -> i32 {
    use canon_gate::artifact_store::{blob_display, stored_blob, working_tree_path, StoreError, StoreLock, StoreOutcome, MIB};

    let repo = resolve_repo_root(repo);
    // The gate's own context, so the reviews vaulted are exactly the ones
    // it re-checks. Its evidence read is `validate_evidence_batch` over
    // every committed record, split into declared records and audits.
    let mut gate_context = match GateCtx::from_repo(&repo).map_err(|e| e.to_string()).and_then(|ctx| GateContext::load(ctx, &SchemaRegistry::load(), Utc::now()).map_err(|e| e.to_string())) {
        Ok(gate_context) => gate_context,
        Err(e) => {
            return CliError::refused("CONFIG_UNREADABLE", e.to_string()).emit("canon evidence vault", json);
        }
    };
    crate::inventory::resolve_legacy_pins(&mut gate_context);
    let files: std::collections::BTreeSet<(String, String)> = gate_context
        .evidence
        .iter()
        .chain(&gate_context.audits)
        .flat_map(|r| r.attachments.iter().map(|a| (a.path.clone(), a.sha256.clone())))
        .chain(canon_gate::review_gate::current_reviews(&gate_context).into_iter().flat_map(|r| r.artifacts.iter().map(|a| (a.path.clone(), a.sha256.clone()))))
        .collect();

    let max_bytes = max_artifact_mib.saturating_mul(MIB);
    let (mut stored, mut already) = (Vec::new(), 0usize);
    let mut missing = Vec::new();
    // One hold of the store lock for the whole pass, so "already
    // stored" is never decided while another writer may still roll back.
    // Nothing to store takes no lock.
    let lock = match (!files.is_empty()).then(|| StoreLock::acquire(&repo)).transpose() {
        Ok(lock) => lock,
        Err(e) => {
            return CliError::refused("REPO_UNREADABLE", e.to_string()).emit("canon evidence vault", json);
        }
    };
    for (path, sha256) in &files {
        let lock = lock.as_ref().expect("the lock is taken whenever there is a file");
        if stored_blob(&repo, sha256).as_str() == sha256 {
            already += 1;
            continue;
        }
        let Some(source) = working_tree_path(&repo, path) else {
            missing.push(format!("{path} (recorded sha256 {sha256}): the recorded path leaves the repository"));
            continue;
        };
        let file = match std::fs::File::open(&source) {
            Ok(file) => file,
            Err(_) => {
                missing.push(format!("{path} (recorded sha256 {sha256}): working tree: missing"));
                continue;
            }
        };
        let size = file.metadata().map(|m| m.len()).unwrap_or(0);
        if size > max_bytes {
            missing.push(format!(
                "{path} (recorded sha256 {sha256}): {size} bytes, over the limit of {max_bytes} bytes; raise it with {} <MiB>",
                canon_gate::MAX_ARTIFACT_FLAG
            ));
            continue;
        }
        match lock.store(sha256, file) {
            Ok(StoreOutcome::Stored) => stored.push(format!("{} <- {path}", blob_display(sha256))),
            Ok(StoreOutcome::AlreadyStored) => already += 1,
            Err(StoreError::Changed { actual, .. }) => missing.push(format!("{path} (recorded sha256 {sha256}): working tree: {actual}")),
            Err(e @ (StoreError::Io(_) | StoreError::InvalidDigest(_) | StoreError::UnsafeStoreDir | StoreError::Locked(_))) => {
                return CliError::refused("EVIDENCE_VAULT_FAILED", format!("could not write {}: {e}", blob_display(sha256))).emit("canon evidence vault", json);
            }
        }
    }

    crate::say!(
        json,
        "canon evidence vault: {} bound file(s) — stored {}, already stored {already}, could not store {}",
        files.len(),
        stored.len(),
        missing.len()
    );
    for line in &stored {
        crate::say!(json, "  stored {line}");
    }
    for line in &missing {
        crate::say!(json, "  not stored {line}");
    }
    if !stored.is_empty() {
        crate::say!(json, "commit {}/ with the ledger", canon_model::paths::ARTIFACTS_SHA256_DIR);
    }
    if !missing.is_empty() {
        crate::say!(json, "the bytes behind a file not stored are gone; `canon gate check` reports a latest record bound to one as stale-evidence — re-run and re-attest it with `canon evidence add`");
    }
    if json {
        crate::json_out::data(
            "evidence vault",
            serde_json::json!({ "boundFiles": files.len(), "stored": stored, "alreadyStored": already, "notStored": missing }),
        );
    }
    0
}

/// An [`EvidenceVerdict`]'s operator-facing spelling, matching
/// [`parse_verdict`]'s own accepted domain. An exhaustive match rather
/// than a `Debug` render, so a variant added later has to choose its
/// wording here instead of silently leaking a Rust identifier —
/// `crate::dispatch::status_slug`'s established shape.
fn verdict_slug(verdict: EvidenceVerdict) -> &'static str {
    match verdict {
        EvidenceVerdict::Faithful => "faithful",
        EvidenceVerdict::NotApplicable => "not-applicable",
        EvidenceVerdict::Divergent => "divergent",
    }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use canon_store::tier::TierQuery;
    use tempfile::TempDir;

    use super::*;

    /// The smallest repo `canon evidence add` → `canon gate promote` →
    /// `canon gate task` can actually run against: one configured
    /// openspec plan source carrying one change with an open row `1.1`.
    /// `proposal.md` is the openspec dialect's own admission bar — a
    /// change dir without one yields no `Task` candidate at all, so no
    /// id under it would be bindable.
    fn repo_with_plan_corpus() -> TempDir {
        let tmp = TempDir::new().expect("a temp dir");
        std::fs::write(tmp.path().join("canon.yaml"), "plans:\n  sources:\n    - dialect: openspec\n      root: plans\n").expect("writing canon.yaml");
        let change_dir = tmp.path().join("plans").join("demo-change");
        std::fs::create_dir_all(&change_dir).expect("creating the change dir");
        std::fs::write(change_dir.join("proposal.md"), "# demo-change\n\n## Why\n\nTo exercise the evidence loop.\n").expect("writing proposal.md");
        std::fs::write(change_dir.join("tasks.md"), "# demo-change — tasks\n\n- [ ] 1.1 Author evidence for a real flip\n").expect("writing tasks.md");
        tmp
    }

    fn tasks_md(repo: &Path) -> String {
        std::fs::read_to_string(repo.join("plans").join("demo-change").join("tasks.md")).expect("reading tasks.md")
    }

    fn args(summary: Option<&str>) -> EvidenceArgs {
        EvidenceArgs {
            task_id: Some(TaskId::parse("demo-change#1.1").expect("a literal task id")),
            project_id: None,
            kind: "test-run".to_string(),
            evidence_ref: "cargo test -p canon-cli evidence".to_string(),
            verdict: None,
            summary: summary.map(str::to_string),
            command_result: None,
            scenario_id: None,
            run_id: None,
            surface_ref: Vec::new(),
            actor_id: "canon".to_string(),
            role: RoleId::parse("implementer").expect("a literal role"),
            session_id: None,
            approval_by: None,
            approval_role: None,
            approval_signature_file: None,
            approval_at: None,
            artifact_sha: None,
            artifacts: Vec::new(),
            reports: Vec::new(),
            report_cases: Vec::new(),
            all_scenarios: false,
            max_artifact_mib: canon_gate::DEFAULT_MAX_ARTIFACT_MIB,
            evaluator_id: None,
            evaluator_version: None,
            evaluator_digest: None,
            audit_result: None,
            drop_attachments: Vec::new(),
            drop_reason: None,
            override_reason: None,
            promote: crate::write_mode::PromoteFlag::Config,
            json: false,
        }
    }

    fn staged_count(repo: &Path) -> usize {
        let root = evidence_staging_dir(&GateCtx::from_repo(repo).unwrap().ledger_root);
        GitTier::new(root).read(&TierQuery::kind(RecordKind::EvidenceRecord)).map(|read| read.records.len()).unwrap_or(0)
    }

    fn committed(repo: &Path) -> Vec<RawRecord> {
        GitTier::new(GateCtx::from_repo(repo).unwrap().ledger_root).read(&TierQuery::kind(RecordKind::EvidenceRecord)).expect("reading the committed ledger").records
    }

    /// s42 task 4.3, the whole point of the change: a checkbox flips on
    /// AUTHORED evidence, through the three real commands, with no
    /// `--force` and no hand edit of `tasks.md` anywhere in the path.
    #[test]
    fn the_full_add_promote_task_path_flips_a_real_checkbox() {
        let tmp = repo_with_plan_corpus();
        let repo: PathBuf = tmp.path().to_path_buf();

        assert_eq!(run_add(&repo, &args(Some("the add -> promote -> task loop exercised this row"))), 0, "staging must succeed");
        assert_eq!(staged_count(&repo), 1, "the record must be in _staging, not committed");
        assert!(committed(&repo).is_empty(), "a staged record must be invisible to a committed read");

        // The gate refuses the flip while the evidence is only staged —
        // proving the flip below is caused by the promotion, not by the
        // gate reading the staging directory.
        assert_eq!(crate::gate::run_task(&repo, "demo-change#1.1", false), 1, "an unpromoted record must not satisfy the gate");
        assert!(tasks_md(&repo).contains("- [ ] 1.1"), "a refused flip must leave the document untouched");

        assert_eq!(crate::gate::run_promote(&repo, false, false), 0, "promotion must be clean");
        assert_eq!(staged_count(&repo), 0, "promotion drains staging");
        assert_eq!(committed(&repo).len(), 1, "the record is now committed");

        assert_eq!(crate::gate::run_task(&repo, "demo-change#1.1", false), 0, "the promoted record must satisfy the gate");
        let flipped = tasks_md(&repo);
        assert!(flipped.contains("- [x] 1.1"), "the row must be checked: {flipped}");
        assert!(flipped.contains("the add -> promote -> task loop exercised this row"), "the authored summary must become the row's evidence note: {flipped}");
    }

    /// Promotion is what assigns `run_seq` (module doc) — a record that
    /// never went through it would carry none, so this asserts the
    /// authored record actually acquires one rather than merely moving.
    #[test]
    fn promotion_stamps_the_authored_record_with_a_run_seq() {
        let tmp = repo_with_plan_corpus();
        assert_eq!(run_add(tmp.path(), &args(None)), 0);
        assert_eq!(crate::gate::run_promote(tmp.path(), false, false), 0);

        let records = committed(tmp.path());
        assert_eq!(records.len(), 1);
        assert_eq!(records[0].0.get("run_seq").and_then(serde_json::Value::as_u64), Some(1), "promotion must stamp run_seq: {:?}", records[0].0);
        assert_eq!(
            records[0].0.get("evidence"),
            Some(&serde_json::json!({ "kind": "test-run", "ref": "cargo test -p canon-cli evidence" })),
            "the `evidence` companion must survive promotion: {:?}",
            records[0].0
        );
    }

    /// The shared-admission requirement of task 4.1: an id the plan
    /// corpus does not carry is refused by
    /// `crate::dispatch::validate_task_binding`, the same decision
    /// `canon dispatch begin --task` makes — never a second local copy.
    #[test]
    fn an_unknown_task_is_refused_through_the_shared_admission_and_stages_nothing() {
        let tmp = repo_with_plan_corpus();
        let mut unknown = args(None);
        unknown.task_id = Some(TaskId::parse("demo-change#9.9").expect("a literal task id"));

        assert_eq!(run_add(tmp.path(), &unknown), 2, "an unknown task is a fixable invocation, exit 2");
        assert_eq!(staged_count(tmp.path()), 0, "a refused add must stage nothing");
    }

    /// A change no configured source carries is the OTHER admission
    /// refusal, and it must not be reachable by simply naming a
    /// plausible id — the corpus, not the grammar, decides.
    #[test]
    fn a_task_under_an_unknown_change_is_refused() {
        let tmp = repo_with_plan_corpus();
        let mut unknown = args(None);
        unknown.task_id = Some(TaskId::parse("no-such-change#1.1").expect("a literal task id"));

        assert_eq!(run_add(tmp.path(), &unknown), 2);
        assert_eq!(staged_count(tmp.path()), 0);
    }

    /// The fabrication scan runs at AUTHORING time (module doc): once
    /// `canon gate promote` has committed a note carrying a blocklist
    /// marker, the record is in the append-only ledger and the flip is
    /// permanently gate-red. Graded `1`, the same code `canon gate task`
    /// returns for `fabricated-evidence`.
    #[test]
    fn a_fabricated_summary_is_refused_before_anything_is_staged() {
        let tmp = repo_with_plan_corpus();

        assert_eq!(run_add(tmp.path(), &args(Some("this would pass once the suite is wired up"))), 1);
        assert_eq!(staged_count(tmp.path()), 0, "a fabricated note must stage nothing");
    }

    /// `canon_gate::evidence_note_of` requires `summary`, so a
    /// command-result-only companion makes `crate::gate::notes_of`
    /// return "present but unparseable" and blocks the flip. Refusing
    /// the combination here is the difference between an error at
    /// authoring time and an unfixable one after promotion.
    #[test]
    fn a_command_result_without_a_summary_is_refused() {
        let tmp = repo_with_plan_corpus();
        let mut orphaned = args(None);
        orphaned.command_result = Some("test result: ok. 42 passed; 0 failed".to_string());

        assert_eq!(run_add(tmp.path(), &orphaned), 2);
        assert_eq!(staged_count(tmp.path()), 0);
    }

    /// An empty `--kind`/`--ref` would serialize an `evidence`
    /// companion that matches no typed contract and tells a reader
    /// nothing — refused rather than written.
    #[test]
    fn an_empty_kind_or_ref_is_refused() {
        let tmp = repo_with_plan_corpus();
        let mut blank_kind = args(None);
        blank_kind.kind = "  ".to_string();
        assert_eq!(run_add(tmp.path(), &blank_kind), 2);

        let mut blank_ref = args(None);
        blank_ref.evidence_ref = String::new();
        assert_eq!(run_add(tmp.path(), &blank_ref), 2);

        assert_eq!(staged_count(tmp.path()), 0);
    }

    /// `--verdict divergent` is authorable — it is the model's own "the
    /// evidence says this did NOT hold" state — but `canon_gate::
    /// gate_task` is the thing that refuses to flip on it. Asserting
    /// both halves keeps the authoring command from quietly acquiring a
    /// veto the gate already owns.
    #[test]
    fn a_divergent_verdict_stages_and_promotes_but_never_flips() {
        let tmp = repo_with_plan_corpus();
        let mut divergent = args(Some("the implementation contradicts the scenario"));
        divergent.verdict = Some(EvidenceVerdict::Divergent);

        assert_eq!(run_add(tmp.path(), &divergent), 0, "recording a divergent verdict is a real operation");
        assert_eq!(crate::gate::run_promote(tmp.path(), false, false), 0);
        assert_eq!(crate::gate::run_task(tmp.path(), "demo-change#1.1", false), 1, "a divergent record must not satisfy the gate");
        assert!(tasks_md(tmp.path()).contains("- [ ] 1.1"), "the row must stay open");
    }

    /// Every accepted spelling round-trips to the wording
    /// [`verdict_slug`] prints, so the parser's domain and the
    /// reporter's vocabulary cannot drift apart.
    #[test]
    fn the_verdict_vocabulary_round_trips() {
        for slug in ["faithful", "not-applicable", "divergent"] {
            let verdict = parse_verdict(slug).expect("an accepted spelling");
            assert_eq!(verdict_slug(verdict), slug);
        }
        let err = parse_verdict("passing").expect_err("an unknown verdict is refused");
        assert!(err.contains("faithful"), "the refusal must name the domain: {err}");
    }

    #[test]
    fn surface_ref_validation_preserves_effect_and_rejects_unsafe_paths() {
        assert_eq!(parse_surface_ref("effect:secret-access").unwrap(), "effect:secret-access");
        assert_eq!(parse_surface_ref("src/auth/login.rs").unwrap(), "src/auth/login.rs");
        assert!(parse_surface_ref("../outside").is_err());
        assert!(parse_surface_ref("/absolute/path").is_err());
        assert!(parse_surface_ref("effect:Secret-Access").is_err());
    }

    /// LAYER ONE of the newline-injection fix (module doc): the exact
    /// forgery s42's review demonstrated, refused as a usage error before
    /// anything is staged — for `--summary`, which becomes the row suffix
    /// verbatim, and for `--actor-id`, which becomes it via
    /// `default_evidence_text` when no summary is given.
    ///
    /// Both spellings of the vector, and the row grammar's WHOLE
    /// mandatory-break set rather than just `\n`, so widening
    /// `ROW_LINE_BREAKS` later cannot leave this refusal behind.
    #[test]
    fn a_line_separator_in_a_document_bound_field_is_refused_before_staging() {
        let tmp = repo_with_plan_corpus();

        assert_eq!(run_add(tmp.path(), &args(Some("ok\n- [x] 9.9 Forged task"))), 2, "--summary must refuse a newline");

        let mut forged_actor = args(None);
        forged_actor.actor_id = "canon\n- [x] 9.9 Forged task".to_string();
        assert_eq!(run_add(tmp.path(), &forged_actor), 2, "--actor-id reaches the default suffix, so it must refuse one too");

        for separator in canon_ingest::task_rows::ROW_LINE_BREAKS {
            let summary = format!("ok{separator}- [x] 9.9 Forged task");
            assert_eq!(run_add(tmp.path(), &args(Some(&summary))), 2, "separator {separator:?} must be refused at authoring");
        }

        assert_eq!(staged_count(tmp.path()), 0, "a refused add must stage nothing");
        assert_eq!(
            tasks_md(tmp.path()),
            "# demo-change — tasks\n\n- [ ] 1.1 Author evidence for a real flip\n",
            "a refused add must leave the plan document byte-identical"
        );
    }

    /// A multi-line `--command-result` is NOT refused (module doc): it
    /// never reaches the row suffix, and captured output is legitimately
    /// multi-line. Pinned so the refusal above cannot quietly widen into
    /// "no newline anywhere", which would make the one field that raises
    /// the cost of a false attestation unusable.
    #[test]
    fn a_multi_line_command_result_is_authorable() {
        let tmp = repo_with_plan_corpus();
        let mut captured = args(Some("the suite is green"));
        captured.command_result = Some("running 3 tests\ntest result: ok. 3 passed; 0 failed\n".to_string());

        assert_eq!(run_add(tmp.path(), &captured), 0, "pasted output is the point of --command-result");
        assert_eq!(staged_count(tmp.path()), 1);
    }

    /// Stage a record by HAND, copying `summary` into the `evidence_note`
    /// companion verbatim — the second author [`run_add`]'s own refusal
    /// cannot speak for (module doc). Deliberately not `run_add`: the
    /// point is to reach `canon gate task` carrying a note `run_add`
    /// would have refused, exactly as a record authored before that
    /// refusal existed would.
    fn stage_note_verbatim(repo: &Path, summary: &str) {
        let record = EvidenceRecord::new(
            Envelope::current(RecordKind::EvidenceRecord, Utc::now(), Actor::new("canon", RoleId::parse("implementer").expect("a literal role"))),
            Some(TaskId::parse("demo-change#1.1").expect("a literal task id")),
            None,
            None,
            EvidenceVerdict::Faithful,
        );
        let mut body = serde_json::to_value(&record).expect("an EvidenceRecord always serializes");
        let object = body.as_object_mut().expect("an EvidenceRecord's serialized body is always a JSON object");
        object.insert("evidence".to_string(), serde_json::json!({ "kind": "test-run", "ref": "cargo test -p canon-cli evidence" }));
        object.insert("evidence_note".to_string(), serde_json::json!({ "summary": summary }));
        GitTier::new(evidence_staging_dir(&GateCtx::from_repo(repo).unwrap().ledger_root))
            .write(&RawWrite(RawRecord(body)))
            .expect("staging a hand-built record");
    }

    /// LAYER TWO, exercised INDEPENDENTLY of layer one: a committed
    /// record whose note carries a newline still cannot forge a row.
    /// `canon gate promote` commits it (it validates record structure,
    /// not note shape) and `canon_gate::gate_task` approves it (the note
    /// carries no fabrication marker), so the refusal here is the
    /// write-back's alone — which is the whole point of having it.
    #[test]
    fn a_committed_multi_line_note_cannot_forge_a_second_checked_row() {
        let tmp = repo_with_plan_corpus();
        stage_note_verbatim(tmp.path(), "ok\n- [x] 9.9 Forged task");

        assert_eq!(crate::gate::run_promote(tmp.path(), false, false), 0, "promotion validates the record, not the note — the hole has to be closed downstream");
        assert_eq!(committed(tmp.path()).len(), 1, "the malformed note IS in the append-only ledger");

        assert_eq!(crate::gate::run_task(tmp.path(), "demo-change#1.1", false), 1, "a multi-line note is gate-red, not a flip");
        let doc = tasks_md(tmp.path());
        assert!(!doc.contains("9.9"), "no forged row may reach the document: {doc}");
        assert!(!doc.contains("- [x]"), "nothing may flip on a multi-line note: {doc}");
        assert!(doc.contains("- [ ] 1.1"), "the real row stays open: {doc}");
    }

    /// A scenario-keyed evidence body by `actor`, as a batch stages it.
    fn batch_body(scenario: &str, actor: Actor) -> serde_json::Value {
        let record = EvidenceRecord::new(Envelope::current(RecordKind::EvidenceRecord, Utc::now(), actor), None, Some(ScenarioId::parse(scenario).unwrap()), None, EvidenceVerdict::Faithful)
            .with_project_id(ProjectId::parse("root").unwrap());
        serde_json::to_value(&record).unwrap()
    }

    /// evidence.add.83: a batch that fails partway — its second record's
    /// staging write fails, or its promotion is refused — leaves no
    /// committed record, no staged record and no new blob.
    #[test]
    fn evidence_add_83_a_batch_that_fails_partway_leaves_nothing_behind() {
        let tmp = repo_with_plan_corpus();
        let repo = tmp.path();
        let ledger_root = GateCtx::from_repo(repo).unwrap().ledger_root;
        std::fs::write(repo.join("junit.xml"), "<testsuite/>").unwrap();
        let files: Vec<crate::evidence_attach::BoundFile> =
            crate::evidence_attach::bind_artifacts(repo, &[repo.join("junit.xml")], 1024).unwrap().into_iter().map(|(_, file)| file).collect();
        let store = canon_gate::artifact_store::store_dir(repo);
        let blobs = || std::fs::read_dir(&store).map(|d| d.count()).unwrap_or(0);
        let implementer = || Actor::new("impl", RoleId::parse("implementer").unwrap());
        let bodies = || vec![batch_body("cart.add.01", implementer()), batch_body("cart.add.02", implementer())];

        // The second staging write fails after the first one landed.
        let mut calls = 0;
        let mut failing_second = |tier: &GitTier, write: &RawWrite| {
            calls += 1;
            if calls == 2 {
                return Err(canon_store::tier::StoreError::Io(std::io::Error::other("disk full")));
            }
            tier.write(write)
        };
        let Err((error, code)) = land_batch(repo, &ledger_root, &files, bodies(), true, &mut failing_second) else { panic!("the second write fails") };
        assert_eq!(code, 2, "{error}");
        assert!(error.contains("staging record 2 of the batch failed: ") && error.contains("disk full"), "{error}");
        assert_eq!((staged_count(repo), committed(repo).len(), blobs()), (0, 0, 0), "nothing staged, committed or stored");

        // Both stage; the second's promotion is refused (it has no role)
        // after the first was committed.
        let refused = vec![batch_body("cart.add.01", implementer()), batch_body("cart.add.02", Actor::new_unattributed("legacy-writer"))];
        let Err((error, code)) = land_batch(repo, &ledger_root, &files, refused, true, &mut |tier, write| tier.write(write)) else { panic!("the second promotion is refused") };
        assert_eq!(code, 1, "{error}");
        assert!(error.contains("promoting record 2 of the batch"), "{error}");
        assert_eq!((staged_count(repo), committed(repo).len(), blobs()), (0, 0, 0), "the first committed record is removed too");

        // A read-only destination is refused before anything is written.
        let staging_kind = evidence_staging_dir(&ledger_root).join("kind=evidence_record");
        let mut readonly = std::fs::metadata(&staging_kind).unwrap().permissions();
        readonly.set_readonly(true);
        std::fs::set_permissions(&staging_kind, readonly.clone()).unwrap();
        let outcome = land_batch(repo, &ledger_root, &files, bodies(), true, &mut |tier, write| tier.write(write));
        readonly.set_readonly(false);
        std::fs::set_permissions(&staging_kind, readonly).unwrap();
        assert!(matches!(&outcome, Err((e, 2)) if e.contains("is read-only")), "{:?}", outcome.as_ref().err());
        assert_eq!(blobs(), 0);

        let landed = land_batch(repo, &ledger_root, &files, bodies(), true, &mut |tier, write| tier.write(write)).unwrap();
        assert_eq!((landed.stored, landed.locations.len(), committed(repo).len(), blobs()), (1, 2, 2, 1), "the same batch lands whole");
        assert!(landed.locations.iter().all(|l| l.promoted.is_some()));
    }
}
