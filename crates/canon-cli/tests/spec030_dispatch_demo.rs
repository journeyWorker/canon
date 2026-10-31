//! 0.30 spec backfill, flows F1 (dispatch inputs) and F5 (demo): one test per
//! scenario in `dispatch/manifest.feature` (08-28) and `authoring/demo.feature`
//! (01-04), each pinning today's behaviour through the built `canon` binary.

use std::collections::BTreeMap;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

const ADAPTER_FIXTURES: &str = concat!(env!("CARGO_MANIFEST_DIR"), "/tests/fixtures/adapter");
const ZERO_PACK: &str = "sha256:0000000000000000000000000000000000000000000000000000000000000000";
const REGIME: &str = "dev/canon/join-spine/9c93d024b1a2";

fn run(dir: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).arg("--json").current_dir(dir).env_remove("CANON_ROLE").output().expect("spawn canon")
}

fn body(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("stdout is not one JSON document ({e}): {}", String::from_utf8_lossy(&out.stdout)))
}

fn ok(dir: &Path, args: &[&str]) -> Value {
    let out = run(dir, args);
    assert_eq!(out.status.code(), Some(0), "{args:?}: {}", String::from_utf8_lossy(&out.stdout));
    body(&out)
}

/// Runs `args`, expecting exit `code` and `error.code == err`; returns the body.
fn refused(dir: &Path, args: &[&str], code: i32, err: &str) -> Value {
    let out = run(dir, args);
    assert_eq!(out.status.code(), Some(code), "{args:?}: {}", String::from_utf8_lossy(&out.stdout));
    let b = body(&out);
    assert_eq!(b["ok"], false, "{b}");
    assert_eq!(b["error"]["code"], err, "{b}");
    b
}

/// A git repo with one commit, set up by `canon init`.
fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    for args in [&["init", "-q", "-b", "main"][..], &["config", "user.name", "t"], &["config", "user.email", "t@example.com"], &["commit", "-q", "--allow-empty", "-m", "x"]] {
        let out = Command::new("git").args(args).current_dir(d).output().unwrap();
        assert!(out.status.success(), "git {args:?}");
    }
    ok(d, &["init", "--providers", "none"]);
    dir
}

/// Every file under `dir` (except `.git`) with its bytes.
fn snapshot(dir: &Path) -> BTreeMap<PathBuf, Vec<u8>> {
    fn walk(root: &Path, at: &Path, out: &mut BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(at).unwrap() {
            let p = entry.unwrap().path();
            if p.file_name().is_some_and(|n| n == ".git") {
                continue;
            }
            if p.is_dir() {
                walk(root, &p, out);
            } else {
                out.insert(p.strip_prefix(root).unwrap().to_path_buf(), std::fs::read(&p).unwrap());
            }
        }
    }
    let mut out = BTreeMap::new();
    walk(dir, dir, &mut out);
    out
}

fn adapter(name: &str) -> String {
    format!("{ADAPTER_FIXTURES}/{name}")
}

const CONTEXT_MANIFEST: &str = r#"{"manifestVersion":1,"capabilityVersion":1,"selectedDocs":["README.md"],"examples":[],"toolSchemas":[],"injectedGuidance":[]}"#;

/// A repo holding README.md and a context manifest selecting it.
fn repo_with_context() -> TempDir {
    let dir = repo();
    std::fs::write(dir.path().join("README.md"), "hello\n").unwrap();
    std::fs::write(dir.path().join("cm.json"), CONTEXT_MANIFEST).unwrap();
    dir
}

fn add_pack(d: &Path) -> String {
    let b = ok(d, &["context-pack", "add", "--manifest", "cm.json"]);
    b["data"]["id"].as_str().unwrap().to_string()
}

