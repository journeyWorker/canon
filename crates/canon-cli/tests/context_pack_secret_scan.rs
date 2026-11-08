//! `canon-0-39-secret-scan`: `canon context-pack add` refuses a text only for what the `canon-model` scanner finds in it, and the refusal names
//! the file, the line and the rule. Against the built `canon` binary.

#[path = "support/hermetic.rs"]
mod hermetic;
use std::path::Path;
use std::process::{Command, Output};

use canon_learn::{ParquetStrategyStore, StrategyId, StrategyItem, StrategyItemFields, StrategyStore, TrajectoryId};
use canon_model::ids::{RegimeKey, RoleId};
use canon_model::secret_scan;
use chrono::Utc;
use serde_json::Value;
use tempfile::TempDir;

const REGIME: &str = "dev/canon/join-spine/9c93d024b1a2";

fn canon(dir: &Path, args: &[&str]) -> Output {
    hermetic::canon().args(args).arg("--json").current_dir(dir).output().expect("spawn canon")
}

fn body(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("stdout is not one JSON document ({e}): {}", String::from_utf8_lossy(&out.stdout)))
}

/// A git repository with one commit, set up by `canon init`.
fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    let d = dir.path();
    for args in [&["init", "-q", "-b", "main"][..], &["config", "user.name", "t"], &["config", "user.email", "t@example.com"], &["commit", "-q", "--allow-empty", "-m", "x"]] {
        let out = Command::new("git").args(args).current_dir(d).output().unwrap();
        assert!(out.status.success(), "git {args:?}");
    }
    let out = canon(d, &["init", "--no-harness"]);
    assert_eq!(out.status.code(), Some(0), "{}", String::from_utf8_lossy(&out.stdout));
    dir
}

const MANIFEST: &str = r#"{"manifestVersion":1,"capabilityVersion":1,"selectedDocs":["doc.md"],"examples":[],"toolSchemas":[],"injectedGuidance":[]}"#;

/// `context-pack add` of a file that holds `text`: the pack id, or the refusal's `error`.
fn add_file(dir: &Path, text: &str) -> Result<String, Value> {
    std::fs::write(dir.join("doc.md"), text).unwrap();
    std::fs::write(dir.join("m.json"), MANIFEST).unwrap();
    let out = canon(dir, &["context-pack", "add", "--manifest", "m.json"]);
    let b = body(&out);
    if b["ok"] == true {
        assert_eq!(out.status.code(), Some(0), "{b}");
        return Ok(b["data"]["id"].as_str().expect("a pack id").to_string());
    }
    assert_eq!(out.status.code(), Some(2), "{b}");
    Err(b["error"].clone())
}

fn upper(len: usize) -> String {
    "A1".repeat(len).chars().take(len).collect()
}

fn lower(len: usize, unit: &str) -> String {
    unit.repeat(len).chars().take(len).collect()
}

