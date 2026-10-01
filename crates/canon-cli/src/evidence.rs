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
//! ` — ✅ ` suffix is built from.
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
//! - `canon_gate::scan_fake_markers` over the `evidence_note` being
//!   authored (the identical call `canon_gate::gate_task` makes).
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
//! # Exit-code contract (`crate::gate`'s, unchanged)
//! `0` staged, `1` the authored record would be gate-red (a fabrication
//! marker, or a typed atom that fails vocabulary validation), `2`
//! usage-or-infra (an unknown/ungrammatical task, no plan corpus, a
//! malformed flag combination, a line separator in a document-bound
//! field, an unparseable typed-atoms file, a tier write failure). Each
//! grade matches what `canon gate task` returns for the SAME condition,
//! so the operator sees one verdict, earlier.

use std::path::{Path, PathBuf};

use canon_gate::{verify_risk_approval, scan_fake_markers, EvidenceNote, GateCtx};
use canon_ingest::task_rows::first_row_line_break;
use canon_model::{approval_payload_bytes, Actor, Envelope, EvidenceApproval, EvidenceRecord, EvidenceVerdict, ProjectId, RawRecord, RecordKind, RoleId, RunId, ScenarioId, Sha, TaskId, APPROVAL_NAMESPACE};
use canon_store::git_tier::GitTier;
use canon_store::tier::{RawWrite, Tier};
use chrono::{DateTime, Utc};

