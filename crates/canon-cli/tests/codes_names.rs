//! `canon-0-36-codes-rename`: the registry's names follow the contract's `<SUBJECT>_<CONDITION>` rule (spec 7.2,
//! `atip::code_name_problem` at 1.3). The codes a later change of 0.36 still renames are listed here; each such change takes
//! its codes out of the list, and `canon-0-36-codes-table` checks that the list is empty.

use atip::Minor;

/// The codes that do not follow the rule yet, and the change of 0.36 that renames each.
const NOT_YET_RENAMED: &[&str] = &[
    "ADAPTER_EXECUTION_NOT_AUTHORIZED",
    "ADAPTER_REFUSED",
    "ADAPTER_RESPONSE_INVALID",
    "ARTIFACTS_MOVE_REFUSED",
    "ARTIFACTS_STORE_UNREACHABLE",
    "BRIEF_APPROVE_REFUSED",
    "BRIEF_CHANGED_SINCE_APPROVAL",
    "BRIEF_UNAPPROVED",
    "BRIEF_UNREADABLE",
    "CONFIG_UNREADABLE",
    "CONTEXT_GIT_UNAVAILABLE",
    "CONTEXT_OBJECT_CONFLICT",
    "CONTEXT_PACK_TAMPERED",
    "DASHBOARD_NOT_BUILT",
    "DASHBOARD_PORT_IN_USE",
    "DISPATCH_ALREADY_ENDED",
    "DISPATCH_END_IN_PROGRESS",
    "DISPATCH_MANIFEST_MISMATCH",
    "DISPATCH_MANIFEST_UNREADABLE",
    "DISPATCH_NOT_RUNNING",
    "DIVERGENCE_ACCEPT_REFUSED",
    "DIVERGENCE_REFUSED",
    "EVIDENCE_BATCH_REFUSED",
    "EVIDENCE_BIND_REFUSED",
    "EVIDENCE_CONTRACT_MISMATCH",
    "EVIDENCE_FABRICATED",
    "EVIDENCE_INVALID_ARGUMENT",
    "EVIDENCE_REFUSED",
    "EVIDENCE_REPORT_CONTRADICTION",
    "EVIDENCE_SCENARIO_NOT_INDEXED",
    "GATE_TASK_BLOCKED",
    "GATE_TASK_EVIDENCE_INVALID",
    "HOOKS_INSTALL_REFUSED",
    "INGEST_SOURCE_UNUSABLE",
    "INVENTORY_ABORTED",
    "INVENTORY_REFUSED",
    "LEARN_EVALUATION_REJECTED",
    "LEARN_PROMOTION_BLOCKED",
    "LEARN_SIGNATURE_UNREADABLE",
    "LEDGER_ARCHIVE_UNAVAILABLE",
    "MIGRATE_REFUSED",
    "MIGRATE_STOPPED",
    "NO_BRIEF",
    "OLD_BRIEF_FORMAT",
    "OLD_LAYOUT",
    "PLAN_CORPUS_UNREADABLE",
    "PLUGIN_SYNC_ERRORS",
    "PLUGIN_SYNC_FAILED",
    "POLICY_APPROVAL_REFUSED",
    "POLICY_APPROVAL_UNREADABLE",
    "POLICY_CHANGED_SINCE_APPROVAL",
    "PROMOTE_REFUSED",
    "REPORT_STALE",
    "REPO_UNREADABLE",
    "REPO_WRITE_BUSY",
    "RETENTION_EXPORT_REFUSED",
    "RETENTION_PURGE_REFUSED",
    "REVIEW_ARTIFACT_REFUSED",
    "REVIEW_SCENARIO_NOT_INDEXED",
    "SKILLS_INVALID_ARGUMENT",
    "SPEC_ROOTS_UNREADABLE",
    "STOPPED_MIDWAY",
    "SUBJECT_ADOPT_REFUSED",
    "SUBJECT_GATE_VIOLATION",
    "SUBJECT_UNADOPT_REFUSED",
    "WRITE_UNAVAILABLE",
];

fn problem(info: &atip::CodeInfo) -> Option<String> {
    atip::code_name_problem(Minor::V1_3, info.code, Some(i64::from(info.exit)), Some(info.retryable))
}

/// atip.codenames.03: every code of the registry that is not listed follows the rule.
#[test]
fn atip_codenames_03_every_code_but_the_listed_ones_follows_the_name_rule() {
    let bad: Vec<String> = canon_cli::cli_error::all_codes().iter().filter(|info| !NOT_YET_RENAMED.contains(&info.code)).filter_map(problem).collect();
    assert!(bad.is_empty(), "a code breaks the name rule and is not on the list of codes still to rename: {bad:#?}");
    let all: Vec<&str> = canon_cli::cli_error::all_codes().iter().map(|info| info.code).collect();
    for listed in NOT_YET_RENAMED {
        assert!(all.contains(listed), "`{listed}` is on the list but not in the registry: take it off");
    }
}

/// atip.codenames.04: a listed code that follows the rule is taken off the list.
#[test]
fn atip_codenames_04_a_listed_code_that_follows_the_rule_fails_the_test() {
    let passing: Vec<&str> = canon_cli::cli_error::all_codes().iter().filter(|info| NOT_YET_RENAMED.contains(&info.code) && problem(info).is_none()).map(|info| info.code).collect();
    assert!(passing.is_empty(), "these codes follow the rule: take them off NOT_YET_RENAMED: {passing:?}");
}

