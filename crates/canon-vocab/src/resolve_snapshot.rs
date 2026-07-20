//! `resolve_snapshot` (design.md D3): THE single capability-snapshot
//! resolution entry point — retargeted from the donor manifest layer's
//! document-snapshot resolution + the donor CLI's build-input path (the
//! project-resolution half; the frontmatter/imports half has no canon
//! analog — task-atom files carry no per-document `profile:`/`plugins:`
//! override, see `crate::manifest::resolve` module doc). This crate's own
//! checker ([`crate::checker`]) and, later, S12's `canon context` and an LSP
//! all call this SAME function (D3's "no consumer computes its own partial
//! vocabulary view").
//!
//! Unlike the donor vocabulary system's core plugin (compile-time
//! `include_str!`-embedded, because the donor CLI
//! ships as a project-independent binary), `canon.core` is scanned from disk
//! at `<project_dir>/canon/vocab/canon.core/` exactly like any consumer
//! plugin (design.md D3: "resolving canon.core + any consumer `canon/vocab/
//! <id>/plugin.yaml`") — canon is a monorepo-embedded tool, not a
//! distributable language runtime.

use std::path::Path;

use crate::checker::Diagnostic;
use crate::manifest::loader::load_plugins_dir;
use crate::manifest::project::load_project;
use crate::manifest::resolve::{resolve_activation, ProfileGraph};
use crate::manifest::snapshot::CapabilitySnapshot;
use crate::span::Severity;

const DEFAULT_PROFILE: &str = "default";

fn resolve_diag(code: &str, message: impl Into<String>) -> Diagnostic {
    Diagnostic { code: code.to_string(), severity: Severity::Error, message: message.into(), subject: "canon.project.yaml".to_string() }
}

