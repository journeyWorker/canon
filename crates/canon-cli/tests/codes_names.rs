//! `canon-0-36-codes-rename`: the registry's names follow the contract's `<SUBJECT>_<CONDITION>` rule (spec 7.2,
//! `atip::code_name_problem` at 1.3). The codes a later change of 0.36 still renames are listed here; each such change takes
//! its codes out of the list, and `canon-0-36-codes-table` checks that the list is empty.

use atip::Minor;

/// The codes that do not follow the rule yet, and the change of 0.36 that renames each.
const NOT_YET_RENAMED: &[&str] = &[
    "INVENTORY_ABORTED",
    "MIGRATE_STOPPED",
    "PLUGIN_SYNC_ERRORS",
    "STOPPED_MIDWAY",
];

/// The codes that pass the name rule (or are listed above) and that `canon-0-36-codes-refusals`, `-stored` or `-incomplete` still change: split by
/// meaning, moved to another exit, merged into another code or renamed with a reason (issue #135). The name check does not see them, so they are
/// listed. Each of those changes deletes its codes from this list; `canon-0-36-codes-table` requires `NOT_YET_RENAMED` and this list to be empty.
const NOT_YET_RECLASSIFIED: &[&str] = &[
    // canon-0-36-codes-incomplete
    "ARTIFACTS_STORE_FAILED",
    "EVIDENCE_STORE_FAILED",
    "HOOKS_INSTALL_FAILED",
    "INVENTORY_ABORTED",
    "LEARN_PROMOTION_FAILED",
    "LEDGER_COMPACT_FAILED",
    "MIGRATE_INDEX_FAILED",
    "MIGRATE_PINS_FAILED",
    "MIGRATE_STOPPED",
    "PLUGIN_SYNC_ERRORS",
    "PROMOTE_FAILED",
    "RETENTION_PURGE_FAILED",
    "STOPPED_MIDWAY",
    "TIER_FAILED",
];

/// atip.codenames.04: every code of the second list is in the registry (a code a change already renamed is taken off), and none of the
/// `canon-0-36-codes-exit` codes is on either list.
#[test]
fn atip_codenames_04_the_list_of_codes_still_to_reclassify_holds_only_registered_codes() {
    let all: Vec<&str> = canon_cli::cli_error::all_codes().iter().map(|info| info.code).collect();
    for listed in NOT_YET_RECLASSIFIED {
        assert!(all.contains(listed), "`{listed}` is on NOT_YET_RECLASSIFIED but not in the registry: take it off");
    }
    let mut sorted_unique: Vec<&&str> = NOT_YET_RECLASSIFIED.iter().collect();
    sorted_unique.sort_unstable();
    sorted_unique.dedup();
    assert_eq!(sorted_unique.len(), NOT_YET_RECLASSIFIED.len(), "a code is listed twice");
    for exit_code in ["BRIEF_APPROVAL_REQUIRED", "BRIEF_APPROVAL_STALE", "SUBJECT_INVALID", "PROMOTION_INVALID", "REPORT_CHECK_FAILED", "CONTEXT_PACK_VERIFY_FAILED"] {
        assert!(!NOT_YET_RECLASSIFIED.contains(&exit_code) && !NOT_YET_RENAMED.contains(&exit_code), "`{exit_code}` was settled by codes-exit");
    }
}

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
            Some(reason) => assert!(info.reasons.iter().any(|r| r.reason == *reason), "{code}: the reason is registered"),
            None => assert!(info.reasons.is_empty(), "{code}"),
        }
    }
}

