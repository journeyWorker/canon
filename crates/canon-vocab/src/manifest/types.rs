//! Retargeted from the donor manifest layer's types (design.md D1's verified
//! shape: "`type` is one of a bare scalar (`string`, `number`, `bool`), an
//! inline `{enum: […]}`, or a `{list: <type>}`", from the donor core plugin's
//! staging directives). Two additions beyond that verified
//! shape, both explicit design decisions:
//!
//! - [`Type::Domain`] (lifted from the donor's `Type::Domain(String)`): a named
//!   reference into `enums.yaml`'s shared vocabulary (D1: "`enums.yaml`
//!   declares shared enums (task `status`, ... handoff domain names)"),
//!   resolved against [`crate::manifest::snapshot::CapabilitySnapshot::enums`]
//!   at check time — distinct from an inline `Type::Enum`, whose members are
//!   embedded in the attr declaration itself.
//! - [`Type::Evidence`] (D4, new — no donor analog): `{kind, ref}`, whose
//!   `kind` domain resolves from S5's policy at check time
//!   ([`crate::policy_bridge`]), never a locally-declared enum.
//!
//! **Deliberately NOT lifted**: `Type::Record`/`Type::Map`/
//! `Type::EnumFromOption`/`Type::ProviderRef`/`Type::SlotId`/`Type::AssetKind`
//! and their `shape:`/state-slot machinery — every one of those exists to
//! express the donor's scene-state/provider-catalog/asset-id domain, which has no
//! task-atom or handoff-template analog (D2 Non-Goals).
//!
//! `Type::AppliesWhen` (D7): reserved, NOT a variant here. D7 records the
//! pointer only — "a future attribute-type addition is `Type::AppliesWhen`
//! resolved through `canon-policy`, never a second, parallel mechanism" — an
//! unused variant today would be dead code; this doc comment IS the pointer
//! D7 asks to be recorded.
//!
//! Like the donor, `Type`'s YAML shape is a bare string for a unit variant
//! (`bool`/`number`/`string`/`evidence`) or a single-key map for a
//! data-carrying variant (`{enum: […]}`/`{list: <type>}`/`{domain: <name>}`),
//! via `serde_yaml::with::singleton_map_recursive` through the private
//! `TypeDef` shadow (verbatim technique from the donor manifest layer's types,
//! doc comment: "serde_yaml 0.9 serializes externally-tagged enums as YAML
//! `!tags`, not the single-key maps the spec mandates").

use serde::{Deserialize, Deserializer, Serialize, Serializer};