fn prompt_manifest(file: &str) -> String {
    format!(r#"{{"manifestVersion":1,"capabilityVersion":1,"selectedDocs":[],"prompt":"{file}","examples":[],"toolSchemas":[],"injectedGuidance":[]}}"#)
}

// ---- adapter authorize ------------------------------------------------------

#[test]
fn dispatch_manifest_08_adapter_authorize_reports_execution_authorized_when_the_policy_budget_covers_the_declared_capabilities() {
    let dir = repo();
    let d = dir.path();
    let policy = d.join(".canon/policy.yaml");
    let mut text = std::fs::read_to_string(&policy).unwrap();
    text.push_str("\nadapter_capabilities:\n  runtime_enforced: true\n  filesystem: read-only\n");
    std::fs::write(&policy, text).unwrap();
    let b = ok(d, &["adapter", "authorize", "--response", &adapter("omp.json")]);
    let data = &b["data"];
    assert_eq!(data["executionAuthorized"], true, "{b}");
    assert_eq!(data["executionPerformed"], false);
    assert_eq!(data["sandboxEnforced"], false);
    assert_eq!(data["reasons"].as_array().unwrap().len(), 0);
    assert_eq!(data["capabilities"]["filesystem"], "read-only");
    assert_eq!(data["capabilities"]["network"], "none");
}

#[test]
fn dispatch_manifest_09_adapter_authorize_denies_by_default_when_the_policy_declares_no_capability_budget() {
    let dir = repo();
    let b = refused(dir.path(), &["adapter", "authorize", "--response", &adapter("omp.json")], 1, "ADAPTER_EXECUTION_NOT_AUTHORIZED");
    let data = &b["data"];
    assert_eq!(data["executionAuthorized"], false);
    assert_eq!(data["executionPerformed"], false);
    let reasons: Vec<&str> = data["reasons"].as_array().unwrap().iter().map(|r| r.as_str().unwrap()).collect();
    assert!(reasons.iter().any(|r| r.contains("runtime_enforced=false")), "{reasons:?}");
    assert!(reasons.iter().any(|r| r.starts_with("filesystem: declared capability exceeds policy budget")), "{reasons:?}");
}

#[test]
fn dispatch_manifest_10_adapter_authorize_on_a_missing_response_file_is_refused_as_a_fixable_invocation() {
    let dir = repo();
    let before = snapshot(dir.path());
    let b = refused(dir.path(), &["adapter", "authorize", "--response", "nope.json"], 2, "ADAPTER_REFUSED");
    assert!(b["error"]["message"].as_str().unwrap().contains("nope.json"), "{b}");
    assert_eq!(snapshot(dir.path()), before);
}

// ---- adapter validate -------------------------------------------------------

#[test]
fn dispatch_manifest_11_adapter_validate_prints_the_normalized_summary_of_a_well_formed_response() {
    let dir = repo();
    let b = ok(dir.path(), &["adapter", "validate", "--response", &adapter("omp.json")]);
    let data = &b["data"];
    assert_eq!(data["runId"], "run-omp-001");
    assert_eq!(data["provider"], "omp");
    assert_eq!(data["model"], "omp-default");
    assert_eq!(data["contextPackId"], "sha256:0123456789abcdef0123456789abcdef0123456789abcdef0123456789abcdef");
    assert_eq!(data["status"], "succeeded");
    assert_eq!(data["capabilities"]["filesystem"], "read-only");
    assert_eq!(data["evidenceCount"], 1);
    assert_eq!(data["telemetry"]["toolCalls"], 2);
    assert_eq!(data["extensionKeys"][0], "omp");
    assert_eq!(data["contextJoinVerified"], false);
}

#[test]
fn dispatch_manifest_12_adapter_validate_rejects_a_response_whose_envelope_breaks_a_rule() {
    let dir = repo();
    let before = snapshot(dir.path());
    let b = refused(dir.path(), &["adapter", "validate", "--response", &adapter("reject-negative-cost.json")], 1, "ADAPTER_RESPONSE_INVALID");
    assert!(b["error"]["message"].as_str().unwrap().contains("telemetry.cost_usd"), "{b}");
    assert_eq!(snapshot(dir.path()), before);
}

#[test]
fn dispatch_manifest_13_adapter_validate_on_a_missing_response_file_is_refused_as_a_fixable_invocation() {
    let dir = repo();
    let before = snapshot(dir.path());
    let b = refused(dir.path(), &["adapter", "validate", "--response", "nope.json"], 2, "ADAPTER_REFUSED");
    assert!(b["error"]["message"].as_str().unwrap().contains("nope.json"), "{b}");
    assert_eq!(snapshot(dir.path()), before);
}

// ---- context-pack -----------------------------------------------------------

#[test]
fn dispatch_manifest_14_context_pack_add_stores_an_immutable_pack_and_prints_its_id() {
    let dir = repo_with_context();
    let d = dir.path();
    let b = ok(d, &["context-pack", "add", "--manifest", "cm.json"]);
    let id = b["data"]["id"].as_str().unwrap();
    assert!(id.starts_with("sha256:") && id.len() == "sha256:".len() + 64, "{id}");
    assert_eq!(b["data"]["selectedDocs"][0]["path"], "README.md");
    assert!(b["data"]["selectedDocs"][0]["version"].as_str().unwrap().starts_with("sha256:"));
    let again = ok(d, &["context-pack", "add", "--manifest", "cm.json"]);
    assert_eq!(again["data"]["id"], id);
}

#[test]
fn dispatch_manifest_15_context_pack_add_refuses_a_manifest_that_names_a_missing_input_and_stores_nothing() {
    let dir = repo_with_context();
    let d = dir.path();
    std::fs::write(d.join("bad.json"), CONTEXT_MANIFEST.replace("README.md", "gone.md")).unwrap();
    let before = snapshot(d);
    let b = refused(d, &["context-pack", "add", "--manifest", "bad.json"], 2, "CONTEXT_INPUT_MISSING");
    assert!(b["error"]["message"].as_str().unwrap().contains("gone.md"), "{b}");
    assert_eq!(snapshot(d), before);
}

#[test]
fn dispatch_manifest_16_context_pack_show_prints_the_manifest_of_a_stored_pack() {
    let dir = repo_with_context();
    let d = dir.path();
    let id = add_pack(d);
    let b = ok(d, &["context-pack", "show", &id]);
    assert_eq!(b["data"]["id"], id.as_str());
    assert_eq!(b["data"]["selectedDocs"][0]["path"], "README.md");
}

#[test]
fn dispatch_manifest_17_context_pack_show_on_an_unknown_pack_id_is_refused() {
    let dir = repo_with_context();
    let d = dir.path();
    add_pack(d);
    let before = snapshot(d);
    let b = refused(d, &["context-pack", "show", ZERO_PACK], 2, "CONTEXT_PACK_NOT_FOUND");
    assert!(b["error"]["message"].as_str().unwrap().contains(ZERO_PACK), "{b}");
    assert_eq!(snapshot(d), before);
}

#[test]
fn dispatch_manifest_18_context_pack_verify_confirms_a_stored_pack_is_intact() {
    let dir = repo_with_context();
    let d = dir.path();
    let id = add_pack(d);
    let b = ok(d, &["context-pack", "verify", &id]);
    assert_eq!(b["data"]["id"], id.as_str());
    assert_eq!(b["data"]["verified"], true);
}

#[test]
fn dispatch_manifest_19_context_pack_verify_on_an_unknown_pack_id_is_refused() {
    let dir = repo_with_context();
    let d = dir.path();
    add_pack(d);
    let before = snapshot(d);
    let b = refused(d, &["context-pack", "verify", ZERO_PACK], 2, "CONTEXT_PACK_NOT_FOUND");
    assert!(b["error"]["message"].as_str().unwrap().contains(ZERO_PACK), "{b}");
    assert_eq!(snapshot(d), before);
}

// ---- prompt -----------------------------------------------------------------

/// A repo holding README.md and a prompt manifest `pm.json` naming it.
fn repo_with_prompt() -> TempDir {
    let dir = repo();
    std::fs::write(dir.path().join("README.md"), "hello\n").unwrap();
    std::fs::write(dir.path().join("pm.json"), prompt_manifest("README.md")).unwrap();
    dir
}

#[test]
fn dispatch_manifest_20_prompt_add_registers_a_prompt_bundle_under_name_and_version() {
    let dir = repo_with_prompt();
    let d = dir.path();
    let b = ok(d, &["prompt", "add", "reviewer@v1", "--manifest", "pm.json"]);
    let data = &b["data"];
    assert_eq!(data["name"], "reviewer");
    assert_eq!(data["version"], "v1");
    let digest = data["digest"].as_str().unwrap();
    assert!(digest.starts_with("sha256:"), "{digest}");
    assert_eq!(data["prompts"]["prompt"]["path"], "README.md");
    let again = ok(d, &["prompt", "add", "reviewer@v1", "--manifest", "pm.json"]);
    assert_eq!(again["data"]["digest"], digest);
}

#[test]
fn dispatch_manifest_21_prompt_add_refuses_an_operand_that_is_not_name_at_version_and_writes_nothing() {
    let dir = repo_with_prompt();
    let d = dir.path();
    let before = snapshot(d);
    let b = refused(d, &["prompt", "add", "reviewer", "--manifest", "pm.json"], 2, "USAGE");
    assert_eq!(b["error"]["field"], "<NAME@VERSION>");
    assert!(b["error"]["message"].as_str().unwrap().contains("NAME@VERSION"), "{b}");
    assert_eq!(snapshot(d), before);
}

#[test]
fn dispatch_manifest_22_prompt_add_refuses_different_content_under_an_already_registered_name_and_version() {
    let dir = repo_with_prompt();
    let d = dir.path();
    let first = ok(d, &["prompt", "add", "reviewer@v1", "--manifest", "pm.json"]);
    std::fs::write(d.join("other.md"), "different\n").unwrap();
    std::fs::write(d.join("pm2.json"), prompt_manifest("other.md")).unwrap();
    refused(d, &["prompt", "add", "reviewer@v1", "--manifest", "pm2.json"], 1, "CONTEXT_OBJECT_CONFLICT");
    let shown = ok(d, &["prompt", "show", "reviewer@v1"]);
    assert_eq!(shown["data"]["digest"], first["data"]["digest"]);
}

#[test]
fn dispatch_manifest_23_prompt_show_prints_a_registered_prompt_bundle() {
    let dir = repo_with_prompt();
    let d = dir.path();
    let added = ok(d, &["prompt", "add", "reviewer@v1", "--manifest", "pm.json"]);
    let b = ok(d, &["prompt", "show", "reviewer@v1"]);
    assert_eq!(b["data"]["name"], "reviewer");
    assert_eq!(b["data"]["version"], "v1");
    assert_eq!(b["data"]["digest"], added["data"]["digest"]);
    assert_eq!(b["data"]["prompts"]["prompt"]["path"], "README.md");
}

#[test]
fn dispatch_manifest_24_prompt_show_on_an_unregistered_bundle_is_refused() {
    let dir = repo_with_prompt();
    let d = dir.path();
    let before = snapshot(d);
    let b = refused(d, &["prompt", "show", "nope@v1"], 2, "PROMPT_BUNDLE_INVALID");
    assert!(b["error"]["message"].as_str().unwrap().contains("version is not registered"), "{b}");
    assert_eq!(snapshot(d), before);
}

// ---- dispatch end / diff ----------------------------------------------------

fn begin(d: &Path) -> String {
    let b = ok(d, &["dispatch", "begin", "--role", "dev", "--regime", REGIME]);
    b["data"]["runId"].as_str().unwrap().to_string()
}

fn manifest(d: &Path, run_id: &str) -> Vec<u8> {
    std::fs::read(d.join(".canon/dispatch").join(format!("{run_id}.json"))).unwrap()
}

#[test]
fn dispatch_manifest_25_dispatch_end_closes_a_begun_run_with_its_outcome() {
    let dir = repo();
    let d = dir.path();
    let run_id = begin(d);
    let b = ok(d, &["dispatch", "end", &run_id, "--run-outcome", "succeeded"]);
    assert_eq!(b["data"]["runId"], run_id.as_str());
    assert_eq!(b["data"]["status"], "succeeded");
    assert!(b["data"]["endedAt"].as_str().is_some_and(|t| !t.is_empty()), "{b}");
    let m: Value = serde_json::from_slice(&manifest(d, &run_id)).unwrap();
    assert_eq!(m["status"], "succeeded");
    assert!(m["ended_at"].is_string(), "{m}");
}

#[test]
fn dispatch_manifest_26_dispatch_end_on_an_already_ended_run_is_refused_and_the_manifest_is_unchanged() {
    let dir = repo();
    let d = dir.path();
    let run_id = begin(d);
    ok(d, &["dispatch", "end", &run_id, "--run-outcome", "succeeded"]);
    let before = manifest(d, &run_id);
    refused(d, &["dispatch", "end", &run_id, "--run-outcome", "failed"], 2, "DISPATCH_ALREADY_ENDED");
    assert_eq!(manifest(d, &run_id), before);
}

#[test]
fn dispatch_manifest_27_dispatch_end_on_a_run_that_was_never_begun_is_refused_and_writes_nothing() {
    let dir = repo();
    let d = dir.path();
    let before = snapshot(d);
    refused(d, &["dispatch", "end", "01ARZ3NDEKTSV4RRFFQ69G5FAV", "--run-outcome", "succeeded"], 2, "DISPATCH_RUN_NOT_FOUND");
    assert_eq!(snapshot(d), before);
}

#[test]
fn dispatch_manifest_28_dispatch_diff_outside_a_canon_repository_is_refused_and_writes_nothing() {
    let dir = TempDir::new().unwrap();
    let b = refused(dir.path(), &["dispatch", "diff"], 2, "CONFIG_MISSING");
    assert_eq!(b["error"]["field"], "--repo");
    assert!(snapshot(dir.path()).is_empty());
}

// ---- demo -------------------------------------------------------------------

fn demo_repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    ok(dir.path(), &["demo", "init", "--repo", dir.path().to_str().unwrap()]);
    dir
}

