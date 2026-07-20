//! Resolution-time validation + snapshot assembly (design.md D1/D2,
//! tasks.md 1.2/1.4; `plugin-overlay-registry` spec): folds every
//! [`LoadedPlugin`] the loader accepted into a [`PluginSnapshot`],
//! rejecting (as diagnostics, never a panic) any plugin whose manifest
//! fails ANY of: the kebab-token grammar (`namespace` or an overlay
//! `kind`), a non-`scenario` `attaches_to.core_kind`, an overlay
//! identity (`<namespace>.<kind>`) colliding with a core
//! `RecordKind::as_str()` value, or an overlay identity duplicated
//! (within the SAME manifest, or against an earlier-resolved plugin).
//!
//! # Per-plugin atomicity
//!
//! A plugin package resolves as ONE atomic unit -- every spec.md
//! scenario for this requirement excludes the WHOLE owning plugin on any
//! single defect (a bad namespace, one bad overlay kind, one unsupported
//! `core_kind`), never just the offending overlay. So a consumer of
//! [`PluginSnapshot`] never observes a partially-valid plugin: some of
//! its declared overlays present, others silently missing because one
//! sibling overlay happened to be malformed.
//!
//! Combines what canon-vocab splits across `manifest::resolve` (plugin
//! activation) and `manifest::assemble` (folding active plugins into a
//! snapshot) -- canon-plugin has no separate activation step (every
//! installed package is always active; there is no profile that
//! selectively enables a subset), so there is nothing left to split.

use std::collections::{BTreeMap, BTreeSet};

use canon_model::RecordKind;

use crate::diagnostic::{Diagnostic, E_PLUGIN_CORE_COLLISION, E_PLUGIN_CORE_KIND, E_PLUGIN_DUP_OVERLAY, E_PLUGIN_EMPTY, E_PLUGIN_GRAMMAR};
use crate::manifest::grammar::is_kebab_token;
use crate::manifest::loader::LoadedPlugin;
use crate::manifest::snapshot::{OverlayDecl, PluginSnapshot, ResolvedPlugin};

/// s16 supports exactly this `attaches_to.core_kind` (tasks.md 1.4) -- a
/// generic projection over other core kinds is explicit FUTURE work
/// (`plugin-overlay-projection` spec).
pub const SUPPORTED_CORE_KIND: &str = "scenario";

/// `<namespace>.<kind>` colliding with a core `RecordKind::as_str()`
/// value -- a bare string comparison against every one of the twelve
/// closed kinds (design.md R5). Under the kebab-token grammar
/// (`namespace`/`kind` both `[a-z0-9]+(-[a-z0-9]+)*`, no `.` allowed),
/// the joined identity always contains exactly one `.` while every
/// `RecordKind::as_str()` value is dot-free -- so this specific
/// literal-equality path is unreachable through a grammar-valid
/// manifest today. Kept anyway as the resolution-time half of
/// design.md D2/R5's defense in depth (the write-time half, P2's
/// `write_namespaced`, out of this change's scope, checks the identical
/// condition again at write time); this module's own unit test
/// exercises the function directly for that reason, rather than
/// attempting to construct a dot-free identity through a full manifest.
pub fn identity_collides_with_core_kind(identity: &str) -> bool {
    RecordKind::ALL.iter().any(|k| k.as_str() == identity)
}

fn diag(code: &str, message: impl Into<String>, subject: impl Into<String>) -> Diagnostic {
    Diagnostic::error(code, message, subject)
}

