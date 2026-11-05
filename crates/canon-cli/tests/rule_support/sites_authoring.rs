//! The classified sites of this group (see `sites.rs`).

use super::sites::{site, Kind, Site};

pub const SITES: &[Site] = &[
    // brief approve: the human's answer is checked before anything is read
    site("brief_approve.rs:answer:APPROVAL_INVALID#1", Kind::Validator { command: "brief approve", arguments: &["approver", "note"] }),
    site("brief_approve.rs:answer:APPROVAL_INVALID#2", Kind::Validator { command: "brief approve", arguments: &["approver", "note"] }),
    site("brief_approve.rs:hint_for:APPROVAL_INVALID#1", Kind::Other("the next step the hint names for the code; not a check")),
    site("brief_approve.rs:check_approve:USAGE#1", Kind::Validator { command: "brief approve", arguments: &["digest", "change"] }),
    site("brief_approve.rs:verify_shown:USAGE#1", Kind::Precondition("a relayed approval of the repo's only live change (no operand names it) needs the digest the human was shown: the change is read from the repo first")),
    // change new
    site("change.rs:run_new:CHANGE_ARGUMENT_INVALID#1", Kind::Unreachable("the staged brief is canon's own template and no --title (blank, several lines, a markdown heading, a colon, a tab) was found to make the plan adapter reject it; what is left is a failure of the staging directory, not an argument")),
    // context packs
    site("context_pack.rs:pack_id_usage_error:USAGE#1", Kind::Domain { command: "context-pack show", argument: "id" }),
    // task done
    site("gate.rs:run_task:TASK_ID_INVALID#1", Kind::Domain { command: "task done", argument: "task" }),
    // strategies
    site("learn.rs:run_promote_with_evidence:STRATEGY_ARGUMENT_INVALID#1", Kind::Unreachable("clap's `requires` between --evaluation and --approval (declared as rules of `strategy promote`) answers first")),
    site("learn.rs:run_rollback:STRATEGY_ARGUMENT_INVALID#1", Kind::Precondition("the id is parsed after the strategy is looked up in the store (an unknown strategy is refused first), so a bad --contradicting-trajectory is only reached after a state read")),
    site("learn.rs:rollback_refusal:STRATEGY_ARGUMENT_INVALID#1", Kind::Unreachable("the domains of `--reason` and `--actor` refuse a blank one first (the rollback's own check is the library's, for a caller that has no command line)")),
    site("learn.rs:run_rollback:STRATEGY_ARGUMENT_INVALID#2", Kind::Unreachable("--approval-at is normalised to RFC 3339 by clap's value parser before the handler runs; the calendar check is the validator of `strategy rollback`")),
    // policy approve
    site("policy_approval.rs:run_approve:APPROVAL_INVALID#1", Kind::Precondition("the code of the policy record's own refusals (no policy file, an unreadable one); a blank approver is refused before it, by the domain of `--approver`")),
    // relayed approvals
    site("relay.rs:check_role:USAGE#1", Kind::Validator { command: "brief approve", arguments: &["relayed-via", "as"] }),
    site("relay.rs:check_via:USAGE#1", Kind::Validator { command: "brief approve", arguments: &["relayed-via"] }),
    site("relay.rs:check_actor:USAGE#1", Kind::Validator { command: "brief approve", arguments: &["relayed-via", "actor"] }),
    site("relay.rs:check_actor:USAGE#2", Kind::Validator { command: "brief approve", arguments: &["relayed-via", "actor"] }),
    site("relay.rs:check_answer:USAGE#1", Kind::Validator { command: "policy approve", arguments: &["relayed-via", "approver", "note"] }),
    // brief approve --digest
    site("brief_approve.rs:shown_digests:USAGE#1", Kind::Domain { command: "brief approve", argument: "digest" }),
    site("brief_approve.rs:shown_digests:USAGE#2", Kind::Validator { command: "brief approve", arguments: &["digest"] }),
    // scenario new
    site("scaffold.rs:plan:SCAFFOLD_ARGUMENT_INVALID#1", Kind::Precondition("--flow is resolved against the brief of the change: reads the repo")),
    site("scaffold.rs:plan:SCAFFOLD_ARGUMENT_INVALID#2", Kind::Precondition("--lane and --case are members of the vocabulary the repo activates: reads the repo's vocabulary")),
    // subject
    site("subject.rs:run_move:SUBJECT_ARGUMENT_INVALID#1", Kind::Domain { command: "subject move", argument: "reason" }),
    site("subject.rs:run_move:SUBJECT_REASON_REQUIRED#1", Kind::Precondition("reopening is a move out of the subject's stored status `shipped`; whether --reason is needed depends on that state")),
    site("subject.rs:run_new:SUBJECT_ARGUMENT_INVALID#1", Kind::Domain { command: "subject new", argument: "domain" }),
    site("subject.rs:run_new:SUBJECT_ARGUMENT_INVALID#2", Kind::Precondition("the domain is a member of the repo's activated `domain` vocabulary: reads the repo")),
];
