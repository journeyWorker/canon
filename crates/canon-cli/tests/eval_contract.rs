//! Deterministic black-box eval contract for the Canon CLI.
//!
//! These tests intentionally invoke the built `canon` binary and grade only
//! consumer-visible behavior: context JSON stability/shape, canonical skill
//! projections for OMP and Pi, and a dispatch usage refusal. Temp roots are
//! fixtures, not production records; no generated IDs, timestamps, or paths
//! are part of the assertions.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::Command;

use serde_json::Value;
use sha2::{Digest, Sha256};

type FixtureFiles = BTreeMap<&'static str, &'static [u8]>;

const REGIME: &str = "dev/canon/join-spine/9c93d024b1a2";
const SKILL_SOURCE: &[u8] =
    b"---\nname: canon\ndescription: Eval skill\nx-extra: preserved\n---\n\n# Canon Eval\n";
const REFERENCE_TOPIC: &[u8] = b"# Topic\n";
const SCRIPT: &[u8] = b"#!/bin/sh\necho eval\n";

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

fn canonical_source_files() -> FixtureFiles {
    BTreeMap::from([
        ("SKILL.src.md", SKILL_SOURCE),
        ("reference/topic.md", REFERENCE_TOPIC),
        ("scripts/pre-dispatch.sh", SCRIPT),
    ])
}

fn bundle_hash(files: &FixtureFiles) -> String {
    let mut hasher = Sha256::new();
    for (relative, bytes) in files {
        hasher.update(relative.as_bytes());
        hasher.update([0]);
        hasher.update(bytes);
        hasher.update([0]);
    }
    format!("sha256:{:x}", hasher.finalize())
}

fn write_skill_source(root: &Path) {
    std::fs::create_dir_all(root.join("reference")).unwrap();
    std::fs::create_dir_all(root.join("scripts")).unwrap();
    std::fs::write(root.join("SKILL.src.md"), SKILL_SOURCE).unwrap();
    std::fs::write(root.join("reference/topic.md"), REFERENCE_TOPIC).unwrap();
    std::fs::write(root.join("scripts/pre-dispatch.sh"), SCRIPT).unwrap();
}

#[test]
fn context_json_is_byte_stable_and_has_the_public_capability_shape() {
    let repo = tempfile::tempdir().unwrap();
    let first = Command::new(env!("CARGO_BIN_EXE_canon"))
        .args(["context", "--repo"])
        .arg(repo.path())
        .arg("--json")
        .output()
        .expect("spawn canon context");
    let second = Command::new(env!("CARGO_BIN_EXE_canon"))
        .args(["context", "--repo"])
        .arg(repo.path())
        .arg("--json")
        .output()
        .expect("spawn canon context");

    assert!(first.status.success(), "stderr: {}", String::from_utf8_lossy(&first.stderr));
    assert!(second.status.success(), "stderr: {}", String::from_utf8_lossy(&second.stderr));
    assert_eq!(first.stdout, second.stdout, "unchanged context input must produce byte-identical JSON");

    let surface: Value = serde_json::from_slice(&first.stdout).expect("context --json must be valid JSON");
    let object = surface.as_object().expect("context JSON must be an object");
    let mut keys: Vec<_> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        ["capabilityVersion", "cel", "enums", "joinKeys", "kinds", "policy", "review", "vocab"],
        "context JSON public top-level shape changed"
    );
    assert_eq!(surface["capabilityVersion"], 5);
    assert_eq!(surface["review"]["findingSeverities"], serde_json::json!(["blocker", "should-fix", "note"]));
    assert_eq!(surface["review"]["findingDispositions"], serde_json::json!(["open", "fixed", "rejected", "deferred"]));
    assert_eq!(surface["review"]["reviewFields"], surface["kinds"]["review"]["envelope_fields"], "review fields come from the kinds projection");
    assert!(surface["review"]["requireReview"].is_null(), "a repo with no policy does not require review");
    assert_eq!(surface["kinds"].as_object().map(|k| k.len()), Some(14));
    assert_eq!(
        surface["cel"].as_object().map(|k| k.len()),
        surface["kinds"].as_object().map(|k| k.len()),
        "CEL projection must cover exactly the kind projection"
    );
    assert!(surface["enums"].as_object().is_some_and(|v| !v.is_empty()));
    assert!(surface["joinKeys"].as_object().is_some_and(|v| !v.is_empty()));
    assert!(surface["policy"].is_object());
    assert!(surface["policy"]["risk_tiers"].is_object());
    assert!(surface["vocab"].is_object());
}

