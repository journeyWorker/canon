//! The classified sites of this group (see `sites.rs`): evidence, reviews, divergences, findings, dispatch.

use super::sites::{site, Kind, Site};

pub const SITES: &[Site] = &[
    // ---------------------------------------------------------------------------------------- evidence.rs
    site("evidence.rs:fabricated:EVIDENCE_NOTE_INVALID#1", Kind::Undeclared("a note that a fabrication-marker scan rejects is refused with exit 1 (gate-red wording), and the contract's validators carry an exit 2 code; ATIP feedback")),
    // run_add: the checks of `evidence add` and `evidence audit`, in the order they run.
    site("evidence.rs:run_add:EVIDENCE_ARGUMENT_INVALID#1", Kind::Domain { command: "evidence add", argument: "class" }),
    site("evidence.rs:run_add:EVIDENCE_ARGUMENT_INVALID#2", Kind::Domain { command: "evidence add", argument: "override-reason" }),
    site("evidence.rs:run_add:EVIDENCE_ARGUMENT_INVALID#3", Kind::Unreachable("clap's `parse_surface_ref` value parser refuses the same values first, with USAGE (the surface-ref domain and validator)")),
    site("evidence.rs:at:EVIDENCE_ARGUMENT_INVALID#1", Kind::Other("the constructor of an argument refusal: each caller states its own check (`audit_of`, `check_all_scenarios_flags`, `approval_binding`)")),
    site("evidence.rs:required:EVIDENCE_ARGUMENT_REQUIRED#1", Kind::Other("the constructor of an argument refusal that needs another argument: each caller states its own check")),
    site("evidence.rs:conflict:EVIDENCE_ARGUMENT_CONFLICT#1", Kind::Other("the constructor of an argument refusal that contradicts another argument: each caller states its own check")),
    site("evidence.rs:subject_required:EVIDENCE_REQUIRED#1", Kind::Other("the constructor of the refusal of a record with no task or scenario: the caller states the check")),
    site("evidence.rs:check_approval:EVIDENCE_APPROVAL_INVALID#1", Kind::Domain { command: "evidence add", argument: "approver" }), // reason MALFORMED: the approver is one trimmed line, the role `human`, the time RFC 3339
    site("evidence.rs:check_approval:EVIDENCE_APPROVAL_REQUIRED#1", Kind::Rule { command: "evidence add", rule: "requires", arguments: &["approver", "approval-role", "signature-file", "approval-at"] }), // the four go together; the approver one is the rule's first
    site("evidence.rs:run_add:EVIDENCE_APPROVAL_INVALID#1", Kind::Precondition("reads the file system: the detached signature file must exist")),
    site("evidence.rs:run_add:EVIDENCE_ARGUMENT_REQUIRED#1", Kind::Rule { command: "evidence add", rule: "requires", arguments: &["command-result", "summary"] }),
    site("evidence.rs:run_add:EVIDENCE_ARGUMENT_INVALID#4", Kind::Domain { command: "evidence add", argument: "summary" }),
    site("evidence.rs:run_add:EVIDENCE_REQUIRED#1", Kind::Rule { command: "evidence add", rule: "atLeastOne", arguments: &["task", "scenario", "all-scenarios"] }),
    site("evidence.rs:run_add:EVIDENCE_ARGUMENT_REQUIRED#2", Kind::Unreachable("run/evidence.rs resolves `--project` (the only spec root when none is given, a refusal when there are several: a precondition) before `run_add`")),
    site("evidence.rs:run_add:EVIDENCE_ARGUMENT_REQUIRED#3", Kind::Rule { command: "evidence add", rule: "requires", arguments: &["report-case", "report"] }),
    site("evidence.rs:run_add:EVIDENCE_APPROVAL_INVALID#2", Kind::Precondition("reads the file system: the detached signature file must be readable")),
    site("evidence.rs:run_add:EVIDENCE_APPROVAL_INVALID#3", Kind::Precondition("reads .canon/policy.yaml: no pinned allowed_signers verifier")),
    site("evidence.rs:run_add:EVIDENCE_APPROVAL_INVALID#4", Kind::Precondition("reads .canon/policy.yaml: the approval settings are malformed")),
    site("evidence.rs:run_add:EVIDENCE_APPROVAL_INVALID#5", Kind::Precondition("verifies the signature against the policy's allowed_signers (exit 1)")),
    site("evidence.rs:run_add:EVIDENCE_APPROVAL_INVALID#6", Kind::Precondition("reads the signature file's bytes: they must be UTF-8 armored text")),
    site("evidence.rs:check_artifact_scopes:EVIDENCE_ARGUMENT_CONFLICT#1", Kind::Validator { command: "evidence add", arguments: &["artifact", "scenario", "all-scenarios"] }),
    site("evidence.rs:run_add_all:EVIDENCE_ARGUMENT_INVALID#1", Kind::Unreachable("the scenario ids come from the indexed scenarios of the spec root, which parsed as scenario ids when they were indexed")),
    // ------------------------------------------------------------------------------------------ finding.rs
    site("finding.rs:check_add:FINDING_ARGUMENT_INVALID#1", Kind::Domain { command: "finding add", argument: "round" }),
    site("finding.rs:check_add:FINDING_ARGUMENT_INVALID#2", Kind::Domain { command: "finding add", argument: "summary" }),
    site("finding.rs:check_add:FINDING_ARGUMENT_INVALID#3", Kind::Domain { command: "finding add", argument: "file-ref" }),
    site("finding.rs:check_add:FINDING_RESOLUTION_REV_CONFLICT#1", Kind::Rule { command: "finding add", rule: "forbids", arguments: &["disposition", "resolution-rev"] }),
    site("finding.rs:check_add:FINDING_ARGUMENT_INVALID#4", Kind::Rule { command: "finding add", rule: "atMostOne", arguments: &["introduced-rev", "introduced-unknown"] }),
    site("finding.rs:check_add:FINDING_INTRODUCED_REQUIRED#1", Kind::Validator { command: "finding add", arguments: &["severity", "introduced-rev", "introduced-unknown"] }),
    site("finding.rs:run_add:FINDING_ARGUMENT_INVALID#1", Kind::Unreachable("the natural key is built from the change id, round and seq, which the parser and the round/seq domain already hold to the key's grammar")),
    site("finding.rs:run_close:FINDING_ARGUMENT_INVALID#1", Kind::Domain { command: "finding close", argument: "round" }),
    site("finding.rs:closing_role:FINDING_ARGUMENT_INVALID#1", Kind::Precondition("reads the declared role (--as, CANON_ROLE): --role must name it")),
    site("finding.rs:run_close:FINDING_RESOLUTION_REV_CONFLICT#1", Kind::Rule { command: "finding close", rule: "forbids", arguments: &["disposition", "resolution-rev"] }),
    site("finding.rs:run_close:FINDING_REASON_REQUIRED#1", Kind::Rule { command: "finding close", rule: "requires", arguments: &["disposition", "reason"] }),
    site("finding.rs:run_close:FINDING_REASON_REQUIRED#2", Kind::Domain { command: "finding close", argument: "reason" }),
    site("finding.rs:run_close:FINDING_REASON_REQUIRED#3", Kind::Domain { command: "finding close", argument: "reason" }),
    // --------------------------------------------------------------------------------------- divergence.rs
    site("divergence.rs:run_accept:DIVERGENCE_ARGUMENT_INVALID#1", Kind::Domain { command: "divergence accept", argument: "tracking" }),
    site("divergence.rs:not_reviewer:DIVERGENCE_DENIED#1", Kind::Validator { command: "divergence accept", arguments: &["role"] }),
    site("divergence.rs:run_accept:DIVERGENCE_ARGUMENT_INVALID#2", Kind::Precondition("reads the scenario index: a scenario the index does not know")),
    site("divergence.rs:run_accept:DIVERGENCE_DENIED#1", Kind::Precondition("reads the scenario's latest evidence and the actors and sessions that attested it")),
    // ------------------------------------------------------------------------------------------ review.rs
    site("review.rs:run_add:REVIEW_ARGUMENT_INVALID#1", Kind::Unreachable("clap's `provenance` ArgGroup (exactly one of `--upstream-ref`, `--original-spec-ref`) refuses first; declared as the `exactlyOne` rule")),
    site("review.rs:run_add:REVIEW_ARGUMENT_INVALID#2", Kind::Unreachable("clap's `provenance` ArgGroup refuses the pair first; declared as the `exactlyOne` rule")),
    site("review.rs:run_add:REVIEW_ARGUMENT_INVALID#3", Kind::Precondition("reads the change's brief: a change-level review is for a chore whose brief is in the repository")),
    // ----------------------------------------------------------------------------------------- dispatch.rs
    site("dispatch.rs:to_cli_error:DISPATCH_ROLE_CONFLICT#1", Kind::Validator { command: "dispatch begin", arguments: &["role", "regime"] }),
    site("dispatch.rs:to_cli_error:DISPATCH_ARGUMENT_INVALID#1", Kind::Rule { command: "dispatch begin", rule: "requires", arguments: &["skill-digest", "skill"] }),
    // --------------------------------------------------------------------------------------------- run/
    site("run/evidence.rs:project_for:EVIDENCE_ARGUMENT_REQUIRED#1", Kind::Precondition("reads canon.yaml: `--project` defaults to the only spec root and is required when there are several")),
    site("evidence.rs:check_add:EVIDENCE_ARGUMENT_INVALID#1", Kind::Validator { command: "evidence add", arguments: &["class"] }),
    site("run/divergence.rs:project_for:DIVERGENCE_ARGUMENT_INVALID#1", Kind::Precondition("reads canon.yaml: `--project` defaults to the only spec root and is required when there are several")),
    site("run/divergence.rs:run_divergence_add:DIVERGENCE_ARGUMENT_INVALID#1", Kind::Rule { command: "divergence add", rule: "requires", arguments: &["state", "reason", "expiry"] }),
    site("run/review.rs:run_review:REVIEW_ARGUMENT_INVALID#1", Kind::Precondition("reads canon.yaml: `--project` defaults to the only spec root and is required when there are several")),
    site("run/mod.rs:record_role:USAGE#1", Kind::Undeclared("the role is the ambient `--as` / CANON_ROLE when `--role` is not given: a check of the environment, not of arguments")),
];
