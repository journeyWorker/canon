//! The contract repository's self-description examples as input of the L1 document rules (approaches/checker.md 7.2, last paragraph).
//!
//! `tests/fixtures/schema-docs/` holds every `examples/schema-*.json` and every `examples/invalid/schema-*.json` of the tag v1.2.0 (copies,
//! unmodified). The same functions a run of `atip check --level L1` calls judge each one: the self-description schema (`L1.tree`, `L1.walk`), the
//! tree and group rules, the command rules 1-11 and 13, the grammar, the examples, the patterns, the registry and the reasons and warnings.
//! A valid example passes all of them; an invalid one is rejected by the checks of its row in `l1_examples.table` and by no other (one owner per
//! pass condition). The report examples that break only a data rule (`L1.fields`, `L1.output-data`) are judged the same way, from the files
//! of `tests/fixtures/atip-examples/`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde_json::Value;

use atip_check::checks::documents::{
    examples_findings, fields_findings, grammar_findings, output_findings, pattern_findings, reasons_findings, registry_findings,
    reserved_applicable_findings, schema_doc_problems, warnings_findings, Finding,
};
use atip_check::claim::Claim;
use atip_check::l1::model::{CommandDoc, GroupDoc, Tree};
use atip_check::l1::rules::{command_findings, group_findings, tree_findings};

fn dir(relative: &str) -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures").join(relative)
}

/// The document of a file, `None` when it is not JSON a Rust program reads (a number beyond a double: `1e999`).
fn load(path: &Path) -> Option<Value> {
    serde_json::from_str(&std::fs::read_to_string(path).unwrap_or_else(|error| panic!("{}: {error}", path.display()))).ok()
}

fn claim_of(document: &Value) -> Claim {
    match document.pointer("/meta/contractVersion").and_then(Value::as_str) {
        Some("1.2") => Claim::V1_2,
        Some("1.1") => Claim::V1_1,
        _ => Claim::V1_0,
    }
}

/// The exit a registered code has: the reserved ones and the rows of the contract's `schema-errors*.json` examples.
fn exit_of(code: &str) -> Option<i64> {
    let reserved = atip::reserved_code(code, atip::Minor::V1_2).map(|reserved| i64::from(reserved.exit));
    reserved.or_else(|| {
        ["schema-errors-acme.json", "schema-errors.json", "schema-errors-ledger.json"].iter().find_map(|file| {
            let document = load(&dir("schema-docs").join(file))?;
            document["data"]["errors"]
                .as_array()?
                .iter()
                .find(|entry| entry["code"] == code)
                .and_then(|entry| entry["exit"].as_i64())
        })
    })
}

