//! The contract's own JSON Schemas, vendored verbatim.
//!
//! `crates/atip/schema/` holds the envelope, the error object and the registry
//! entry exactly as the atip repository publishes them (spec sections 6, 7.1
//! and 7.3). A test pins each copy by digest, so an edit is a deliberate
//! re-vendoring and never an accident. [`envelope_problems`] checks a document
//! against them; a tool's debug build and its tests call it on every envelope
//! they print, so a document that breaks the contract fails where it is made.

use std::sync::LazyLock;

use jsonschema::{Registry, Resource, Validator};
use serde_json::Value;

/// `schema/envelope.schema.json`: the one document `--json` prints.
pub const ENVELOPE_SCHEMA: &str = include_str!("../schema/envelope.schema.json");
/// `schema/error.schema.json`: the `error` member of a failure envelope.
pub const ERROR_SCHEMA: &str = include_str!("../schema/error.schema.json");
/// `schema/registry-entry.schema.json`: one row of a tool's error registry.
pub const REGISTRY_ENTRY_SCHEMA: &str = include_str!("../schema/registry-entry.schema.json");
/// `schema/warning-entry.schema.json`: one row of a tool's warning registry (since 1.1).
pub const WARNING_ENTRY_SCHEMA: &str = include_str!("../schema/warning-entry.schema.json");
/// `schema/stream-line.schema.json`: one line of a stream (since 1.0; `canon` has no stream before 0.35).
pub const STREAM_LINE_SCHEMA: &str = include_str!("../schema/stream-line.schema.json");

/// `meta.contract`: the contract every document follows.
pub const CONTRACT: &str = "atip/1";
/// `meta.contractVersion`: the minor of the contract the vendored schemas are (spec 15.7). The value is never `"1.0"` written out.
pub const CONTRACT_VERSION: &str = "1.1";

/// `$id` of the error schema, which the envelope and the registry entry `$ref`.
const ERROR_ID: &str = "https://atip.dev/1/error.schema.json";

fn parse(text: &str) -> Value {
    serde_json::from_str(text).expect("a vendored contract schema is JSON")
}

fn compile(text: &str) -> Validator {
    let registry = Registry::new().add(ERROR_ID, Resource::from_contents(parse(ERROR_SCHEMA))).expect("the error schema registers").prepare().expect("the registry prepares");
    jsonschema::options().with_registry(&registry).build(&parse(text)).expect("a vendored contract schema compiles")
}

static ENVELOPE: LazyLock<Validator> = LazyLock::new(|| compile(ENVELOPE_SCHEMA));
static ERROR: LazyLock<Validator> = LazyLock::new(|| compile(ERROR_SCHEMA));
static REGISTRY_ENTRY: LazyLock<Validator> = LazyLock::new(|| compile(REGISTRY_ENTRY_SCHEMA));
static WARNING_ENTRY: LazyLock<Validator> = LazyLock::new(|| compile(WARNING_ENTRY_SCHEMA));

fn problems_of(validator: &Validator, document: &Value) -> Vec<String> {
    validator.iter_errors(document).map(|error| format!("{error} at {}", error.instance_path())).collect()
}

/// What breaks the envelope schema in `document`: one line per violation,
/// empty when the document is a conforming envelope. The schema alone cannot say that
/// `meta.contractVersion` is never `"1.0"` written out (spec 10.13 rule 16: an absent value means 1.0), so that
/// rule is checked here beside it.
pub fn envelope_problems(document: &Value) -> Vec<String> {
    let mut problems = problems_of(&ENVELOPE, document);
    if document.pointer("/meta/contractVersion").and_then(Value::as_str) == Some("1.0") {
        problems.push("\"1.0\" is never written out as meta.contractVersion: absent means 1.0 (spec 10.13 rule 16) at /meta/contractVersion".to_string());
    }
    problems
}

/// What breaks the error-object schema in `error` (the `error` member of a failure envelope), empty when it conforms.
pub fn error_problems(error: &Value) -> Vec<String> {
    problems_of(&ERROR, error)
}

