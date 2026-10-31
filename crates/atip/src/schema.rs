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
static REGISTRY_ENTRY: LazyLock<Validator> = LazyLock::new(|| compile(REGISTRY_ENTRY_SCHEMA));

fn problems_of(validator: &Validator, document: &Value) -> Vec<String> {
    validator.iter_errors(document).map(|error| format!("{error} at {}", error.instance_path())).collect()
}

/// What breaks the envelope schema in `document`: one line per violation,
/// empty when the document is a conforming envelope.
pub fn envelope_problems(document: &Value) -> Vec<String> {
    problems_of(&ENVELOPE, document)
}

/// What breaks the registry-entry schema in `row`, empty when it conforms.
pub fn registry_entry_problems(row: &Value) -> Vec<String> {
    problems_of(&REGISTRY_ENTRY, row)
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
    #[test]
    fn atip_l0_07_the_vendored_schemas_are_the_published_bytes() {
        assert_eq!(digest(ENVELOPE_SCHEMA), "055ace2ffb10eddba2c62fdff63155a83be4a8c1ba6ece73f6922f561ac8b299");
        assert_eq!(digest(ERROR_SCHEMA), "49afaa3bf93d848b1e477b19b3969680b4a0818e46dbdc6c54db248d4ecfdcfb");
        assert_eq!(digest(REGISTRY_ENTRY_SCHEMA), "411228ac4a2cc79ef4079161e9d015c5352bca49953aa33bf7007d0b100daedc");
    }

    #[test]
    fn atip_l0_07_one_changed_character_changes_the_digest() {
        let changed = ENVELOPE_SCHEMA.replacen("atip/1 envelope", "atip/1 envelopf", 1);
        assert_ne!(digest(&changed), digest(ENVELOPE_SCHEMA));
    }

    fn meta() -> Value {
        json!({ "schemaVersion": "1.0", "tool": "canon", "toolVersion": "0.33.0", "contract": "atip/1", "command": "status" })
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
