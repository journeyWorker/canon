//! Retargeted from the donor manifest layer's project-file loader (design.md
//! D1: "a `canon.project.yaml` ... analog of the donor's
//! `pluginsDir`/`profiles` shape"). Renamed
//! `pluginsDir` -> `vocabDir` (default `.canon/vocab/`) since that is where
//! BOTH `canon.core` and every consumer `.canon/vocab/<id>/plugin.yaml` live
//! (D1/D3) — unlike the donor, canon's core plugin is not
//! compile-time-embedded,
//! it is scanned from the same directory as any other plugin (see
//! `crate::resolve_snapshot` module doc).

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};

use serde::Deserialize;

use crate::manifest::resolve::{ActivationMap, Profile, ProfileGraph};
use crate::manifest::types::Literal;

pub const PROJECT_YAML_RELATIVE_PATH: &str = "canon.project.yaml";
const DEFAULT_VOCAB_DIR: &str = canon_model::paths::VOCAB_DIR;

#[derive(Clone, Debug)]
pub struct ProjectConfig {
    pub graph: ProfileGraph,
    /// Resolved vocab dir (`project_dir.join(vocabDir)`; defaults to
    /// `project_dir/.canon/vocab/`).
    pub vocab_dir: PathBuf,
}

#[derive(Debug, Deserialize)]
struct RawProject {
    #[serde(rename = "vocabDir")]
    vocab_dir: Option<String>,
    #[serde(rename = "defaultProfile")]
    default_profile: String,
    #[serde(default)]
    profiles: BTreeMap<String, RawProfile>,
}

#[derive(Debug, Deserialize)]
struct RawProfile {
    #[serde(default)]
    extends: Option<String>,
    #[serde(default)]
    plugins: BTreeMap<String, serde_yaml::Value>,
}

fn plugin_options(value: &serde_yaml::Value) -> BTreeMap<String, Literal> {
    match Literal::from_yaml(value) {
        Some(Literal::Map(m)) => m,
        _ => BTreeMap::new(),
    }
}

/// Why [`load_project`] could not produce a [`ProjectConfig`].
#[derive(Debug, thiserror::Error)]
pub enum ProjectConfigError {
    #[error("cannot read {}: {source}", path.display())]
    Read { path: PathBuf, #[source] source: std::io::Error },
    #[error("invalid {}: {source}", path.display())]
    Invalid { path: PathBuf, #[source] source: serde_yaml::Error },
}

/// Read `<project_dir>/canon.project.yaml` into a [`ProjectConfig`].
/// Distinguishes an absent config (`Ok(None)`, legitimately resolves
/// core-only) from a broken one (`Err`, so the caller surfaces it instead of
/// silently mis-validating) from a valid one (`Ok(Some(cfg))`). Never panics.
pub fn load_project(project_dir: &Path) -> Result<Option<ProjectConfig>, ProjectConfigError> {
    let path = project_dir.join(PROJECT_YAML_RELATIVE_PATH);
    let text = match std::fs::read_to_string(&path) {
        Ok(t) => t,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(None),
        Err(source) => return Err(ProjectConfigError::Read { path, source }),
    };
    let raw: RawProject = serde_yaml::from_str(&text).map_err(|source| ProjectConfigError::Invalid { path, source })?;

    let mut profiles = BTreeMap::new();
    for (name, rp) in raw.profiles {
        let plugins: ActivationMap = rp.plugins.iter().map(|(id, value)| (id.clone(), plugin_options(value))).collect();
        profiles.insert(name, Profile { extends: rp.extends, plugins });
    }

    let graph = ProfileGraph { profiles, default_profile: raw.default_profile };
    let vocab_dir = project_dir.join(raw.vocab_dir.as_deref().unwrap_or(DEFAULT_VOCAB_DIR));

    Ok(Some(ProjectConfig { graph, vocab_dir }))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn missing_project_yaml_resolves_to_none() {
        let tmp = tempfile::TempDir::new().unwrap();
        assert!(load_project(tmp.path()).unwrap().is_none());
    }

    #[test]
    fn malformed_project_yaml_is_an_err_not_a_panic() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(tmp.path().join(PROJECT_YAML_RELATIVE_PATH), "not: [valid").unwrap();
        load_project(tmp.path()).unwrap_err();
    }

    #[test]
    fn invalid_project_yaml_error_string_matches_the_old_format() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join(PROJECT_YAML_RELATIVE_PATH);
        std::fs::write(&path, "not: [valid").unwrap();
        let yaml_err = serde_yaml::from_str::<RawProject>("not: [valid").unwrap_err();
        let err = load_project(tmp.path()).unwrap_err();
        assert_eq!(err.to_string(), format!("invalid {}: {yaml_err}", path.display()));
        assert!(std::error::Error::source(&err).is_some());
    }

    #[test]
    fn unreadable_project_yaml_error_string_matches_the_old_format() {
        let tmp = tempfile::TempDir::new().unwrap();
        let path = tmp.path().join(PROJECT_YAML_RELATIVE_PATH);
        // A directory at the config path: readable-by-name but not as a file.
        std::fs::create_dir(&path).unwrap();
        let io_err = std::fs::read_to_string(&path).unwrap_err();
        let err = load_project(tmp.path()).unwrap_err();
        assert_eq!(err.to_string(), format!("cannot read {}: {io_err}", path.display()));
        assert!(std::error::Error::source(&err).is_some());
    }

    #[test]
    fn valid_project_yaml_resolves_profiles_and_vocab_dir() {
        let tmp = tempfile::TempDir::new().unwrap();
        std::fs::write(
            tmp.path().join(PROJECT_YAML_RELATIVE_PATH),
            "defaultProfile: default\nprofiles:\n  default:\n    plugins:\n      consumer.extra: true\n",
        )
        .unwrap();
        let cfg = load_project(tmp.path()).unwrap().unwrap();
        assert_eq!(cfg.vocab_dir, tmp.path().join(DEFAULT_VOCAB_DIR));
        assert!(cfg.graph.profiles["default"].plugins.contains_key("consumer.extra"));
    }
}