/// atip.codenames.04 (failure case): a name that does not end in a condition is refused by the rule the tests use.
#[test]
fn atip_codenames_04_a_name_without_a_condition_or_with_the_wrong_exit_is_refused_by_the_rule() {
    for name in ["HUMAN_ONLY", "NO_COMMIT", "UNKNOWN_ROLE", "FINDING_STAGED"] {
        assert!(atip::code_name_problem(Minor::V1_3, name, Some(2), Some(false)).is_some(), "{name}");
    }
    assert!(atip::code_name_problem(Minor::V1_3, "ACTOR_DENIED", Some(2), Some(false)).is_none());
    assert!(atip::code_name_problem(Minor::V1_3, "ACTOR_DENIED", Some(1), Some(false)).is_some(), "a wrong exit is refused too");
}

/// The renamed codes, each with its exit, retryable and reason: what the table of the brief says.
const RENAMED: &[(&str, i64, Option<&str>)] = &[
    ("APPROVAL_INVALID", 2, None),
    ("BRIEF_PAGE_STALE", 2, None),
    ("BRIEF_CHECK_FAILED", 1, Some("BLOCKERS")),
    ("BRIEF_INVALID", 2, Some("UNAPPROVABLE")),
    ("CHANGE_REQUIRED", 2, None),
    ("CHANGE_ARGUMENT_INVALID", 2, None),
    ("CONFIG_REQUIRED", 2, None),
    ("CONTEXT_INPUT_NOT_FOUND", 2, None),
    ("CONTEXT_INPUT_INVALID", 2, Some("SECRET")),
    ("CONTEXT_PATH_INVALID", 2, None),
    ("DASHBOARD_REQUEST_INVALID", 2, None),
    ("CONFIG_INVALID", 2, Some("DASHBOARD")),
    ("DASHBOARD_REQUEST_DENIED", 2, None),
    ("DEMO_REQUIRED", 2, None),
    ("DISPATCH_ARGUMENT_INVALID", 2, None),
    ("DISPATCH_ROLE_CONFLICT", 2, None),
    ("DIVERGENCE_ARGUMENT_INVALID", 2, None),
    ("FEATURE_PATH_INVALID", 2, None),
    ("FINDING_ARGUMENT_INVALID", 2, None),
    ("FINDING_RESOLUTION_REV_CONFLICT", 2, None),
    ("FINDING_EXISTS", 2, None),
    ("FINDING_CONFLICT", 2, None),
    ("SCENARIO_NOT_FOUND", 2, None),
    ("FORMAT_CHECK_FAILED", 1, None),
    ("TASK_ID_INVALID", 2, None),
    ("TASK_UNSUPPORTED", 2, None),
    ("GATE_CHECK_FAILED", 1, None),
    ("ACTOR_DENIED", 2, None),
    ("CONFIG_EXISTS", 2, None),
    ("INIT_ARGUMENT_INVALID", 2, None),
    ("STRATEGY_DEMOTION_FAILED", 1, None),
    ("STRATEGY_ARGUMENT_INVALID", 2, None),
    ("STRATEGY_FILE_INVALID", 2, None),
    ("STRATEGY_NOT_FOUND", 2, None),
    ("TRAJECTORY_NOT_FOUND", 2, None),
    ("MIGRATION_CONFLICT", 2, None),
    ("COMMIT_REQUIRED", 2, None),
    ("PLAN_SOURCES_REQUIRED", 2, None),
    ("QUERY_ARGUMENT_INVALID", 2, None),
    ("RELAY_DIGEST_STALE", 2, None),
    ("RETRIEVE_INVALID", 2, None),
    ("REVIEW_ARGUMENT_INVALID", 2, None),
    ("SCAFFOLD_ARGUMENT_INVALID", 2, None),
    ("SEARCH_QUERY_REQUIRED", 2, None),
    ("SKILLS_CHECK_FAILED", 1, Some("DRIFT")),
    ("SPEC_ROOT_REQUIRED", 2, None),
    ("SUBJECT_ARGUMENT_INVALID", 2, None),
    ("SUBJECT_CONFLICT", 2, Some("TRANSITION")),
    ("ROLE_INVALID", 2, None),
    ("SPEC_ROOT_NOT_FOUND", 2, None),
];

/// atip.codenames.03: each renamed code is in the registry with the exit, retryable `false` and reason of the brief's table, and follows the rule.
#[test]
fn atip_codenames_03_each_renamed_code_has_the_exit_and_reason_of_the_table() {
    assert_eq!(RENAMED.len(), 50);
    for (code, exit, reason) in RENAMED {
        let info = canon_cli::cli_error::all_codes().iter().find(|info| info.code == *code).unwrap_or_else(|| panic!("`{code}` is not in the registry"));
        assert_eq!((i64::from(info.exit), info.retryable), (*exit, false), "{code}");
        assert_eq!(problem(info), None, "{code}");
        match reason {
            Some(reason) => assert_eq!(info.reasons.iter().map(|r| r.reason).collect::<Vec<_>>(), [*reason], "{code}: the reason is registered"),
            None => assert!(info.reasons.is_empty(), "{code}"),
        }
    }
}
