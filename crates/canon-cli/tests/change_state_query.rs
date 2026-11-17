//! `canon-0-42-change-drop` F4: a closing decision is read back with `canon query --kind change_state` and `canon search`.
//!
//! change.drop.16 the decision is found by change, by state and with its history, and by a word of its reason or its refs with the change
//! and its subject as neighbours; change.drop.17 the refusals and the empty answers: an unknown `--status` is a `USAGE` listing the three
//! states, a change never closed is an empty list, and a `canon.yaml` without the routing line fails the query naming the kind.
//!
//! The records are written straight into a temporary repo's ledger, not by `canon change defer|drop|reopen`.

#[path = "support/hermetic.rs"]
mod hermetic;

use std::path::Path;
use std::process::{Command, Output};

use serde_json::{json, Value};
use tempfile::TempDir;

const YAML: &str = "tiers:\n  local: { backend: git, root: .canon/ledger }\nrouting:\n  change: local\n  subject: local\n  scenario: local\n  review: local\n  divergence: local\n  evidence: local\n  finding: local\n  change_state: local\nspecs:\n  roots:\n    - id: root\n      root: .canon/specs\nwrite:\n  promote: auto\n";
const IGNORE: &str = ".canon/index/\n.canon/hot.db*\n";

fn canon(args: &[&str], cwd: &Path) -> Output {
    hermetic::canon().args(args).current_dir(cwd).env_remove("CANON_ROLE").env("CANON_ACTOR", "change-state-test").output().expect("spawn canon")
}

fn text(output: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr))
}

fn json_of(output: &Output) -> Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("not JSON ({e}): {}", text(output)))
}

fn data(output: &Output) -> Value {
    let doc = json_of(output);
    assert_eq!(doc["ok"], true, "{}", text(output));
    doc["data"].clone()
}

fn git(dir: &Path, args: &[&str]) {
    let out = Command::new("git").args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "gc.auto=0"]).args(args).current_dir(dir).output().expect("git");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

/// One record straight into the ledger.
fn put(repo: &Path, body: Value) {
    use canon_store::tier::{RawWrite, Tier};
    let tier = canon_store::git_tier::GitTier::new(repo.join(".canon/ledger"));
    tier.write(&RawWrite(canon_model::RawRecord(body))).expect("write the record");
}

fn state(change: &str, state: &str, at: &str, reason: &str, refs: &[&str]) -> Value {
    json!({
        "actor": {"agent_id": "test-agent", "role": "planner"},
        "approver": "journey",
        "at": at,
        "change_id": change,
        "kind": "change_state",
        "reason": reason,
        "refs": refs,
        "schema": 1,
        "state": state,
    })
}

/// A git repo whose ledger holds: the change `alpha-change` (of the subject `alpha-subject`) deferred, then reopened; the change
/// `beta-change` dropped; and the change `gamma-change`, never closed. `canon.yaml` is `yaml`.
fn repo_with(yaml: &str) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    std::fs::write(repo.join("canon.yaml"), yaml).unwrap();
    std::fs::write(repo.join(".gitignore"), IGNORE).unwrap();
    std::fs::create_dir_all(repo.join(".canon/specs")).unwrap();
    std::fs::write(repo.join(".canon/specs/.keep"), "").unwrap();
    let actor = json!({"agent_id": "plan", "role": "planner"});
    for (change, subjects) in [("alpha-change", json!(["alpha-subject"])), ("beta-change", json!([])), ("gamma-change", json!([]))] {
        put(repo, json!({"actor": actor, "at": "2026-11-01T00:00:00Z", "change_id": change, "kind": "change", "schema": 2, "status": "proposed", "subject_ids": subjects, "summary": change, "title": change}));
    }
    put(repo, json!({"actor": actor, "at": "2026-11-01T00:00:00Z", "change_ids": ["alpha-change"], "domain": "dev", "kind": "subject", "owner_role": "implementer", "schema": 1, "status": "building", "subject_id": "alpha-subject", "summary": "the alpha subject", "title": "alpha subject"}));
    put(repo, state("alpha-change", "deferred", "2026-11-14T09:00:00Z", "the offline job exceeds budgetword minutes", &["https://example.com/pull/147"]));
    put(repo, state("alpha-change", "reopened", "2026-11-15T09:00:00Z", "the owner chose revivalword scope reduction", &["see refword in the run log"]));
    put(repo, state("beta-change", "dropped", "2026-11-14T10:00:00Z", "the plan was cancelled", &["chat 2026-11-14"]));
    git(repo, &["init", "-q", "-b", "main"]);
    git(repo, &["add", "-A"]);
    git(repo, &["commit", "-q", "-m", "base"]);
    dir
}

