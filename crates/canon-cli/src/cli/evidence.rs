//! `canon evidence` — the clap tree of the group.

use canon_model::{EvidenceVerdict, ProjectId, RoleId, RunId, ScenarioId, TaskId};
use clap::{Args, Subcommand};
use std::path::PathBuf;
use super::PromoteArgs;

#[derive(Subcommand)]
pub enum EvidenceCommand {
    /// Emit exact bytes for external `ssh-keygen -Y sign -n canon-approval-v1`
    ApprovalPayload(EvidenceApprovalPayloadArgs),
    /// Record one attributed EvidenceRecord for a task or scenario (promoted in the same run under `write: { promote: auto }` or with --promote; under `write.promote: manual`, commit it with `canon gate promote`)
    #[command(after_help = "ATTESTATION, NOT PROOF.\nThe evidence command never runs, resolves, or checks --ref. The attestor can authorize its own checkbox; this is an attestation, not independent approval.\nThe gate skips the staleness, trust-ladder, release-trust, and divergence dimensions.\nDetached approvals require an external signature; canon never signs on the user's behalf.\n\nEXPERIMENTAL BINDING (--artifact, --report).\nCanon reads files your runner or agent already wrote; it never runs a test. --artifact binds any file by sha256; --report junit:<path>, cucumber:<path> or swift-testing:<path> (the JSON-lines event stream of `swift test --experimental-event-stream-output`) also records the matched case and its outcome (a faithful verdict needs every bound case to pass). Every case whose own scenario id (the first id in its classname, then name, a Swift Testing display name, then function name, or a Cucumber tag) is this one is bound; a case that only mentions it is not; --report-case adds more. With --task and no --scenario-id, every case in the report is bound. Bound files are copied into .canon/artifacts/sha256/<hex> (commit them with the ledger; per-file limit --max-artifact-mib, default 25), and canon gate check re-hashes them: a bound file whose bytes are gone is stale-evidence. Strength requirements are opt-in via policy experimental.evidence_binding (off by default).\n\nONE REPORT, MANY SCENARIOS (--all-scenarios).\nWith --report, writes one record per indexed scenario that owns a case in the report, each with its own cases and its own verdict: --verdict when given, else derived (faithful when every case passed, divergent when one failed, not-applicable when one was skipped). A case whose id is not indexed is named and skipped. All or nothing: any refusal (a claimed faithful verdict over a failed case, a fabrication marker, an unapproved brief, a dropped binding) writes no record. The records share the stored report file.\n\nFILES BELONG TO TESTS (--artifact).\nA record binds the attachments of the cases it owns: JUnit `[[ATTACHMENT|<path>]]` lines in a test's <system-out> (Playwright's reporter; the path is relative to the report's directory) and Cucumber JSON embeddings, in a batch and with --scenario-id alike. A missing, out-of-repo or oversized attachment is refused naming the case and the path. With --all-scenarios a bare --artifact <path> is refused (it would bind to every record); bind a file to one scenario with --artifact <scenario-id>=<path> (the part before the first `=` counts only when shaped like a scenario id; write ./<path> for a file named otherwise).\n\nA WRITE THAT BREAKS A REVIEW PIN SAYS SO.\nWhen the new record supersedes evidence a current pinned review saw, one line per review now stale names the reviewer, the scenario and the `canon review scenario ... --pin <sha>` that re-pins it.\n\nRE-ATTESTING NEVER DROPS A BINDING SILENTLY (--drop-attachment).\nFor a scenario, a record that leaves out a file the scenario's latest record binds is refused (exit 2) until it re-binds the file (new bytes at the same path are fine) or names it with --drop-attachment <path> (repeatable) --drop-reason <text>. The drop is recorded on the record and listed by canon status. Task-only evidence has no drop guard.\n\nNOTHING IS ATTESTED BEFORE THE BRIEF IS APPROVED (--override-reason).\nWhile a subject this record attests for (the scenario's @subject owner, or any subject adopting the task's change) has a newest change whose proposal.md has no `Approved:` line, the add is refused (exit 1, nothing staged): stop, report the brief and wait for the human's answer. --override-reason <text> attests anyway; the reason, the waived subjects and --actor-id are recorded on the record and listed by canon status and canon gate check. A subject with no proposal.md to answer is unaffected; review scenario and finding add are never refused.\n\nAUDITS.\nAn external evaluator's re-audit is `canon evidence audit <SCENARIO_ID>`, not `--kind audit`.")]
    Add(EvidenceAddArgs),
    /// Record an external evaluator's re-audit of one scenario (class `audit`; the verdict follows --audit-result; canon never runs the evaluator)
    #[command(after_help = "Records an external evaluator's re-audit of one scenario: --evaluator-id, --evaluator-version, --evaluator-digest <sha256>, --audit-result confirmed|refuted|indeterminate, and optional --report/--artifact counterexamples, bound and stored like any report. canon never generates or runs the audit. An audit never becomes the scenario's declared verdict; a refuted latest audit is false-success in canon gate check, whatever the declared verdict says.")]
    Audit(EvidenceAuditArgs),
}

