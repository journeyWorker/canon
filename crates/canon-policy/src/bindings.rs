//! CEL binding generation (design D2, tasks.md group 2): [`bindings_for`]
//! derives a [`BindingSet`] — the `record` variable's field types plus the
//! fixed pure-function allowlist — from a [`SchemaRegistry`], never from a
//! second hand-written field list. [`crate::validate`]'s write-time
//! type-checker and (once S12 lands) `canon context`'s CEL section both
//! read the identical `BindingSet` a given `bindings_for(kind, registry)`
//! call produces, so the two can never disagree (design D6).

use std::collections::BTreeMap;

use canon_model::RecordKind;
use sha2::{Digest, Sha256};

use crate::registry::{record_fields, CelType, SchemaRegistry};

/// One entry in the fixed pure-function allowlist (design D2/D4). Every
/// entry here is either:
/// - a `canon-policy`-registered Rust function ([`crate::functions`]),
///   `native_macro: false`, subject to the write-time arity/type check
///   ([`crate::validate`]) against a literal `Call` AST node, or
/// - a CEL language-native macro (currently just `has`), `native_macro:
///   true`, listed here purely so the allowlist `canon context` shows and
///   the write-time validator's "expected one of" diagnostics agree on
///   the *complete* available surface — a well-formed `has(...)` call
///   never reaches the validator as a `Call` node at all (the parser
///   rewrites it into a field-presence `Select`, see
///   [`crate::validate`]'s module doc), so there is nothing for
///   `canon-policy` to register.
#[derive(Debug, Clone, PartialEq)]
pub struct FunctionSig {
    pub name: &'static str,
    pub args: Vec<CelType>,
    pub returns: CelType,
    pub native_macro: bool,
    pub doc: &'static str,
}

impl std::fmt::Display for FunctionSig {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        let args = self.args.iter().map(ToString::to_string).collect::<Vec<_>>().join(", ");
        write!(f, "{}({args}) -> {}", self.name, self.returns)
    }
}

/// The fixed, reviewed, pure-function allowlist (design D4): the complete
/// set of non-operator callables a CEL expression bound to canon-model
/// data may use, beyond CEL's own operator/comparison grammar. Adding an
/// entry is a reviewed `canon-policy` source change — never a per-
/// consumer `policy.yaml` registration (design D4's rejected
/// alternative).
pub fn allowlisted_functions() -> Vec<FunctionSig> {
    vec![
        FunctionSig {
            name: "age_days",
            args: vec![CelType::Timestamp],
            returns: CelType::Int,
            native_macro: false,
            doc: "age_days(ts) -> int: whole days between `ts` and the evaluation call's caller-supplied `now` (never the wall clock read inside the function itself — see this crate's purity audit in lib.rs and design D4).",
        },
        FunctionSig {
            name: "has",
            args: vec![CelType::Dyn],
            returns: CelType::Bool,
            native_macro: true,
            doc: "has(path) -> bool: CEL's built-in field-presence macro (e.g. `has(record.actor.model)`); not a canon-policy-registered function, listed for completeness so canon context's CEL section and the write-time validator agree on the full available surface (design D6).",
        },
    ]
}

/// A schema-derived, versioned snapshot of what a CEL expression bound to
/// `kind` may read and call (design D2/D3): the `record` variable's field
/// types and the pure-function allowlist. Both `canon-policy`'s own
/// write-time validator ([`crate::validate::compile`]) and (once S12
/// lands) `canon context`'s CEL section are populated from the identical
/// `BindingSet` a `bindings_for` call produces.
#[derive(Debug, Clone, PartialEq)]
pub struct BindingSet {
    pub kind: RecordKind,
    /// A content hash of the resolved field-type tree this `BindingSet`
    /// was derived from (design task 2.3 — "reproducible for a given
    /// schema state"): unchanged for repeated `bindings_for` calls against
    /// an unchanged schema, and changes if-and-only-if `record_fields`
    /// changes shape, independent of `RecordKind::ALL`'s enum-variant
    /// order or any other incidental detail.
    pub version: String,
    pub record_fields: BTreeMap<String, CelType>,
    pub functions: Vec<FunctionSig>,
}

impl BindingSet {
    /// Field names, for "expected one of: …" diagnostics
    /// ([`crate::diagnostics`]) and `canon context`'s future CEL section
    /// listing — always derived from `record_fields`, never a separate
    /// list.
    pub fn field_names(&self) -> Vec<String> {
        self.record_fields.keys().cloned().collect()
    }

    /// Non-macro, canon-policy-registered function names — the set a
    /// literal `Call` AST node's `func_name` is checked against (design
    /// D3); excludes `native_macro` entries like `has`, which the parser
    /// rewrites away before a well-formed use ever reaches the validator
    /// as a `Call`.
    pub fn callable_function_names(&self) -> Vec<String> {
        self.functions.iter().filter(|f| !f.native_macro).map(|f| f.name.to_string()).collect()
    }

    pub fn function(&self, name: &str) -> Option<&FunctionSig> {
        self.functions.iter().find(|f| f.name == name && !f.native_macro)
    }
}

/// Derives a [`BindingSet`] for `kind` from `registry` — the single call
/// site every consumer (the write-time validator, `canon context` once
/// S12 lands) shares (design D2).
pub fn bindings_for(kind: RecordKind, registry: &SchemaRegistry) -> BindingSet {
    let schema = registry.get(kind).unwrap_or_else(|| {
        panic!("SchemaRegistry has no schema for {kind:?} — every RecordKind::ALL member is exported by canon_model::schema_export::record_schemas()");
    });
    let record_fields = record_fields(schema);
    let version = fingerprint(&record_fields);
    BindingSet { kind, version, record_fields, functions: allowlisted_functions() }
}

fn fingerprint(fields: &BTreeMap<String, CelType>) -> String {
    // `BTreeMap`'s iteration order is key-sorted and stable, so this
    // `Debug` rendering is deterministic across repeated calls against an
    // unchanged schema — no field ordering, `HashMap` iteration, or
    // wall-clock input can perturb it.
    let mut hasher = Sha256::new();
    hasher.update(format!("{fields:?}").as_bytes());
    let digest = hasher.finalize();
    digest.iter().map(|b| format!("{b:02x}")).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn bindings_for_change_exposes_envelope_fields() {
        let registry = SchemaRegistry::load();
        let bindings = bindings_for(RecordKind::Change, &registry);
        for expected in ["kind", "at", "actor", "schema", "title", "summary", "status"] {
            assert!(bindings.record_fields.contains_key(expected), "missing field {expected}");
        }
        assert_eq!(bindings.callable_function_names(), vec!["age_days".to_string()]);
    }

    #[test]
    fn version_is_stable_across_repeated_calls() {
        let registry = SchemaRegistry::load();
        let a = bindings_for(RecordKind::Task, &registry);
        let b = bindings_for(RecordKind::Task, &registry);
        assert_eq!(a.version, b.version);
    }

    #[test]
    fn version_differs_across_kinds_with_different_fields() {
        let registry = SchemaRegistry::load();
        let change = bindings_for(RecordKind::Change, &registry);
        let task = bindings_for(RecordKind::Task, &registry);
        assert_ne!(change.version, task.version);
    }
}