fn query(repo: &Path, extra: &[&str]) -> Output {
    let mut argv = vec!["query", "--kind", "change_state"];
    argv.extend_from_slice(extra);
    argv.push("--json");
    canon(&argv, repo)
}

/// `(change_id, state)` of each record of a query answer, in order.
fn pairs(data: &Value) -> Vec<(String, String)> {
    data["records"].as_array().expect("records").iter().map(|r| (r["change_id"].as_str().unwrap().to_string(), r["state"].as_str().unwrap().to_string())).collect()
}

fn pair(change: &str, state: &str) -> (String, String) {
    (change.to_string(), state.to_string())
}

/// change.drop.16: the current state is the latest record of each change; `--change`, `--status` and `--history` pick from there.
#[test]
fn change_drop_16_query_reads_the_current_state_by_change_by_state_and_with_history() {
    let dir = repo_with(YAML);
    let r = dir.path();

    let all = data(&query(r, &[]));
    assert_eq!(all["kind"], "change_state");
    assert_eq!(pairs(&all), [pair("alpha-change", "reopened"), pair("beta-change", "dropped")], "one current record per change: the reopen replaced the deferral: {all}");

    let alpha = data(&query(r, &["--change", "alpha-change"]));
    assert_eq!(pairs(&alpha), [pair("alpha-change", "reopened")], "{alpha}");
    assert_eq!(alpha["records"][0]["approver"], "journey");
    assert_eq!(alpha["records"][0]["refs"], json!(["see refword in the run log"]));

    let dropped = data(&query(r, &["--status", "dropped"]));
    assert_eq!(pairs(&dropped), [pair("beta-change", "dropped")], "{dropped}");
    let deferred = data(&query(r, &["--status", "deferred"]));
    assert!(pairs(&deferred).is_empty(), "alpha is no longer deferred, its latest record is the reopen: {deferred}");

    let history = data(&query(r, &["--change", "alpha-change", "--history"]));
    assert_eq!(pairs(&history), [pair("alpha-change", "deferred"), pair("alpha-change", "reopened")], "the archive keeps the record the reopen superseded, oldest first: {history}");
    let old = data(&query(r, &["--change", "alpha-change", "--history", "--status", "deferred"]));
    assert_eq!(pairs(&old), [pair("alpha-change", "deferred")], "{old}");
    assert_eq!(old["records"][0]["reason"], "the offline job exceeds budgetword minutes");
}

