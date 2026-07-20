//! Task 2.4: "add a field to a fixture schema and assert it appears in
//! `bindings_for`'s output with no second edit." `record_fields`/
//! `bindings_for` derive every field purely by walking whatever JSON
//! Schema a `SchemaRegistry` hands them — this fixture proves that by
//! resolving two synthetic schemas (a "before" and an "after" adding one
//! field) through the exact same, unmodified `bindings_for` call.
//!
//! This exercises the real canon-model-backed registry too
//! (`registry.rs`'s own `change_fields_resolve_expected_types` unit
//! test); this fixture isolates the mechanism itself, independent of
//! which concrete record kind's schema is used — the same proof design
//! D2 makes about canon-model's real schemas.

use canon_model::RecordKind;
use canon_policy::bindings_for;
use schemars::Schema;
use serde_json::json;

fn schema_before() -> Schema {
    Schema::try_from(json!({
        "type": "object",
        "properties": {
            "kind": {"type": "string"},
        },
        "required": ["kind"],
    }))
    .unwrap()
}

fn schema_after() -> Schema {
    // The only change from `schema_before`: one new field, `severity`.
    Schema::try_from(json!({
        "type": "object",
        "properties": {
            "kind": {"type": "string"},
            "severity": {"type": "string", "enum": ["low", "high"]},
        },
        "required": ["kind"],
    }))
    .unwrap()
}

#[test]
fn a_field_added_to_the_schema_appears_in_bindings_for_with_no_second_edit() {
    let before = bindings_for_test_registry(RecordKind::Run, schema_before());
    assert!(before.record_fields.contains_key("kind"));
    assert!(!before.record_fields.contains_key("severity"), "the 'before' schema fixture must not already declare `severity`");

    let after = bindings_for_test_registry(RecordKind::Run, schema_after());
    assert!(after.record_fields.contains_key("kind"), "pre-existing field must still be present");
    assert!(after.record_fields.contains_key("severity"), "the field added to the schema must appear in bindings_for's output");

    // The version fingerprint (task 2.3) must change alongside the field
    // set — a `BindingSet` snapshot is reproducible for, and tied to, a
    // given schema state.
    assert_ne!(before.version, after.version);
}

/// Routes through the public `SchemaRegistry::single`/`bindings_for`
/// surface (`registry.rs`) — a genuine one-schema registry, not a
/// second binding mechanism.
fn bindings_for_test_registry(kind: RecordKind, schema: Schema) -> canon_policy::BindingSet {
    let registry = canon_policy::SchemaRegistry::single(kind, schema);
    bindings_for(kind, &registry)
}
