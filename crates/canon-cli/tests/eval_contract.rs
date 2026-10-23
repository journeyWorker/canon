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

const REGIME: &str = "dev/canon/join-spine/9c93d024b1a2";
const SKILL_SOURCE: &[u8] =
    b"---\nname: canon\ndescription: Eval skill\nx-extra: preserved\n---\n\n# Canon Eval\n";
const REFERENCE_TOPIC: &[u8] = b"# Topic\n";
const VERSION: &str = env!("CARGO_PKG_VERSION");
const SCRIPT: &[u8] = b"#!/bin/sh\necho eval\n";

fn sha256(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

/// The stub `SKILL.md` an install writes from `SKILL_SOURCE`'s frontmatter.
fn stub() -> Vec<u8> {
    canon_cli::guide::stub(std::str::from_utf8(SKILL_SOURCE).unwrap(), VERSION).into_bytes()
}

/// What an install bundles (the stub and the scripts, never `reference/**`), by path in the bundle.
fn bundle_files() -> BTreeMap<&'static str, Vec<u8>> {
    BTreeMap::from([("SKILL.src.md", stub()), ("scripts/pre-dispatch.sh", SCRIPT.to_vec())])
}

fn bundle_hash(files: &BTreeMap<&'static str, Vec<u8>>) -> String {
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

    let envelope: Value = serde_json::from_slice(&first.stdout).expect("context --json must be valid JSON");
    let surface = envelope["data"].clone();
    let object = surface.as_object().expect("context JSON must be an object");
    let mut keys: Vec<_> = object.keys().map(String::as_str).collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        ["capabilityVersion", "cel", "commands", "enums", "families", "joinKeys", "kinds", "policy", "review", "vocab"],
        "context JSON public top-level shape changed"
    );
    assert_eq!(surface["capabilityVersion"], 6);
    assert_eq!(surface["review"]["findingSeverities"], serde_json::json!(["blocker", "should-fix", "note"]));
    assert_eq!(surface["review"]["findingDispositions"], serde_json::json!(["open", "fixed", "rejected", "deferred"]));
    assert_eq!(surface["review"]["reviewFields"], surface["kinds"]["review"]["envelopeFields"], "review fields come from the kinds projection");
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
        .args(["skills", "install", "--skills-source"])
        .arg(&source)
        .args(["--repo"])
        .arg(&target)
        .args(["--providers", "pi,OMP"])
        .output()
        .expect("spawn canon skills install");
    assert!(first.status.success(), "stderr: {}", String::from_utf8_lossy(&first.stderr));
    let stub = stub();
    let expected_files: BTreeMap<String, Vec<u8>> = [
        (".omp/skills/canon/SKILL.md".to_string(), stub.clone()),
        (".omp/skills/canon/scripts/pre-dispatch.sh".to_string(), SCRIPT.to_vec()),
        (".pi/skills/canon/SKILL.md".to_string(), stub),
        (".pi/skills/canon/scripts/pre-dispatch.sh".to_string(), SCRIPT.to_vec()),
    ]
    .into_iter()
    .collect();
    for (relative, expected) in &expected_files {
        assert_eq!(&std::fs::read(target.join(relative)).unwrap(), expected, "projection bytes for {relative}");
    }
    for provider in [".omp", ".pi"] {
        assert!(!target.join(provider).join("skills/canon/reference").exists(), "the guide is not projected: `canon skills show` prints it");
    }
    assert!(!target.join(".claude").exists());
    assert!(!target.join(".codex").exists());
    assert!(!target.join(".agents").exists());

    let manifest: Value = serde_json::from_slice(&std::fs::read(target.join(".canon/skills/.install-lock.json")).unwrap())
        .expect("install manifest must be valid JSON");
    assert_eq!(manifest["version"], 1);
    assert_eq!(manifest["providers"], serde_json::json!(["omp", "pi"]));
    assert_eq!(manifest["source_hash"], bundle_hash(&bundle_files()));
    assert_eq!(manifest["canon_version"], VERSION);
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
        .args(["skills", "install", "--skills-source"])
        .arg(&source)
        .args(["--repo"])
        .arg(&target)
        .args(["--providers", "OMP,pi"])
        .output()
        .expect("spawn canon skills install");
    assert!(second.status.success(), "stderr: {}", String::from_utf8_lossy(&second.stderr));
    let again = Command::new(env!("CARGO_BIN_EXE_canon"))
        .args(["skills", "install", "--json", "--skills-source"])
        .arg(&source)
        .args(["--repo"])
        .arg(&target)
        .args(["--providers", "OMP,pi"])
        .output()
        .expect("spawn canon skills install");
    let again: Value = serde_json::from_slice(&again.stdout).expect("the install report is JSON");
    assert_eq!(again["data"]["changed"], false, "a second install changes nothing");
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
    assert!(String::from_utf8_lossy(&output.stderr).contains("DISPATCH_ROLE_REGIME_MISMATCH"), "{}", String::from_utf8_lossy(&output.stderr));
    assert!(!repo.path().join(".canon/dispatch").exists(), "a refused dispatch must not write a manifest");
}

