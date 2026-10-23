//! T13: `canon context` is a projection of the model and the generated spec,
//! never a list of its own — checked against the built binary.

use std::collections::BTreeSet;
use std::process::{Command, Output};

use canon_model::family::FamilyKind;
use canon_model::RecordKind;
use serde_json::Value;

fn context(args: &[&str], envs: &[(&str, &str)]) -> Output {
    let dir = tempfile::tempdir().unwrap();
    let mut cmd = Command::new(env!("CARGO_BIN_EXE_canon"));
    cmd.arg("context").args(args).arg("--json").current_dir(dir.path());
    for (k, v) in envs {
        cmd.env(k, v);
    }
    cmd.output().expect("spawn canon binary")
}

fn data(out: &Output) -> Value {
    let doc: Value = serde_json::from_slice(&out.stdout).expect("one JSON document");
    doc["data"].clone()
}

fn names(value: &Value) -> BTreeSet<String> {
    value.as_object().expect("an object").keys().cloned().collect()
}

#[test]
fn kinds_are_every_record_kind_under_its_user_facing_name() {
    let out = context(&[], &[]);
    assert!(out.status.success());
    let doc = data(&out);
    let expected: BTreeSet<String> = RecordKind::ALL.iter().map(|k| canon_cli::query::kind_cli_name(*k).to_string()).collect();
    assert_eq!(names(&doc["kinds"]), expected);
    assert_eq!(names(&doc["cel"]), expected);
    assert!(expected.contains("evidence") && !expected.contains("evidence_record"));
    assert_eq!(doc["capabilityVersion"], 6);
}

#[test]
fn families_are_every_family_kind_in_order() {
    let doc = data(&context(&[], &[]));
    let got: Vec<&str> = doc["families"].as_array().unwrap().iter().map(|f| f["family"].as_str().unwrap()).collect();
    let expected: Vec<&str> = FamilyKind::ALL.iter().map(|f| f.as_str()).collect();
    assert_eq!(got, expected);
}

#[test]
fn every_kind_option_value_is_a_context_kind() {
    let kinds = names(&data(&context(&[], &[]))["kinds"]);
    let mut root = canon_cli::cli::command();
    root.build();
    let mut checked = BTreeSet::new();
    for node in canon_cli::spec::nodes(&root) {
        let Some(leaf) = node.leaf else { continue };
        for option in leaf.options.iter().filter(|o| o.name == "kind" && !o.possible_values.is_empty()) {
            checked.insert(node.command.clone());
            for value in &option.possible_values {
                assert!(kinds.contains(value), "`{} --kind {value}` is not in context.kinds", node.command);
            }
        }
    }
    for command in ["query", "export", "purge"] {
        assert!(checked.contains(command), "{command} has a closed --kind set");
    }
}

#[test]
fn commands_are_the_spec_leaves_each_role_uses() {
    let mut root = canon_cli::cli::command();
    root.build();
    let leaves: Vec<_> = canon_cli::spec::nodes(&root).into_iter().filter(|n| n.leaf.is_some()).collect();
    let command_set = |doc: &Value| -> BTreeSet<String> { doc["commands"].as_array().unwrap().iter().map(|c| c["command"].as_str().unwrap().to_string()).collect() };

    let all = data(&context(&[], &[]));
    assert_eq!(command_set(&all), leaves.iter().map(|n| n.command.clone()).collect::<BTreeSet<_>>());
    for c in all["commands"].as_array().unwrap() {
        assert!(c["effect"].is_string() && c["roles"].is_array(), "{c}");
    }

    for role in canon_cli::role::ALL {
        let doc = data(&context(&["--as", role.as_str()], &[]));
        let expected: BTreeSet<String> = leaves.iter().filter(|n| canon_cli::schema::meta_for(&n.command).is_some_and(|m| m.roles.contains(role))).map(|n| n.command.clone()).collect();
        assert!(!expected.is_empty(), "{role} has commands");
        assert_eq!(command_set(&doc), expected, "--as {role}");
    }
}

#[test]
fn a_bad_canon_role_is_a_warning_and_never_filters_or_fails_context() {
    let out = context(&[], &[("CANON_ROLE", "bogus")]);
    assert!(out.status.success(), "{}", String::from_utf8_lossy(&out.stderr));
    let doc: Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(!doc["meta"]["warnings"].as_array().expect("warnings").is_empty());
    let plain = data(&context(&[], &[]));
    assert_eq!(doc["data"]["commands"], plain["commands"]);

    let reviewer = data(&context(&[], &[("CANON_ROLE", "reviewer")]));
    assert_eq!(reviewer["commands"], plain["commands"], "CANON_ROLE does not filter context");
}

#[test]
fn an_unknown_field_is_refused_in_text_mode_too() {
    let dir = tempfile::tempdir().unwrap();
    let out = Command::new(env!("CARGO_BIN_EXE_canon")).args(["context", "--fields", "nope"]).current_dir(dir.path()).output().unwrap();
    assert_eq!(out.status.code(), Some(2));
}