/// The checks that find something wrong with one self-description document, as `L1.<check>/<condition>` strings.
fn rejected_by(document: &Value) -> Vec<String> {
    let claim = claim_of(document);
    let minor = atip_check::checks::documents::minor_of(claim);
    let mut found: Vec<String> = Vec::new();
    let mut add = |check: &str, findings: Vec<Finding>| {
        for (condition, problem) in findings {
            if problem.is_some() {
                found.push(format!("{check}/{condition}"));
            }
        }
    };
    let data = document.get("data").unwrap_or(&Value::Null);
    let valid = schema_doc_problems(document, claim);
    // The schema's own findings belong to the check that reads the document: `L1.tree` for a tree, `L1.walk` for the others.
    let kind = data.get("kind").and_then(Value::as_str).unwrap_or_default();
    let schema_owner = if kind == "tree" { "L1.tree" } else { "L1.walk" };
    // The registry answers are `L0.registry`'s and `L1.registry`'s, `L1.reasons`' and `L1.warnings`': a run does not hand them to the self-description schema.
    if !matches!(kind, "errors" | "error" | "warnings" | "warning") {
        add(schema_owner, vec![("valid", (!valid.is_empty()).then(|| valid.join("; ")))]);
    }
    match kind {
        "tree" => {
            if let Some(tree) = Tree::parse(data) {
                add("L1.tree", tree_findings(document, &tree, tree.level.as_str()));
                let paths: Vec<(&str, &[String])> = tree.commands.iter().map(|entry| (entry.command.as_str(), entry.path.as_slice())).collect();
                add("L1.grammar", grammar_findings(&paths, None, minor));
            }
        }
        "group" => {
            if let Some(group) = GroupDoc::parse(data) {
                let tree = Tree { level: "L1".to_string(), commands: group.commands.clone() };
                let _ = &tree;
                let paths: Vec<(&str, &[String])> = vec![(group.command.as_str(), group.path.as_slice())];
                add("L1.grammar", grammar_findings(&paths, None, minor));
                let own = Tree { level: "L1".to_string(), commands: group.commands.clone() };
                add("L1.walk", group_findings(&group, &own));
            }
        }
        "command" => {
            if let Some(doc) = CommandDoc::parse(data) {
                let tool = document.pointer("/meta/tool").and_then(Value::as_str).unwrap_or_default();
                add("L1.walk", command_findings(&doc, tool, claim, &exit_of).into_iter().filter(|(condition, _)| *condition != "rule-3-domain-pattern").collect());
                add("L1.grammar", grammar_findings(&[], Some(&doc), minor));
                add("L1.examples", examples_findings(&doc, tool));
                add("L1.patterns", pattern_findings(&doc));
                add("L1.reserved-applicable", reserved_applicable_findings(&doc));
            }
        }
        "errors" => {
            let entries = data.get("errors").and_then(Value::as_array).cloned().unwrap_or_default();
            add("L1.registry", registry_findings(&entries, minor));
            add("L1.reasons", reasons_findings(&entries));
        }
        "error" => {
            let entries: Vec<Value> = data.get("error").cloned().into_iter().collect();
            add("L1.reasons", reasons_findings(&entries));
        }
        "warnings" => {
            let entries = data.get("warnings").and_then(Value::as_array).cloned().unwrap_or_default();
            add("L1.warnings", warnings_findings(&entries));
        }
        _ => {}
    }
    found.sort();
    found.dedup();
    found
}

fn names_in(directory: &Path) -> Vec<PathBuf> {
    let mut files: Vec<PathBuf> = std::fs::read_dir(directory)
        .expect("fixture directory")
        .filter_map(Result::ok)
        .map(|entry| entry.path())
        .filter(|path| {
            path.extension().is_some_and(|extension| extension == "json")
                && path.file_name().and_then(|name| name.to_str()).is_some_and(|name| name.starts_with("schema-"))
        })
        .collect();
    files.sort();
    files
}