#[test]
fn authoring_demo_01_demo_init_scaffolds_a_demo_repo_whose_gate_starts_red() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let target = d.to_str().unwrap();
    let b = ok(d, &["demo", "init", "--repo", target]);
    assert_eq!(b["data"]["repo"], target);
    assert_eq!(b["data"]["scenario"], "auth.login.01");
    assert!(d.join("canon.yaml").is_file());
    refused(d, &["gate", "check"], 1, "GATE_VIOLATIONS");
}

#[test]
fn authoring_demo_02_demo_init_in_a_directory_that_already_has_a_canon_yaml_is_refused_and_changes_nothing() {
    let dir = demo_repo();
    let d = dir.path();
    let before = snapshot(d);
    refused(d, &["demo", "init", "--repo", d.to_str().unwrap()], 2, "INIT_CONFIG_EXISTS");
    assert_eq!(snapshot(d), before);
}

// 0.31 F1: a canon.yaml whose .gitignore holds no canon lines refuses too, and its policy file survives.
#[test]
fn authoring_demo_02_demo_init_refuses_a_config_whose_gitignore_has_no_canon_lines_and_keeps_the_policy() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    std::fs::write(d.join("canon.yaml"), "tiers: {}\n").unwrap();
    std::fs::create_dir_all(d.join(".canon")).unwrap();
    std::fs::write(d.join(".canon/policy.yaml"), "version: mine\n").unwrap();
    let before = snapshot(d);
    refused(d, &["demo", "init", "--repo", d.to_str().unwrap()], 2, "INIT_CONFIG_EXISTS");
    assert_eq!(snapshot(d), before);
    assert_eq!(std::fs::read_to_string(d.join(".canon/policy.yaml")).unwrap(), "version: mine\n");
    assert!(!d.join(".gitignore").exists());
}