#[derive(Args)]
pub struct EvidenceApprovalPayloadArgs {
    #[arg(long, value_parser = crate::dispatch::parse_task_id)]
    pub task_id: Option<TaskId>,
    #[arg(long, value_parser = crate::review::parse_project_id)]
    pub project_id: Option<ProjectId>,
    #[arg(long, value_parser = crate::review::parse_scenario_id)]
    pub scenario_id: Option<ScenarioId>,
    #[arg(long, value_parser = crate::dispatch::parse_run_id)]
    pub run_id: Option<RunId>,
    #[command(flatten)]
    pub rev: crate::cli::RevArgs,
    #[arg(long = "surface-ref", value_parser = crate::evidence::parse_surface_ref)]
    pub surface_ref: Vec<String>,
    /// Who approved (the human whose detached signature the payload is for)
    #[arg(long)]
    pub approver: String,
    /// The approval time that is signed into the payload: RFC3339, or YYYY-MM-DD (UTC midnight)
    #[arg(long, value_name = "TIMESTAMP", value_parser = crate::timestamp::normalize)]
    pub approval_at: String,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}

#[derive(Args)]
pub struct EvidenceAddArgs {
    /// Plan task this evidence attests to (<change_id>#<n>); required unless --scenario-id is given
    #[arg(long, value_parser = crate::dispatch::parse_task_id)]
    pub task_id: Option<TaskId>,
    /// Spec corpus id for --scenario-id (canon keys a scenario by the composite (project_id, scenario_id)). Must name a spec root configured in canon.yaml, or the add is refused (UNKNOWN_SPEC_ROOT) before anything is written
    #[arg(long, value_parser = crate::review::parse_project_id)]
    pub project_id: Option<ProjectId>,
    /// What CLASS of evidence is attested to (test-run, review, ...); default `test-run` when a --report is given. `audit` is refused: an external evaluator's re-audit is `canon evidence audit`
    #[arg(long, value_name = "CLASS", required_unless_present = "reports")]
    pub class: Option<String>,
    /// The reference a reader can resolve: a command line, commit sha, or report path (default: the --report paths, comma-separated, when one is given). NEVER run or resolved by canon
    #[arg(long = "ref", value_name = "REF", required_unless_present = "reports")]
    pub evidence_ref: Option<String>,
    /// faithful / not-applicable / divergent. Given a --report and no --verdict, the verdict is derived from the bound cases (any failed: divergent; a skipped case and none failed: not-applicable; else faithful), one record at a time with --all-scenarios; with no report the default is faithful. An explicit faithful over a failed or skipped case is refused
    #[arg(long, value_parser = crate::evidence::verdict_parser())]
    pub verdict: Option<EvidenceVerdict>,
    /// One-line note, with or without --task; a task's flipped row shows it as the latest summary (scanned for fabrication markers; line breaks refused)
    #[arg(long)]
    pub summary: Option<String>,
    /// The real captured output backing --summary (requires --summary; scanned for fabrication markers)
    #[arg(long)]
    pub command_result: Option<String>,
    /// Scenario this evidence also joins to
    #[arg(long, value_parser = crate::review::parse_scenario_id)]
    pub scenario_id: Option<ScenarioId>,
    /// RunId (ULID) of the run that produced this evidence
    #[arg(long, value_parser = crate::dispatch::parse_run_id)]
    pub run_id: Option<RunId>,
    /// Explicit risk binding: repository-relative path or effect:<kebab-slug>; persisted as surface_ref
    #[arg(long = "surface-ref", value_parser = crate::evidence::parse_surface_ref)]
    pub surface_ref: Vec<String>,
    /// The attesting actor's id (recorded as the attestation's author; line breaks refused)
    #[arg(long, default_value = "canon")]
    pub actor_id: String,
    /// The attesting role; `canon gate promote` derives its run_seq partition key from it. Default: the declared role (--as / CANON_ROLE); required when none is declared
    #[arg(long, value_parser = crate::retrieve::parse_role)]
    pub role: Option<RoleId>,
    /// The attesting session (recorded as actor.session_id). Under require_review.distinct_actor a review from this session does not count
    #[arg(long, value_parser = crate::review::parse_session_id)]
    pub session_id: Option<canon_model::SessionId>,
    /// Human approval identity; authentication comes only from the detached signature.
    #[arg(long)]
    pub approver: Option<String>,
    /// Approval role; only human is eligible for risk approval.
    #[arg(long, value_parser = crate::retrieve::parse_role)]
    pub approval_role: Option<RoleId>,
    /// Armored SSH detached signature file. Canon never signs on the user's behalf.
    #[arg(long)]
    pub signature_file: Option<PathBuf>,
    /// Timestamp (RFC3339 or YYYY-MM-DD) included in the signed approval payload.
    #[arg(long, value_name = "TIMESTAMP", value_parser = crate::timestamp::normalize)]
    pub approval_at: Option<String>,
    #[command(flatten)]
    pub rev: crate::cli::RevArgs,
    /// EXPERIMENTAL: bind any file (trace, screenshots, agent QA log, report) by sha256; repeatable; must be inside the repo; with --all-scenarios only as <scenario-id>=<path>, bound to that scenario's record alone
    #[arg(long = "artifact", value_name = "PATH")]
    pub artifacts: Vec<PathBuf>,
    /// EXPERIMENTAL: bind and parse a test report, `junit:<path>`, `cucumber:<path>` or `swift-testing:<path>`; repeatable
    #[arg(long = "report", value_name = "FORMAT:PATH", value_parser = crate::evidence_attach::parse_report_spec)]
    pub reports: Vec<(canon_model::ReportFormat, PathBuf)>,
    /// EXPERIMENTAL: also bind this report case (a Rust test: its function name); repeatable. Cases whose own scenario id is this one are always bound
    #[arg(long = "report-case", value_name = "NAME")]
    pub report_cases: Vec<String>,
    /// One record per indexed scenario of --project-id that owns a case in the --report(s), all or nothing (see below)
    #[arg(long)]
    pub all_scenarios: bool,
    /// Per-file size limit for the artifact store (.canon/artifacts/sha256/), in MiB; a larger bound file is refused
    #[arg(long, value_name = "MIB", default_value_t = canon_gate::DEFAULT_MAX_ARTIFACT_MIB, value_parser = clap::value_parser!(u64).range(1..))]
    pub max_artifact_mib: u64,
    /// Attest anyway while a subject this record attests for has a brief awaiting the human's answer (one line, recorded on the record with --actor-id)
    #[arg(long)]
    pub override_reason: Option<String>,
    #[command(flatten)]
    pub promote: PromoteArgs,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}


