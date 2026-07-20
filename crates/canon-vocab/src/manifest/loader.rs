//! Retargeted from the donor manifest layer's plugin package loader,
//! pruned to canon's two export kinds — `directives` (a dir of `*.yaml`,
//! merged by directive name) and `enums` (a single `*.yaml` file) — dropping
//! `state`/`providers`/`bridge`/`defs`/`frontmatter`/`docs`/`assetkinds`/
//! `events` (no canon analog, D2 Non-Goals). Never panics: every failure is a
//! [`LoadError`] in the returned vec, exactly as the donor's loader guarantees.
//!
//! `manifest.exports`' key vocabulary is closed at the TYPE level
//! ([`crate::manifest::schema::ExportKind`], the closed-vocabulary audit
//! pattern) — an
//! export key outside `directives`/`enums` now fails `plugin.yaml` parsing
//! itself (`LoadError::Manifest`) before this module ever sees the
//! manifest, so the export-kind match below is exhaustive over
//! [`ExportKind`] with no runtime "unknown export" fallback left to reach.

use std::collections::BTreeMap;
use std::path::Path;

use crate::manifest::schema::{DirectiveDecl, DirectivesFile, EnumsFile, ExportKind, PluginManifest};

#[derive(Clone, Debug)]
pub struct LoadedPlugin {
    pub manifest: PluginManifest,
    pub directives: Vec<DirectiveDecl>,
    pub enums: BTreeMap<String, Vec<String>>,
}

#[derive(Clone, Debug, PartialEq)]
pub enum LoadError {
    Manifest { dir: String, msg: String },
    Parse { file: String, msg: String },
    DuplicateId { kind: String, id: String },
    MissingExportDir { export: String, path: String },
    Io { path: String, msg: String },
    /// A directive manifest bug ([`crate::manifest::schema::duplicate_attr_names`]).
    DuplicateAttr { directive: String, attr: String },
}

impl LoadError {
    /// Stable, machine-readable code per variant (mirrors the checker's
    /// `E-*` diagnostic family, from the donor manifest layer's loader).
    pub fn code(&self) -> &'static str {
        match self {
            LoadError::Manifest { .. } => "E-PLUGIN-MANIFEST",
            LoadError::Parse { .. } => "E-PLUGIN-PARSE",
            LoadError::DuplicateId { .. } => "E-PLUGIN-DUP-ID",
            LoadError::MissingExportDir { .. } => "E-PLUGIN-MISSING-EXPORT",
            LoadError::Io { .. } => "E-PLUGIN-IO",
            LoadError::DuplicateAttr { .. } => "E-PLUGIN-DUP-ATTR",
        }
    }
}

/// Read one plugin package. `dir` MUST contain `plugin.yaml`.
pub fn load_plugin_dir(dir: &Path) -> Result<LoadedPlugin, Vec<LoadError>> {
    let manifest_path = dir.join("plugin.yaml");
    let manifest: PluginManifest = match std::fs::read_to_string(&manifest_path) {
        Ok(s) => match serde_yaml::from_str(&s) {
            Ok(m) => m,
            Err(e) => return Err(vec![LoadError::Manifest { dir: dir.display().to_string(), msg: e.to_string() }]),
        },
        Err(e) => return Err(vec![LoadError::Manifest { dir: dir.display().to_string(), msg: e.to_string() }]),
    };

    let mut errs = Vec::new();
    let mut out = LoadedPlugin { manifest: manifest.clone(), directives: Vec::new(), enums: BTreeMap::new() };

    for (export, rel) in &manifest.exports {
        let path = dir.join(rel);
        if !path.exists() {
            errs.push(LoadError::MissingExportDir { export: export.as_str().to_string(), path: path.display().to_string() });
            continue;
        }
        match export {
            ExportKind::Directives => read_directives(&path, &mut out.directives, &mut errs),
            ExportKind::Enums => read_enums(&path, &mut out.enums, &mut errs),
        }
    }

    if errs.is_empty() {
        Ok(out)
    } else {
        Err(errs)
    }
}

/// Scan `dir` for plugin packages (each immediate subdirectory containing a
/// `plugin.yaml`), in sorted order, indexed by manifest id. A duplicate id
/// across packages drops the later package (`LoadError::DuplicateId`). A
/// missing `dir` yields an empty registry — never a panic.
pub fn load_plugins_dir(dir: &Path) -> (crate::manifest::resolve::InstalledPlugins, Vec<LoadError>) {
    use crate::manifest::resolve::{InstalledPlugin, InstalledPlugins};
    let mut reg = InstalledPlugins::default();
    let mut errs = Vec::new();
    let mut subs: Vec<_> = match std::fs::read_dir(dir) {
        Ok(rd) => rd.filter_map(|e| e.ok().map(|e| e.path())).filter(|p| p.is_dir()).collect(),
        Err(_) => return (reg, errs),
    };
    subs.sort();
    for sub in subs {
        if !sub.join("plugin.yaml").is_file() {
            continue;
        }
        match load_plugin_dir(&sub) {
            Ok(loaded) => {
                let id = loaded.manifest.id.clone();
                match reg.by_id.entry(id) {
                    std::collections::btree_map::Entry::Occupied(e) => {
                        errs.push(LoadError::DuplicateId { kind: "plugin".into(), id: e.key().clone() });
                    }
                    std::collections::btree_map::Entry::Vacant(e) => {
                        e.insert(InstalledPlugin { loaded });
                    }
                }
            }
            Err(mut e) => errs.append(&mut e),
        }
    }
    (reg, errs)
}