#[test]
fn authoring_demo_03_demo_review_records_the_reviewer_review_and_the_gate_turns_green() {
    let dir = demo_repo();
    let d = dir.path();
    refused(d, &["gate", "check"], 1, "GATE_VIOLATIONS");
    let b = ok(d, &["demo", "review", "--repo", d.to_str().unwrap()]);
    assert_eq!(b["data"]["scenario"], "auth.login.01");
    let gate = ok(d, &["gate", "check"]);
    assert_eq!(gate["data"]["clean"], true);
    assert_eq!(gate["data"]["violations"].as_array().unwrap().len(), 0);
}

#[test]
fn authoring_demo_04_demo_review_in_a_directory_that_demo_init_never_set_up_is_refused_and_changes_nothing() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let before = snapshot(d);
    refused(d, &["demo", "review", "--repo", d.to_str().unwrap()], 2, "DEMO_NOT_INITIALIZED");
    assert_eq!(snapshot(d), before);
}

// ---- 0.31 fixes: F3 pack id format, F5 adapter exits --------------------------

const MALFORMED_PACK_IDS: [&str; 4] = ["sha256:abc", "", "sha256:ABCDEF0000000000000000000000000000000000000000000000000000000000", "nothex"];

fn pack_id_format_is_a_usage_error(verb: &str) {
    let dir = repo_with_context();
    let d = dir.path();
    add_pack(d);
    let before = snapshot(d);
    for id in MALFORMED_PACK_IDS {
        let b = refused(d, &["context-pack", verb, id], 2, "USAGE");
        assert!(b["error"]["message"].as_str().unwrap().contains("PACK_ID"), "{id}: {b}");
        assert_eq!(snapshot(d), before);
    }
    refused(d, &["context-pack", verb, ZERO_PACK], 2, "CONTEXT_PACK_NOT_FOUND");
}

