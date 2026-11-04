//! The validators run before any state is read (atip/1 spec 10.8.3, 11.7 step 6): the first `rejects` call of each validator
//! that `canon` moved ahead of the preconditions is refused with the validator's own code in an empty directory (no repo,
//! no `canon.yaml`, an empty HOME), and nothing is written. The sweep over every validator is `tests/arg_rules.rs` (07).

mod harness;
mod rule_support;

use harness::leaf;

/// Runs the first `rejects` call of the validator of `command` with exactly these `arguments` in an empty directory.
fn refused_in_an_empty_directory(command: &str, arguments: &[&str], code: &str) {
    let doc = leaf(command);
    let validator = doc["validators"]
        .as_array()
        .into_iter()
        .flatten()
        .find(|validator| validator["code"] == code && validator["arguments"].as_array().is_some_and(|names| names.iter().filter_map(|name| name.as_str()).eq(arguments.iter().copied())))
        .unwrap_or_else(|| panic!("`{command}` declares no {code} validator of {arguments:?}: {:?}", doc["validators"]));
    let call = validator["rejects"][0].as_str().expect("a validator lists a rejected call");
    let mut words = atip::split_argv(call);
    assert_eq!(words.remove(0), "canon");
    words.retain(|word| word != "--json");
    let outcome = rule_support::run_empty(&words, None);
    assert_eq!(outcome.exit, 2, "`{call}`: {:?} {:?}", outcome.code, outcome.message);
    assert_eq!(outcome.code.as_deref(), Some(code), "`{call}`: {:?}", outcome.message);
    let field = outcome.field.as_deref().unwrap_or_default().trim_start_matches("--");
    assert!(arguments.contains(&field), "`{call}`: field {:?} is none of {arguments:?}", outcome.field);
    assert!(outcome.changed.is_empty(), "`{call}` wrote {:?}", outcome.changed);
}

#[test]
fn atip_constraints_07_brief_approve_a_relayed_approval_without_its_digest_is_refused_before_any_state_is_read() {
    refused_in_an_empty_directory("brief approve", &["relayed-via", "digest", "change"], "USAGE");
}

#[test]
fn atip_constraints_07_brief_approve_one_change_with_two_digests_is_refused_before_any_state_is_read() {
    refused_in_an_empty_directory("brief approve", &["digest"], "USAGE");
}

#[test]
fn atip_constraints_07_policy_approve_a_blank_note_under_relay_is_refused_before_any_state_is_read() {
    refused_in_an_empty_directory("policy approve", &["relayed-via", "approver", "note"], "USAGE");
}

#[test]
fn atip_constraints_07_evidence_add_class_audit_is_refused_before_any_state_is_read() {
    refused_in_an_empty_directory("evidence add", &["class"], "EVIDENCE_INVALID_ARGUMENT");
}

#[test]
fn atip_constraints_07_evidence_add_an_artifact_scope_is_refused_before_any_state_is_read() {
    refused_in_an_empty_directory("evidence add", &["artifact", "scenario", "all-scenarios"], "EVIDENCE_INVALID_ARGUMENT");
}

#[test]
fn atip_constraints_07_evidence_audit_an_artifact_scope_is_refused_before_any_state_is_read() {
    refused_in_an_empty_directory("evidence audit", &["artifact", "scenario"], "EVIDENCE_INVALID_ARGUMENT");
}

#[test]
fn atip_constraints_07_export_an_empty_window_is_refused_before_any_state_is_read() {
    refused_in_an_empty_directory("export", &["after", "before"], "USAGE");
}

#[test]
fn atip_constraints_07_finding_add_a_defect_without_its_origin_is_refused_before_any_state_is_read() {
    refused_in_an_empty_directory("finding add", &["severity", "introduced-rev", "introduced-unknown"], "FINDING_INTRODUCED_REQUIRED");
}

#[test]
fn atip_constraints_07_query_a_status_foreign_to_the_kind_is_refused_before_any_state_is_read() {
    refused_in_an_empty_directory("query", &["kind", "status"], "QUERY_ARGUMENT_INVALID");
}