fn read_directives(path: &Path, dst: &mut Vec<DirectiveDecl>, errs: &mut Vec<LoadError>) {
    for file in yaml_files(path) {
        let s = match std::fs::read_to_string(&file) {
            Ok(s) => s,
            Err(e) => {
                errs.push(LoadError::Io { path: file.display().to_string(), msg: e.to_string() });
                continue;
            }
        };
        let parsed: DirectivesFile = match serde_yaml::from_str(&s) {
            Ok(f) => f,
            Err(e) => {
                errs.push(LoadError::Parse { file: file.display().to_string(), msg: e.to_string() });
                continue;
            }
        };
        for d in parsed.directives {
            for attr in crate::manifest::schema::duplicate_attr_names(&d) {
                errs.push(LoadError::DuplicateAttr { directive: d.name.clone(), attr });
            }
            if dst.iter().any(|existing| existing.name == d.name) {
                errs.push(LoadError::DuplicateId { kind: "directive".into(), id: d.name.clone() });
                continue;
            }
            dst.push(d);
        }
    }
}

fn read_enums(path: &Path, dst: &mut BTreeMap<String, Vec<String>>, errs: &mut Vec<LoadError>) {
    for file in yaml_files(path) {
        let s = match std::fs::read_to_string(&file) {
            Ok(s) => s,
            Err(e) => {
                errs.push(LoadError::Io { path: file.display().to_string(), msg: e.to_string() });
                continue;
            }
        };
        let parsed: EnumsFile = match serde_yaml::from_str(&s) {
            Ok(f) => f,
            Err(e) => {
                errs.push(LoadError::Parse { file: file.display().to_string(), msg: e.to_string() });
                continue;
            }
        };
        for (name, members) in parsed.enums {
            if dst.contains_key(&name) {
                errs.push(LoadError::DuplicateId { kind: "enum".into(), id: name });
                continue;
            }
            dst.insert(name, members);
        }
    }
}

/// Every `*.yaml`/`*.yml` under `path` (a dir), sorted byte-wise; or `[path]`
/// itself if `path` is a file (plugin §4 sort determinism, from the donor
/// manifest layer's loader). An unreadable dir yields no files — never panics.
fn yaml_files(path: &Path) -> Vec<std::path::PathBuf> {
    if path.is_file() {
        return vec![path.to_path_buf()];
    }
    let Ok(rd) = std::fs::read_dir(path) else {
        return Vec::new();
    };
    let mut files: Vec<_> = rd
        .filter_map(|e| e.ok().map(|e| e.path()))
        .filter(|p| p.is_file() && matches!(p.extension().and_then(|e| e.to_str()), Some("yaml") | Some("yml")))
        .collect();
    files.sort();
    files
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(dir: &Path, rel: &str, contents: &str) {
        let path = dir.join(rel);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, contents).unwrap();
    }

    #[test]
    fn load_plugin_dir_reads_directives_and_enums() {
        let tmp = tempfile::TempDir::new().unwrap();
        write(
            tmp.path(),
            "plugin.yaml",
            "id: canon.core\nversion: \"0.1.0\"\nkind: core\nexports:\n  directives: directives/\n  enums: enums.yaml\n",
        );
        write(
            tmp.path(),
            "directives/task.yaml",
            "directives:\n  - name: task\n    attrs:\n      - name: desc\n        type: string\n        required: true\n",
        );
        write(tmp.path(), "enums.yaml", "enums:\n  task-status: [open, done]\n");

        let loaded = load_plugin_dir(tmp.path()).expect("loads cleanly");
        assert_eq!(loaded.manifest.id, "canon.core");
        assert_eq!(loaded.directives.len(), 1);
        assert_eq!(loaded.enums.get("task-status"), Some(&vec!["open".to_string(), "done".to_string()]));
    }

    #[test]
    fn missing_manifest_yields_a_load_error_not_a_panic() {
        let tmp = tempfile::TempDir::new().unwrap();
        let errs = load_plugin_dir(tmp.path()).unwrap_err();
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].code(), "E-PLUGIN-MANIFEST");
    }

    #[test]
    fn an_unknown_export_key_is_rejected_at_manifest_parse_time() {
        let tmp = tempfile::TempDir::new().unwrap();
        write(tmp.path(), "plugin.yaml", "id: consumer.extra\nversion: \"0.1.0\"\nkind: project\nexports:\n  docs: docs/\n");
        let errs = load_plugin_dir(tmp.path()).unwrap_err();
        assert_eq!(errs.len(), 1);
        assert_eq!(errs[0].code(), "E-PLUGIN-MANIFEST");
    }

    #[test]
    fn load_plugins_dir_on_missing_dir_returns_empty_registry() {
        let (reg, errs) = load_plugins_dir(Path::new("/does/not/exist/at/all"));
        assert!(reg.by_id.is_empty());
        assert!(errs.is_empty());
    }
}