/// Validate one loaded plugin's manifest, returning its resolved
/// overlays iff EVERY overlay (and the manifest's own `namespace`)
/// passes every resolution-time check -- otherwise `Err` with every
/// violation found, so the caller can report all of them at once rather
/// than stopping at the first. Pure, never panics.
fn validate_plugin(loaded: &LoadedPlugin) -> Result<Vec<OverlayDecl>, Vec<Diagnostic>> {
    let subject = format!("canon/plugins/{}/plugin.yaml", loaded.manifest.id);
    let mut errs = Vec::new();

    if !is_kebab_token(&loaded.manifest.namespace) {
        errs.push(diag(
            E_PLUGIN_GRAMMAR,
            format!("namespace `{}` does not match [a-z0-9]+(-[a-z0-9]+)*", loaded.manifest.namespace),
            subject.clone(),
        ));
    }

    if loaded.manifest.overlays.is_empty() {
        errs.push(diag(E_PLUGIN_EMPTY, "manifest declares no overlays -- one or more required", subject.clone()));
    }

    let mut decls = Vec::new();
    let mut seen_kinds: BTreeSet<&str> = BTreeSet::new();
    for entry in &loaded.manifest.overlays {
        if !is_kebab_token(&entry.kind) {
            errs.push(diag(
                E_PLUGIN_GRAMMAR,
                format!("overlay kind `{}` does not match [a-z0-9]+(-[a-z0-9]+)*", entry.kind),
                subject.clone(),
            ));
            continue;
        }
        if entry.attaches_to.core_kind != SUPPORTED_CORE_KIND {
            errs.push(diag(
                E_PLUGIN_CORE_KIND,
                format!("core_kind `{}` is unsupported -- s16 projects onto `scenario` only", entry.attaches_to.core_kind),
                subject.clone(),
            ));
            continue;
        }
        let identity = format!("{}.{}", loaded.manifest.namespace, entry.kind);
        if identity_collides_with_core_kind(&identity) {
            errs.push(diag(E_PLUGIN_CORE_COLLISION, format!("overlay identity `{identity}` collides with a core RecordKind"), subject.clone()));
            continue;
        }
        if !seen_kinds.insert(entry.kind.as_str()) {
            errs.push(diag(E_PLUGIN_DUP_OVERLAY, format!("duplicate overlay kind `{}` declared twice in the same plugin", entry.kind), subject.clone()));
            continue;
        }
        if entry.attaches_to.join_key.is_empty() {
            errs.push(diag(
                E_PLUGIN_EMPTY,
                format!("overlay kind `{}` declares an empty attaches_to.join_key -- one or more join-key field(s) required", entry.kind),
                subject.clone(),
            ));
            continue;
        }
        decls.push(OverlayDecl {
            namespace: loaded.manifest.namespace.clone(),
            kind: entry.kind.clone(),
            identity,
            core_kind: entry.attaches_to.core_kind.clone(),
            join_key: entry.attaches_to.join_key.clone(),
            fields: entry.fields.clone(),
        });
    }

    if errs.is_empty() { Ok(decls) } else { Err(errs) }
}

