//! Raw `plugin.yaml` deserialization shapes (design.md D2, tasks.md 1.1;
//! `plugin-overlay-registry` spec): `{id, namespace, overlays: [{kind,
//! attaches_to: {core_kind, join_key}, fields: [{name, type}]}]}`.
//!
//! A required field carrying no `#[serde(default)]` fails
//! `serde_yaml::from_str` outright when absent from the YAML document --
//! "a manifest missing any of `id`/`namespace`/`overlays[].kind`/
//! `attaches_to.core_kind`/`attaches_to.join_key` SHALL fail to load...
//! never silently defaulted" (spec.md) falls out of ordinary serde
//! behavior here, no extra procedural check needed. `fields` is the one
//! overlay-entry field the spec does NOT list as required -- an overlay
//! with no declared fields is a legal (if pointless) manifest, so it
//! defaults to empty.

use serde::{Deserialize, Serialize};

use crate::manifest::types::Type;

/// A `canon/plugins/<id>/plugin.yaml` document, exactly as authored --
/// this is PARSE INPUT, not the resolved/validated shape
/// ([`crate::manifest::snapshot::PluginSnapshot`]/[`crate::manifest::
/// snapshot::OverlayDecl`] is that). `attaches_to.core_kind`'s grammar
/// and support ([`crate::manifest::grammar`]/`core_kind == "scenario"`)
/// and every overlay identity's uniqueness are resolution-time checks
/// ([`crate::manifest::resolve`]), not parse-time ones -- a manifest
/// naming an unsupported-but-otherwise-valid `core_kind` (e.g. `task`)
/// still parses cleanly here and fails later with a specific,
/// actionable diagnostic rather than an opaque serde error.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct PluginManifest {
    pub id: String,
    pub namespace: String,
    pub overlays: Vec<OverlayEntry>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct OverlayEntry {
    pub kind: String,
    pub attaches_to: AttachesTo,
    #[serde(default)]
    pub fields: Vec<FieldDecl>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct AttachesTo {
    /// A bare `String`, deliberately NOT `canon_model::RecordKind` --
    /// see this module's doc comment: unsupported-but-real core kinds
    /// must still parse, to yield a resolution-time diagnostic instead
    /// of an opaque parse failure.
    pub core_kind: String,
    pub join_key: Vec<String>,
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct FieldDecl {
    pub name: String,
    #[serde(rename = "type")]
    pub ty: Type,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn porting_manifest() -> PluginManifest {
        PluginManifest {
            id: "porting".to_string(),
            namespace: "porting".to_string(),
            overlays: vec![OverlayEntry {
                kind: "coverage".to_string(),
                attaches_to: AttachesTo {
                    core_kind: "scenario".to_string(),
                    join_key: vec!["project_id".to_string(), "scenario_id".to_string()],
                },
                fields: vec![
                    FieldDecl { name: "covered".to_string(), ty: Type::Bool },
                    FieldDecl { name: "surface_ref".to_string(), ty: Type::List(Box::new(Type::Str)) },
                ],
            }],
        }
    }

    #[test]
    fn manifest_round_trips_through_yaml() {
        let manifest = porting_manifest();
        let yaml = serde_yaml::to_string(&manifest).expect("serializes");
        let parsed: PluginManifest = serde_yaml::from_str(&yaml).expect("deserializes");
        assert_eq!(parsed, manifest);
    }

    #[test]
    fn manifest_parses_from_hand_authored_yaml() {
        let yaml = "id: porting\nnamespace: porting\noverlays:\n  - kind: coverage\n    attaches_to:\n      core_kind: scenario\n      join_key: [project_id, scenario_id]\n    fields:\n      - name: covered\n        type: bool\n      - name: surface_ref\n        type: { list: string }\n";
        let parsed: PluginManifest = serde_yaml::from_str(yaml).expect("parses");
        assert_eq!(parsed, porting_manifest());
    }

    #[test]
    fn a_missing_join_key_fails_to_parse() {
        let yaml = "id: bad\nnamespace: bad\noverlays:\n  - kind: coverage\n    attaches_to:\n      core_kind: scenario\n";
        assert!(serde_yaml::from_str::<PluginManifest>(yaml).is_err());
    }

    #[test]
    fn a_missing_namespace_fails_to_parse() {
        let yaml = "id: bad\noverlays: []\n";
        assert!(serde_yaml::from_str::<PluginManifest>(yaml).is_err());
    }

    #[test]
    fn an_overlay_with_no_fields_defaults_to_empty() {
        let yaml = "id: bare\nnamespace: bare\noverlays:\n  - kind: coverage\n    attaches_to:\n      core_kind: scenario\n      join_key: [project_id]\n";
        let parsed: PluginManifest = serde_yaml::from_str(yaml).expect("parses");
        assert!(parsed.overlays[0].fields.is_empty());
    }
}