/// Every line of the brief's tables with the rule that refuses it (`None`: the line is plain text and becomes a pack). Each rule has a line that
/// matches and the two lines around its minimum length.
fn table() -> Vec<(String, Option<&'static str>)> {
    let plain = [
        "risk-high", "RISK-HIGH", "task-list", "disk-usage", "mask-all", "ask-user", "Akiashi", "AKIATSU", "sk-live-123456", "sk-documentation-for-onboarding-flow",
        "ghp_", "github_pat_", "-----BEGIN CERTIFICATE-----", "-----BEGIN PUBLIC KEY-----", "api_key=", "api_key=<your key>", "api_key=${API_KEY}", "api_key=YOUR_API_KEY",
        "api_key: string", "max_api_key_len = 4", "sk-word-another-word-and-another-word-and-more-words-without-any-digit",
    ];
    let mut rows: Vec<(String, Option<&'static str>)> = plain.iter().map(|line| ((*line).to_string(), None)).collect();
    let refused = |text: String, rule: &'static str| (text, Some(rule));
    rows.extend([
        refused(format!("AKIA{}", upper(16)), "aws-access-key-id"),
        (format!("AKIA{}", upper(15)), None),
        refused(format!("ghp_{}", lower(36, "a1")), "github-token"),
        (format!("ghp_{}", lower(35, "a1")), None),
        refused(format!("github_pat_{}", lower(22, "a_1")), "github-fine-grained-token"),
        (format!("github_pat_{}", lower(21, "a_1")), None),
        refused(format!("sk-{}", lower(20, "ab1-")), "api-secret-key"),
        (format!("sk-{}", lower(19, "ab1-")), None),
        refused("sk-proj-abcdEFGH_ijkl-mnop1234".to_string(), "api-secret-key"),
        refused("-----BEGIN RSA PRIVATE KEY-----".to_string(), "private-key"),
        refused("-----BEGIN OPENSSH PRIVATE KEY-----".to_string(), "private-key"),
        refused("api_key=do-not-store".to_string(), "named-value"),
        refused("api_key = \"abcd1234\"".to_string(), "named-value"),
        refused("export OPENAI_API_KEY=abcd1234".to_string(), "named-value"),
        refused(r#"{"access_token": "abcd1234"}"#.to_string(), "named-value"),
        ("api_key=abcdefg".to_string(), None),
    ]);
    rows
}

/// One row of the table: the scanner and `context-pack add` both judge `line`, and judge it as the table says (`None`: plain text that becomes a pack).
fn judge(repo: &Path, line: &str, rule: Option<&str>) {
    let text = format!("{line}\n");
    let spans = secret_scan::scan(&text);
    assert_eq!(spans.first().map(|span| span.kind.rule_name()), rule, "the scanner on `{line}`");
    match (add_file(repo, &text), rule) {
        (Ok(id), None) => assert!(id.starts_with("sha256:"), "`{line}`: {id}"),
        (Err(error), Some(rule)) => {
            assert_eq!(error["code"], "CONTEXT_INPUT_INVALID", "`{line}`: {error}");
            assert_eq!(error["reason"], "SECRET", "`{line}`: {error}");
            assert_eq!(error["message"], format!("selected context input contains plaintext secret material: doc.md (line 1, {rule})"), "`{line}`");
        }
        (Ok(_), Some(rule)) => panic!("`{line}` should be refused ({rule}) but became a pack"),
        (Err(error), None) => panic!("`{line}` should become a pack but was refused: {error}"),
    }
}

/// dispatch.secret-scan.01 (F1): the lines of the table that are plain text, the lookalikes and the lines just below a rule's minimum length, become packs.
#[test]
fn dispatch_secret_scan_01_the_lines_that_only_look_like_secrets_become_packs() {
    let repo = repo();
    for (line, rule) in table().into_iter().filter(|(_, rule)| rule.is_none()) {
        judge(repo.path(), &line, rule);
    }
}

/// dispatch.secret-scan.14 (F2): the lines of the table that match a rule are refused with `CONTEXT_INPUT_INVALID`, reason `SECRET`, naming the rule.
#[test]
fn dispatch_secret_scan_14_the_lines_that_match_a_rule_are_refused_naming_the_rule() {
    let repo = repo();
    for (line, rule) in table().into_iter().filter(|(_, rule)| rule.is_some()) {
        judge(repo.path(), &line, rule);
    }
}

/// dispatch.secret-scan.23 (F4 failure path): every line of the table is judged as the table says, the scanner and `context-pack add` agree on each,
/// and a line they judge differently fails the test.
#[test]
fn dispatch_secret_scan_23_every_line_of_the_table_is_judged_the_same_by_the_scanner_and_by_context_pack_add() {
    let repo = repo();
    for (line, rule) in table() {
        judge(repo.path(), &line, rule);
    }
}

/// dispatch.secret-scan.13 (F2 failure path): a refusal names the file, the line and the rule, never the value, and stores nothing.
#[test]
fn dispatch_secret_scan_13_a_refusal_names_the_file_the_line_and_the_rule_and_stores_nothing() {
    let repo = repo();
    let d = repo.path();
    let key = format!("AKIA{}", upper(16));
    let error = add_file(d, &format!("first line\nsecond line\nthe key is {key}\n")).unwrap_err();
    let message = error["message"].as_str().unwrap();
    assert_eq!(message, "selected context input contains plaintext secret material: doc.md (line 3, aws-access-key-id)");
    assert!(!message.contains(&key) && !message.contains("AKIA"), "the message does not carry the value: {message}");
    assert_eq!(error["reason"], "SECRET");
    assert_eq!(error["retryable"], false);
    assert!(!d.join(".canon/context-packs").exists(), "a refusal leaves nothing in the pack store");

    // The first secret of the file is the one named, and a CRLF file counts the same lines.
    let error = add_file(d, &format!("one\r\ntwo\r\napi_key=abcd1234\r\n{key}\r\n")).unwrap_err();
    assert_eq!(error["message"], "selected context input contains plaintext secret material: doc.md (line 3, named-value)");
    assert!(!d.join(".canon/context-packs").exists());
}

fn seed_strategy(repo: &Path, content: &str) -> StrategyId {
    let store = ParquetStrategyStore::open(repo.join(".canon").join("learn").join("strategies"));
    let id = StrategyId::new();
    let item = StrategyItem::new(StrategyItemFields {
        id,
        regime_key: RegimeKey::parse(REGIME).unwrap(),
        role: RoleId::parse("dev").unwrap(),
        title: "guidance".into(),
        description: "one-liner".into(),
        content: content.into(),
        source_trajectory_ids: vec![TrajectoryId::new()],
        recorded_at: Utc::now(),
    });
    store.append(&item).expect("seed strategy");
    id
}

/// dispatch.secret-scan.15 (F3 failure path): `dispatch begin` scans the injected guidance as written and refuses a secret, naming the strategy; plain guidance becomes a pack.
#[test]
fn dispatch_secret_scan_15_injected_guidance_is_scanned_with_the_same_rules_as_a_file() {
    let key = format!("AKIA{}", upper(16));
    let secret = repo();
    let id = seed_strategy(secret.path(), &format!("do not\ncommit {key}"));
    let out = canon(secret.path(), &["dispatch", "begin", "--role", "dev", "--regime", REGIME]);
    let b = body(&out);
    assert_eq!(out.status.code(), Some(2), "{b}");
    assert_eq!(b["error"]["code"], "CONTEXT_INPUT_INVALID", "{b}");
    assert_eq!(b["error"]["retryable"], false, "{b}");
    assert_eq!(b["error"]["message"], format!("selected context input contains plaintext secret material: injected guidance {id} (line 2, aws-access-key-id)"), "{b}");
    assert!(!secret.path().join(".canon/context-packs").exists(), "a refused dispatch stores no pack");

    let plain = repo();
    seed_strategy(plain.path(), "risk-high");
    let out = canon(plain.path(), &["dispatch", "begin", "--role", "dev", "--regime", REGIME]);
    let b = body(&out);
    assert_eq!(out.status.code(), Some(0), "{b}");
    assert_eq!(b["data"]["injectedGuidance"][0]["content"], "risk-high");
}

/// dispatch.secret-scan.17 (F3 failure path): a quoted value in guidance is judged as written, not as its JSON text (whose `\"` would hide the quote), so the verdict is the file's.
#[test]
fn dispatch_secret_scan_17_a_quoted_value_in_guidance_is_refused_like_the_same_text_in_a_file() {
    let repo = repo();
    let d = repo.path();
    for (text, rule) in [(r#"{"api_key": "abcd1234efgh"}"#, Some("named-value")), (r#"{"api_key": "short"}"#, None), ("risk-high", None)] {
        let from_file = add_file(d, text);
        let manifest = serde_json::json!({
            "manifestVersion": 1, "capabilityVersion": 1, "selectedDocs": [], "examples": [], "toolSchemas": [],
            "injectedGuidance": [{"strategy_id": "s-1", "title": "t", "content": text}],
        });
        std::fs::write(d.join("g.json"), manifest.to_string()).unwrap();
        let out = canon(d, &["context-pack", "add", "--manifest", "g.json"]);
        let b = body(&out);
        match rule {
            Some(rule) => {
                let message = format!("selected context input contains plaintext secret material: injected guidance s-1 (line 1, {rule})");
                assert_eq!(b["error"]["message"], message.as_str(), "guidance `{text}`: {b}");
                assert_eq!(from_file.unwrap_err()["message"], message.replace("injected guidance s-1", "doc.md").as_str(), "file `{text}`");
            }
            None => {
                assert_eq!(b["ok"], true, "guidance `{text}`: {b}");
                assert!(from_file.is_ok(), "file `{text}`");
            }
        }
    }
    // The title is scanned too.
    let manifest = serde_json::json!({
        "manifestVersion": 1, "capabilityVersion": 1, "selectedDocs": [], "examples": [], "toolSchemas": [],
        "injectedGuidance": [{"strategy_id": "s-2", "title": "token ghp_x\napi_key=abcd1234", "content": "plain"}],
    });
    std::fs::write(d.join("t.json"), manifest.to_string()).unwrap();
    let b = body(&canon(d, &["context-pack", "add", "--manifest", "t.json"]));
    assert_eq!(b["error"]["message"], "selected context input contains plaintext secret material: injected guidance s-2 (line 2, named-value)", "{b}");
}
