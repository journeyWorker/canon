//! `plugin.yaml` package loader (design.md D2, tasks.md 1.1): scans
//! `<project_dir>/canon/plugins/<id>/plugin.yaml`, mirroring
//! `canon_vocab::manifest::loader::load_plugins_dir`'s shape
//! (`crates/canon-vocab/src/manifest/loader.rs:86-119`) by INSPIRATION --
//! sorted directory order, per-package, a duplicate manifest `id` DROPS
//! the later package as a diagnostic, never a silent overwrite. No
//! `canon-vocab` crate dependency (design.md D2/R4). Never panics: every
//! failure is a [`LoadError`] in the returned vec, and a missing `dir`
//! yields an empty registry rather than an `Err`.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use crate::diagnostic::{Diagnostic, E_PLUGIN_DUP_ID, E_PLUGIN_MANIFEST};
use crate::manifest::schema::PluginManifest;

#[derive(Clone, Debug, PartialEq)]
pub struct LoadedPlugin {
    pub manifest: PluginManifest,
    pub dir: PathBuf,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LoadError {
    /// `plugin.yaml` is missing, unreadable, or fails to parse as
    /// [`PluginManifest`] (including a missing required field).
    Manifest { dir: String, msg: String },
    /// A second `canon/plugins/<dir>/plugin.yaml` declared the same
    /// manifest `id` as an earlier (directory-sort order) package.
    DuplicateId { id: String, first_dir: String, second_dir: String },
}

impl LoadError {
    pub fn code(&self) -> &'static str {
        match self {
            LoadError::Manifest { .. } => E_PLUGIN_MANIFEST,
            LoadError::DuplicateId { .. } => E_PLUGIN_DUP_ID,
        }
    }

    pub fn diagnostic(&self) -> Diagnostic {
        match self {
            LoadError::Manifest { dir, msg } => {
                Diagnostic::error(self.code(), format!("failed to load plugin.yaml in {dir}: {msg}"), dir.clone())
            }
            LoadError::DuplicateId { id, first_dir, second_dir } => Diagnostic::error(
                self.code(),
                format!("duplicate plugin id `{id}`: keeping {first_dir}, dropping {second_dir}"),
                id.clone(),
            ),
        }
    }
}

/// Read one plugin package. `dir` MUST contain `plugin.yaml`.
pub fn load_plugin_dir(dir: &Path) -> Result<LoadedPlugin, LoadError> {
    let manifest_path = dir.join("plugin.yaml");
    let text = std::fs::read_to_string(&manifest_path)
        .map_err(|e| LoadError::Manifest { dir: dir.display().to_string(), msg: e.to_string() })?;
    let manifest: PluginManifest = serde_yaml::from_str(&text)
        .map_err(|e| LoadError::Manifest { dir: dir.display().to_string(), msg: e.to_string() })?;
    Ok(LoadedPlugin { manifest, dir: dir.to_path_buf() })
}

