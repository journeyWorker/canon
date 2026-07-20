//! Shared "check-generated" / "write" logic for canon-model's two
//! generated artifacts: `JOIN_SPINE.md` (task 2.3) and
//! `schemas/*.schema.json` (task 3.2). Used by both the `xtask` binary
//! (`cargo xtask check-generated` / `cargo xtask write`) and this
//! crate's own `#[test]`s (`gen::tests`, below) — so drift fails
//! `cargo test --workspace` directly, not only a separate CI step
//! someone has to remember to run.

use std::collections::HashSet;
use std::path::{Path, PathBuf};

const MANIFEST_DIR: &str = env!("CARGO_MANIFEST_DIR");

pub fn join_spine_doc_path() -> PathBuf {
    Path::new(MANIFEST_DIR).join("JOIN_SPINE.md")
}

pub fn schemas_dir() -> PathBuf {
    Path::new(MANIFEST_DIR).join("schemas")
}

/// Every generated file this crate commits, paired with its
/// freshly-regenerated expected content — the closed-twelve
/// `RecordKind` schemas AND the S11 artifact-family schemas
/// (`crate::schema_export::pretty_family_schemas`), both committed
/// into the SAME `schemas/` directory (distinguished by the
/// `family-` filename prefix) so this one function/one drift check
/// covers both registries.
pub fn generated_files() -> Vec<(PathBuf, String)> {
    let mut files = vec![(join_spine_doc_path(), crate::join_spine_doc::render())];
    for (name, content) in crate::schema_export::pretty_schemas() {
        files.push((schemas_dir().join(name), content));
    }
    for (name, content) in crate::schema_export::pretty_family_schemas() {
        files.push((schemas_dir().join(name), content));
    }
    files
}

#[derive(Debug, Default)]
pub struct DriftReport {
    /// Committed files that are missing, or whose content no longer
    /// matches what the generator produces from the current source.
    pub missing_or_stale: Vec<PathBuf>,
    /// Committed `schemas/*.schema.json` files the generator no longer
    /// produces at all (a record kind was removed/renamed).
    pub unexpected: Vec<PathBuf>,
}

impl DriftReport {
    pub fn is_clean(&self) -> bool {
        self.missing_or_stale.is_empty() && self.unexpected.is_empty()
    }
}

/// Regenerate every artifact in memory and diff it against the
/// committed files on disk. Never writes anything.
pub fn check() -> DriftReport {
    let mut report = DriftReport::default();

    for (path, expected) in generated_files() {
        match std::fs::read_to_string(&path) {
            Ok(actual) if actual == expected => {}
            _ => report.missing_or_stale.push(path),
        }
    }

    let expected_schema_names: HashSet<String> = crate::schema_export::pretty_schemas()
        .into_iter()
        .chain(crate::schema_export::pretty_family_schemas())
        .map(|(name, _)| name)
        .collect();
    if let Ok(entries) = std::fs::read_dir(schemas_dir()) {
        for entry in entries.flatten() {
            let name = entry.file_name().to_string_lossy().into_owned();
            if name.ends_with(".schema.json") && !expected_schema_names.contains(&name) {
                report.unexpected.push(entry.path());
            }
        }
    }

    report
}

/// Regenerate every artifact and overwrite the committed files — the
/// developer workflow after editing a join-key grammar doc comment or a
/// record kind's fields (`cargo xtask write`).
pub fn write() -> std::io::Result<()> {
    std::fs::create_dir_all(schemas_dir())?;
    for (path, content) in generated_files() {
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent)?;
        }
        std::fs::write(path, content)?;
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    /// The drift check this test runs is exactly what `cargo xtask
    /// check-generated` runs — if this test is green, the committed
    /// `JOIN_SPINE.md` and `schemas/*.schema.json` are guaranteed
    /// current with the Rust source that generated them (spec
    /// scenario: "the generation step is part of ... selftest's
    /// diff-against-committed-output check").
    #[test]
    fn committed_generated_output_matches_current_source() {
        let report = check();
        assert!(
            report.is_clean(),
            "generated output drifted from committed files — run `cargo xtask write` and commit the diff.\nstale/missing: {:?}\nunexpected: {:?}",
            report.missing_or_stale,
            report.unexpected
        );
    }
}
