//! `resolve_plugin_snapshot` (design.md D2, tasks.md 1.2; `plugin-
//! overlay-registry` spec's "One resolution entry point" requirement):
//! THE single plugin-snapshot resolution entry point -- mirrors
//! `canon_vocab::resolve_snapshot::resolve_snapshot`'s contract (pure,
//! total, never panics) by INSPIRATION, no `canon-vocab` dependency. A
//! future P2 overlay-write path and a future P3 projection-read path
//! (both out of this change's scope) both call this SAME function for a
//! given project directory -- no second, independently-computed plugin
//! view exists anywhere in this crate or its consumers.

use std::path::Path;

use crate::diagnostic::Diagnostic;
use crate::manifest::loader::load_plugins_dir;
use crate::manifest::resolve::assemble_snapshot;
use crate::manifest::snapshot::PluginSnapshot;

/// Ledger-overlay plugins live at `<project_dir>/canon/plugins/<id>/
/// plugin.yaml` -- a directory distinct from `canon_vocab`'s OWN
/// `canon/vocab/<id>/plugin.yaml` (design.md D2/R4). This crate's loader
/// never descends into `canon/vocab/`, and never even reads it.
pub const PLUGINS_DIR_RELATIVE_PATH: &str = "canon/plugins";

/// Resolve `project_dir`'s installed ledger-overlay plugins into one
/// [`PluginSnapshot`], plus every resolution diagnostic (a missing
/// `canon/plugins/` directory, a malformed manifest, a grammar
/// violation, a duplicate id, a duplicate overlay identity, an
/// unsupported `core_kind`). Pure, total, NEVER panics -- every failure
/// degrades to a usable (possibly plugin-empty) snapshot plus a
/// diagnostic, mirroring `resolve_snapshot`'s own "pure, total, never
/// panics" contract verbatim (design.md R3).
pub fn resolve_plugin_snapshot(project_dir: &Path) -> (PluginSnapshot, Vec<Diagnostic>) {
    let mut diags = Vec::new();
    let plugins_dir = project_dir.join(PLUGINS_DIR_RELATIVE_PATH);

    let (installed, load_errs) = load_plugins_dir(&plugins_dir);
    diags.extend(load_errs.iter().map(|e| e.diagnostic()));

    let (snapshot, assemble_diags) = assemble_snapshot(&installed);
    diags.extend(assemble_diags);

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

    const PORTING_YAML: &str = "id: porting\nnamespace: porting\noverlays:\n  - kind: coverage\n    attaches_to:\n      core_kind: scenario\n      join_key: [project_id, scenario_id]\n    fields:\n      - name: covered\n        type: bool\n      - name: surface_ref\n        type: { list: string }\n";

    #[test]
    fn a_well_formed_manifest_resolves_its_declared_shape() {
        let tmp = tempfile::TempDir::new().unwrap();
        write(tmp.path(), "canon/plugins/porting/plugin.yaml", PORTING_YAML);
        let (snap, diags) = resolve_plugin_snapshot(tmp.path());
        assert!(diags.is_empty(), "diags: {diags:?}");
        let decl = snap.overlay("porting.coverage").expect("overlay present");
        assert_eq!(decl.core_kind, "scenario");
        assert_eq!(decl.join_key, vec!["project_id".to_string(), "scenario_id".to_string()]);
        assert_eq!(decl.fields.len(), 2);
    }

    #[test]
    fn a_missing_required_field_fails_to_load_and_excludes_the_plugin() {
        let tmp = tempfile::TempDir::new().unwrap();
        write(
            tmp.path(),
            "canon/plugins/bad/plugin.yaml",
            "id: bad\nnamespace: bad\noverlays:\n  - kind: coverage\n    attaches_to:\n      core_kind: scenario\n",
        );
        let (snap, diags) = resolve_plugin_snapshot(tmp.path());
        assert!(snap.is_empty());
        assert!(diags.iter().any(|d| d.code == "E-PLUGIN-MANIFEST"));
    }

    #[test]
    fn duplicate_plugin_ids_drop_the_later_package_with_a_diagnostic() {
        let tmp = tempfile::TempDir::new().unwrap();
        write(tmp.path(), "canon/plugins/pkg-a/plugin.yaml", "id: porting\nnamespace: porting-a\noverlays:\n  - kind: coverage\n    attaches_to:\n      core_kind: scenario\n      join_key: [project_id, scenario_id]\n");
        write(tmp.path(), "canon/plugins/pkg-b/plugin.yaml", "id: porting\nnamespace: porting-b\noverlays:\n  - kind: coverage\n    attaches_to:\n      core_kind: scenario\n      join_key: [project_id, scenario_id]\n");
        let (snap, diags) = resolve_plugin_snapshot(tmp.path());
        assert!(snap.plugins.contains_key("porting"));
        assert_eq!(snap.plugins["porting"].namespace, "porting-a");
        assert!(diags.iter().any(|d| d.code == "E-PLUGIN-DUP-ID"));
    }

    #[test]
    fn a_namespace_or_overlay_kind_failing_the_kebab_grammar_is_excluded() {
        let tmp = tempfile::TempDir::new().unwrap();
        write(
            tmp.path(),
            "canon/plugins/bad-ns/plugin.yaml",
            "id: bad-ns\nnamespace: Porting_Two\noverlays:\n  - kind: coverage\n    attaches_to:\n      core_kind: scenario\n      join_key: [project_id]\n",
        );
        write(
            tmp.path(),
            "canon/plugins/bad-kind/plugin.yaml",
            "id: bad-kind\nnamespace: porting\noverlays:\n  - kind: \"coverage/extra\"\n    attaches_to:\n      core_kind: scenario\n      join_key: [project_id]\n",
        );
        let (snap, diags) = resolve_plugin_snapshot(tmp.path());
        assert!(snap.is_empty());
        assert_eq!(diags.iter().filter(|d| d.code == "E-PLUGIN-GRAMMAR").count(), 2);
    }

    #[test]
    fn a_non_scenario_core_kind_fails_resolution_loud() {
        let tmp = tempfile::TempDir::new().unwrap();
        write(
            tmp.path(),
            "canon/plugins/bad/plugin.yaml",
            "id: bad\nnamespace: bad\noverlays:\n  - kind: coverage\n    attaches_to:\n      core_kind: task\n      join_key: [project_id, task_id]\n",
        );
        let (snap, diags) = resolve_plugin_snapshot(tmp.path());
        assert!(snap.is_empty());
        assert!(diags.iter().any(|d| d.code == "E-PLUGIN-CORE-KIND"));
    }

    #[test]
    fn an_absent_canon_plugins_dir_resolves_an_empty_valid_snapshot_never_a_panic() {
        let tmp = tempfile::TempDir::new().unwrap();
        let (snap, diags) = resolve_plugin_snapshot(tmp.path());
        assert!(snap.is_empty());
        assert!(diags.is_empty());
    }

    #[test]
    fn a_project_dir_that_is_not_even_a_directory_never_panics() {
        let tmp = tempfile::TempDir::new().unwrap();
        let not_a_dir = tmp.path().join("not-a-directory.txt");
        std::fs::write(&not_a_dir, "just a file").unwrap();
        let (snap, diags) = resolve_plugin_snapshot(&not_a_dir);
        assert!(snap.is_empty());
        assert!(diags.is_empty());
    }

    #[test]
    fn the_canon_vocab_directory_is_never_read_by_this_crates_loader() {
        // Proves design.md D2/R4's directory disjointness from this
        // crate's own side: a `canon/vocab/<id>/plugin.yaml` (canon-vocab's
        // authoring-vocabulary schema, unrelated to ours) sitting alongside
        // a well-formed `canon/plugins/<id>/plugin.yaml` is entirely
        // ignored -- this crate's loader only ever globs `canon/plugins/`.
        let tmp = tempfile::TempDir::new().unwrap();
        write(tmp.path(), "canon/plugins/porting/plugin.yaml", PORTING_YAML);
        write(
            tmp.path(),
            "canon/vocab/my-tasks/plugin.yaml",
            "id: my-tasks\nversion: \"0.1.0\"\nkind: project\nexports:\n  directives: directives/\n",
        );
        let (snap, diags) = resolve_plugin_snapshot(tmp.path());
        assert!(diags.is_empty(), "diags: {diags:?}");
        assert_eq!(snap.plugins.len(), 1);
        assert!(snap.plugins.contains_key("porting"));
        assert!(!snap.plugins.contains_key("my-tasks"));
    }
}