use crate::context::resolve_repo_root;
use crate::gate::{evidence_staging_dir, typed_evidence_contract_for_task};

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
    pub verdict: EvidenceVerdict,
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
    /// Optional authenticated approval. A complete approval additionally
    /// requires a detached SSH signature and exact artifact SHA binding.
    pub approval_by: Option<String>,
    pub approval_role: Option<RoleId>,
    pub approval_signature_file: Option<PathBuf>,
    /// RFC3339 timestamp covered by the external signature.
    pub approval_at: Option<String>,
    /// Exact Git artifact SHA bound by an approval and persisted on the record.
    pub artifact_sha: Option<Sha>,
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
        Ok((payload, _, _)) => match std::io::stdout().lock().write_all(&payload) {
            Ok(()) => 0,
            Err(error) => {
                eprintln!("canon evidence approval-payload: {error}");
                2
            }
        },
        Err(error) => {
            eprintln!("canon evidence approval-payload: refused — {error}");
            2
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

    if args.kind.trim().is_empty() || args.evidence_ref.trim().is_empty() {
        eprintln!("canon evidence add: refused — --kind and --ref must both be non-empty; an empty companion narrows the gate's typed evidence slice to nothing");
        return 2;
    }
    for surface in &args.surface_ref {
        if let Err(error) = parse_surface_ref(surface) {
            eprintln!("canon evidence add: refused — --surface-ref `{surface}` is invalid: {error}");
            return 2;
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
            eprintln!("canon evidence add: refused — authenticated approval requires --approval-by, --approval-role human, --approval-signature-file, --approval-at, and --artifact-sha");
            return 2;
        }
        let approver = args.approval_by.as_deref().unwrap_or_default();
        if approver.trim().is_empty() || approver.trim() != approver || approver.chars().any(|ch| matches!(ch, '\n' | '\r')) {
            eprintln!("canon evidence add: refused — approval identity must be a non-empty trimmed single-line value");
            return 2;
        }
        if args.approval_role.as_ref().map_or(true, |role| role.as_str() != "human") {
            eprintln!("canon evidence add: refused — only the human approval role can satisfy risk approval");
            return 2;
        }
        if args.approval_at.as_deref().and_then(|value| value.parse::<DateTime<Utc>>().ok()).is_none() {
            eprintln!("canon evidence add: refused — --approval-at must be an RFC3339 timestamp");
            return 2;
        }
        let signature_path = args.approval_signature_file.as_ref().unwrap();
        if !signature_path.is_file() {
            eprintln!("canon evidence add: refused — detached signature file does not exist: {}", signature_path.display());
            return 2;
        }
    }
    if args.summary.is_none() && args.command_result.is_some() {
        eprintln!(
            "canon evidence add: refused — --command-result requires --summary; `canon_gate::evidence_note_of` requires `summary`, so a summary-less note is unparseable to the gate rather than partially read"
        );
        return 2;
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
        eprintln!(
            "canon evidence add: refused — {flag} carries the line separator {separator:?} at byte offset {offset}; it is written into a plan document as ONE checkbox row, so a separator there appends a second row (a `- [x] ` one, if the value says so) that no evidence record backs"
        );
        return 2;
    }

    // At least one coverage subject, checked before anything else
    // touches the corpus. A record with neither key is invisible to
    // every gate (`canon_gate::coverage::CellSubject::of` yields
    // `None`), so staging one would be writing an attestation nothing
    // can ever read back.
    if args.task_id.is_none() && args.scenario_id.is_none() {
        eprintln!(
            "canon evidence add: refused — give --task, --scenario-id, or both; a record with neither carries no coverage subject and no gate can read it back"
        );
        return 2;
    }

    // A scenario-keyed record needs the COMPOSITE key, because
    // `Scenario`'s own `project_id` is required and two spec roots may
    // carry the same scenario id. Refused rather than defaulted: there
    // is no safe guess when `specs.roots[]` holds more than one entry.
    if args.scenario_id.is_some() && args.project_id.is_none() {
        eprintln!(
            "canon evidence add: refused — --scenario-id needs --project-id; canon keys a scenario by the composite (project_id, scenario_id) and two spec roots may carry the same scenario id"
        );
        return 2;
    }

    // The SAME plan-corpus admission `canon dispatch begin --task` makes
    // (module doc) — an id no import pass would persist as a `Task`
    // record is not an id evidence may be attested against. Skipped
    // entirely for a scenario-only record: there is no task to admit.
    if let Some(task_id) = &args.task_id {
        if let Err(e) = crate::dispatch::validate_task_binding(&repo, task_id) {
            eprintln!("canon evidence add: {e}");
            return if e.is_usage() { 2 } else { 1 };
        }
    }

    // `EvidenceNote` is keyed by `TaskId` because its only consumer is
    // the checkbox flip, which is a task-side operation. A
    // scenario-only record therefore carries no note — and `--summary`
    // on one would be silently dropped, so it is refused instead.
    if args.task_id.is_none() && args.summary.is_some() {
        eprintln!(
            "canon evidence add: refused — --summary is the flipped checkbox row's suffix and is keyed by task; a scenario-only attestation has no row to flip"
        );
        return 2;
    }
    let note = args
        .task_id
        .as_ref()
        .zip(args.summary.as_ref())
        .map(|(task_id, summary)| EvidenceNote::new(task_id.clone(), summary.clone(), args.command_result.clone()));
    if let Some(note) = &note {
        // The gate's own scan, run here so a fabricated note is refused
        // while it is still a flag value — after `canon gate promote`
        // the record is in the append-only committed ledger and the
        // refusal is no longer fixable by re-running the command.
        let violations = scan_fake_markers(note);
        if !violations.is_empty() {
            for violation in &violations {
                eprintln!("{}", violation.line());
            }
            return 1;
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
                eprintln!(
                    "canon evidence add: refused — {} declares a typed evidence contract kind=`{}` ref=`{}`, but this record carries kind=`{}` ref=`{}`; `canon gate task` narrows to records matching the atom exactly, so this one would be refused as an unevidenced flip",
                    task_id, contract.kind, contract.evidence_ref, args.kind, args.evidence_ref
                );
                return 1;
            }
            Ok(_) => {}
            Err(e) => {
                eprintln!("canon evidence add: {e}");
                return if e.is_usage() { 2 } else { 1 };
            }
        }
    }

    let at = Utc::now();
    let mut record = EvidenceRecord::new(
        Envelope::current(RecordKind::EvidenceRecord, at, Actor::new(args.actor_id.as_str(), args.role.clone())),
        args.task_id.clone(),
        args.scenario_id.clone(),
        args.run_id.clone(),
        args.verdict,
    );
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
        }) {
            Ok(binding) => binding,
            Err(error) => {
                eprintln!("canon evidence add: refused — {error}");
                return 2;
            }
        };
        record = record.with_surface_ref(surface.clone());
        let signature_path = args.approval_signature_file.as_ref().unwrap();
        let signature = match std::fs::read(signature_path) {
            Ok(signature) => signature,
            Err(error) => {
                eprintln!("canon evidence add: refused — read detached signature: {error}");
                return 2;
            }
        };
        let allowed_signers = match canon_gate::policy::allowed_signers_path(&repo) {
            Ok(Some(path)) => path,
            Ok(None) => {
                eprintln!("canon evidence add: refused — no policy-pinned allowed_signers verifier is configured");
                return 2;
            }
            Err(error) => {
                eprintln!("canon evidence add: refused — approval policy is malformed: {error}");
                return 2;
            }
        };
        if let Err(error) = verify_risk_approval(&payload, &signature, approver, &allowed_signers) {
            eprintln!("canon evidence add: refused — {error}");
            return 1;
        }
        let signature = match String::from_utf8(signature) {
            Ok(signature) => signature,
            Err(_) => {
                eprintln!("canon evidence add: refused — detached signature must be UTF-8 armored SSH signature text");
                return 2;
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

    // `serde_json::to_value` on a record canon just constructed, and
    // `as_object_mut` on the object that produced — the identical pair
    // of canon-originated-data `expect`s `canon_gate::promote` states
    // when it stamps `run_seq` onto a body it just read.
    let mut body = serde_json::to_value(&record).expect("an EvidenceRecord always serializes");
    let object = body.as_object_mut().expect("an EvidenceRecord's serialized body is always a JSON object");
    object.insert("evidence".to_string(), serde_json::json!({ "kind": args.kind, "ref": args.evidence_ref }));
    if let Some(note) = &note {
        // Serialized from the SAME `EvidenceNote` that was scanned, so
        // the bytes on disk are the bytes the scan cleared. `task_id` is
        // already the record's own typed field; the companion carries
        // only what `evidence_note_of` reads back.
        let mut companion = serde_json::Map::new();
        companion.insert("summary".to_string(), serde_json::Value::String(note.summary.clone()));
        if let Some(command_result) = &note.command_result {
            companion.insert("command_result".to_string(), serde_json::Value::String(command_result.clone()));
        }
        object.insert("evidence_note".to_string(), serde_json::Value::Object(companion));
    }

    let staging = GitTier::new(evidence_staging_dir(&GateCtx::from_repo(&repo).ledger_root));
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
            println!(
                "canon evidence add: staged {} for {} (verdict {}) — run `canon gate promote` to commit it, {}",
                receipt.location,
                subject,
                verdict_slug(args.verdict),
                next
            );
            0
        }
        Err(e) => {
            eprintln!("canon evidence add: {e}");
            2
        }
    }
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
            verdict: EvidenceVerdict::Faithful,
            summary: summary.map(str::to_string),
            command_result: None,
            scenario_id: None,
            run_id: None,
            surface_ref: Vec::new(),
            actor_id: "canon".to_string(),
            role: RoleId::parse("implementer").expect("a literal role"),
            approval_by: None,
            approval_role: None,
            approval_signature_file: None,
            approval_at: None,
            artifact_sha: None,
        }
    }

    fn staged_count(repo: &Path) -> usize {
        let root = evidence_staging_dir(&GateCtx::from_repo(repo).ledger_root);
        GitTier::new(root).read(&TierQuery::kind(RecordKind::EvidenceRecord)).map(|read| read.records.len()).unwrap_or(0)
    }

    fn committed(repo: &Path) -> Vec<RawRecord> {
        GitTier::new(GateCtx::from_repo(repo).ledger_root).read(&TierQuery::kind(RecordKind::EvidenceRecord)).expect("reading the committed ledger").records
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
        assert_eq!(crate::gate::run_task(&repo, "demo-change#1.1"), 1, "an unpromoted record must not satisfy the gate");
        assert!(tasks_md(&repo).contains("- [ ] 1.1"), "a refused flip must leave the document untouched");

        assert_eq!(crate::gate::run_promote(&repo, false), 0, "promotion must be clean");
        assert_eq!(staged_count(&repo), 0, "promotion drains staging");
        assert_eq!(committed(&repo).len(), 1, "the record is now committed");

        assert_eq!(crate::gate::run_task(&repo, "demo-change#1.1"), 0, "the promoted record must satisfy the gate");
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
        assert_eq!(crate::gate::run_promote(tmp.path(), false), 0);

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
        divergent.verdict = EvidenceVerdict::Divergent;

        assert_eq!(run_add(tmp.path(), &divergent), 0, "recording a divergent verdict is a real operation");
        assert_eq!(crate::gate::run_promote(tmp.path(), false), 0);
        assert_eq!(crate::gate::run_task(tmp.path(), "demo-change#1.1"), 1, "a divergent record must not satisfy the gate");
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
        GitTier::new(evidence_staging_dir(&GateCtx::from_repo(repo).ledger_root))
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

        assert_eq!(crate::gate::run_promote(tmp.path(), false), 0, "promotion validates the record, not the note — the hole has to be closed downstream");
        assert_eq!(committed(tmp.path()).len(), 1, "the malformed note IS in the append-only ledger");

        assert_eq!(crate::gate::run_task(tmp.path(), "demo-change#1.1"), 1, "a multi-line note is gate-red, not a flip");
        let doc = tasks_md(tmp.path());
        assert!(!doc.contains("9.9"), "no forged row may reach the document: {doc}");
        assert!(!doc.contains("- [x]"), "nothing may flip on a multi-line note: {doc}");
        assert!(doc.contains("- [ ] 1.1"), "the real row stays open: {doc}");
    }
}
