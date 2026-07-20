//! The schema registry `canon-policy`'s CEL bindings are generated from
//! (design D2): [`SchemaRegistry::load`] wraps canon-model's own single
//! source of truth for "what fields does a record kind have" —
//! [`canon_model::schema_export::record_schemas`], the SAME call
//! `canon fmt`'s validator drives (S11) and S12's `canon context` will
//! drive once it lands. There is no second, hand-written field list
//! anywhere in this crate; every [`CelType`] in a [`crate::BindingSet`]
//! is derived by walking the JSON Schema `schemars::schema_for!` produces
//! directly from each record type's Rust definition.
//!
//! Scope note: S12 (`openspec/changes/s12-canon-context`) specifies a
//! `SchemaRegistry::load(repo: &Path) -> SchemaRegistry` living IN
//! `canon-model` itself, shared by `canon fmt`/`canon gate`/`canon
//! context`. That type has not landed yet — S12 is proposal-only as of
//! this change (no `canon-cli`/`canon-model` code implements it), and
//! `canon-policy`'s own territory this batch excludes `canon-model`
//! (S13Policy/S11Finish split, this repo's coordination doc). This
//! module's [`SchemaRegistry`] is therefore a canon-policy-local newtype
//! wrapping the one function canon-model DOES already expose
//! (`schema_export::record_schemas()`) — not a second registry, just an
//! adapter in front of the single existing source, ready to be replaced
//! by a thin re-export of S12's `canon_model::SchemaRegistry` the moment
//! that type exists (a one-line change here, no [`crate::bindings_for`]
//! caller has to change).

use std::collections::BTreeMap;

use canon_model::RecordKind;
use serde_json::{Map, Value as Json};

/// The CEL-visible type of one bindable field or function argument/return
/// value, derived from a JSON Schema fragment. Deliberately coarser than
/// full JSON Schema (canon's record schemas never need `oneOf`/pattern
/// validation at the CEL boundary) — just enough shape to drive write-time
/// type-checking (design D3) and the runtime JSON→CEL value conversion
/// ([`crate::eval::json_to_cel`]).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum CelType {
    String,
    /// A closed string enum (e.g. `RecordKind`, `TaskStatus`) — CEL-typed
    /// as a string, but the member set is carried through for "expected
    /// one of: …" diagnostics.
    Enum(Vec<String>),
    Int,
    UInt,
    Double,
    Bool,
    Timestamp,
    List(Box<CelType>),
    /// A nested object with a known, closed field set (e.g. `Actor`).
    Map(BTreeMap<String, CelType>),
    /// Unconstrained: no JSON Schema `type` (e.g. `Event.detail`,
    /// `serde_json::Value` fields), a depth-bounded cutoff, or an
    /// unresolvable `$ref`. Never rejected at write time — a field this
    /// crate cannot type is not an argument for rejecting a policy
    /// expression that reads it, only for skipping the type check.
    Dyn,
}

impl std::fmt::Display for CelType {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            CelType::String => write!(f, "string"),
            CelType::Enum(members) => write!(f, "string (one of: {})", members.join(", ")),
            CelType::Int => write!(f, "int"),
            CelType::UInt => write!(f, "uint"),
            CelType::Double => write!(f, "double"),
            CelType::Bool => write!(f, "bool"),
            CelType::Timestamp => write!(f, "timestamp"),
            CelType::List(inner) => write!(f, "list<{inner}>"),
            CelType::Map(_) => write!(f, "map"),
            CelType::Dyn => write!(f, "dyn"),
        }
    }
}

/// How many `$ref`/nested-object hops [`resolve`] will follow before
/// giving up and returning [`CelType::Dyn`] — a defensive bound against a
/// pathological or (in principle, though none exist today) self-
/// referential schema, not a limit any of the twelve closed record kinds'
/// real field trees comes close to.
const MAX_RESOLVE_DEPTH: u8 = 6;

