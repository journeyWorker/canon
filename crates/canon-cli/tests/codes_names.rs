//! `canon-0-36-codes-rename`, `canon-0-36-codes-table`: the registry's names follow the contract's `<SUBJECT>_<CONDITION>` rule (spec 7.2,
//! `atip::code_name_problem` at 1.3). Every code of the registry follows it: the contract's core codes pass the rule by its own exemption, and
//! canon lists no other exception. The lists of codes still to rename or reclassify that the first changes of 0.36 kept are gone.

use atip::Minor;

mod old_codes;

fn problem(info: &atip::CodeInfo) -> Option<String> {
    atip::code_name_problem(Minor::V1_3, info.code, Some(i64::from(info.exit)), Some(info.retryable))
}

/// atip.codenames.03: every code of the registry follows the rule, with its exit and retryable, and no list exempts one.
#[test]
fn atip_codenames_03_every_code_follows_the_name_rule() {
    let bad: Vec<String> = canon_cli::cli_error::all_codes().iter().filter_map(problem).collect();
    assert!(bad.is_empty(), "a code breaks the name rule: {bad:#?}");
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
    ("CONFIG_INVALID", 2, false, &["ARTIFACT_STORE", "CANON_YAML", "DASHBOARD", "POLICY", "SKILLS", "SPECS", "TIERS"]),
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
    ("STRATEGY_SIGNATURE_REQUIRED", 2, false, &[]),
    ("STRATEGY_SIGNER_DENIED", 2, false, &[]),
    ("STRATEGY_VERIFIER_REQUIRED", 2, false, &[]),
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
    for old in old_codes::codes_0_35().iter().copied() {
        let gone = ["ADAPTER_REFUSED", "ARTIFACTS_MOVE_REFUSED", "BRIEF_APPROVE_REFUSED", "DISPATCH_ALREADY_ENDED", "DISPATCH_NOT_RUNNING", "DIVERGENCE_ACCEPT_REFUSED", "EVIDENCE_BIND_REFUSED", "EVIDENCE_INVALID_ARGUMENT", "EVIDENCE_REFUSED", "EVIDENCE_SCENARIO_NOT_INDEXED", "HOOKS_INSTALL_REFUSED", "INVENTORY_REFUSED", "LEARN_ROLLBACK_FAILED", "MIGRATE_REFUSED", "NO_BRIEF", "OLD_BRIEF_FORMAT", "OLD_LAYOUT", "PLUGIN_SYNC_FAILED", "POLICY_APPROVAL_REFUSED", "RETENTION_EXPORT_REFUSED", "RETENTION_PURGE_REFUSED", "REVIEW_ARTIFACT_REFUSED", "REVIEW_SCENARIO_NOT_INDEXED", "SKILLS_INVALID_ARGUMENT", "SUBJECT_ADOPT_REFUSED", "SUBJECT_UNADOPT_REFUSED"];
        if gone.contains(&old) {
            assert!(canon_cli::cli_error::code_info(old).is_none(), "`{old}` is still registered");
        }
    }
}