/// What each self-description example is rejected by: `L1.<check>/<condition>`. An empty list is an example that no L1 check rejects: the valid ones, and
/// the invalid ones whose rule belongs to an L0 check (`L0.contract-version`, `L0.no-null`, `L0.envelope`), to `L1.reserved-applicable`'s runs or to L2.
/// A document that mutates one name leaves the other documents that read it inconsistent, so the checks that read those are listed too
/// (`schema-option-id-suffix`, `schema-domain-pattern-unrecorded`, `schema-unknown-value-type`: every example line then names the old option or value).
const SCHEMA_DOCS: &[(&str, &[&str])] = &[
    ("invalid/schema-artifact-without-schemaref.json", &["L1.walk/valid"]),
    ("invalid/schema-batch-without-batch-output.json", &["L1.walk/rule-5"]),
    ("invalid/schema-command-missing-reserved-code.json", &["L1.reserved-applicable/cursor-code"]),
    ("invalid/schema-constraint-code-not-in-errors.json", &["L1.walk/rule-4-codes"]),
    ("invalid/schema-constraint-duplicate.json", &["L1.walk/rule-4-relation-once"]),
    ("invalid/schema-constraint-in-numeric-string.json", &["L1.examples/constraints"]),
    ("invalid/schema-constraint-in-on-switch.json", &["L1.walk/rule-4-names"]),
    ("invalid/schema-constraint-in-wrong-type.json", &["L1.walk/rule-4-names"]),
    ("invalid/schema-constraint-unknown-name.json", &["L1.walk/rule-4-names"]),
    ("invalid/schema-dangling-schemaref.json", &["L1.walk/rule-2-refs"]),
    ("invalid/schema-domain-bound-unsafe.json", &["L1.walk/rule-3-domain-profile"]),
    ("invalid/schema-domain-count-default-outside.json", &["L1.walk/rule-3-defaults"]),
    ("invalid/schema-domain-default-outside.json", &["L1.walk/rule-3-defaults"]),
    ("invalid/schema-domain-number-bound-not-finite.json", &["unreadable"]),
    ("invalid/schema-domain-numeric-keyword-on-string.json", &["L1.walk/rule-3-domain-profile"]),
    ("invalid/schema-domain-outside-profile.json", &["L1.walk/rule-3-domain-profile"]),
    ("invalid/schema-domain-pattern-alternation.json", &["L1.patterns/form"]),
    ("invalid/schema-domain-pattern-class-intersection.json", &["L1.patterns/form"]),
    ("invalid/schema-domain-pattern-class-symmetric-difference.json", &["L1.patterns/form"]),
    ("invalid/schema-domain-pattern-dot.json", &["L1.patterns/form"]),
    ("invalid/schema-domain-pattern-lookahead.json", &["L1.patterns/form", "L1.patterns/rust-compiles"]),
    ("invalid/schema-domain-pattern-rust-size-limit.json", &["L1.patterns/rust-compiles"]),
    ("invalid/schema-domain-pattern-shorthand.json", &["L1.patterns/form"]),
    ("invalid/schema-domain-pattern-unanchored.json", &["L1.patterns/form"]),
    ("invalid/schema-domain-pattern-unrecorded.json", &["L1.examples/input-inline", "L1.examples/values", "L1.walk/rule-13", "L1.walk/rule-7"]),
    ("invalid/schema-domain-ref.json", &["L1.walk/rule-3-domain-profile"]),
    ("invalid/schema-domain-structural-enum.json", &["L1.walk/rule-3-domain-profile"]),
    ("invalid/schema-enum-default-outside.json", &["L1.walk/rule-3-defaults"]),
    ("invalid/schema-error-acme-reason-lowercase.json", &["L1.reasons/reason-form"]),
    ("invalid/schema-errors-acme-duplicate-code.json", &["L1.registry/codes-unique"]),
    ("invalid/schema-errors-acme-duplicate-reason.json", &["L1.reasons/reasons-unique"]),
    ("invalid/schema-errors-acme-reasons-in-1-0.json", &[]),
    ("invalid/schema-errors-acme-stopped-midway-exit-2.json", &["L1.registry/reserved-exit"]),
    ("invalid/schema-example-count-outside-domain.json", &["L1.examples/values"]),
    ("invalid/schema-example-fails-arguments-ref.json", &["L1.walk/rule-7"]),
    ("invalid/schema-example-input-incomplete.json", &["L1.examples/requirements"]),
    ("invalid/schema-example-input-json-shape-conflict.json", &["L1.examples/input-inline"]),
    ("invalid/schema-example-input-scalar-repeatable.json", &["L1.examples/input-inline"]),
    ("invalid/schema-example-integer-max-plus-one.json", &["L1.examples/values"]),
    ("invalid/schema-example-integer-min-minus-one.json", &["L1.examples/values"]),
    ("invalid/schema-example-integer-unsafe.json", &["L1.examples/values"]),
    ("invalid/schema-example-member-tag-zero.json", &["L1.examples/arguments-ref", "L1.examples/values"]),
    ("invalid/schema-example-number-form.json", &["L1.examples/values"]),
    ("invalid/schema-example-operand-type.json", &["L1.examples/values"]),
    ("invalid/schema-example-outside-domain-json.json", &["L1.examples/arguments-ref", "L1.examples/values"]),
    ("invalid/schema-example-outside-domain.json", &["L1.examples/values"]),
    ("invalid/schema-example-placeholder.json", &["L1.examples/no-placeholder"]),
    ("invalid/schema-example-reads-stdin.json", &["L1.examples/input-inline"]),
    ("invalid/schema-example-short-undeclared.json", &["L1.examples/grammar"]),
    ("invalid/schema-example-unclosed-quote.json", &["L1.examples/grammar"]),
    ("invalid/schema-example-variadic-without-default.json", &["L1.examples/arguments-ref", "L1.walk/rule-7"]),
    ("invalid/schema-example-violates-constraint.json", &["L1.examples/constraints"]),
    ("invalid/schema-fields-bracket-identifier.json", &["L1.walk/rule-11"]),
    ("invalid/schema-fields-bracket-in-1-0.json", &[]),
    ("invalid/schema-fields-cut-node-boolean.json", &["L1.walk/rule-11"]),
    ("invalid/schema-fields-cut-node-const.json", &["L1.walk/rule-11"]),
    ("invalid/schema-fields-cut-node-enum.json", &["L1.walk/rule-11"]),
    ("invalid/schema-fields-cut-node-root-allof.json", &["L1.walk/rule-11"]),
    ("invalid/schema-fields-cut-node-root-const.json", &["L1.walk/rule-11"]),
    ("invalid/schema-fields-cut-node-unique-items.json", &["L1.walk/rule-11"]),
    ("invalid/schema-fields-cut-node-untyped.json", &["L1.walk/rule-11"]),
    ("invalid/schema-fields-default-outside.json", &["L1.walk/rule-11"]),
    ("invalid/schema-fields-keep-overlap.json", &["L1.walk/rule-11"]),
    ("invalid/schema-fields-map-cut-node-min-properties.json", &["L1.walk/rule-11"]),
    ("invalid/schema-fields-not-projectable.json", &["L1.walk/rule-11"]),
    ("invalid/schema-fields-path-array-without-brackets.json", &["L1.walk/rule-11"]),
    ("invalid/schema-fields-path-malformed.json", &["L1.walk/rule-11", "L1.walk/valid"]),
    ("invalid/schema-fields-path-undeclared.json", &["L1.walk/rule-11"]),
    ("invalid/schema-fields-records-not-pointer.json", &["L1.reserved-applicable/cursor-code", "L1.walk/rule-11", "L1.walk/valid"]),
    ("invalid/schema-flag-only-not-marked.json", &["L1.grammar/reserved-shape"]),
    ("invalid/schema-command-acme-member-list-limit-not-flag-only.json", &["L1.grammar/reserved-shape"]),
    ("invalid/schema-fragment-flag-only-property.json", &["L1.walk/rule-10"]),
    ("invalid/schema-fragment-open.json", &["L1.walk/rule-10"]),
    ("invalid/schema-fragment-required.json", &["L1.walk/rule-10"]),
    ("invalid/schema-group-id-mismatch.json", &["L1.walk/group-path"]),
    ("invalid/schema-input-example-violates-constraint.json", &["L1.walk/rule-7"]),
    ("invalid/schema-ledger-meta-bracket-field.json", &[]),
    ("invalid/schema-ledger-meta-warning-unregistered.json", &[]),
    ("invalid/schema-missing-contract.json", &["L1.walk/valid"]),
    ("invalid/schema-missing-schemas.json", &["L1.walk/rule-2-refs", "L1.walk/valid"]),
    ("invalid/schema-operand-untyped-default.json", &["L1.walk/rule-3-defaults", "L1.walk/valid"]),
    ("invalid/schema-option-id-suffix.json", &["L1.examples/grammar", "L1.examples/input-inline", "L1.examples/requirements", "L1.grammar/no-id-suffix", "L1.walk/rule-10", "L1.walk/rule-7"]),
    ("invalid/schema-outputs-indistinct.json", &["L1.walk/rule-6"]),
    ("invalid/schema-outputs-share-ref.json", &["L1.walk/rule-2-outputs"]),
    ("invalid/schema-path-mismatch.json", &["L1.walk/rule-1"]),
    ("invalid/schema-repeatable-json-scalar-default.json", &["L1.walk/valid"]),
    ("invalid/schema-schemas-not-compiling.json", &["L1.reserved-applicable/cursor-code", "L1.walk/rule-2-compile"]),
    ("invalid/schema-short-duplicate.json", &["L1.walk/rule-9"]),
    ("invalid/schema-tree-acme-markers-missing.json", &[]),
    ("invalid/schema-tree-empty.json", &["L1.tree/valid"]),
    ("invalid/schema-unknown-value-type.json", &["L1.examples/input-inline", "L1.examples/values", "L1.walk/rule-7", "L1.walk/valid"]),
    ("invalid/schema-untyped-default.json", &["L1.walk/rule-3-defaults", "L1.walk/valid"]),
    ("invalid/schema-validator-code-exit-1.json", &["L1.walk/rule-13"]),
    ("invalid/schema-validator-code-not-listed.json", &["L1.walk/rule-13"]),
    ("invalid/schema-validator-rejects-fails-domain.json", &["L1.walk/rule-13"]),
    ("invalid/schema-validator-rejects-is-example.json", &["L1.walk/rule-13"]),
    ("invalid/schema-validator-unknown-argument.json", &["L1.walk/rule-13"]),
    ("invalid/schema-version-not-contract-version.json", &[]),
    ("invalid/schema-warnings-acme-duplicate.json", &["L1.warnings/codes-unique"]),
    ("invalid/schema-warnings-acme-in-1-0.json", &[]),
    ("invalid/schema-warnings-acme-with-exit.json", &[]),
    ("invalid/schema-write-without-idempotency.json", &["L1.walk/valid"]),
    ("schema-command-acme-build.json", &[]),
    ("schema-command-acme-deploy-start.json", &[]),
    ("schema-command-acme-deploy-status.json", &[]),
    ("schema-command-acme-filter-test.json", &[]),
    ("schema-command-acme-lint.json", &[]),
    ("schema-command-acme-member-export.json", &[]),
    ("schema-command-acme-member-list.json", &[]),
    ("schema-command-tally-entry-list.json", &[]),
    ("schema-command-acme-member-roster.json", &[]),
    ("schema-command-acme-member-show.json", &[]),
    ("schema-command-acme-member-tag.json", &[]),
    ("schema-command-acme-page-label.json", &[]),
    ("schema-command-canon-finding-close.json", &[]),
    ("schema-command-canon-finding-list.json", &[]),
    ("schema-command-canon-purge.json", &[]),
    ("schema-command-canon-skills-show.json", &[]),
    ("schema-command-ledger-entry-list.json", &[]),
    ("schema-error-acme-input-invalid.json", &[]),
    ("schema-error.json", &[]),
    ("schema-errors-acme.json", &[]),
    ("schema-errors-tally.json", &[]),
    ("schema-errors-ledger.json", &[]),
    ("schema-errors.json", &[]),
    ("schema-group.json", &[]),
    ("schema-tree-acme.json", &[]),
    ("schema-tree-search.json", &[]),
    ("schema-tree.json", &[]),
    ("schema-warning-acme-member-archived.json", &[]),
    ("schema-warnings-acme.json", &[]),
];