/// What breaks the registry-entry schema in `row`, empty when it conforms.
pub fn registry_entry_problems(row: &Value) -> Vec<String> {
    problems_of(&REGISTRY_ENTRY, row)
}

/// What breaks the warning-entry schema in `row` (one row of the warning registry, since 1.1), empty when it conforms.
pub fn warning_entry_problems(row: &Value) -> Vec<String> {
    problems_of(&WARNING_ENTRY, row)
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use sha2::{Digest, Sha256};

    use super::*;

    fn digest(text: &str) -> String {
        Sha256::digest(text.as_bytes()).iter().map(|byte| format!("{byte:02x}")).collect()
    }

    /// The digests of the files as the atip repository published them. A
    /// changed copy fails here; re-vendoring updates the file and its digest
    /// together.
    const ENVELOPE_DIGEST: &str = "d8b42e992a7750f857b7b9286925645ee4401ead264e3d27ff62db382a902a37";
    const ERROR_DIGEST: &str = "b89419efd8731950ac458a59e3631b50cf29f0168c141e087a2dfa18a15d4903";
    const REGISTRY_ENTRY_DIGEST: &str = "18e73eec11f662d6c19e18574a1c1d3424f037f0ad5d232f24ab0a56fab28622";
    const WARNING_ENTRY_DIGEST: &str = "66d2eaaea38eaf723343c262dea4b710bcd1b22dc547124f68f0abc2ee0fcfde";
    const STREAM_LINE_DIGEST: &str = "cfa0647e4907a9447a992be8f9226b5873dc9b3cae89b5a9073c5fafef75c145";

    #[test]
    fn atip_l0_07_the_vendored_schemas_are_the_published_bytes() {
        assert_eq!(digest(ENVELOPE_SCHEMA), ENVELOPE_DIGEST);
        assert_eq!(digest(ERROR_SCHEMA), ERROR_DIGEST);
        assert_eq!(digest(REGISTRY_ENTRY_SCHEMA), REGISTRY_ENTRY_DIGEST);
        assert_eq!(digest(WARNING_ENTRY_SCHEMA), WARNING_ENTRY_DIGEST);
        assert_eq!(digest(STREAM_LINE_SCHEMA), STREAM_LINE_DIGEST);
    }

    /// A copy with one character changed no longer matches the pin: the pin test above would fail on it.
    #[test]
    fn atip_l0_07_one_changed_character_fails_the_pin() {
        let changed = ENVELOPE_SCHEMA.replacen("atip/1 envelope", "atip/1 envelopf", 1);
        assert_ne!(changed, ENVELOPE_SCHEMA, "the edit changed the copy");
        assert_ne!(digest(&changed), ENVELOPE_DIGEST);
    }

    #[test]
    fn an_error_object_with_a_tool_extension_passes_and_one_with_an_empty_member_does_not() {
        let with_extension = json!({ "code": "INPUT_INVALID", "message": "m", "retryable": false, "field": "--input", "x-canon": { "note": "n" } });
        assert_eq!(error_problems(&with_extension), Vec::<String>::new());
        assert!(!error_problems(&json!({ "code": "USAGE", "message": "m", "retryable": false, "hint": "" })).is_empty());
        assert!(!error_problems(&json!({ "code": "USAGE", "message": "m", "retryable": false, "unknown": 1 })).is_empty());
    }

    fn meta() -> Value {
        json!({ "schemaVersion": "1.0", "tool": "canon", "toolVersion": "0.34.0", "contract": CONTRACT, "contractVersion": CONTRACT_VERSION, "command": "status" })
    }

    #[test]
    fn atip_contract11_02_a_contract_version_without_contract_or_of_another_major_or_written_as_1_0_is_refused() {
        let mut without = meta();
        without.as_object_mut().expect("meta is an object").remove("contract");
        assert!(!envelope_problems(&json!({ "ok": true, "data": {}, "meta": without })).is_empty(), "`contractVersion` without `contract` is refused");
        let mut lone = meta();
        lone["contractVersion"] = json!("2.0");
        assert!(!envelope_problems(&json!({ "ok": true, "data": {}, "meta": lone })).is_empty(), "another major is refused");
        let mut written = meta();
        written["contractVersion"] = json!("1.0");
        assert!(!envelope_problems(&json!({ "ok": true, "data": {}, "meta": written })).is_empty(), "1.0 is never written out");
        assert_eq!(envelope_problems(&json!({ "ok": true, "data": {}, "meta": meta() })), Vec::<String>::new(), "contract atip/1 with contractVersion 1.1 passes");
    }

    #[test]
    fn atip_contract11_03_a_reason_and_a_warning_row_validate() {
        let failure = json!({ "ok": false, "error": { "code": "INPUT_INVALID", "reason": "NOT_JSON", "message": "m", "retryable": false, "field": "--input", "received": "" }, "meta": meta() });
        assert_eq!(envelope_problems(&failure), Vec::<String>::new());
        let lower = json!({ "code": "INPUT_INVALID", "reason": "not_json", "message": "m", "retryable": false });
        assert!(!error_problems(&lower).is_empty(), "a reason is UPPER_SNAKE_CASE");
        assert!(warning_entry_problems(&json!({ "code": "OLD_LAYOUT_FOUND", "meaning": "m", "fix": "f" })).is_empty());
        assert!(!warning_entry_problems(&json!({ "code": "OLD_LAYOUT_FOUND", "meaning": "m", "fix": "f", "exit": 1 })).is_empty(), "a warning row has no exit");
        let with_reasons = json!({ "code": "INPUT_INVALID", "exit": 2, "retryable": false, "meaning": "m", "fix": "f", "reasons": [{ "reason": "NOT_JSON", "meaning": "m", "fix": "f" }] });
        assert!(registry_entry_problems(&with_reasons).is_empty());
    }

    #[test]
    fn atip_l0_06_a_conforming_success_and_failure_pass() {
        assert_eq!(envelope_problems(&json!({ "ok": true, "data": { "a": 1 }, "meta": meta() })), Vec::<String>::new());
        let failure = json!({ "ok": false, "error": { "code": "USAGE", "message": "m", "retryable": false, "field": "--x" }, "meta": meta() });
        assert_eq!(envelope_problems(&failure), Vec::<String>::new());
    }

    #[test]
    fn atip_l0_07_a_null_meta_a_null_member_and_an_undefined_meta_key_are_refused() {
        assert!(!envelope_problems(&json!({ "ok": true, "data": {}, "meta": null })).is_empty());
        let mut with_null = meta();
        with_null["context"] = Value::Null;
        assert!(!envelope_problems(&json!({ "ok": true, "data": {}, "meta": with_null })).is_empty());
        let mut stray = meta();
        stray["unchanged"] = json!(true);
        assert!(!envelope_problems(&json!({ "ok": true, "data": {}, "meta": stray })).is_empty());
        let mut extension = meta();
        extension["x-canon"] = json!({ "unchanged": true });
        assert!(envelope_problems(&json!({ "ok": true, "data": {}, "meta": extension })).is_empty());
    }

    #[test]
    fn atip_l0_07_an_error_with_a_null_hint_is_refused() {
        let failure = json!({ "ok": false, "error": { "code": "USAGE", "message": "m", "retryable": false, "hint": null }, "meta": meta() });
        assert!(!envelope_problems(&failure).is_empty());
    }

    #[test]
    fn atip_l0_19_a_registry_row_has_an_exit_of_one_or_two() {
        let row = json!({ "code": "USAGE", "exit": 2, "retryable": false, "meaning": "m", "fix": "f" });
        assert!(registry_entry_problems(&row).is_empty());
        assert!(!registry_entry_problems(&json!({ "code": "USAGE", "exit": 3, "retryable": false, "meaning": "m", "fix": "f" })).is_empty());
    }
}
