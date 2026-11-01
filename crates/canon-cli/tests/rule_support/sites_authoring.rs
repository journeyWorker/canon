//! The classified sites of this group (see `sites.rs`).

use super::sites::{site, Kind, Site};

pub const SITES: &[Site] = &[
    // brief approve: the human's answer is checked before anything is read
    site("brief_approve.rs:answer:APPROVAL_INCOMPLETE#1", Kind::Domain { command: "brief approve", argument: "approver" }),
    site("brief_approve.rs:answer:APPROVAL_INCOMPLETE#2", Kind::Domain { command: "brief approve", argument: "note" }),
    site("brief_approve.rs:hint_for:APPROVAL_INCOMPLETE#1", Kind::Other("the next step the hint names for the code; not a check")),
    site("brief_approve.rs:run_approve:USAGE#1", Kind::Validator { command: "brief approve", arguments: &["digest", "change"] }),
    site("brief_approve.rs:verify_shown:USAGE#1", Kind::Precondition("a relayed approval of the repo's only live change (no operand names it) needs the digest the human was shown: the change is read from the repo first")),
    // change new
    site("change.rs:run_new:CHANGE_INVALID_ARGUMENT#1", Kind::Precondition("the staged brief and the plan adapter that reads it fail: reads and writes the staging directory, not an argument's shape")),
    // context packs
    site("context_pack.rs:pack_id_usage_error:USAGE#1", Kind::Domain { command: "context-pack show", argument: "id" }),
    // task done
    site("gate.rs:run_task:GATE_TASK_INVALID_ARGUMENT#1", Kind::Domain { command: "task done", argument: "task" }),
    // strategies
    site("learn.rs:run_promote_with_evidence:LEARN_INVALID_ARGUMENT#1", Kind::Unreachable("clap's `requires` between --evaluation and --approval (declared as rules of `strategy promote`) answers first")),
    site("learn.rs:run_rollback:LEARN_INVALID_ARGUMENT#1", Kind::Precondition("the id is parsed after the strategy is looked up in the store (an unknown strategy is refused first), so a bad --contradicting-trajectory is only reached after a state read")),
    site("learn.rs:run_rollback:LEARN_INVALID_ARGUMENT#2", Kind::Unreachable("--approval-at is normalised to RFC 3339 by clap's value parser before the handler runs; the calendar check is the validator of `strategy rollback`")),
    // policy approve
    site("policy_approval.rs:run_approve:POLICY_APPROVAL_REFUSED#1", Kind::Domain { command: "policy approve", argument: "approver" }),
    // relayed approvals
    site("relay.rs:authorize:USAGE#1", Kind::Undeclared("--relayed-via is refused unless an agent role is declared (--as or CANON_ROLE): the ambient role is the environment, not an argument of the call")),
    site("relay.rs:authorize:USAGE#2", Kind::Domain { command: "brief approve", argument: "relayed-via" }),
    site("relay.rs:authorize:USAGE#3", Kind::Domain { command: "brief approve", argument: "actor" }),
    site("relay.rs:authorize:USAGE#4", Kind::Domain { command: "brief approve", argument: "actor" }),
    site("relay.rs:authorize:USAGE#5", Kind::Validator { command: "policy approve", arguments: &["relayed-via", "note"] }),
    // brief approve --digest
    site("run/brief.rs:run:USAGE#1", Kind::Domain { command: "brief approve", argument: "digest" }),
    site("run/brief.rs:run:USAGE#2", Kind::Validator { command: "brief approve", arguments: &["digest"] }),
    // scenario new
    site("scaffold.rs:plan:SCAFFOLD_INVALID_ARGUMENT#1", Kind::Precondition("--flow is resolved against the brief of the change: reads the repo")),
    site("scaffold.rs:plan:SCAFFOLD_INVALID_ARGUMENT#2", Kind::Precondition("--lane and --case are members of the vocabulary the repo activates: reads the repo's vocabulary")),
    // subject
    site("subject.rs:run_move:SUBJECT_INVALID_ARGUMENT#1", Kind::Domain { command: "subject move", argument: "reason" }),
    site("subject.rs:run_move:SUBJECT_REASON_REQUIRED#1", Kind::Precondition("reopening is a move out of the subject's stored status `shipped`; whether --reason is needed depends on that state")),
    site("subject.rs:run_new:SUBJECT_INVALID_ARGUMENT#1", Kind::Domain { command: "subject new", argument: "domain" }),
    site("subject.rs:run_new:SUBJECT_INVALID_ARGUMENT#2", Kind::Precondition("the domain is a member of the repo's activated `domain` vocabulary: reads the repo")),
];