/// The same for the data rules of the report examples (`L1.output-data`, `L1.fields`); a report not listed is rejected by none of them.
const REPORTS: &[(&str, &[&str])] = &[
    ("invalid/report-acme-member-list-fields-empty-key-leak.json", &["L1.fields/no-unselected"]),
    ("invalid/report-acme-member-list-fields-leak.json", &["L1.fields/no-unselected"]),
    ("invalid/report-acme-member-list-fields-map-key-in-1-0.json", &["L1.fields/selection-declared"]),
    ("invalid/report-acme-member-list-fields-map-key-leak.json", &["L1.fields/no-unselected"]),
    ("invalid/report-acme-member-list-fields-map-key-nul.json", &["L1.fields/no-unselected", "L1.fields/selection-declared"]),
    ("invalid/report-acme-member-list-fields-map-key-without-descent.json", &["L1.fields/selection-declared"]),
    ("invalid/report-acme-member-list-fields-missing-child.json", &["L1.fields/projected-schema"]),
    ("invalid/report-acme-member-list-fields-missing-nested.json", &["L1.fields/projected-schema"]),
    ("invalid/report-acme-member-list-fields-not-a-map-key.json", &["L1.fields/projected-schema", "L1.fields/selection-declared"]),
    ("invalid/report-acme-member-list-fields-parent-child-leak.json", &["L1.fields/no-unselected"]),
    ("invalid/report-batch-wrong-version.json", &["L1.output-data/schema-version"]),
    ("invalid/report-fields-missing-selected.json", &["L1.fields/projected-schema"]),
    ("invalid/report-fields-unselected.json", &["L1.fields/no-unselected"]),
    ("invalid/report-ledger-entry-list-map-key.json", &["L1.fields/selection-declared"]),
    ("invalid/report-next-placeholder.json", &["L1.output-data/data-validates"]),
    ("invalid/report-next-without-json.json", &["L1.output-data/data-validates"]),
    ("invalid/report-output-missing.json", &["L1.output-data/data-validates"]),
    ("invalid/report-skills-show-markdown-key.json", &["L1.output-data/data-validates"]),
];