/// Fold every loaded plugin into one [`PluginSnapshot`], rejecting (as
/// diagnostics) any plugin whose own manifest fails resolution
/// ([`validate_plugin`]), and any plugin whose overlay identity collides
/// with an EARLIER plugin's already-accepted overlay (iteration order:
/// ascending plugin id, `installed`'s own `BTreeMap` order -- a
/// deterministic, documented tie-break, distinct from `load_plugins_dir`'s
/// directory-sort tie-break for same-`id` duplicates). Never panics,
/// never a partial merge of one plugin's overlays.
pub fn assemble_snapshot(installed: &BTreeMap<String, LoadedPlugin>) -> (PluginSnapshot, Vec<Diagnostic>) {
    let mut snapshot = PluginSnapshot::default();
    let mut diags = Vec::new();

    for (id, loaded) in installed {
        match validate_plugin(loaded) {
            Ok(decls) => {
                if let Some(collision) = decls.iter().find(|d| snapshot.overlays.contains_key(&d.identity)) {
                    diags.push(diag(
                        E_PLUGIN_DUP_OVERLAY,
                        format!("overlay identity `{}` already declared by another plugin -- dropping plugin `{id}`", collision.identity),
                        id.clone(),
                    ));
                    continue;
                }
                let owned: Vec<String> = decls.iter().map(|d| d.identity.clone()).collect();
                snapshot.plugins.insert(id.clone(), ResolvedPlugin { namespace: loaded.manifest.namespace.clone(), overlays: owned });
                for decl in decls {
                    snapshot.overlays.insert(decl.identity.clone(), decl);
                }
            }
            Err(mut e) => diags.append(&mut e),
        }
    }

    (snapshot, diags)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::schema::{AttachesTo, FieldDecl, OverlayEntry, PluginManifest};
    use crate::manifest::types::Type;
    use std::path::PathBuf;

    fn plugin(id: &str, namespace: &str, overlays: Vec<OverlayEntry>) -> LoadedPlugin {
        LoadedPlugin {
            manifest: PluginManifest { id: id.to_string(), namespace: namespace.to_string(), overlays },
            dir: PathBuf::from(format!("canon/plugins/{id}")),
        }
    }

    fn scenario_overlay(kind: &str) -> OverlayEntry {
        OverlayEntry {
            kind: kind.to_string(),
            attaches_to: AttachesTo { core_kind: "scenario".to_string(), join_key: vec!["project_id".to_string(), "scenario_id".to_string()] },
            fields: vec![FieldDecl { name: "covered".to_string(), ty: Type::Bool }],
        }
    }

    #[test]
    fn identity_collides_with_core_kind_matches_a_bare_record_kind_string() {
        // Unreachable via a grammar-valid manifest (module doc) -- exercised
        // directly against the pure comparison function instead.
        assert!(identity_collides_with_core_kind("scenario"));
        assert!(identity_collides_with_core_kind("task"));
        assert!(!identity_collides_with_core_kind("porting.coverage"));
    }

    #[test]
    fn a_well_formed_plugin_resolves_its_declared_shape_exactly() {
        let mut installed = BTreeMap::new();
        installed.insert("porting".to_string(), plugin("porting", "porting", vec![scenario_overlay("coverage")]));
        let (snap, diags) = assemble_snapshot(&installed);
        assert!(diags.is_empty(), "diags: {diags:?}");
        assert!(snap.plugins.contains_key("porting"));
        let decl = snap.overlay("porting.coverage").expect("overlay present");
        assert_eq!(decl.core_kind, "scenario");
        assert_eq!(decl.join_key, vec!["project_id".to_string(), "scenario_id".to_string()]);
        assert_eq!(decl.fields.len(), 1);
    }

    #[test]
    fn a_grammar_violating_namespace_excludes_the_whole_plugin() {
        let mut installed = BTreeMap::new();
        installed.insert("bad".to_string(), plugin("bad", "Porting_Two", vec![scenario_overlay("coverage")]));
        let (snap, diags) = assemble_snapshot(&installed);
        assert!(snap.is_empty());
        assert!(diags.iter().any(|d| d.code == E_PLUGIN_GRAMMAR));
    }

    #[test]
    fn a_grammar_violating_overlay_kind_excludes_the_whole_plugin() {
        let mut installed = BTreeMap::new();
        installed.insert("bad".to_string(), plugin("bad", "porting", vec![scenario_overlay("coverage/extra")]));
        let (snap, diags) = assemble_snapshot(&installed);
        assert!(snap.is_empty());
        assert!(diags.iter().any(|d| d.code == E_PLUGIN_GRAMMAR));
    }

    #[test]
    fn a_non_scenario_core_kind_excludes_the_whole_plugin() {
        let mut overlay = scenario_overlay("coverage");
        overlay.attaches_to.core_kind = "task".to_string();
        let mut installed = BTreeMap::new();
        installed.insert("bad".to_string(), plugin("bad", "porting", vec![overlay]));
        let (snap, diags) = assemble_snapshot(&installed);
        assert!(snap.is_empty());
        assert!(diags.iter().any(|d| d.code == E_PLUGIN_CORE_KIND));
    }

    #[test]
    fn two_overlays_with_the_same_kind_in_one_manifest_excludes_the_whole_plugin() {
        let mut installed = BTreeMap::new();
        installed.insert("bad".to_string(), plugin("bad", "porting", vec![scenario_overlay("coverage"), scenario_overlay("coverage")]));
        let (snap, diags) = assemble_snapshot(&installed);
        assert!(snap.is_empty());
        assert!(diags.iter().any(|d| d.code == E_PLUGIN_DUP_OVERLAY));
    }

    #[test]
    fn a_manifest_with_no_overlays_excludes_the_whole_plugin() {
        let mut installed = BTreeMap::new();
        installed.insert("empty".to_string(), plugin("empty", "porting", vec![]));
        let (snap, diags) = assemble_snapshot(&installed);
        assert!(snap.is_empty());
        assert!(diags.iter().any(|d| d.code == E_PLUGIN_EMPTY));
    }

    #[test]
    fn an_overlay_with_an_empty_join_key_excludes_the_whole_plugin() {
        let mut overlay = scenario_overlay("coverage");
        overlay.attaches_to.join_key = vec![];
        let mut installed = BTreeMap::new();
        installed.insert("bad".to_string(), plugin("bad", "porting", vec![overlay]));
        let (snap, diags) = assemble_snapshot(&installed);
        assert!(snap.is_empty());
        assert!(diags.iter().any(|d| d.code == E_PLUGIN_EMPTY));
    }

    #[test]
    fn two_different_plugins_declaring_the_same_identity_drops_the_later_one() {
        let mut installed = BTreeMap::new();
        installed.insert("plugin-a".to_string(), plugin("plugin-a", "porting", vec![scenario_overlay("coverage")]));
        installed.insert("plugin-b".to_string(), plugin("plugin-b", "porting", vec![scenario_overlay("coverage")]));
        let (snap, diags) = assemble_snapshot(&installed);
        // BTreeMap iterates "plugin-a" before "plugin-b" -- the first
        // survives, the second is dropped with a diagnostic.
        assert!(snap.plugins.contains_key("plugin-a"));
        assert!(!snap.plugins.contains_key("plugin-b"));
        assert_eq!(snap.overlays.len(), 1);
        assert!(diags.iter().any(|d| d.code == E_PLUGIN_DUP_OVERLAY));
    }

    #[test]
    fn an_empty_installed_map_resolves_an_empty_valid_snapshot() {
        let (snap, diags) = assemble_snapshot(&BTreeMap::new());
        assert!(snap.is_empty());
        assert!(diags.is_empty());
    }
}
