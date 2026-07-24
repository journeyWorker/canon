//! Retargeted from the donor manifest layer's schema (design.md D1's verified
//! `plugin.yaml`/`directives/*.yaml`/`enums.yaml` shapes, from the donor core
//! plugin's manifest/directives/enums assets), pruned to the fields D1
//! actually cites.
//!
//! `DirectiveDecl` here is `{name, attrs}` only — the donor's `layer`/`state`/
//! `effects`/`bridge`/`lower` fields are dropped: they express scene-DSL
//! lowering-to-engine-state concepts (a directive's declared state slots, its
//! effect writes, its bridge-service call, its compile target) with no
//! task-atom/handoff-template analog (D2 Non-Goals: "only the manifest
//! declaration format + resolution + validation algorithm are lifted, not
//! the scene-script surface syntax"). Canon's own compile step (task 4.2/4.3)
//! is a dedicated Rust function against `canon_model::Task`/`HandoffBody`
//! ([`crate::compile`]/[`crate::handoff_compile`]), not a manifest-declared
//! `lower:` mapping. `semantics` (the donor's closed `writes.sceneState`/…
//! vocabulary) is dropped for the same
//! reason — no canon directive semantics-flag vocabulary exists.

use serde::{Deserialize, Serialize};

use crate::manifest::types::{AttrDecl, Literal, Type};

#[derive(Debug, Deserialize)]
pub struct DirectivesFile {
    pub directives: Vec<DirectiveDecl>,
}

#[derive(Debug, Deserialize)]
pub struct EnumsFile {
    pub enums: std::collections::BTreeMap<String, Vec<String>>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct DirectiveDecl {
    pub name: String,
    pub attrs: Vec<AttrDecl>,
}

/// plugin §5's `kind:` field (verified, module doc) — CLOSED here
/// (the closed-vocabulary audit pattern): only the two values this workspace's own
/// manifests ever declare (the real `.canon/vocab/canon.core/plugin.yaml`'s
/// `kind: core`, and every consumer-repo `.canon/vocab/<id>/plugin.yaml`'s
/// `kind: project`) parse; anything else is an `E-PLUGIN-MANIFEST` at
/// `plugin.yaml` parse time (serde's own "unknown variant" diagnostic),
/// never a value silently carried through and left unchecked. Nothing in
/// this crate branches on `kind` today (`crate::manifest::resolve`
/// distinguishes `canon.core` from a consumer plugin by `id ==
/// CORE_PLUGIN_ID`, never by `kind`) — closing the type is the fix itself
/// (a manifest bug becomes a load-time diagnostic instead of a value
/// nothing ever validates), not a prerequisite for new dispatch logic.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum PluginKind {
    Core,
    Project,
}

/// plugin §5's `exports:` map KEY vocabulary — CLOSED here (the
/// closed-vocabulary audit pattern, this crate's own two export kinds, module doc's "pruned to
/// canon's two export kinds"). Closing this at the type level moves
/// [`crate::manifest::loader`]'s former `LoadError::UnknownExport`
/// procedural check (a `match export.as_str() { … other => … }` runtime
/// fallback) to `plugin.yaml` PARSE time — an unrecognized export key is
/// now an `E-PLUGIN-MANIFEST` at load, and `loader`'s own export-kind match
/// is exhaustive over this enum (no `other` arm left to reach).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ExportKind {
    Directives,
    Enums,
}

impl ExportKind {
    pub fn as_str(&self) -> &'static str {
        match self {
            ExportKind::Directives => "directives",
            ExportKind::Enums => "enums",
        }
    }
}