#[test]
fn atip_checkl1_26_contract_examples_every_self_description_example_is_judged_by_its_owner_checks() {
    let mut judged = 0;
    for (directory, prefix) in [(dir("schema-docs"), ""), (dir("schema-docs/invalid"), "invalid/")] {
        for path in names_in(&directory) {
            let name = format!("{prefix}{}", path.file_name().and_then(|name| name.to_str()).unwrap_or_default());
            let expected: Vec<String> = SCHEMA_DOCS
                .iter()
                .find(|(file, _)| *file == name)
                .unwrap_or_else(|| panic!("{name} has no row in SCHEMA_DOCS"))
                .1
                .iter()
                .map(ToString::to_string)
                .collect();
            let found = load(&path).map_or_else(|| vec!["unreadable".to_string()], |document| rejected_by(&document));
            let mut expected = expected;
            expected.sort();
            assert_eq!(found, expected, "{name}");
            judged += 1;
        }
    }
    assert_eq!(judged, SCHEMA_DOCS.len(), "every row has a file");
    assert!(judged >= 120, "{judged}");
    // The valid examples pass; most invalid ones have one owner.
    let valid_rejected = SCHEMA_DOCS.iter().filter(|(name, found)| !name.starts_with("invalid/") && !found.is_empty()).count();
    assert_eq!(valid_rejected, 0, "a valid example is rejected by no check");
}

