//! Retargeted from the donor manifest layer's plugin activation
//! resolution (profile `extends` chain + `depends` closure + cycle
//! detection). One simplification from the donor's shape: no "scene-local
//! `plugins:` frontmatter override" layer — canon's [`resolve_activation`]
//! resolves a project + a profile name only (design.md D3's frozen
//! `resolve_snapshot(project_dir, profile)` signature has no third,
//! per-document override parameter; the donor's own `scene_profile`/
//! `scene_plugins` exist because a donor scene can locally override its
//! project's defaults, which canon's flat task-atom/handoff-body files never
//! do). `canon.core` plays the donor core plugin's "always first,
//! language-required" role — always active regardless
//! of what a profile declares.

use std::collections::BTreeMap;

use crate::manifest::types::Literal;

pub const CORE_PLUGIN_ID: &str = "canon.core";

#[derive(Clone, Debug)]
pub struct InstalledPlugin {
    pub loaded: crate::manifest::loader::LoadedPlugin,
}

#[derive(Clone, Debug, Default)]
pub struct InstalledPlugins {
    pub by_id: BTreeMap<String, InstalledPlugin>,
}

impl InstalledPlugins {
    pub fn get(&self, id: &str) -> Option<&InstalledPlugin> {
        self.by_id.get(id)
    }
}

pub type ActivationMap = BTreeMap<String, BTreeMap<String, Literal>>;

#[derive(Clone, Debug)]
pub struct Profile {
    pub extends: Option<String>,
    pub plugins: ActivationMap,
}

#[derive(Clone, Debug)]
pub struct ProfileGraph {
    pub profiles: BTreeMap<String, Profile>,
    pub default_profile: String,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ActivePlugin {
    pub id: String,
    pub options: BTreeMap<String, Literal>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum ResolveError {
    UnknownProfile(String),
    ExtendsCycle(String),
    UnresolvedDepends { plugin: String, dep: String },
    DependsVersionMismatch { plugin: String, dep: String, need: String, found: String },
    DependsCycle(String),
}

impl ResolveError {
    pub fn code(&self) -> &'static str {
        match self {
            ResolveError::UnknownProfile(_) => "E-PROFILE-UNKNOWN",
            ResolveError::ExtendsCycle(_) => "E-PROFILE-EXTENDS-CYCLE",
            ResolveError::UnresolvedDepends { .. } => "E-DEPENDS-UNRESOLVED",
            ResolveError::DependsVersionMismatch { .. } => "E-DEPENDS-VERSION",
            ResolveError::DependsCycle(_) => "E-DEPENDS-CYCLE",
        }
    }
}

impl ProfileGraph {
    /// A default, empty graph: only `canon.core` (always active) resolves.
    pub fn empty(default_profile: impl Into<String>) -> Self {
        Self { profiles: BTreeMap::new(), default_profile: default_profile.into() }
    }