/// Resolve `project_dir`'s active vocabulary (canon.core + every consumer
/// `canon/vocab/<id>/plugin.yaml` a `canon.project.yaml` profile activates)
/// into a merged capability snapshot, plus every resolution diagnostic
/// (missing/malformed `canon.project.yaml`, plugin load errors, an
/// unresolvable `depends` range, an assembly-time duplicate). `profile`
/// selects a `canon.project.yaml` profile by name; `None` uses its
/// `defaultProfile` (or `"default"` when no `canon.project.yaml` exists at
/// all). Pure, total, NEVER panics — every failure degrades to a usable
/// (possibly core-only) snapshot plus a diagnostic, mirroring
/// `resolve_document_snapshot`'s exact fail-soft contract.
pub fn resolve_snapshot(project_dir: &Path, profile: Option<&str>) -> (CapabilitySnapshot, Vec<Diagnostic>) {
    let mut diags = Vec::new();

    let project = match load_project(project_dir) {
        Ok(p) => p,
        Err(e) => {
            diags.push(resolve_diag("E-PROJECT-MALFORMED", e));
            None
        }
    };

    let (vocab_dir, graph, selected_owned) = match &project {
        Some(p) => (p.vocab_dir.clone(), p.graph.clone(), profile.map(str::to_string).unwrap_or_else(|| p.graph.default_profile.clone())),
        None => (project_dir.join("canon/vocab/"), ProfileGraph::empty(DEFAULT_PROFILE), profile.map(str::to_string).unwrap_or_else(|| DEFAULT_PROFILE.to_string())),
    };
    let selected = selected_owned.as_str();

    let (installed, load_errs) = load_plugins_dir(&vocab_dir);
    diags.extend(load_errs.into_iter().map(|e| resolve_diag(e.code(), format!("{e:?}"))));

    let active = match resolve_activation(&graph, selected, &installed) {
        Ok(active) => active,
        Err(e) => {
            diags.push(resolve_diag(e.code(), format!("{e:?}")));
            // No conforming activation -> fall back to core-only so the
            // caller still gets a usable snapshot (mirrors
            // resolve_document_snapshot's own fallback).
            let fallback_graph = ProfileGraph::empty(DEFAULT_PROFILE);
            let active = resolve_activation(&fallback_graph, DEFAULT_PROFILE, &installed).unwrap_or_default();
            let (mut snap, assemble_errs) = crate::manifest::assemble::assemble_snapshot(&active, &installed);
            diags.extend(assemble_errs.into_iter().map(|e| resolve_diag(e.code(), format!("{e:?}"))));
            snap.evidence_kinds = crate::policy_bridge::evidence_kind_domain(project_dir);
            return (snap, diags);
        }
    };

    let (mut snapshot, assemble_errs) = crate::manifest::assemble::assemble_snapshot(&active, &installed);
    diags.extend(assemble_errs.into_iter().map(|e| resolve_diag(e.code(), format!("{e:?}"))));

    // D4: the evidence-kind domain is snapshot-dependent-but-not-manifest-
    // declared -- folded in after assembly, from S5's live policy.
    snapshot.evidence_kinds = crate::policy_bridge::evidence_kind_domain(project_dir);

    // `capability_version` folds the FULLY-resolved vocabulary (D3's
    // module doc, `crate::manifest::snapshot`'s own doc) -- must run LAST,
    // after `evidence_kinds` above, or it would hash a still-incomplete
    // snapshot.
    snapshot.capability_version = snapshot.compute_capability_version();

    (snapshot, diags)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, rel: &str, contents: &str) {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    fn write_canon_core(project_dir: &Path) {
        write(
            project_dir,
            "canon/vocab/canon.core/plugin.yaml",
            "id: canon.core\nversion: \"0.1.0\"\nkind: core\nexports:\n  directives: directives/\n  enums: enums.yaml\n",
        );
        write(
            project_dir,
            "canon/vocab/canon.core/directives/task.yaml",
            "directives:\n  - name: task\n    attrs:\n      - name: desc\n        type: string\n        required: true\n      - name: status\n        type: { domain: task-status }\n        required: true\n",
        );
        write(project_dir, "canon/vocab/canon.core/enums.yaml", "enums:\n  task-status: [open, done]\n");
    }

    #[test]
    fn a_repo_with_no_canon_project_yaml_resolves_canon_core_only() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_canon_core(tmp.path());
        let (snap, diags) = resolve_snapshot(tmp.path(), None);
        assert!(diags.is_empty(), "diags: {diags:?}");
        assert!(snap.directive("task").is_some());
        assert_eq!(snap.enums.get("task-status"), Some(&vec!["open".to_string(), "done".to_string()]));
    }

    #[test]
    fn missing_canon_core_never_panics_and_yields_an_empty_but_usable_snapshot() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (snap, diags) = resolve_snapshot(tmp.path(), None);
        assert!(diags.is_empty());
        assert!(snap.directives.is_empty());
        assert!(snap.evidence_kinds.is_empty());
    }

    #[test]
    fn an_unknown_profile_falls_back_to_core_only_with_a_diagnostic() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_canon_core(tmp.path());
        write(tmp.path(), "canon.project.yaml", "defaultProfile: default\nprofiles:\n  default:\n    plugins: {}\n");
        let (snap, diags) = resolve_snapshot(tmp.path(), Some("does-not-exist"));
        assert!(diags.iter().any(|d| d.code == "E-PROFILE-UNKNOWN"));
        // Still usable: canon.core resolved via the core-only fallback.
        assert!(snap.directive("task").is_some());
    }

    #[test]
    fn a_consumer_plugin_activated_by_a_profile_merges_into_the_snapshot() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_canon_core(tmp.path());
        write(
            tmp.path(),
            "canon/vocab/consumer.extra/plugin.yaml",
            "id: consumer.extra\nversion: \"0.1.0\"\nkind: project\nexports:\n  directives: directives/\n",
        );
        write(
            tmp.path(),
            "canon/vocab/consumer.extra/directives/extra.yaml",
            "directives:\n  - name: extra-task\n    attrs:\n      - name: note\n        type: string\n        required: false\n",
        );
        write(
            tmp.path(),
            "canon.project.yaml",
            "defaultProfile: default\nprofiles:\n  default:\n    plugins:\n      consumer.extra: true\n",
        );
        let (snap, diags) = resolve_snapshot(tmp.path(), None);
        assert!(diags.is_empty(), "diags: {diags:?}");
        assert!(snap.directive("task").is_some());
        assert!(snap.directive("extra-task").is_some());
    }

    #[test]
    fn capability_version_is_stable_across_two_identical_resolutions() {
        let tmp1 = tempfile::TempDir::new().unwrap();
        write_canon_core(tmp1.path());
        let (snap1, _) = resolve_snapshot(tmp1.path(), None);

        let tmp2 = tempfile::TempDir::new().unwrap();
        write_canon_core(tmp2.path());
        let (snap2, _) = resolve_snapshot(tmp2.path(), None);

        assert!(!snap1.capability_version.is_empty());
        assert_eq!(snap1.capability_version, snap2.capability_version);
    }

    #[test]
    fn capability_version_flips_when_a_manifest_atom_changes() {
        let tmp = tempfile::TempDir::new().unwrap();
        write_canon_core(tmp.path());
        let (before, _) = resolve_snapshot(tmp.path(), None);

        // A manifest content change with no code change at all: one new
        // enum member in canon.core's own enums.yaml.
        write(tmp.path(), "canon/vocab/canon.core/enums.yaml", "enums:\n  task-status: [open, done, blocked]\n");
        let (after, _) = resolve_snapshot(tmp.path(), None);

        assert_ne!(before.capability_version, after.capability_version);
    }
}