#[test]
fn atip_checkl1_26_contract_examples_every_invalid_self_description_example_with_one_rule_is_rejected_by_one_check() {
    // One owner per pass condition: an invalid example that breaks one rule is rejected by the checks of that rule only. The row of each file says which.
    let multi: Vec<&str> = SCHEMA_DOCS.iter().filter(|(_, found)| found.iter().map(|entry| entry.split('/').next().unwrap_or_default()).collect::<std::collections::BTreeSet<_>>().len() > 1).map(|(name, _)| *name).collect();
    for name in &multi {
        assert!(
            ["invalid/schema-domain-pattern-unrecorded.json", "invalid/schema-example-variadic-without-default.json", "invalid/schema-fields-records-not-pointer.json", "invalid/schema-option-id-suffix.json", "invalid/schema-schemas-not-compiling.json", "invalid/schema-unknown-value-type.json"]
            .contains(name),
            "{name} is rejected by more than one check: {multi:?}"
        );
    }
}

#[test]
fn atip_checkl1_27_contract_examples_every_report_example_is_judged_by_the_data_checks() {
    let directory = dir("atip-examples");
    let mut judged = 0;
    for sub in ["", "invalid"] {
        let mut files: Vec<PathBuf> = std::fs::read_dir(directory.join(sub))
            .expect("dir")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "json") && path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("report-")))
            .collect();
        files.sort();
        for path in files {
            let Some(report) = load(&path) else { continue };
            let name = format!("{sub}/{}", path.file_name().and_then(|n| n.to_str()).unwrap_or_default());
            let expected: Vec<&str> = REPORTS.iter().find(|(file, _)| *file == name).map(|(_, found)| found.to_vec()).unwrap_or_default();
            assert_eq!(report_rejected_by(&report), expected, "{name}");
            judged += 1;
        }
    }
    assert!(judged >= 60, "{judged}");
    for (name, _) in REPORTS {
        assert!(directory.join(name.trim_start_matches('/')).exists(), "{name} exists");
    }
}