#[test]
fn dispatch_manifest_29_context_pack_show_on_a_malformed_pack_id_is_a_usage_error() {
    pack_id_format_is_a_usage_error("show");
}

#[test]
fn dispatch_manifest_30_context_pack_verify_on_a_malformed_pack_id_is_a_usage_error() {
    pack_id_format_is_a_usage_error("verify");
}

#[test]
fn dispatch_manifest_31_adapter_authorize_on_a_response_whose_envelope_breaks_a_rule_is_refused_as_a_fixable_invocation() {
    let dir = repo();
    let d = dir.path();
    let before = snapshot(d);
    let b = refused(d, &["adapter", "authorize", "--response", &adapter("reject-negative-cost.json")], 2, "ADAPTER_REFUSED");
    assert!(b["error"]["message"].as_str().unwrap().contains("telemetry.cost_usd"), "{b}");
    assert_eq!(snapshot(d), before);

    let policy = d.join(".canon/policy.yaml");
    let good = std::fs::read_to_string(&policy).unwrap();
    std::fs::write(&policy, "adapter_capabilities: [unclosed\n").unwrap();
    let broken = snapshot(d);
    let b = refused(d, &["adapter", "authorize", "--response", &adapter("omp.json")], 2, "ADAPTER_REFUSED");
    assert!(b["error"]["hint"].as_str().unwrap().contains("policy file"), "{b}");
    assert_eq!(snapshot(d), broken);

    std::fs::write(&policy, good).unwrap();
    refused(d, &["adapter", "authorize", "--response", &adapter("omp.json")], 1, "ADAPTER_EXECUTION_NOT_AUTHORIZED");
}