/// change.drop.16: `canon search` finds the current record by a word of its reason or of its refs, and the change and its subject are
/// among the neighbours; a superseded version is no hit.
#[test]
fn change_drop_16_search_finds_the_record_by_reason_and_refs_with_its_change_and_subject_as_neighbours() {
    let dir = repo_with(YAML);
    let r = dir.path();

    for word in ["revivalword", "refword"] {
        let out = canon(&["search", word, "--kind", "change_state", "--json"], r);
        let doc = data(&out);
        let hits: Vec<&str> = doc["hits"].as_array().unwrap().iter().map(|h| h["id"].as_str().unwrap()).collect();
        assert_eq!(hits, ["change_state:alpha-change"], "{word}: {doc}");
        assert_eq!(doc["hits"][0]["kind"], "change_state");
        let neighbors: Vec<&str> = doc["neighbors"].as_array().unwrap().iter().map(|n| n["id"].as_str().unwrap()).collect();
        assert!(neighbors.contains(&"change:alpha-change") && neighbors.contains(&"subject:alpha-subject"), "{word}: the change and its subject: {doc}");
        let edges = doc["edges"].as_array().unwrap();
        assert!(edges.iter().any(|e| e["from"] == "change_state:alpha-change" && e["to"] == "change:alpha-change" && e["rel"] == "change_id"), "{edges:?}");
        assert!(edges.iter().any(|e| e["from"] == "change_state:alpha-change" && e["to"] == "subject:alpha-subject"), "{edges:?}");
    }

    let dropped = data(&canon(&["search", "cancelled", "--kind", "change_state", "--json"], r));
    assert_eq!(dropped["hits"][0]["id"], "change_state:beta-change", "{dropped}");

    // Failure path: the reason of the record the reopen replaced is history, not a hit.
    let old = data(&canon(&["search", "budgetword", "--kind", "change_state", "--json"], r));
    assert!(old["hits"].as_array().unwrap().is_empty(), "{old}");
}

/// change.drop.17: a `--status` outside the three states is a usage fault that lists them, and nothing is read.
#[test]
fn change_drop_17_an_unknown_status_is_a_usage_error_listing_the_three_states() {
    let dir = repo_with(YAML);
    let r = dir.path();
    for value in ["open", "closed", "Deferred", "proposed"] {
        let out = query(r, &["--status", value]);
        assert_eq!(out.status.code(), Some(2), "{value}: {}", text(&out));
        let doc = json_of(&out);
        assert_eq!(doc["ok"], false, "{doc}");
        assert_eq!(doc["error"]["code"], "USAGE", "{value}: {doc}");
        assert_eq!(doc["error"]["field"], "--status", "{doc}");
        assert_eq!(doc["error"]["expected"], json!(["deferred", "dropped", "reopened"]), "{doc}");
    }
    // The other kinds keep their own code for a status outside their domain.
    let task = canon(&["query", "--kind", "task", "--status", "deferred", "--json"], r);
    assert_eq!(json_of(&task)["error"]["code"], "QUERY_ARGUMENT_INVALID");
    // `--domain` is a subject's alone.
    let domain = query(r, &["--domain", "dev"]);
    assert_eq!(domain.status.code(), Some(2), "{}", text(&domain));
    assert_eq!(json_of(&domain)["error"]["code"], "QUERY_ARGUMENT_INVALID");
}

/// change.drop.17: a change that was never closed is an empty list, not an error; so is a ledger with no change_state record.
#[test]
fn change_drop_17_a_change_never_closed_reads_as_an_empty_list() {
    let dir = repo_with(YAML);
    let r = dir.path();
    let gamma = data(&query(r, &["--change", "gamma-change"]));
    assert_eq!(gamma["count"], 0, "{gamma}");
    assert!(gamma["records"].as_array().unwrap().is_empty());
    let history = data(&query(r, &["--change", "gamma-change", "--history"]));
    assert_eq!(history["count"], 0, "{history}");

    let empty = tempfile::tempdir().unwrap();
    std::fs::write(empty.path().join("canon.yaml"), YAML).unwrap();
    let none = data(&query(empty.path(), &[]));
    assert_eq!(none["count"], 0, "{none}");
}

/// change.drop.17: with no `change_state` line in `routing:` the query fails (`QUERY_FAILED`, exit 1) and the message names the kind.
#[test]
fn change_drop_17_without_the_routing_line_the_query_fails_naming_the_kind() {
    let yaml = YAML.replace("  change_state: local\n", "");
    let dir = repo_with(&yaml);
    let out = query(dir.path(), &[]);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out));
    let doc = json_of(&out);
    assert_eq!(doc["ok"], false, "{doc}");
    assert_eq!(doc["error"]["code"], "QUERY_FAILED", "{doc}");
    // The kinds that are routed still read.
    let change = canon(&["query", "--kind", "change", "--json"], dir.path());
    assert_eq!(json_of(&change)["ok"], true, "{}", text(&change));
}