#[derive(Clone, Debug, PartialEq)]
pub enum Type {
    Bool,
    Number,
    Str,
    Enum(Vec<String>),
    List(Box<Type>),
    /// A named reference into `enums.yaml`'s shared vocabulary.
    Domain(String),
    /// D4: `{kind: <policy-resolved evidence kind>, ref: <string>}`.
    Evidence,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttrDecl {
    pub name: String,
    #[serde(default)]
    pub required: bool,
    #[serde(rename = "type")]
    pub ty: Type,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<Literal>,
}

/// A record field's literal value (plugin §7 lineage, from the donor manifest
/// layer's types): the runtime shape [`type_accepts`] judges an authored
/// YAML value against a declared [`Type`]. `Map` carries `evidence`'s
/// `{kind, ref}` structure (and any other future record-shaped attr).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(untagged)]
pub enum Literal {
    Bool(bool),
    Num(f64),
    Str(String),
    List(Vec<Literal>),
    Map(std::collections::BTreeMap<String, Literal>),
}

impl Literal {
    /// Convert a parsed YAML value into a `Literal`. `None` for a value with
    /// no literal representation (null, a YAML tag, or a non-string map key)
    /// — never panics.
    pub fn from_yaml(v: &serde_yaml::Value) -> Option<Literal> {
        use serde_yaml::Value;
        match v {
            Value::Bool(b) => Some(Literal::Bool(*b)),
            Value::Number(n) => n.as_f64().map(Literal::Num),
            Value::String(s) => Some(Literal::Str(s.clone())),
            Value::Sequence(items) => items.iter().map(Literal::from_yaml).collect::<Option<Vec<_>>>().map(Literal::List),
            Value::Mapping(m) => {
                let mut out = std::collections::BTreeMap::new();
                for (k, val) in m {
                    let key = k.as_str()?.to_string();
                    out.insert(key, Literal::from_yaml(val)?);
                }
                Some(Literal::Map(out))
            }
            Value::Null | Value::Tagged(_) => None,
        }
    }
}

/// Structural type-membership check (lifted algorithm from the donor manifest
/// layer's types, pruned to canon's [`Type`] set). `Type::Enum`'s inline
/// members are checked here (its domain is embedded in the type itself);
/// `Type::Domain`'s named-enum membership and `Type::Evidence`'s policy-kind
/// membership are snapshot-dependent and are therefore checked by
/// [`crate::checker::check_attr_value`], NOT here — mirroring exactly how
/// the donor's own `type_accepts` never resolves `Type::ProviderRef`/
/// `Type::EnumFromOption` membership either (that snapshot lookup happens in
/// `check_attr_value`, in the donor checker). This function
/// only judges the STRUCTURAL shape those two variants require: any string
/// for `Domain`, and a two-key `{kind: string, ref: string}` map for
/// `Evidence`.
pub fn type_accepts(ty: &Type, lit: &Literal) -> bool {
    match (ty, lit) {
        (Type::Bool, Literal::Bool(_)) => true,
        (Type::Number, Literal::Num(_)) => true,
        (Type::Str, Literal::Str(_)) => true,
        (Type::Enum(members), Literal::Str(s)) => members.iter().any(|m| m == s),
        (Type::List(inner), Literal::List(items)) => items.iter().all(|i| type_accepts(inner, i)),
        (Type::Domain(_), Literal::Str(_)) => true,
        (Type::Evidence, Literal::Map(m)) => {
            m.len() == 2 && matches!(m.get("kind"), Some(Literal::Str(_))) && matches!(m.get("ref"), Some(Literal::Str(_)))
        }
        _ => false,
    }
}

// --- serde representation (verbatim technique from the donor manifest layer's types) ---

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
enum TypeDef {
    Bool,
    Number,
    #[serde(rename = "string")]
    Str,
    Enum(Vec<String>),
    List(Box<TypeDef>),
    Domain(String),
    Evidence,
}

impl From<TypeDef> for Type {
    fn from(d: TypeDef) -> Self {
        match d {
            TypeDef::Bool => Type::Bool,
            TypeDef::Number => Type::Number,
            TypeDef::Str => Type::Str,
            TypeDef::Enum(m) => Type::Enum(m),
            TypeDef::List(inner) => Type::List(Box::new((*inner).into())),
            TypeDef::Domain(s) => Type::Domain(s),
            TypeDef::Evidence => Type::Evidence,
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
            Type::Domain(s) => TypeDef::Domain(s.clone()),
            Type::Evidence => TypeDef::Evidence,
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

    #[test]
    fn bare_scalar_types_parse_from_plain_strings() {
        assert_eq!(serde_yaml::from_str::<Type>("bool").unwrap(), Type::Bool);
        assert_eq!(serde_yaml::from_str::<Type>("number").unwrap(), Type::Number);
        assert_eq!(serde_yaml::from_str::<Type>("string").unwrap(), Type::Str);
        assert_eq!(serde_yaml::from_str::<Type>("evidence").unwrap(), Type::Evidence);
    }

    #[test]
    fn inline_enum_and_list_and_domain_parse_from_single_key_maps() {
        assert_eq!(serde_yaml::from_str::<Type>("enum: [a, b]").unwrap(), Type::Enum(vec!["a".into(), "b".into()]));
        assert_eq!(serde_yaml::from_str::<Type>("list: string").unwrap(), Type::List(Box::new(Type::Str)));
        assert_eq!(serde_yaml::from_str::<Type>("domain: task-status").unwrap(), Type::Domain("task-status".into()));
    }

    #[test]
    fn type_accepts_checks_inline_enum_membership() {
        let ty = Type::Enum(vec!["open".into(), "done".into()]);
        assert!(type_accepts(&ty, &Literal::Str("open".into())));
        assert!(!type_accepts(&ty, &Literal::Str("closed".into())));
    }

    #[test]
    fn type_accepts_checks_evidence_structural_shape_only() {
        let mut m = std::collections::BTreeMap::new();
        m.insert("kind".to_string(), Literal::Str("test-run".into()));
        m.insert("ref".to_string(), Literal::Str("scenario://x".into()));
        assert!(type_accepts(&Type::Evidence, &Literal::Map(m.clone())));

        // A kind value outside the policy domain is STRUCTURALLY accepted here
        // (kind-membership is check_attr_value's job, not type_accepts's).
        m.insert("kind".to_string(), Literal::Str("anything".into()));
        assert!(type_accepts(&Type::Evidence, &Literal::Map(m)));

        // Missing `ref` fails structurally.
        let mut missing_ref = std::collections::BTreeMap::new();
        missing_ref.insert("kind".to_string(), Literal::Str("test-run".into()));
        assert!(!type_accepts(&Type::Evidence, &Literal::Map(missing_ref)));
    }
}