/// The flags of `evidence audit`: an external evaluator's re-audit of one scenario.
#[derive(Args)]
pub struct EvidenceAuditArgs {
    /// The re-audited scenario (`<area>.<surface>.<nn>`)
    #[arg(value_name = "SCENARIO_ID", value_parser = crate::review::parse_scenario_id)]
    pub scenario_id: ScenarioId,
    /// Spec corpus id of the scenario. Required only when canon.yaml configures more than one spec root; refused (UNKNOWN_SPEC_ROOT) when it names none
    #[arg(long, value_parser = crate::review::parse_project_id)]
    pub project_id: Option<ProjectId>,
    /// The reference a reader can resolve: a command line, commit sha, or report path. NEVER run or resolved by canon
    #[arg(long = "ref", value_name = "REF")]
    pub evidence_ref: String,
    /// One-line note (scanned for fabrication markers; line breaks refused)
    #[arg(long)]
    pub summary: Option<String>,
    /// The real captured output backing --summary (requires --summary; scanned for fabrication markers)
    #[arg(long)]
    pub command_result: Option<String>,
    /// RunId (ULID) of the run that produced this evidence
    #[arg(long, value_parser = crate::dispatch::parse_run_id)]
    pub run_id: Option<RunId>,
    /// The attesting actor's id (recorded as the attestation's author; line breaks refused)
    #[arg(long, default_value = "canon")]
    pub actor_id: String,
    /// The attesting role; `canon gate promote` derives its run_seq partition key from it. Default: the declared role (--as / CANON_ROLE); required when none is declared
    #[arg(long, value_parser = crate::retrieve::parse_role)]
    pub role: Option<RoleId>,
    /// The attesting session (recorded as actor.session_id)
    #[arg(long, value_parser = crate::review::parse_session_id)]
    pub session_id: Option<canon_model::SessionId>,
    #[command(flatten)]
    pub rev: crate::cli::RevArgs,
    /// EXPERIMENTAL: bind any file (trace, log, report) by sha256; repeatable; must be inside the repo
    #[arg(long = "artifact", value_name = "PATH")]
    pub artifacts: Vec<PathBuf>,
    /// EXPERIMENTAL: bind and parse a test report, `junit:<path>`, `cucumber:<path>` or `swift-testing:<path>`; repeatable
    #[arg(long = "report", value_name = "FORMAT:PATH", value_parser = crate::evidence_attach::parse_report_spec)]
    pub reports: Vec<(canon_model::ReportFormat, PathBuf)>,
    /// EXPERIMENTAL: also bind this report case (a Rust test: its function name); repeatable
    #[arg(long = "report-case", value_name = "NAME")]
    pub report_cases: Vec<String>,
    /// Per-file size limit for the artifact store (.canon/artifacts/sha256/), in MiB; a larger bound file is refused
    #[arg(long, value_name = "MIB", default_value_t = canon_gate::DEFAULT_MAX_ARTIFACT_MIB, value_parser = clap::value_parser!(u64).range(1..))]
    pub max_artifact_mib: u64,
    /// The external evaluator that re-audited the scenario (canon never runs it)
    #[arg(long)]
    pub evaluator_id: String,
    /// The evaluator's version
    #[arg(long)]
    pub evaluator_version: String,
    /// sha256 of the evaluator (64 lowercase hex)
    #[arg(long)]
    pub evaluator_digest: String,
    /// confirmed / refuted / indeterminate; the record's verdict follows it (confirmed → faithful, refuted → divergent, indeterminate → not-applicable)
    #[arg(long, value_parser = ["confirmed", "refuted", "indeterminate"])]
    pub audit_result: String,
    /// Attest anyway while a subject this record attests for has a brief awaiting the human's answer (one line, recorded on the record with --actor-id)
    #[arg(long)]
    pub override_reason: Option<String>,
    #[command(flatten)]
    pub promote: PromoteArgs,
    #[command(flatten)]
    pub repo: crate::cli::RepoArgs,
    /// Output JSON instead of the human-readable form
    #[arg(long)]
    pub json: bool,
}