/// The codes `canon-0-36-codes-refusals` registers: each with its exit, retryable and the reasons registered under it (the brief's table).
const SPLIT: &[(&str, i64, bool, &[&str])] = &[
    ("ADAPTER_FILE_DENIED", 2, false, &[]),
    ("ADAPTER_FILE_INVALID", 2, false, &[]),
    ("ADAPTER_FILE_NOT_FOUND", 2, false, &[]),
    ("ARTIFACTS_BLOB_CORRUPT", 2, false, &[]),
    ("ARTIFACTS_TIER_INVALID", 2, false, &[]),
    ("BRIEF_MIGRATION_REQUIRED", 2, false, &[]),
    ("BRIEF_REQUIRED", 2, false, &[]),
    ("CHANGE_NOT_FOUND", 2, false, &[]),
    ("CONFIG_INVALID", 2, false, &["ARTIFACT_STORE", "DASHBOARD", "POLICY", "SKILLS", "SPECS"]),
    ("DISPATCH_CONFLICT", 2, false, &["ALREADY_ENDED", "NOT_RUNNING"]),
    ("DIVERGENCE_CONFLICT", 2, false, &["NOT_DIVERGENT"]),
    ("DIVERGENCE_DENIED", 2, false, &["NOT_REVIEWER", "SELF_ATTESTED"]),
    ("EVIDENCE_APPROVAL_INVALID", 2, false, &["MALFORMED", "SIGNATURE_MISMATCH"]),
    ("EVIDENCE_APPROVAL_REQUIRED", 2, false, &[]),
    ("EVIDENCE_ARGUMENT_CONFLICT", 2, false, &[]),
    ("EVIDENCE_ARGUMENT_INVALID", 2, false, &[]),
    ("EVIDENCE_ARGUMENT_REQUIRED", 2, false, &[]),
    ("EVIDENCE_BINDING_DENIED", 2, false, &[]),
    ("EVIDENCE_BINDING_INVALID", 2, false, &[]),
    ("EVIDENCE_BINDING_NOT_FOUND", 2, false, &[]),
    ("EVIDENCE_BINDING_STALE", 2, false, &[]),
    ("EVIDENCE_INVALID", 2, false, &[]),
    ("EVIDENCE_REQUIRED", 2, false, &[]),
    ("HOOKS_FILE_CONFLICT", 2, false, &[]),
    ("HOOKS_FILE_DENIED", 2, false, &[]),
    ("HOOKS_REPO_INVALID", 2, false, &[]),
    ("INVENTORY_INDEX_UNAVAILABLE", 2, true, &[]),
    ("LAYOUT_MIGRATION_REQUIRED", 2, false, &[]),
    ("MIGRATION_DENIED", 2, false, &[]),
    ("MIGRATION_INVALID", 2, false, &[]),
    ("PLUGIN_REQUIRED", 2, false, &[]),
    ("POLICY_INVALID", 2, false, &[]),
    ("POLICY_REQUIRED", 2, false, &[]),
    ("RETENTION_EXPORT_DENIED", 2, false, &[]),
    ("RETENTION_TIERS_INVALID", 2, false, &[]),
    ("RETENTION_TIERS_UNAVAILABLE", 2, true, &[]),
    ("REVIEW_ARTIFACT_DENIED", 2, false, &[]),
    ("REVIEW_ARTIFACT_INVALID", 2, false, &[]),
    ("REVIEW_ARTIFACT_NOT_FOUND", 2, false, &[]),
    ("REVIEW_ARTIFACT_STALE", 2, false, &[]),
    ("SCENARIO_INDEX_REQUIRED", 2, false, &[]),
    ("SKILLS_ARGUMENT_INVALID", 2, false, &[]),
    ("SKILLS_SOURCE_NOT_FOUND", 2, false, &[]),
    ("STRATEGY_ROLLBACK_FAILED", 1, false, &[]),
    ("STRATEGY_SIGNATURE_INVALID", 2, false, &[]),
    ("SUBJECT_CONFLICT", 2, false, &["APPROVED_BRIEF", "FLOW_TAGGED", "TRANSITION"]),
    ("TRACE_CONFLICT", 2, false, &[]),
    ("TRACE_NOT_FOUND", 2, false, &[]),
];

/// atip.codesplit.01: each code the split registers has the exit, retryable and reasons of the table and follows the name rule; none of the 28
/// old names is in the registry.
#[test]
fn atip_codesplit_01_each_code_of_the_split_is_registered_as_the_table_says_and_no_old_name_remains() {
    for (code, exit, retryable, reasons) in SPLIT {
        let info = canon_cli::cli_error::all_codes().iter().find(|info| info.code == *code).unwrap_or_else(|| panic!("`{code}` is not in the registry"));
        assert_eq!((i64::from(info.exit), info.retryable), (*exit, *retryable), "{code}");
        assert_eq!(problem(info), None, "{code}");
        assert_eq!(info.reasons.iter().map(|r| r.reason).collect::<Vec<_>>(), *reasons, "{code}");
    }
    for old in include_str!("fixtures/codes-0.35.txt").lines() {
        let gone = ["ADAPTER_REFUSED", "ARTIFACTS_MOVE_REFUSED", "BRIEF_APPROVE_REFUSED", "DISPATCH_ALREADY_ENDED", "DISPATCH_NOT_RUNNING", "DIVERGENCE_ACCEPT_REFUSED", "EVIDENCE_BIND_REFUSED", "EVIDENCE_INVALID_ARGUMENT", "EVIDENCE_REFUSED", "EVIDENCE_SCENARIO_NOT_INDEXED", "HOOKS_INSTALL_REFUSED", "INVENTORY_REFUSED", "LEARN_ROLLBACK_FAILED", "MIGRATE_REFUSED", "NO_BRIEF", "OLD_BRIEF_FORMAT", "OLD_LAYOUT", "PLUGIN_SYNC_FAILED", "POLICY_APPROVAL_REFUSED", "RETENTION_EXPORT_REFUSED", "RETENTION_PURGE_REFUSED", "REVIEW_ARTIFACT_REFUSED", "REVIEW_SCENARIO_NOT_INDEXED", "SKILLS_INVALID_ARGUMENT", "SUBJECT_ADOPT_REFUSED", "SUBJECT_UNADOPT_REFUSED"];
        if gone.contains(&old) {
            assert!(canon_cli::cli_error::code_info(old).is_none(), "`{old}` is still registered");
        }
    }
}
