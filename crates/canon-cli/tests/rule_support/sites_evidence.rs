//! The classified sites of this group (see `sites.rs`): evidence, reviews, divergences, findings, dispatch.

use super::sites::{site, Kind, Site};

pub const SITES: &[Site] = &[
    // ---------------------------------------------------------------------------------------- evidence.rs
    site("evidence.rs:fabricated:EVIDENCE_FABRICATED#1", Kind::Undeclared("a note that a fabrication-marker scan rejects is refused with exit 1 (gate-red wording), and the contract's validators carry an exit 2 code; ATIP feedback")),
    site("evidence.rs:run_approval_payload:EVIDENCE_REFUSED#1", Kind::Rule { command: "evidence approval-payload", rule: "atLeastOne", arguments: &["task", "scenario"] }),
    // run_add: the checks of `evidence add` and `evidence audit`, in the order they run.
    site("evidence.rs:run_add:EVIDENCE_INVALID_ARGUMENT#1", Kind::Domain { command: "evidence add", argument: "class" }),
    site("evidence.rs:run_add:EVIDENCE_INVALID_ARGUMENT#2", Kind::Rule { command: "evidence add", rule: "forbids", arguments: &["all-scenarios", "scenario"] }),
    site("evidence.rs:run_add:EVIDENCE_INVALID_ARGUMENT#3", Kind::Domain { command: "evidence audit", argument: "evaluator" }),
    site("evidence.rs:run_add:EVIDENCE_INVALID_ARGUMENT#4", Kind::Domain { command: "evidence add", argument: "override-reason" }),
    site("evidence.rs:run_add:EVIDENCE_INVALID_ARGUMENT#5", Kind::Unreachable("clap's `parse_surface_ref` value parser refuses the same values first, with USAGE (the surface-ref domain and validator)")),
    site("evidence.rs:run_add:EVIDENCE_APPROVAL_INVALID#1", Kind::Rule { command: "evidence add", rule: "requires", arguments: &["approver", "approval-role", "signature-file", "approval-at"] }),
    site("evidence.rs:run_add:EVIDENCE_APPROVAL_INVALID#2", Kind::Domain { command: "evidence add", argument: "approver" }),
    site("evidence.rs:run_add:EVIDENCE_APPROVAL_INVALID#3", Kind::Unreachable("`--approval-role` is a closed set (`human`) in clap, which refuses any other value first")),
    site("evidence.rs:run_add:EVIDENCE_APPROVAL_INVALID#4", Kind::Unreachable("clap's `timestamp::normalize` value parser hands over an RFC 3339 string or refuses the value first (the TIMESTAMP domain and the calendar validator)")),
    site("evidence.rs:run_add:EVIDENCE_APPROVAL_INVALID#5", Kind::Precondition("reads the file system: the detached signature file must exist")),
    site("evidence.rs:run_add:EVIDENCE_INVALID_ARGUMENT#6", Kind::Rule { command: "evidence add", rule: "requires", arguments: &["command-result", "summary"] }),
    site("evidence.rs:run_add:EVIDENCE_INVALID_ARGUMENT#7", Kind::Domain { command: "evidence add", argument: "summary" }),
    site("evidence.rs:run_add:EVIDENCE_INVALID_ARGUMENT#8", Kind::Rule { command: "evidence add", rule: "atLeastOne", arguments: &["task", "scenario", "all-scenarios"] }),
    site("evidence.rs:run_add:EVIDENCE_INVALID_ARGUMENT#9", Kind::Unreachable("run/evidence.rs resolves `--project` (the only spec root when none is given, a refusal when there are several: a precondition) before `run_add`")),
    site("evidence.rs:run_add:EVIDENCE_INVALID_ARGUMENT#10", Kind::Rule { command: "evidence add", rule: "requires", arguments: &["report-case", "report"] }),
    site("evidence.rs:run_add:EVIDENCE_REFUSED#1", Kind::Precondition("the approval binding reads the plan corpus (`--task`) and the git diff of `--rev`; its argument checks repeat the approver, `--approval-at` and subject checks answered above")),
    site("evidence.rs:run_add:EVIDENCE_APPROVAL_INVALID#6", Kind::Precondition("reads the file system: the detached signature file must be readable")),
    site("evidence.rs:run_add:EVIDENCE_APPROVAL_INVALID#7", Kind::Precondition("reads .canon/policy.yaml: no pinned allowed_signers verifier")),
    site("evidence.rs:run_add:EVIDENCE_APPROVAL_INVALID#8", Kind::Precondition("reads .canon/policy.yaml: the approval settings are malformed")),
    site("evidence.rs:run_add:EVIDENCE_APPROVAL_INVALID#9", Kind::Precondition("verifies the signature against the policy's allowed_signers (exit 1)")),
    site("evidence.rs:run_add:EVIDENCE_APPROVAL_INVALID#10", Kind::Precondition("reads the signature file's bytes: they must be UTF-8 armored text")),
    site("evidence.rs:check_artifact_scopes:EVIDENCE_INVALID_ARGUMENT#1", Kind::Validator { command: "evidence add", arguments: &["artifact", "scenario", "all-scenarios"] }),
    site("evidence.rs:run_add_all:EVIDENCE_INVALID_ARGUMENT#1", Kind::Unreachable("the scenario ids come from the indexed scenarios of the spec root, which parsed as scenario ids when they were indexed")),
    // ------------------------------------------------------------------------------------------ finding.rs
    site("finding.rs:run_add:FINDING_INVALID_ARGUMENT#1", Kind::Domain { command: "finding add", argument: "round" }),
    site("finding.rs:run_add:FINDING_INVALID_ARGUMENT#2", Kind::Domain { command: "finding add", argument: "summary" }),
    site("finding.rs:run_add:FINDING_INVALID_ARGUMENT#3", Kind::Domain { command: "finding add", argument: "file-ref" }),
    site("finding.rs:run_add:FINDING_RESOLUTION_REV_UNEXPECTED#1", Kind::Rule { command: "finding add", rule: "forbids", arguments: &["disposition", "resolution-rev"] }),
    site("finding.rs:run_add:FINDING_INVALID_ARGUMENT#4", Kind::Rule { command: "finding add", rule: "atMostOne", arguments: &["introduced-rev", "introduced-unknown"] }),
    site("finding.rs:run_add:FINDING_INTRODUCED_REQUIRED#1", Kind::Validator { command: "finding add", arguments: &["severity", "introduced-rev", "introduced-unknown"] }),
    site("finding.rs:run_add:FINDING_INVALID_ARGUMENT#5", Kind::Unreachable("the natural key is built from the change id, round and seq, which the parser and the round/seq domain already hold to the key's grammar")),
    site("finding.rs:run_close:FINDING_INVALID_ARGUMENT#1", Kind::Domain { command: "finding close", argument: "round" }),
    site("finding.rs:run_close:FINDING_RESOLUTION_REV_UNEXPECTED#1", Kind::Rule { command: "finding close", rule: "forbids", arguments: &["disposition", "resolution-rev"] }),
    site("finding.rs:run_close:FINDING_REASON_REQUIRED#1", Kind::Rule { command: "finding close", rule: "requires", arguments: &["disposition", "reason"] }),
    site("finding.rs:run_close:FINDING_REASON_REQUIRED#2", Kind::Domain { command: "finding close", argument: "reason" }),
    site("finding.rs:run_close:FINDING_REASON_REQUIRED#3", Kind::Domain { command: "finding close", argument: "reason" }),
    // --------------------------------------------------------------------------------------- divergence.rs
    site("divergence.rs:run_accept:DIVERGENCE_INVALID_ARGUMENT#1", Kind::Domain { command: "divergence accept", argument: "tracking" }),
    site("divergence.rs:run_accept:DIVERGENCE_ACCEPT_REFUSED#1", Kind::Domain { command: "divergence accept", argument: "role" }),
    site("divergence.rs:run_accept:DIVERGENCE_INVALID_ARGUMENT#2", Kind::Precondition("reads the scenario index: a scenario the index does not know")),
    site("divergence.rs:run_accept:DIVERGENCE_ACCEPT_REFUSED#2", Kind::Precondition("reads the scenario's latest evidence and the actors and sessions that attested it")),
    // ------------------------------------------------------------------------------------------ review.rs
    site("review.rs:run_add:REVIEW_INVALID_ARGUMENT#1", Kind::Unreachable("clap's `provenance` ArgGroup (exactly one of `--upstream-ref`, `--original-spec-ref`) refuses first; declared as the `exactlyOne` rule")),
    site("review.rs:run_add:REVIEW_INVALID_ARGUMENT#2", Kind::Unreachable("clap's `provenance` ArgGroup refuses the pair first; declared as the `exactlyOne` rule")),
    site("review.rs:run_add:REVIEW_INVALID_ARGUMENT#3", Kind::Precondition("reads the change's brief: a change-level review is for a chore whose brief is in the repository")),
    // ----------------------------------------------------------------------------------------- dispatch.rs
    site("dispatch.rs:to_cli_error:DISPATCH_ROLE_REGIME_MISMATCH#1", Kind::Validator { command: "dispatch begin", arguments: &["role", "regime"] }),
    site("dispatch.rs:to_cli_error:DISPATCH_INVALID_ARGUMENT#1", Kind::Rule { command: "dispatch begin", rule: "requires", arguments: &["skill-digest", "skill"] }),
    // --------------------------------------------------------------------------------------------- run/
    site("run/evidence.rs:project_for:EVIDENCE_INVALID_ARGUMENT#1", Kind::Precondition("reads canon.yaml: `--project` defaults to the only spec root and is required when there are several")),
    site("run/evidence.rs:run:EVIDENCE_INVALID_ARGUMENT#1", Kind::Validator { command: "evidence add", arguments: &["class"] }),
    site("run/divergence.rs:project_for:DIVERGENCE_INVALID_ARGUMENT#1", Kind::Precondition("reads canon.yaml: `--project` defaults to the only spec root and is required when there are several")),
    site("run/divergence.rs:run_divergence_add:DIVERGENCE_INVALID_ARGUMENT#1", Kind::Rule { command: "divergence add", rule: "requires", arguments: &["state", "reason", "expiry"] }),
    site("run/review.rs:run_review:REVIEW_INVALID_ARGUMENT#1", Kind::Precondition("reads canon.yaml: `--project` defaults to the only spec root and is required when there are several")),
    site("run/mod.rs:record_role:USAGE#1", Kind::Undeclared("the role is the ambient `--as` / CANON_ROLE when `--role` is not given: a check of the environment, not of arguments")),
];
