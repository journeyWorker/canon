//! Overlay field `Type` (design.md D2, tasks.md 1.3): the same bare-
//! scalar/`{enum}`/`{list}` STRUCTURAL shape `canon_vocab::manifest::
//! types::Type` validates -- reused by INSPIRATION, not import (this
//! crate's own small type; no `canon-vocab` crate dependency, keeping
//! the two manifest surfaces genuinely separate per design.md D2/R4).
//!
//! Unlike `canon_vocab::manifest::types::Type`, this enum carries
//! NEITHER `Domain` (a named reference into canon-vocab's OWN
//! `enums.yaml` shared vocabulary -- an authoring-vocabulary concept
//! with no ledger-overlay analog) NOR `Evidence` (canon-vocab's D4:
//! `{kind, ref}` resolved against S5's policy-derived evidence-kind
//! domain, also authoring-vocabulary-specific). s16's overlay fields are
//! typed purely structurally: a bare scalar (`bool`/`number`/`string`),
//! an inline enum (`{enum: […]}`), or a list (`{list: <type>}`).
//!
//! Same YAML representation technique as canon-vocab's `Type`
//! (`crates/canon-vocab/src/manifest/types.rs:267-278`'s documented
//! workaround, ported here independently, no shared code): serde_yaml
//! 0.9 serializes an externally-tagged enum's data-carrying variants as
//! YAML `!tag` nodes, not the single-key maps (`{enum: […]}`,
//! `{list: <type>}`) the plugin manifest schema mandates -- so [`Type`]'s
//! [`Serialize`]/[`Deserialize`] impls route through a private `TypeDef`
//! shadow via `serde_yaml::with::singleton_map_recursive`.

use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Debug, PartialEq)]
pub enum Type {
    Bool,
    Number,
    Str,
    Enum(Vec<String>),
    List(Box<Type>),
}

/// Structural type-membership check (lifted ALGORITHM only, `crates/
/// canon-vocab/src/manifest/types.rs:113-126`'s `type_accepts`, pruned to
/// this crate's five-variant [`Type`]). Judges an overlay record body's
/// authored JSON value against a declared field [`Type`] -- P2's
/// `validate_overlay_body` (out of this change's scope) is this
/// function's first real caller. Values are `serde_json::Value` directly
/// (not a re-derived `Literal` shadow type): the overlay-body wire shape
/// P2 validates is `canon_model::RawRecord`, which already wraps
/// `serde_json::Value` verbatim, so there is nothing a separate literal
/// type would add here.
pub fn type_accepts(ty: &Type, value: &serde_json::Value) -> bool {
    use serde_json::Value;
    match (ty, value) {
        (Type::Bool, Value::Bool(_)) => true,
        (Type::Number, Value::Number(_)) => true,
        (Type::Str, Value::String(_)) => true,
        (Type::Enum(members), Value::String(s)) => members.iter().any(|m| m == s),
        (Type::List(inner), Value::Array(items)) => items.iter().all(|i| type_accepts(inner, i)),
        _ => false,
    }
}

// --- serde representation (verbatim technique, canon-vocab's own types.rs) ---

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
enum TypeDef {
    Bool,
    Number,
    #[serde(rename = "string")]
    Str,
    Enum(Vec<String>),
    List(Box<TypeDef>),
}

impl From<TypeDef> for Type {
    fn from(d: TypeDef) -> Self {
        match d {
            TypeDef::Bool => Type::Bool,
            TypeDef::Number => Type::Number,
            TypeDef::Str => Type::Str,
            TypeDef::Enum(m) => Type::Enum(m),
            TypeDef::List(inner) => Type::List(Box::new((*inner).into())),
        }
    }
}

impl From<&Type> for TypeDef {
    fn from(t: &Type) -> Self {
        match t {
            Type::Bool => TypeDef::Bool,
            Type::Number => TypeDef::Number,
            Type::Str => TypeDef::Str,
            Type::Enum(m) => TypeDef::Enum(m.clone()),
            Type::List(inner) => TypeDef::List(Box::new((&**inner).into())),
        }
    }
}

impl Serialize for Type {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serde_yaml::with::singleton_map_recursive::serialize(&TypeDef::from(self), serializer)
    }
}

impl<'de> Deserialize<'de> for Type {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        let def: TypeDef = serde_yaml::with::singleton_map_recursive::deserialize(deserializer)?;
        Ok(def.into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    fn roundtrip(yaml: &str, expected: &Type) {
        let parsed: Type = serde_yaml::from_str(yaml).expect("parses");
        assert_eq!(&parsed, expected);
        let rendered = serde_yaml::to_string(&parsed).expect("renders");
        let reparsed: Type = serde_yaml::from_str(&rendered).expect("re-parses");
        assert_eq!(&reparsed, expected);
    }

    #[test]
    fn bare_scalar_variants_round_trip() {
        roundtrip("bool\n", &Type::Bool);
        roundtrip("number\n", &Type::Number);
        roundtrip("string\n", &Type::Str);
    }

    #[test]
    fn enum_and_list_single_key_maps_round_trip() {
        roundtrip("enum: [open, closed]\n", &Type::Enum(vec!["open".to_string(), "closed".to_string()]));
        roundtrip("list: string\n", &Type::List(Box::new(Type::Str)));
        roundtrip("list:\n  enum: [a, b]\n", &Type::List(Box::new(Type::Enum(vec!["a".to_string(), "b".to_string()]))));
    }

    #[test]
    fn type_accepts_bare_scalars_structurally() {
        assert!(type_accepts(&Type::Bool, &json!(true)));
        assert!(!type_accepts(&Type::Bool, &json!("true")));
        assert!(type_accepts(&Type::Number, &json!(3)));
        assert!(type_accepts(&Type::Str, &json!("hi")));
        assert!(!type_accepts(&Type::Str, &json!(1)));
    }

    #[test]
    fn type_accepts_enum_membership_and_list_elementwise() {
        let ty = Type::Enum(vec!["open".to_string(), "closed".to_string()]);
        assert!(type_accepts(&ty, &json!("open")));
        assert!(!type_accepts(&ty, &json!("blocked")));

        let list_ty = Type::List(Box::new(Type::Str));
        assert!(type_accepts(&list_ty, &json!(["a", "b"])));
        assert!(!type_accepts(&list_ty, &json!(["a", 1])));
        assert!(type_accepts(&list_ty, &json!([])));
    }
}
