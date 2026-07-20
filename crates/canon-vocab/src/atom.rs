//! The task-atom / handoff-body YAML record (design.md D2): "a typed task
//! atom is one YAML record — `{id, tag, attrs}` where `tag` names a
//! directive ... and `attrs` supplies that directive's declared attributes
//! ... A handoff body is likewise `{tag: "handoff-<domain>", attrs: {…}}`".
//! A NEW, canon-authored parser — deliberately NOT the donor vocabulary
//! system's
//! line-based `::tag attr=value` scene-DSL grammar (D2 Non-Goals): an atom
//! file is a plain YAML sequence of these records, validated by
//! [`crate::checker::check_directive`] exactly as a scene directive is.

use std::collections::BTreeMap;

use serde::Deserialize;

use crate::checker::{check_directive, Diagnostic};
use crate::manifest::snapshot::CapabilitySnapshot;

/// One `{id, tag, attrs}` record — a task atom (`tag: "task"`) or a handoff
/// body (`tag: "handoff-<domain>"`), both validated through the identical
/// pipeline (D5). `attrs` keeps each value as raw `serde_yaml::Value` so the
/// checker (which needs the ORIGINAL shape — a mapping for `evidence`, a
/// plain scalar otherwise) can judge it against its declared [`crate::
/// manifest::types::Type`] without a lossy intermediate conversion.
#[derive(Clone, Debug, PartialEq, Deserialize, serde::Serialize)]
pub struct AtomRecord {
    pub id: String,
    pub tag: String,
    #[serde(default)]
    pub attrs: BTreeMap<String, serde_yaml::Value>,
}

#[derive(Debug)]
pub struct ParseError {
    pub message: String,
}

impl std::fmt::Display for ParseError {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", self.message)
    }
}

impl std::error::Error for ParseError {}

/// Parse an atoms file (a YAML sequence of [`AtomRecord`]). Malformed YAML is
/// a structured [`ParseError`], never a panic — mirrors every other
/// `serde_yaml::from_str` call site in this crate (`crate::manifest::loader`
/// module doc's "never panics" guarantee, extended to the atom parser).
pub fn parse_atoms_file(text: &str) -> Result<Vec<AtomRecord>, ParseError> {
    serde_yaml::from_str(text).map_err(|e| ParseError { message: e.to_string() })
}

/// Validate every atom in `atoms` against `snapshot`, anchoring each atom's
/// diagnostics on its own `id` ([`crate::checker`]'s `subject`). A DUPLICATE
/// `id` across atoms is itself an authoring error the checker's per-directive
/// validation cannot see (it validates one record at a time) — reported here
/// as `E-DUPLICATE-ATOM-ID`, canon-vocab's own code (no donor analog: a scene
/// document has no equivalent "many independent records sharing one id
/// namespace" concept).
pub fn validate_atoms(atoms: &[AtomRecord], snapshot: &CapabilitySnapshot) -> Vec<Diagnostic> {
    let mut diags = Vec::new();
    let mut seen = std::collections::BTreeSet::new();
    for atom in atoms {
        if !seen.insert(atom.id.clone()) {
            diags.push(Diagnostic {
                code: "E-DUPLICATE-ATOM-ID".to_string(),
                severity: crate::span::Severity::Error,
                message: format!("atom id `{}` is used more than once in this file", atom.id),
                subject: atom.id.clone(),
            });
        }
        diags.extend(check_directive(&atom.tag, &atom.attrs, snapshot, &atom.id));
    }
    diags
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::schema::DirectiveDecl;
    use crate::manifest::types::{AttrDecl, Type};

    fn snapshot_with_task_directive() -> CapabilitySnapshot {
        let mut snap = CapabilitySnapshot::default();
        snap.directives.insert(
            "task".to_string(),
            DirectiveDecl { name: "task".into(), attrs: vec![AttrDecl { name: "desc".into(), required: true, ty: Type::Str, default: None }] },
        );
        snap
    }

    #[test]
    fn parses_a_flat_sequence_of_records() {
        let text = "- id: s10#4.1\n  tag: task\n  attrs:\n    desc: \"do the thing\"\n";
        let atoms = parse_atoms_file(text).unwrap();
        assert_eq!(atoms.len(), 1);
        assert_eq!(atoms[0].id, "s10#4.1");
        assert_eq!(atoms[0].tag, "task");
        assert_eq!(atoms[0].attrs.get("desc").unwrap().as_str(), Some("do the thing"));
    }

    #[test]
    fn malformed_yaml_is_a_parse_error_not_a_panic() {
        assert!(parse_atoms_file("- id: [this is not").is_err());
    }

    #[test]
    fn duplicate_atom_ids_are_reported() {
        let snap = snapshot_with_task_directive();
        let atoms = vec![
            AtomRecord { id: "a#1".into(), tag: "task".into(), attrs: BTreeMap::from([("desc".to_string(), serde_yaml::Value::String("x".into()))]) },
            AtomRecord { id: "a#1".into(), tag: "task".into(), attrs: BTreeMap::from([("desc".to_string(), serde_yaml::Value::String("y".into()))]) },
        ];
        let diags = validate_atoms(&atoms, &snap);
        assert!(diags.iter().any(|d| d.code == "E-DUPLICATE-ATOM-ID"));
    }

    #[test]
    fn a_well_formed_atoms_file_validates_clean() {
        let snap = snapshot_with_task_directive();
        let atoms = vec![AtomRecord { id: "a#1".into(), tag: "task".into(), attrs: BTreeMap::from([("desc".to_string(), serde_yaml::Value::String("x".into()))]) }];
        assert!(validate_atoms(&atoms, &snap).is_empty());
    }
}
