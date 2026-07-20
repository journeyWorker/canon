//! [`PluginSnapshot`] (design.md D2, tasks.md 1.2): the resolved,
//! validated overlay declarations folded from every `canon/plugins/<id>/
//! plugin.yaml` under a project directory --
//! [`crate::resolve_plugin_snapshot::resolve_plugin_snapshot`]'s sole
//! output type. No `Deserialize` -- a snapshot is a RESOLUTION OUTPUT,
//! never authored input (mirrors `canon_vocab::manifest::snapshot::
//! CapabilitySnapshot`'s own doc comment).

use std::collections::BTreeMap;

use crate::manifest::schema::FieldDecl;

/// One resolved, validated overlay declaration -- `identity`
/// (`<namespace>.<kind>`) is the on-disk `kind=` directory string a
/// future P2 `write_namespaced`/`scan_namespaced_kind` (out of this
/// change's scope) would target.
#[derive(Clone, Debug, PartialEq)]
pub struct OverlayDecl {
    pub namespace: String,
    pub kind: String,
    /// `<namespace>.<kind>` -- this overlay's on-disk identity string,
    /// already validated (kebab-token grammar, no core-kind collision,
    /// no duplicate) by [`crate::manifest::resolve::assemble_snapshot`].
    pub identity: String,
    /// s16 supports `"scenario"` only (tasks.md 1.4). Carried as the raw
    /// manifest string, not `canon_model::RecordKind` -- a future
    /// generic projection over other core kinds (explicit non-goal of
    /// this change) would widen the SET of accepted values here without
    /// a schema change; today [`crate::manifest::resolve::
    /// assemble_snapshot`] guarantees it is always exactly `"scenario"`
    /// for any [`OverlayDecl`] that made it into a [`PluginSnapshot`].
    pub core_kind: String,
    pub join_key: Vec<String>,
    pub fields: Vec<FieldDecl>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ResolvedPlugin {
    pub namespace: String,
    /// The `identity` (`<namespace>.<kind>`) of every overlay THIS plugin
    /// declared -- the ownership key a consumer (`canon query --plugin
    /// <id>`) selects overlays by, never a namespace-equality guess that
    /// would also sweep in another plugin sharing the namespace. Populated
    /// only after `assemble_snapshot`'s cross-plugin identity-collision
    /// check, so it lists exactly this plugin's collision-free overlays.
    pub overlays: Vec<String>,
}

#[derive(Clone, Debug, Default, PartialEq)]
pub struct PluginSnapshot {
    pub plugins: BTreeMap<String, ResolvedPlugin>,
    /// Keyed by `identity` (`<namespace>.<kind>`) -- the SAME lookup key
    /// a future P2/P3 write/read path resolves an overlay declaration
    /// by, never re-derived independently.
    pub overlays: BTreeMap<String, OverlayDecl>,
}

impl PluginSnapshot {
    pub fn overlay(&self, identity: &str) -> Option<&OverlayDecl> {
        self.overlays.get(identity)
    }

    pub fn is_empty(&self) -> bool {
        self.plugins.is_empty() && self.overlays.is_empty()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_snapshot_is_empty() {
        let snap = PluginSnapshot::default();
        assert!(snap.is_empty());
        assert!(snap.overlay("porting.coverage").is_none());
    }

    #[test]
    fn overlay_lookup_is_by_identity() {
        let mut snap = PluginSnapshot::default();
        let decl = OverlayDecl {
            namespace: "porting".to_string(),
            kind: "coverage".to_string(),
            identity: "porting.coverage".to_string(),
            core_kind: "scenario".to_string(),
            join_key: vec!["project_id".to_string(), "scenario_id".to_string()],
            fields: Vec::new(),
        };
        snap.overlays.insert(decl.identity.clone(), decl.clone());
        assert_eq!(snap.overlay("porting.coverage"), Some(&decl));
        assert!(!snap.is_empty());
    }
}