#[test]
fn skills_install_projects_omp_and_pi_with_exact_files_and_hashes() {
    let fixture = tempfile::tempdir().unwrap();
    let source = fixture.path().join("source");
    let target = fixture.path().join("target");
    write_skill_source(&source);

    let first = Command::new(env!("CARGO_BIN_EXE_canon"))
        .args(["skills", "install", "--source"])
        .arg(&source)
        .args(["--target"])
        .arg(&target)
        .args(["--providers", "pi,OMP"])
        .output()
        .expect("spawn canon skills install");
    assert!(first.status.success(), "stderr: {}", String::from_utf8_lossy(&first.stderr));
    assert_eq!(String::from_utf8_lossy(&first.stdout), "canon v1 — installed (omp,pi)\n");

    let source_files = canonical_source_files();
    let expected_files: BTreeMap<String, &[u8]> = [
        (".omp/skills/canon/SKILL.md".to_string(), SKILL_SOURCE),
        (".omp/skills/canon/reference/topic.md".to_string(), REFERENCE_TOPIC),
        (".omp/skills/canon/scripts/pre-dispatch.sh".to_string(), SCRIPT),
        (".pi/skills/canon/SKILL.md".to_string(), SKILL_SOURCE),
        (".pi/skills/canon/reference/topic.md".to_string(), REFERENCE_TOPIC),
        (".pi/skills/canon/scripts/pre-dispatch.sh".to_string(), SCRIPT),
    ]
    .into_iter()
    .collect();
    for (relative, expected) in &expected_files {
        assert_eq!(std::fs::read(target.join(relative)).unwrap(), *expected, "projection bytes for {relative}");
    }
    assert!(!target.join(".claude").exists());
    assert!(!target.join(".codex").exists());

    let manifest: Value = serde_json::from_slice(&std::fs::read(target.join(".canon/skills/.install-lock.json")).unwrap())
        .expect("install manifest must be valid JSON");
    assert_eq!(manifest["version"], 1);
    assert_eq!(manifest["providers"], serde_json::json!(["omp", "pi"]));
    assert_eq!(manifest["source_hash"], bundle_hash(&source_files));
    let files = manifest["files"].as_object().expect("manifest files map");
    assert_eq!(files.len(), expected_files.len());
    for (relative, expected) in &expected_files {
        let expected_hash = sha256(expected);
        assert_eq!(
            files.get(relative).and_then(Value::as_str),
            Some(expected_hash.as_str()),
            "manifest hash for {relative}"
        );
    }

    let second = Command::new(env!("CARGO_BIN_EXE_canon"))
        .args(["skills", "install", "--source"])
        .arg(&source)
        .args(["--target"])
        .arg(&target)
        .args(["--providers", "OMP,pi"])
        .output()
        .expect("spawn canon skills install");
    assert!(second.status.success(), "stderr: {}", String::from_utf8_lossy(&second.stderr));
    assert_eq!(String::from_utf8_lossy(&second.stdout), "canon v1 — unchanged (omp,pi)\n");
}

#[test]
fn dispatch_refuses_a_role_regime_mismatch_as_a_stable_usage_error() {
    let repo = tempfile::tempdir().unwrap();
    let output = Command::new(env!("CARGO_BIN_EXE_canon"))
        .args(["dispatch", "begin", "--role", "reviewer", "--regime", REGIME, "--repo"])
        .arg(repo.path())
        .output()
        .expect("spawn canon dispatch begin");

    assert_eq!(output.status.code(), Some(2), "role/regime mismatch is a usage refusal");
    assert!(output.stdout.is_empty(), "refusal must not emit a success snapshot");
    assert_eq!(
        String::from_utf8_lossy(&output.stderr),
        "canon dispatch begin: --role `reviewer` does not match --regime `dev/canon/join-spine/9c93d024b1a2`'s own leading role segment `dev` — pass the SAME role to both\n"
    );
    assert!(!repo.path().join(".canon/dispatch").exists(), "a refused dispatch must not write a manifest");
}