fn resolve(value: &Json, defs: &Map<String, Json>, depth: u8) -> CelType {
    if depth > MAX_RESOLVE_DEPTH {
        return CelType::Dyn;
    }
    let Some(obj) = value.as_object() else {
        return CelType::Dyn;
    };

    if let Some(reference) = obj.get("$ref").and_then(Json::as_str) {
        return match reference.strip_prefix("#/$defs/").and_then(|name| defs.get(name)) {
            Some(target) => resolve(target, defs, depth + 1),
            None => CelType::Dyn,
        };
    }

    if let Some(variants) = obj.get("anyOf").and_then(Json::as_array) {
        // The nullable-field pattern schemars emits for `Option<T>`:
        // `{"anyOf": [{"$ref": "#/$defs/T"}, {"type": "null"}]}`. CEL has
        // no distinct "nullable" type — a field either resolves to `T`'s
        // type or the value is absent/null at read time — so the bound
        // CEL type is the first non-null branch's.
        for branch in variants {
            if branch.get("type").and_then(Json::as_str) == Some("null") {
                continue;
            }
            return resolve(branch, defs, depth + 1);
        }
        return CelType::Dyn;
    }

    if let Some(members) = obj.get("enum").and_then(Json::as_array) {
        let members = members.iter().filter_map(Json::as_str).map(String::from).collect();
        return CelType::Enum(members);
    }

    match obj.get("type") {
        Some(Json::String(kind)) => resolve_typed(kind, obj, defs, depth),
        Some(Json::Array(kinds)) => {
            // `Option<Scalar>` without a `$ref` indirection: `"type": ["string", "null"]`.
            for kind in kinds.iter().filter_map(Json::as_str) {
                if kind == "null" {
                    continue;
                }
                return resolve_typed(kind, obj, defs, depth);
            }
            CelType::Dyn
        }
        // No `type` key at all: an intentionally open field
        // (`serde_json::Value`, e.g. `Event.detail`) — unconstrained.
        _ => CelType::Dyn,
    }
}

fn resolve_typed(kind: &str, obj: &Map<String, Json>, defs: &Map<String, Json>, depth: u8) -> CelType {
    match kind {
        "string" => {
            if obj.get("format").and_then(Json::as_str) == Some("date-time") {
                CelType::Timestamp
            } else {
                CelType::String
            }
        }
        "integer" => {
            if obj.get("format").and_then(Json::as_str).is_some_and(|f| f.starts_with("uint")) {
                CelType::UInt
            } else {
                CelType::Int
            }
        }
        "number" => CelType::Double,
        "boolean" => CelType::Bool,
        "array" => {
            let inner = obj.get("items").map_or(CelType::Dyn, |items| resolve(items, defs, depth + 1));
            CelType::List(Box::new(inner))
        }
        "object" => match obj.get("properties").and_then(Json::as_object) {
            Some(props) => {
                let fields = props.iter().map(|(name, schema)| (name.clone(), resolve(schema, defs, depth + 1))).collect();
                CelType::Map(fields)
            }
            // An object schema with no closed `properties` set (e.g.
            // `additionalProperties` only) — open, unconstrained.
            None => CelType::Dyn,
        },
        _ => CelType::Dyn,
    }
}

/// Walks a record kind's exported JSON Schema `properties` (top level) +
/// `$defs` (nested types) into a flat `field name -> CelType` map — the
/// same one-hop-per-`$ref` traversal a JSON Schema validator performs,
/// just producing a CEL type instead of a validation verdict.
pub(crate) fn record_fields(schema: &schemars::Schema) -> BTreeMap<String, CelType> {
    let root = schema.as_object().expect("canon-model record schemas are always JSON objects (schema_for! output)");
    let empty_defs = Map::new();
    let defs = root.get("$defs").and_then(Json::as_object).unwrap_or(&empty_defs);
    let empty_props = Map::new();
    let props = root.get("properties").and_then(Json::as_object).unwrap_or(&empty_props);
    props.iter().map(|(name, field_schema)| (name.clone(), resolve(field_schema, defs, 0))).collect()
}

/// Wraps canon-model's schema export (see module doc for why this type
/// lives here rather than in `canon-model` itself, pending S12).
pub struct SchemaRegistry {
    schemas: std::collections::HashMap<RecordKind, schemars::Schema>,
}

impl SchemaRegistry {
    /// Loads every one of the twelve closed [`RecordKind`]s' schemas from
    /// `canon_model::schema_export::record_schemas()` — canon-model's own
    /// single export function, never a second copy.
    pub fn load() -> Self {
        Self { schemas: canon_model::schema_export::record_schemas().into_iter().collect() }
    }

    /// A registry containing exactly one schema, keyed to `kind` —
    /// mainly useful for fixture-driven tests (e.g. this crate's own
    /// `tests/reflected_change.rs`, task 2.4) that need to resolve a
    /// synthetic schema through the real `bindings_for` call without
    /// mutating canon-model's committed schemas.
    pub fn single(kind: RecordKind, schema: schemars::Schema) -> Self {
        Self { schemas: std::collections::HashMap::from([(kind, schema)]) }
    }