/// The command documents of the valid examples by command id, with the tool they belong to.
fn command_docs() -> BTreeMap<String, (String, CommandDoc)> {
    let mut docs = BTreeMap::new();
    for path in names_in(&dir("schema-docs")) {
        let Some(document) = load(&path) else { continue };
        if let Some(doc) = CommandDoc::parse(document.get("data").unwrap_or(&Value::Null)) {
            let tool = document.pointer("/meta/tool").and_then(Value::as_str).unwrap_or_default().to_string();
            docs.insert(format!("{tool} {}", doc.command), (tool, doc));
        }
    }
    docs
}

/// The L1 data checks that find something wrong with one report: `L1.output-data` and `L1.fields`.
fn report_rejected_by(report: &Value) -> Vec<String> {
    let docs = command_docs();
    let (Some(tool), Some(command)) = (report.pointer("/meta/tool").and_then(Value::as_str), report.pointer("/meta/command").and_then(Value::as_str)) else {
        return Vec::new();
    };
    let Some((_, doc)) = docs.get(&format!("{tool} {command}")) else { return Vec::new() };
    let mut found: Vec<String> = Vec::new();
    for (condition, problem) in output_findings(doc, report) {
        if problem.is_some() {
            found.push(format!("L1.output-data/{condition}"));
        }
    }
    if let Some(selection) = report.pointer("/meta/fields").and_then(Value::as_array) {
        let kept: Vec<String> = selection.iter().filter_map(Value::as_str).map(str::to_string).collect();
        for (condition, problem) in fields_findings(doc, report, &kept, None) {
            if problem.is_some() {
                found.push(format!("L1.fields/{condition}"));
            }
        }
    }
    found.sort();
    found.dedup();
    found
}


/// Prints the rows of `SCHEMA_DOCS` as the checks find them now; run by hand when the checks change, and read the diff.
#[test]
#[ignore = "drafts the table by hand"]
fn draft_the_table() {
    for (directory, prefix) in [(dir("schema-docs"), ""), (dir("schema-docs/invalid"), "invalid/")] {
        for path in names_in(&directory) {
            let name = format!("{prefix}{}", path.file_name().and_then(|name| name.to_str()).unwrap_or_default());
            let found = load(&path).map_or_else(|| vec!["unreadable".to_string()], |document| rejected_by(&document));
            println!("ROW {name} => {}", found.join(" "));
        }
    }
}

#[test]
fn atip_checkl1_26_the_embedded_guide_schema_is_the_original_of_the_contract_tag() {
    use sha2::{Digest, Sha256};
    let digest: String = Sha256::digest(atip_check::schema::GUIDE_SCHEMA.as_bytes()).iter().map(|byte| format!("{byte:02x}")).collect();
    assert_eq!(digest, "43229f3cfbd518307b1e6071b9e12adad114c0da4f8fdba695c06d2e16c47d3f", "schema/guide.schema.json of v1.2.0 (65c89a9b45fec06163223ab1d5969f807c20fc26), byte-identical to v1.1.1");
}

/// Prints the rows of `REPORTS` as the checks find them now; run by hand when the checks change, and read the diff.
#[test]
#[ignore = "drafts the table by hand"]
fn draft_the_report_table() {
    let directory = dir("atip-examples");
    for sub in ["", "invalid"] {
        let mut files: Vec<PathBuf> = std::fs::read_dir(directory.join(sub))
            .expect("dir")
            .filter_map(Result::ok)
            .map(|entry| entry.path())
            .filter(|path| path.extension().is_some_and(|extension| extension == "json") && path.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.starts_with("report-")))
            .collect();
        files.sort();
        for path in files {
            if let Some(report) = load(&path) {
                println!("REPORT {sub}/{} => {}", path.file_name().and_then(|n| n.to_str()).unwrap_or_default(), report_rejected_by(&report).join(" "));
            }
        }
    }
}