    fn extends_chain(&self, selected: &str) -> Result<Vec<String>, ResolveError> {
        let mut chain = Vec::new();
        let mut seen = std::collections::BTreeSet::new();
        let mut cur = Some(selected.to_string());
        while let Some(name) = cur {
            if !self.profiles.contains_key(&name) {
                return Err(ResolveError::UnknownProfile(name));
            }
            if !seen.insert(name.clone()) {
                return Err(ResolveError::ExtendsCycle(name));
            }
            chain.push(name.clone());
            cur = self.profiles[&name].extends.clone();
        }
        chain.reverse();
        Ok(chain)
    }
}

/// plugin §11.1 resolution order (lifted, minus the scene-local layer — see
/// module doc) + §11.2 merge. If `graph` has no profile at all (the
/// `canon.project.yaml`-absent case), `selected` resolves against an empty
/// chain and only `canon.core` activates.
pub fn resolve_activation(graph: &ProfileGraph, selected: &str, installed: &InstalledPlugins) -> Result<Vec<ActivePlugin>, ResolveError> {
    let mut order: Vec<String> = Vec::new();
    let mut merged: BTreeMap<String, BTreeMap<String, Literal>> = BTreeMap::new();

    let apply = |acts: &ActivationMap, order: &mut Vec<String>, merged: &mut BTreeMap<String, BTreeMap<String, Literal>>| {
        for (id, opts) in acts {
            if !merged.contains_key(id) {
                order.push(id.clone());
            }
            let entry = merged.entry(id.clone()).or_default();
            for (k, v) in opts {
                match (entry.get_mut(k), v) {
                    (Some(Literal::Map(dst)), Literal::Map(src)) => merge_map(dst, src),
                    _ => {
                        entry.insert(k.clone(), v.clone());
                    }
                }
            }
        }
    };

    // 1. canon.core is always first (language-required, mirrors the donor core plugin).
    order.push(CORE_PLUGIN_ID.into());
    merged.insert(CORE_PLUGIN_ID.into(), BTreeMap::new());

    // 2. profiles.global
    if let Some(g) = graph.profiles.get("global") {
        apply(&g.plugins, &mut order, &mut merged);
    }
    // 3. extends chain (parent-first) then selected -- skipped entirely when
    //    `graph` has NO profiles at all (no `canon.project.yaml` was found,
    //    `crate::resolve_snapshot`'s "core-only" default): resolves to
    //    core-only without an `UnknownProfile` error for that specific case.
    //    A NON-empty graph still errors on a genuinely unknown `selected`.
    if !graph.profiles.is_empty() {
        for name in graph.extends_chain(selected)? {
            if name == "global" {
                continue;
            }
            apply(&graph.profiles[&name].plugins, &mut order, &mut merged);
        }
    }

    // 4. Dependency closure (plugin §11.1 step 6): transitively activate every
    //    `depends` of an active plugin, deterministic (sorted-id) order.
    let mut queue: Vec<String> = order.clone();
    while let Some(id) = queue.pop() {
        let Some(inst) = installed.get(&id) else { continue };
        let mut deps = inst.loaded.manifest.depends.clone();
        deps.sort_by(|a, b| a.id.cmp(&b.id));
        for dep in deps {
            match installed.get(&dep.id) {
                None if dep.id == CORE_PLUGIN_ID => {}
                None => return Err(ResolveError::UnresolvedDepends { plugin: id.clone(), dep: dep.id.clone() }),
                Some(dep_inst) => {
                    if !range_satisfies(&dep.range, &dep_inst.loaded.manifest.version) {
                        return Err(ResolveError::DependsVersionMismatch {
                            plugin: id.clone(),
                            dep: dep.id.clone(),
                            need: dep.range.clone(),
                            found: dep_inst.loaded.manifest.version.clone(),
                        });
                    }
                }
            }
            if !merged.contains_key(&dep.id) {
                order.push(dep.id.clone());
                merged.insert(dep.id.clone(), BTreeMap::new());
                queue.push(dep.id.clone());
            }
        }
    }
    detect_depends_cycle(&order, installed)?;

    Ok(order.into_iter().map(|id| ActivePlugin { options: merged.remove(&id).unwrap_or_default(), id }).collect())
}

fn merge_map(dst: &mut BTreeMap<String, Literal>, src: &BTreeMap<String, Literal>) {
    for (k, v) in src {
        match (dst.get_mut(k), v) {
            (Some(Literal::Map(d)), Literal::Map(s)) => merge_map(d, s),
            _ => {
                dst.insert(k.clone(), v.clone());
            }
        }
    }
}

/// Detect a cycle in the `depends` graph restricted to activated plugins.
/// Iterative DFS with visiting/done marks; deterministic (roots in `order`,
/// deps sorted) — never a panic, never unbounded recursion.
fn detect_depends_cycle(order: &[String], installed: &InstalledPlugins) -> Result<(), ResolveError> {
    let mut visiting = std::collections::BTreeSet::new();
    let mut done = std::collections::BTreeSet::new();

    for root in order {
        if done.contains(root) {
            continue;
        }
        let mut stack: Vec<(String, usize)> = vec![(root.clone(), 0)];
        visiting.insert(root.clone());
        while let Some((node, mut ix)) = stack.pop() {
            let mut deps: Vec<String> = installed.get(&node).map(|i| i.loaded.manifest.depends.iter().map(|d| d.id.clone()).collect()).unwrap_or_default();
            deps.sort();
            let mut advanced = false;
            while ix < deps.len() {
                let dep = deps[ix].clone();
                ix += 1;
                if visiting.contains(&dep) {
                    return Err(ResolveError::DependsCycle(dep));
                }
                if !done.contains(&dep) {
                    stack.push((node.clone(), ix));
                    visiting.insert(dep.clone());
                    stack.push((dep, 0));
                    advanced = true;
                    break;
                }
            }
            if !advanced {
                visiting.remove(&node);
                done.insert(node);
            }
        }
    }
    Ok(())
}

/// Minimal semver-range check for `depends` (lifted from the donor manifest
/// layer): a bare exact version, or a caret range (`^x.y.z`).
/// Pre-1.0 the caret pins to the leftmost non-zero component. An unparseable
/// range/version is NOT satisfied (conservative).
fn range_satisfies(range: &str, version: &str) -> bool {
    fn parts(s: &str) -> Option<(u64, u64, u64)> {
        let mut it = s.split('.');
        let major = it.next()?.parse().ok()?;
        let minor = it.next()?.parse().ok()?;
        let patch = it.next()?.parse().ok()?;
        if it.next().is_some() {
            return None;
        }
        Some((major, minor, patch))
    }

    let Some(v) = parts(version) else { return false };
    if let Some(caret) = range.strip_prefix('^') {
        let Some((rmaj, rmin, rpat)) = parts(caret) else { return false };
        return if rmaj > 0 {
            v.0 == rmaj && (v.1, v.2) >= (rmin, rpat)
        } else if rmin > 0 {
            v.0 == 0 && v.1 == rmin && v.2 >= rpat
        } else {
            v == (0, 0, rpat)
        };
    }
    let Some(exact) = parts(range) else { return false };
    v == exact
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canon_core_is_always_active_even_with_no_profiles() {
        let graph = ProfileGraph::empty("default");
        let installed = InstalledPlugins::default();
        let active = resolve_activation(&graph, "default", &installed).unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].id, CORE_PLUGIN_ID);
    }

    #[test]
    fn unresolvable_depends_range_fails_with_a_diagnostic_not_a_panic() {
        use crate::manifest::loader::LoadedPlugin;
        use crate::manifest::schema::{Depends, PluginKind, PluginManifest};

        let mut installed = InstalledPlugins::default();
        installed.by_id.insert(
            CORE_PLUGIN_ID.into(),
            InstalledPlugin {
                loaded: LoadedPlugin {
                    manifest: PluginManifest {
                        id: CORE_PLUGIN_ID.into(),
                        version: "0.1.0".into(),
                        kind: PluginKind::Core,
                        depends: vec![Depends { id: "consumer.missing".into(), range: "^1.0.0".into() }],
                        exports: Default::default(),
                        options: vec![],
                    },
                    directives: vec![],
                    enums: Default::default(),
                },
            },
        );
        let graph = ProfileGraph::empty("default");
        let err = resolve_activation(&graph, "default", &installed).unwrap_err();
        assert_eq!(err.code(), "E-DEPENDS-UNRESOLVED");
    }

    #[test]
    fn range_satisfies_caret_pre_1_0_pins_leftmost_nonzero() {
        assert!(range_satisfies("^0.1.0", "0.1.5"));
        assert!(!range_satisfies("^0.1.0", "0.2.0"));
        assert!(range_satisfies("^1.2.0", "1.9.9"));
        assert!(!range_satisfies("^1.2.0", "2.0.0"));
        assert!(!range_satisfies("^1.2.0", "1.1.9"));
    }
}