/// plugin §5 manifest entry (pruned: no `options`/`kind` beyond what D1
/// cites as verified — `id`, `version`, `kind`, `depends`, `exports`).
/// `kind`/`exports`' keys are closed-typed ([`PluginKind`]/[`ExportKind`],
/// their own doc comments) — an unrecognized value in either rejects the
/// WHOLE manifest at parse time, never a permissively-accepted open string.
#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub version: String,
    pub kind: PluginKind,
    #[serde(default)]
    pub depends: Vec<Depends>,
    pub exports: std::collections::BTreeMap<ExportKind, String>,
    #[serde(default)]
    pub options: Vec<OptionDecl>,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct Depends {
    pub id: String,
    pub range: String,
}

#[derive(Clone, Debug, Serialize, Deserialize)]
pub struct OptionDecl {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: Type,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub default: Option<Literal>,
}

/// A directive declaring the same attr name twice (manifest bug, not an
/// authoring error) — the one donor manifest-layer validation check this
/// crate keeps (its `SEMANTICS_VOCAB` check is dropped along with
/// `semantics`, see module doc).
pub fn duplicate_attr_names(d: &DirectiveDecl) -> Vec<String> {
    let mut seen = std::collections::BTreeSet::new();
    let mut dups = Vec::new();
    for a in &d.attrs {
        if !seen.insert(a.name.clone()) {
            dups.push(a.name.clone());
        }
    }
    dups
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn duplicate_attr_names_reports_the_repeated_name() {
        let d = DirectiveDecl {
            name: "task".into(),
            attrs: vec![
                AttrDecl { name: "desc".into(), required: true, ty: Type::Str, default: None },
                AttrDecl { name: "desc".into(), required: false, ty: Type::Str, default: None },
            ],
        };
        assert_eq!(duplicate_attr_names(&d), vec!["desc".to_string()]);
    }

    #[test]
    fn directives_file_parses_the_verified_shape() {
        let yaml = r#"
directives:
  - name: task
    attrs:
      - name: desc
        type: string
        required: true
      - name: status
        type: { enum: [open, done] }
        required: true
"#;
        let f: DirectivesFile = serde_yaml::from_str(yaml).unwrap();
        assert_eq!(f.directives.len(), 1);
        assert_eq!(f.directives[0].name, "task");
        assert_eq!(f.directives[0].attrs.len(), 2);
    }

    #[test]
    fn plugin_manifest_round_trips_a_valid_shape() {
        let yaml = "id: canon.core\nversion: \"0.1.0\"\nkind: core\nexports:\n  directives: directives/\n  enums: enums.yaml\n";
        let m: PluginManifest = serde_yaml::from_str(yaml).expect("valid kind/exports parse");
        assert_eq!(m.kind, PluginKind::Core);
        assert_eq!(m.exports.get(&ExportKind::Directives), Some(&"directives/".to_string()));
        assert_eq!(m.exports.get(&ExportKind::Enums), Some(&"enums.yaml".to_string()));

        // Round-trip through serde back to YAML and re-parse — the closed
        // enums must survive a full serialize/deserialize cycle unchanged.
        let rendered = serde_yaml::to_string(&m).expect("serializes");
        let reparsed: PluginManifest = serde_yaml::from_str(&rendered).expect("re-parses its own output");
        assert_eq!(reparsed.kind, PluginKind::Core);
        assert_eq!(reparsed.exports, m.exports);
    }

    #[test]
    fn plugin_manifest_rejects_an_unknown_kind() {
        let yaml = "id: consumer.extra\nversion: \"0.1.0\"\nkind: capability\nexports: {}\n";
        let err = serde_yaml::from_str::<PluginManifest>(yaml).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("kind") || msg.contains("variant"), "expected a clear unknown-variant diagnostic, got: {msg}");
    }

    #[test]
    fn plugin_manifest_rejects_an_unknown_export_key() {
        let yaml = "id: consumer.extra\nversion: \"0.1.0\"\nkind: project\nexports:\n  docs: docs/\n";
        let err = serde_yaml::from_str::<PluginManifest>(yaml).unwrap_err();
        let msg = err.to_string();
        assert!(msg.contains("docs") || msg.contains("variant"), "expected a clear unknown-export diagnostic, got: {msg}");
    }
}
