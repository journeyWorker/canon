//! Retargeted from the donor manifest layer's multi-plugin snapshot
//! assembly, pruned to directives + enums (D2 Non-Goals drop every other
//! export kind, see `crate::manifest::loader` module doc) and dropping the
//! donor's reserved directive-name/timing-attr sets — those
//! reserve the donor's own built-in scene tags (`scene`/`cut`/`on`/…) and
//! timing attrs (`duration`/`delay`/`wait`), none of which canon's directive
//! vocabulary has (no scene-DSL grammar, no timeline clips).

use std::collections::{BTreeMap, BTreeSet};

use crate::manifest::resolve::{ActivePlugin, InstalledPlugins, CORE_PLUGIN_ID};
use crate::manifest::snapshot::{CapabilitySnapshot, ResolvedPlugin};

#[derive(Clone, Debug, PartialEq)]
pub enum AssembleError {
    DuplicateAcrossPlugins { kind: String, id: String, second: String },
    MissingActivePlugin { id: String },
}

impl AssembleError {
    pub fn code(&self) -> &'static str {
        match self {
            AssembleError::DuplicateAcrossPlugins { .. } => "E-DUP-ACROSS-PLUGINS",
            AssembleError::MissingActivePlugin { .. } => "E-MISSING-ACTIVE-PLUGIN",
        }
    }
}

/// Merge every ACTIVE plugin's loaded package into one deterministic
/// capability snapshot. An offending (duplicate/missing) item is dropped,
/// never merged; the `inactive` index is populated from installed-minus-
/// active. Never panics.
pub fn assemble_snapshot(active: &[ActivePlugin], installed: &InstalledPlugins) -> (CapabilitySnapshot, Vec<AssembleError>) {
    let mut snap = CapabilitySnapshot::default();
    let mut errs = Vec::new();
    let mut dir_owner: BTreeMap<String, String> = BTreeMap::new();

    for ap in active {
        let Some(inst) = installed.get(&ap.id) else {
            // `canon.core` is synthetic-always-active even when not found on
            // disk (a misconfigured repo without a canon.core plugin still
            // resolves — empty directives/enums, never a panic); any other
            // missing active id (named by a profile, not canon.core) is a
            // real error.
            if ap.id != CORE_PLUGIN_ID {
                errs.push(AssembleError::MissingActivePlugin { id: ap.id.clone() });
            }
            continue;
        };
        let pkg = &inst.loaded;

        for d in &pkg.directives {
            if let Some(first) = dir_owner.get(&d.name) {
                errs.push(AssembleError::DuplicateAcrossPlugins { kind: "directive".into(), id: d.name.clone(), second: format!("{first} vs {}", ap.id) });
                continue;
            }
            dir_owner.insert(d.name.clone(), ap.id.clone());
            snap.directives.insert(d.name.clone(), d.clone());
        }
        merge_map(&mut snap.enums, pkg.enums.iter().map(|(k, v)| (k.clone(), v.clone())), "enum", &ap.id, &mut errs);

        snap.plugins.insert(ap.id.clone(), ResolvedPlugin { version: pkg.manifest.version.clone(), options: ap.options.clone() });
    }

    let active_ids: BTreeSet<&str> = active.iter().map(|a| a.id.as_str()).collect();
    for (id, inst) in &installed.by_id {
        if active_ids.contains(id.as_str()) {
            continue;
        }
        for d in &inst.loaded.directives {
            snap.inactive.entry(d.name.clone()).or_insert_with(|| id.clone());
        }
    }

    (snap, errs)
}

fn merge_map<V: Clone>(dst: &mut BTreeMap<String, V>, items: impl Iterator<Item = (String, V)>, kind: &str, plugin: &str, errs: &mut Vec<AssembleError>) {
    for (k, v) in items {
        match dst.entry(k) {
            std::collections::btree_map::Entry::Occupied(e) => {
                errs.push(AssembleError::DuplicateAcrossPlugins { kind: kind.into(), id: e.key().clone(), second: plugin.into() });
            }
            std::collections::btree_map::Entry::Vacant(e) => {
                e.insert(v);
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::manifest::loader::LoadedPlugin;
    use crate::manifest::resolve::InstalledPlugin;
    use crate::manifest::schema::{DirectiveDecl, PluginKind, PluginManifest};
    use crate::manifest::types::AttrDecl;

    fn plugin(id: &str, directives: Vec<DirectiveDecl>) -> InstalledPlugin {
        InstalledPlugin {
            loaded: LoadedPlugin {
                manifest: PluginManifest { id: id.into(), version: "0.1.0".into(), kind: PluginKind::Core, depends: vec![], exports: Default::default(), options: vec![] },
                directives,
                enums: Default::default(),
            },
        }
    }

    #[test]
    fn a_well_formed_plugin_resolves_to_a_directive_index() {
        let d = DirectiveDecl { name: "task".into(), attrs: vec![AttrDecl { name: "desc".into(), required: true, ty: crate::manifest::types::Type::Str, default: None }] };
        let mut installed = InstalledPlugins::default();
        installed.by_id.insert(CORE_PLUGIN_ID.into(), plugin(CORE_PLUGIN_ID, vec![d]));
        let active = vec![ActivePlugin { id: CORE_PLUGIN_ID.into(), options: Default::default() }];
        let (snap, errs) = assemble_snapshot(&active, &installed);
        assert!(errs.is_empty());
        assert!(snap.directive("task").is_some());
    }

    #[test]
    fn missing_canon_core_on_disk_never_panics() {
        let installed = InstalledPlugins::default();
        let active = vec![ActivePlugin { id: CORE_PLUGIN_ID.into(), options: Default::default() }];
        let (snap, errs) = assemble_snapshot(&active, &installed);
        assert!(errs.is_empty(), "canon.core missing-on-disk is tolerated, not an assembly error");
        assert!(snap.directives.is_empty());
    }
}