/// Scan `dir` for plugin packages (each immediate subdirectory
/// containing a `plugin.yaml`), in sorted directory order, indexed by
/// manifest id. A duplicate id across packages drops the later package
/// ([`LoadError::DuplicateId`]). A missing `dir` yields an empty
/// registry -- never a panic.
pub fn load_plugins_dir(dir: &Path) -> (BTreeMap<String, LoadedPlugin>, Vec<LoadError>) {
    let mut out = BTreeMap::new();
    let mut errs = Vec::new();
    let mut subs: Vec<PathBuf> = match std::fs::read_dir(dir) {
        Ok(rd) => rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.is_dir()).collect(),
        Err(_) => return (out, errs),
    };
    subs.sort();
    for sub in subs {
        if !sub.join("plugin.yaml").is_file() {
            continue;
        }
        match load_plugin_dir(&sub) {
            Ok(loaded) => {
                let id = loaded.manifest.id.clone();
                match out.entry(id) {
                    std::collections::btree_map::Entry::Occupied(e) => {
                        errs.push(LoadError::DuplicateId {
                            id: e.key().clone(),
                            first_dir: e.get().dir.display().to_string(),
                            second_dir: loaded.dir.display().to_string(),
                        });
                    }
                    std::collections::btree_map::Entry::Vacant(e) => {
                        e.insert(loaded);
                    }
                }
            }
            Err(e) => errs.push(e),
        }
    }
    (out, errs)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, rel: &str, contents: &str) {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    const PORTING_YAML: &str = "id: porting\nnamespace: porting\noverlays:\n  - kind: coverage\n    attaches_to:\n      core_kind: scenario\n      join_key: [project_id, scenario_id]\n    fields:\n      - name: covered\n        type: bool\n";

    #[test]
    fn load_plugin_dir_reads_a_well_formed_manifest() {
        let tmp = tempfile::TempDir::new().unwrap();
        write(tmp.path(), "plugin.yaml", PORTING_YAML);
        let loaded = load_plugin_dir(tmp.path()).expect("loads cleanly");
        assert_eq!(loaded.manifest.id, "porting");
        assert_eq!(loaded.manifest.overlays.len(), 1);
    }

    #[test]
    fn missing_manifest_yields_a_load_error_not_a_panic() {
        let tmp = tempfile::TempDir::new().unwrap();
        let err = load_plugin_dir(tmp.path()).unwrap_err();
        assert_eq!(err.code(), "E-PLUGIN-MANIFEST");
    }

    #[test]
    fn a_missing_required_field_yields_a_load_error_never_defaulted() {
        let tmp = tempfile::TempDir::new().unwrap();
        write(tmp.path(), "plugin.yaml", "id: bad\nnamespace: bad\noverlays:\n  - kind: coverage\n    attaches_to:\n      core_kind: scenario\n");
        let err = load_plugin_dir(tmp.path()).unwrap_err();
        assert_eq!(err.code(), "E-PLUGIN-MANIFEST");
    }

    #[test]
    fn load_plugins_dir_on_missing_dir_returns_an_empty_registry() {
        let (reg, errs) = load_plugins_dir(Path::new("/does/not/exist/at/all/canon/plugins"));
        assert!(reg.is_empty());
        assert!(errs.is_empty());
    }

    #[test]
    fn load_plugins_dir_scans_sorted_and_dedupes_by_id() {
        let tmp = tempfile::TempDir::new().unwrap();
        write(tmp.path(), "porting/plugin.yaml", PORTING_YAML);
        write(tmp.path(), "unrelated/plugin.yaml", "id: unrelated\nnamespace: unrelated\noverlays: []\n");
        let (reg, errs) = load_plugins_dir(tmp.path());
        assert!(errs.is_empty(), "errs: {errs:?}");
        assert_eq!(reg.len(), 2);
        assert!(reg.contains_key("porting"));
        assert!(reg.contains_key("unrelated"));
    }

    #[test]
    fn a_duplicate_id_drops_the_later_directory_and_reports_a_diagnostic() {
        let tmp = tempfile::TempDir::new().unwrap();
        // "pkg-a" sorts before "pkg-b" -- pkg-a's package must survive.
        write(tmp.path(), "pkg-a/plugin.yaml", "id: porting\nnamespace: porting-a\noverlays: []\n");
        write(tmp.path(), "pkg-b/plugin.yaml", "id: porting\nnamespace: porting-b\noverlays: []\n");
        let (reg, errs) = load_plugins_dir(tmp.path());
        assert_eq!(reg.len(), 1);
        assert_eq!(reg["porting"].manifest.namespace, "porting-a");
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].code(), "E-PLUGIN-DUP-ID");
        match &errs[0] {
            LoadError::DuplicateId { id, .. } => assert_eq!(id, "porting"),
            other => panic!("expected DuplicateId, got {other:?}"),
        }
    }

    #[test]
    fn a_non_directory_entry_under_canon_plugins_is_ignored() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join("README.md"), "not a plugin package").unwrap();
        write(tmp.path(), "porting/plugin.yaml", PORTING_YAML);
        let (reg, errs) = load_plugins_dir(tmp.path());
        assert!(errs.is_empty(), "errs: {errs:?}");
        assert_eq!(reg.len(), 1);
    }
}