    pub fn get(&self, kind: RecordKind) -> Option<&schemars::Schema> {
        self.schemas.get(&kind)
    }

    pub fn kinds(&self) -> impl Iterator<Item = RecordKind> + '_ {
        self.schemas.keys().copied()
    }

    /// The closed enum member list for `(kind, field)` (S12 task 6.1) —
    /// the single reusable accessor every enum-domain diagnostic (this
    /// crate's own [`crate::diagnostics`] and `canon-fmt`'s schema
    /// violations alike) sources its "expected one of: …" member list
    /// from, rather than re-deriving it. Walks `kind`'s compiled schema
    /// through the SAME [`record_fields`] `$defs` traversal
    /// [`CelType::Enum`] already resolves its member set from — never a
    /// second, hand-rolled enum extractor. Returns an empty list when
    /// `kind` has no registered schema, `field` is absent from its
    /// top-level `properties`, or the field resolves to a non-enum type
    /// — every case a diagnostic caller can treat identically as "no
    /// closed domain to report".
    pub fn enum_domain(&self, kind: RecordKind, field: &str) -> Vec<String> {
        match self.get(kind).map(record_fields).and_then(|fields| fields.get(field).cloned()) {
            Some(CelType::Enum(members)) => members,
            _ => Vec::new(),
        }
    }
}

impl Default for SchemaRegistry {
    fn default() -> Self {
        Self::load()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn every_record_kind_has_a_schema() {
        let registry = SchemaRegistry::load();
        for kind in RecordKind::ALL {
            assert!(registry.get(kind).is_some(), "{kind:?} missing from SchemaRegistry — canon_model::schema_export::record_schemas() drifted from RecordKind::ALL");
        }
    }

    #[test]
    fn change_fields_resolve_expected_types() {
        let registry = SchemaRegistry::load();
        let schema = registry.get(RecordKind::Change).unwrap();
        let fields = record_fields(schema);

        assert_eq!(fields.get("kind"), Some(&CelType::Enum(RecordKind::ALL.iter().map(|k| k.as_str().to_string()).collect())));
        assert_eq!(fields.get("at"), Some(&CelType::Timestamp));
        assert_eq!(fields.get("schema"), Some(&CelType::UInt));
        assert_eq!(fields.get("title"), Some(&CelType::String));
        assert!(matches!(fields.get("actor"), Some(CelType::Map(_))), "actor should resolve through its $ref to a nested Map");
    }

    #[test]
    fn nested_actor_fields_are_reachable() {
        let registry = SchemaRegistry::load();
        let schema = registry.get(RecordKind::Change).unwrap();
        let fields = record_fields(schema);
        let CelType::Map(actor_fields) = fields.get("actor").unwrap() else {
            panic!("actor did not resolve to a Map");
        };
        assert_eq!(actor_fields.get("agent_id"), Some(&CelType::String));
    }

    #[test]
    fn open_value_field_is_dyn() {
        let registry = SchemaRegistry::load();
        let schema = registry.get(RecordKind::Event).unwrap();
        let fields = record_fields(schema);
        // `Event.detail` is a deliberately open `serde_json::Value` field
        // (see schema_export.rs's own module doc) — no `type` key at all.
        assert_eq!(fields.get("detail"), Some(&CelType::Dyn));
    }

    #[test]
    fn enum_domain_returns_the_closed_member_list_for_an_enum_field() {
        let registry = SchemaRegistry::load();
        let members = registry.enum_domain(RecordKind::Change, "kind");
        assert_eq!(members, RecordKind::ALL.iter().map(|k| k.as_str().to_string()).collect::<Vec<_>>());
    }

    #[test]
    fn enum_domain_is_empty_for_a_non_enum_field() {
        let registry = SchemaRegistry::load();
        assert!(registry.enum_domain(RecordKind::Change, "title").is_empty());
    }

    #[test]
    fn enum_domain_is_empty_for_an_undeclared_field() {
        let registry = SchemaRegistry::load();
        assert!(registry.enum_domain(RecordKind::Change, "not-a-real-field").is_empty());
    }

    #[test]
    fn enum_domain_is_empty_for_an_unregistered_kind() {
        let registry = SchemaRegistry::single(RecordKind::Change, registry_only_schema());
        assert!(registry.enum_domain(RecordKind::Task, "kind").is_empty());
    }

    fn registry_only_schema() -> schemars::Schema {
        schemars::Schema::try_from(serde_json::json!({"type": "object", "properties": {"kind": {"type": "string"}}})).unwrap()
    }
}