#[test]
fn dispatch_manifest_32_schema_error_adapter_refused_states_the_exit_of_each_adapter_call() {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    let err = body(&run(d, &["schema", "--error", "ADAPTER_REFUSED"]));
    let meaning = err["data"]["error"]["meaning"].as_str().unwrap();
    assert_eq!(err["data"]["error"]["exit"], 2, "{err}");
    let exits = |leaf: &[&str]| -> Vec<(i64, String)> {
        let mut args = vec!["schema"];
        args.extend_from_slice(leaf);
        let doc = body(&run(d, &args));
        doc["data"]["exits"].as_array().unwrap_or_else(|| panic!("{doc}")).iter().map(|e| (e["code"].as_i64().unwrap(), e["meaning"].as_str().unwrap().to_string())).collect()
    };
    let authorize_exits = exits(&["adapter", "authorize"]);
    let exit2 = &authorize_exits.iter().find(|(code, _)| *code == 2).expect("authorize lists exit 2").1;
    assert!(exit2.contains("ADAPTER_REFUSED") && exit2.contains("envelope breaks a rule") && exit2.contains("policy file is broken"), "{exit2}");
    let exit1 = &authorize_exits.iter().find(|(code, _)| *code == 1).expect("authorize lists exit 1").1;
    assert!(exit1.contains("ADAPTER_EXECUTION_NOT_AUTHORIZED") && exit1.contains("only exit 1"), "{exit1}");
    let validate_exits = exits(&["adapter", "validate"]);
    assert!(validate_exits.iter().any(|(code, m)| *code == 1 && m.contains("envelope breaks a rule")), "{validate_exits:?}");
    assert!(validate_exits.iter().any(|(code, m)| *code == 2 && m.contains("unreadable")), "{validate_exits:?}");
    assert!(meaning.contains("adapter authorize") && meaning.contains("adapter validate") && meaning.contains("could not read the response file"), "{meaning}");
    let authorize = body(&run(d, &["schema", "adapter", "authorize"])).to_string();
    assert!(authorize.contains("ADAPTER_EXECUTION_NOT_AUTHORIZED") && authorize.contains("the only exit 1") && authorize.contains("policy file is broken"), "{authorize}");
    let validate = body(&run(d, &["schema", "adapter", "validate"])).to_string();
    assert!(validate.contains("envelope breaks a rule"), "{validate}");
}
