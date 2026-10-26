//! Every CLI leaf in the committed command spec (`docs/cli/canon.cli.json`) is
//! covered by at least one success-path and one failure-path scenario, named in
//! `tests/fixtures/leaf-scenarios.tsv` (0.30 spec backfill).
//!
//! The table has one row per leaf: `leaf<TAB>happy ids<TAB>failure ids`, ids
//! comma-separated without the leading `@`. Happy ids are scenarios whose
//! `@case:` is not `failure`; failure ids carry `@case:failure`. Every id must
//! exist in the scenario corpus of both spec roots.

use std::collections::{BTreeMap, BTreeSet};
use std::fs;
use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("../..")
}

fn spec_leaves() -> BTreeSet<String> {
    let path = repo_root().join("docs/cli/canon.cli.json");
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let spec: serde_json::Value = serde_json::from_str(&text).expect("command spec is JSON");
    let commands = spec["commands"].as_array().expect("spec has a commands array");
    commands
        .iter()
        .filter(|node| {
            node["nodeKind"] == "leaf"
                && node
                    .get("children")
                    .and_then(|c| c.as_array())
                    .map_or(true, Vec::is_empty)
        })
        .map(|node| node["command"].as_str().expect("leaf has a command id").to_string())
        .collect()
}

struct Row {
    happy: Vec<String>,
    failure: Vec<String>,
}

fn table() -> BTreeMap<String, Row> {
    let path = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/leaf-scenarios.tsv");
    let text = fs::read_to_string(&path).unwrap_or_else(|e| panic!("read {}: {e}", path.display()));
    let ids = |cell: &str| -> Vec<String> {
        cell.split(',')
            .map(str::trim)
            .filter(|id| !id.is_empty())
            .map(|id| id.trim_start_matches('@').to_string())
            .collect()
    };
    let mut rows = BTreeMap::new();
    for (n, line) in text.lines().enumerate() {
        if line.trim().is_empty() || line.starts_with('#') {
            continue;
        }
        let cells: Vec<&str> = line.split('\t').collect();
        assert!(
            cells.len() == 3,
            "leaf-scenarios.tsv line {}: expected 3 tab-separated columns (leaf, happy ids, failure ids), found {}",
            n + 1,
            cells.len()
        );
        let leaf = cells[0].trim().to_string();
        let row = Row { happy: ids(cells[1]), failure: ids(cells[2]) };
        assert!(
            rows.insert(leaf.clone(), row).is_none(),
            "leaf-scenarios.tsv line {}: leaf `{leaf}` appears twice",
            n + 1
        );
    }
    rows
}

fn feature_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = fs::read_dir(dir) else { return };
    let mut entries: Vec<_> = entries.flatten().map(|e| e.path()).collect();
    entries.sort();
    for path in entries {
        if path.is_dir() {
            feature_files(&path, out);
        } else if path.extension().is_some_and(|e| e == "feature") {
            out.push(path);
        }
    }
}

/// A scenario id tag is `@<area>.<surface>.<nn>`: no colon, three dot parts, digits last.
fn scenario_id(tag: &str) -> Option<&str> {
    let id = tag.strip_prefix('@')?;
    if id.contains(':') {
        return None;
    }
    let parts: Vec<&str> = id.split('.').collect();
    let well_formed = parts.len() == 3
        && parts[..2].iter().all(|p| !p.is_empty())
        && !parts[2].is_empty()
        && parts[2].bytes().all(|b| b.is_ascii_digit());
    well_formed.then_some(id)
}

/// Scenario id -> the `@case:` values it carries (scenario tags plus feature-level tags).
fn corpus() -> BTreeMap<String, BTreeSet<String>> {
    let root = repo_root();
    let mut files = Vec::new();
    feature_files(&root.join(".canon/specs"), &mut files);
    feature_files(&root.join("examples/platformer/specs"), &mut files);
    assert!(!files.is_empty(), "no .feature files found under either spec root");

    let mut scenarios: BTreeMap<String, BTreeSet<String>> = BTreeMap::new();
    for file in files {
        let text = fs::read_to_string(&file).unwrap_or_else(|e| panic!("read {}: {e}", file.display()));
        let mut feature_tags: Vec<&str> = Vec::new();
        let mut pending: Vec<&str> = Vec::new();
        for line in text.lines().map(str::trim) {
            if line.starts_with('@') {
                pending.extend(line.split_whitespace());
            } else if line.starts_with("Feature:") {
                feature_tags = std::mem::take(&mut pending);
            } else if line.starts_with("Scenario:") || line.starts_with("Scenario Outline:") {
                let tags = std::mem::take(&mut pending);
                let cases: BTreeSet<String> = feature_tags
                    .iter()
                    .chain(tags.iter())
                    .filter_map(|t| t.strip_prefix("@case:"))
                    .map(str::to_string)
                    .collect();
                for id in tags.iter().filter_map(|t| scenario_id(t)) {
                    scenarios.entry(id.to_string()).or_default().extend(cases.iter().cloned());
                }
            } else if !line.is_empty() && !line.starts_with('#') {
                pending.clear();
            }
        }
    }
    scenarios
}

#[test]
fn every_leaf_has_a_success_and_a_failure_scenario() {
    let leaves = spec_leaves();
    assert!(!leaves.is_empty(), "the command spec lists no leaves");
    let rows = table();
    let corpus = corpus();
    let mut problems: Vec<String> = Vec::new();

    for leaf in &leaves {
        if !rows.contains_key(leaf) {
            problems.push(format!("leaf `{leaf}`: no row in leaf-scenarios.tsv"));
        }
    }
    for leaf in rows.keys() {
        if !leaves.contains(leaf) {
            problems.push(format!("leaf `{leaf}`: row in leaf-scenarios.tsv but no such leaf in the command spec"));
        }
    }

    for (leaf, row) in &rows {
        if row.happy.is_empty() {
            problems.push(format!("leaf `{leaf}`: no success-path (happy) scenario id"));
        }
        if row.failure.is_empty() {
            problems.push(format!("leaf `{leaf}`: no failure-path scenario id"));
        }
        for id in &row.happy {
            match corpus.get(id) {
                None => problems.push(format!("leaf `{leaf}`: happy scenario `{id}` does not exist in the corpus")),
                Some(cases) if cases.contains("failure") => {
                    problems.push(format!("leaf `{leaf}`: happy scenario `{id}` is tagged @case:failure"))
                }
                Some(_) => {}
            }
        }
        for id in &row.failure {
            match corpus.get(id) {
                None => problems.push(format!("leaf `{leaf}`: failure scenario `{id}` does not exist in the corpus")),
                Some(cases) if !cases.contains("failure") => {
                    problems.push(format!("leaf `{leaf}`: failure scenario `{id}` is not tagged @case:failure"))
                }
                Some(_) => {}
            }
        }
    }

    assert!(
        problems.is_empty(),
        "{} leaf-coverage problem(s):\n  {}",
        problems.len(),
        problems.join("\n  ")
    );
}
