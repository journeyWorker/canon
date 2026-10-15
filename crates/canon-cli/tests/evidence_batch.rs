//! 0.18 D1/D2 through the real binary: one report attests many
//! scenarios (`canon evidence add --all-scenarios`), a task's report with
//! no scenario id binds every case, a Swift Testing event stream binds,
//! one `canon review add` signs off many scenarios, and a write that
//! makes a pinned review stale names the review and its re-pin command.

use std::io::Write;
use std::path::{Path, PathBuf};
use std::process::{Command, Output, Stdio};

use canon_model::{Actor, ChangeId, Envelope, RecordKind, RoleId, Subject, SubjectId, SubjectStatus};
use canon_store::git_tier::GitTier;
use canon_store::tier::Tier;
use chrono::Utc;
use serde_json::Value;

const FEATURE: &str = ".canon/specs/kind=feature/area=shop/cart.feature";

fn feature(subject_tag: &str) -> String {
    let mut text = String::from("Feature: shop cart\n  # canon: {\"schema\":1,\"at\":\"2026-10-12T00:00:00Z\",\"actor\":{\"agent_id\":\"canon\"}}\n");
    for n in 1..=6 {
        let tag = if n == 1 { subject_tag } else { "" };
        text.push_str(&format!(
            "\n  # canon: {{\"schema\":1,\"at\":\"2026-10-12T00:00:00Z\",\"actor\":{{\"agent_id\":\"canon\"}}}}\n{tag}  @shop.cart.0{n}\n  Scenario: Cart behavior number {n}\n    When the shopper does thing {n}\n    Then the cart shows result {n}\n"
        ));
    }
    text
}

fn canon(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).env("CANON_ACTOR", "canon").current_dir(repo).output().expect("spawn canon")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

fn ok(repo: &Path, args: &[&str]) -> String {
    let out = canon(repo, args);
    assert!(out.status.success(), "canon {args:?}: {}{}", stdout(&out), stderr(&out));
    stdout(&out)
}

/// An initialized repo (promote on write) whose spec root `root` holds
/// `shop.cart.01`..`06`, indexed.
fn shop_repo(subject_tag: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    assert!(Command::new("git").args(["init", "-q"]).current_dir(repo).status().unwrap().success());
    ok(repo, &["init", "--repo", ".", "--no-policy"]);
    let path = repo.join(FEATURE);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, feature(subject_tag)).unwrap();
    ok(repo, &["inventory", "sync"]);
    dir
}

fn write(repo: &Path, path: &str, text: &str) {
    let path = repo.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, text).unwrap();
}

fn files_under(root: &Path, needle: &str) -> Vec<PathBuf> {
    fn walk(dir: &Path, needle: &str, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, needle, out);
            } else if path.to_string_lossy().contains(needle) {
                out.push(path);
            }
        }
    }
    let mut out = Vec::new();
    walk(root, needle, &mut out);
    out.sort();
    out
}

fn evidence_files(repo: &Path) -> Vec<PathBuf> {
    files_under(&repo.join(".canon"), "evidence_record")
}

/// The committed evidence records, by scenario id.
fn committed_evidence(repo: &Path) -> Vec<Value> {
    files_under(&repo.join(".canon/ledger/kind=evidence_record"), ".json")
        .into_iter()
        .map(|p| serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap())
        .collect()
}

fn reviews(repo: &Path) -> Vec<Value> {
    files_under(&repo.join(".canon/ledger/kind=review"), ".json").into_iter().map(|p| serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()).collect()
}

fn stored_blobs(repo: &Path) -> usize {
    std::fs::read_dir(repo.join(".canon/artifacts/sha256")).map(|d| d.count()).unwrap_or(0)
}

/// `canon evidence add --all-scenarios` over `report`, plus `extra`.
fn attest_all(repo: &Path, report: &str, extra: &[&str]) -> Output {
    let mut args = vec!["evidence", "add", "--project-id", "root", "--all-scenarios", "--report", report, "--kind", "test-run", "--ref", "npm test", "--role", "implementer", "--actor-id", "impl-agent"];
    args.extend_from_slice(extra);
    canon(repo, &args)
}

const MIXED: &str = r#"<testsuite>
  <testcase classname="cart" name="shop.cart.01 adds"/>
  <testcase classname="cart" name="shop.cart.02 refuses"><failure/></testcase>
  <testcase classname="cart" name="shop.cart.02 counts"/>
  <testcase classname="cart" name="shop.cart.03 keeps both"/>
</testsuite>"#;

/// evidence.add.80: one report, three scenarios, three records — each
/// with its own cases and verdict, all pinning the one stored report.
#[test]
fn evidence_add_80_one_report_writes_one_record_per_owned_scenario() {
    let dir = shop_repo("");
    let repo = dir.path();
    write(repo, "reports/junit.xml", MIXED);

    let out = attest_all(repo, "junit:reports/junit.xml", &[]);
    assert_eq!(out.status.code(), Some(0), "{}{}", stdout(&out), stderr(&out));
    let text = stdout(&out);
    for (id, verdict) in [("shop.cart.01", "faithful"), ("shop.cart.02", "divergent"), ("shop.cart.03", "faithful")] {
        assert_eq!(
            text.lines().filter(|l| l.contains(&format!(" for {id} (verdict {verdict}) — ")) && l.ends_with("then `canon gate check`")).count(),
            1,
            "one line per scenario with its verdict: {text}"
        );
    }
    assert!(text.contains("artifact store holds 1 bound file(s)"), "{text}");

    let records = committed_evidence(repo);
    assert_eq!(records.len(), 3, "{records:?}");
    let digests: Vec<&str> = records.iter().flat_map(|r| r["attachments"].as_array().unwrap()).map(|a| a["sha256"].as_str().unwrap()).collect();
    assert!(digests.windows(2).all(|w| w[0] == w[1]), "every record pins the same report blob");
    let cases = |id: &str| -> Vec<String> {
        let record = records.iter().find(|r| r["scenario_id"] == id).unwrap();
        record["attachments"].as_array().unwrap().iter().map(|a| a["case"].as_str().unwrap().to_string()).collect()
    };
    assert_eq!(cases("shop.cart.01"), ["shop.cart.01 adds"], "only its own cases");
    assert_eq!(cases("shop.cart.02"), ["shop.cart.02 refuses", "shop.cart.02 counts"], "only its own cases");
    assert_eq!(cases("shop.cart.03"), ["shop.cart.03 keeps both"], "only its own cases");
    assert_eq!(stored_blobs(repo), 1, "the report is stored once");
}

/// evidence.add.81: `--verdict faithful` over a batch with one failed
/// case refuses every record.
#[test]
fn evidence_add_81_a_faithful_claim_over_a_failed_case_refuses_the_batch() {
    let dir = shop_repo("");
    let repo = dir.path();
    write(repo, "reports/junit.xml", MIXED);

    let out = attest_all(repo, "junit:reports/junit.xml", &["--verdict", "faithful"]);
    assert_eq!(out.status.code(), Some(1), "{}{}", stdout(&out), stderr(&out));
    let err = stderr(&out);
    assert!(err.contains("shop.cart.02: reports/junit.xml records case `shop.cart.02 refuses` as failed"), "{err}");
    assert!(err.contains("no record of the batch is written"), "{err}");
    assert!(evidence_files(repo).is_empty(), "nothing staged or committed");
    assert_eq!(stored_blobs(repo), 0, "nothing stored");
}

/// evidence.add.82: an unindexed id is named and skipped; a report no
/// indexed scenario owns is refused.
#[test]
fn evidence_add_82_unindexed_ids_are_named_and_skipped() {
    let dir = shop_repo("");
    let repo = dir.path();
    write(
        repo,
        "reports/junit.xml",
        r#"<testsuite><testcase classname="cart" name="shop.cart.01 adds"/><testcase classname="cart" name="shop.cart.09 is not indexed"/><testcase classname="cart" name="helper works"/></testsuite>"#,
    );
    let out = attest_all(repo, "junit:reports/junit.xml", &[]);
    assert_eq!(out.status.code(), Some(0), "{}{}", stdout(&out), stderr(&out));
    let text = stdout(&out);
    assert!(
        text.contains("canon evidence add: skipped shop.cart.09 — not an indexed scenario of spec root `root` (case `shop.cart.09 is not indexed`); index it (`canon inventory sync`) or fix the test name"),
        "{text}"
    );
    assert!(text.contains("canon evidence add: skipped 1 case(s) that carry no scenario id"), "{text}");
    assert_eq!(committed_evidence(repo).len(), 1);

    write(repo, "reports/other.xml", r#"<testsuite><testcase classname="cart" name="shop.cart.09 is not indexed"/></testsuite>"#);
    let before = evidence_files(repo);
    let out = attest_all(repo, "junit:reports/other.xml", &[]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(stderr(&out).contains("belongs to an indexed scenario (not indexed: shop.cart.09)"), "{}", stderr(&out));
    assert_eq!(evidence_files(repo), before, "nothing written");
}

/// evidence.add.83: any refusal one record of a batch meets refuses all
/// of them — a brief, a fabrication marker — and the
/// flags that name one subject do not combine with `--all-scenarios`.
#[test]
fn evidence_add_83_any_refusal_refuses_the_whole_batch() {
    let dir = shop_repo("  @subject:demo\n");
    let repo = dir.path();
    let envelope = Envelope::new(1, RecordKind::Subject, Utc::now(), Actor::new("canon", RoleId::parse("implementer").unwrap()));
    let subject = Subject::new(envelope, SubjectId::parse("demo").unwrap(), "Demo", "s", "dev", SubjectStatus::Building, RoleId::parse("implementer").unwrap())
        .with_change_ids(vec![ChangeId::parse("c-demo").unwrap()]);
    GitTier::new(repo.join(".canon/ledger")).write(&subject).unwrap();
    write(repo, ".canon/changes/c-demo/brief.md", "## Why\n\nSeeded brief.\n\nStatus: draft, awaiting the human.\n");
    write(repo, "reports/junit.xml", r#"<testsuite><testcase name="shop.cart.01 adds"/><testcase name="shop.cart.02 counts"/></testsuite>"#);

    let out = attest_all(repo, "junit:reports/junit.xml", &[]);
    assert_eq!(out.status.code(), Some(1), "{}{}", stdout(&out), stderr(&out));
    let err = stderr(&out);
    assert!(err.contains("canon evidence add: BRIEF_UNAPPROVED — unreviewed-promotion demo — evidence for shop.cart.01: brief not approved"), "{err}");
    assert!(err.contains("hint: no record of the batch is written"), "{err}");
    assert!(evidence_files(repo).is_empty(), "shop.cart.02 is not attested either");

    write(repo, ".canon/changes/c-demo/brief.md", "## Why\n\nSeeded brief.\n\nApproved: Journey 2026-10-12 — \"go\"\n");
    let out = attest_all(repo, "junit:reports/junit.xml", &["--summary", "would pass"]);
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(stderr(&out).contains("canon evidence add: EVIDENCE_FABRICATED — fabricated-evidence"), "{}", stderr(&out));
    assert!(evidence_files(repo).is_empty(), "a fabricated summary writes nothing");

    let before = evidence_files(repo);

    for (extra, flag) in [
        (vec!["--scenario-id", "shop.cart.01"], "--scenario-id"),
        (vec!["--task", "c-demo#1.1"], "--task"),
        (vec!["--report-case", "x"], "--report-case"),
        (vec!["--approval-by", "alice", "--approval-role", "human"], "--approval-by, --approval-role"),
    ] {
        let out = attest_all(repo, "junit:reports/junit.xml", &extra);
        assert_eq!(out.status.code(), Some(2), "{flag}: {}", stderr(&out));
        assert!(stderr(&out).contains(&format!("EVIDENCE_INVALID_ARGUMENT — {flag}")) && stderr(&out).contains("do(es) not combine with --all-scenarios"), "{flag}: {}", stderr(&out));
    }
    let bare = canon(repo, &["evidence", "add", "--project-id", "root", "--all-scenarios", "--kind", "test-run", "--ref", "x", "--role", "implementer"]);
    assert_eq!(bare.status.code(), Some(2));
    assert!(stderr(&bare).contains("none was given"), "{}", stderr(&bare));
    let no_root = canon(repo, &["evidence", "add", "--all-scenarios", "--report", "junit:reports/junit.xml", "--kind", "test-run", "--ref", "x", "--role", "implementer"]);
    assert_eq!(no_root.status.code(), Some(2));
    assert!(stderr(&no_root).contains("--all-scenarios needs --project-id"), "{}", stderr(&no_root));
    assert_eq!(evidence_files(repo), before);
}

const TWO_CASES: &str = r#"<testsuite><testcase name="shop.cart.01 adds"/><testcase name="shop.cart.02 counts"/></testsuite>"#;

/// shop.cart.01 attested on its own with two screenshots, then 02 with
/// none; junit in place.
fn shots_repo() -> tempfile::TempDir {
    let dir = shop_repo("");
    let repo = dir.path();
    write(repo, "reports/junit.xml", TWO_CASES);
    write(repo, "shots/a.png", "frame a");
    write(repo, "shots/b.png", "frame b");
    ok(repo, &["evidence", "add", "--project-id", "root", "--scenario-id", "shop.cart.01", "--kind", "test-run", "--ref", "npm test", "--role", "implementer", "--artifact", "shots/a.png", "--artifact", "shots/b.png"]);
    dir
}

/// The newest committed record of `scenario` (promotion stamps `run_seq`).
fn latest_of(repo: &Path, scenario: &str) -> Value {
    committed_evidence(repo).into_iter().filter(|r| r["scenario_id"] == scenario).max_by_key(|r| r["run_seq"].as_u64().unwrap_or(0)).expect("a committed record")
}

fn paths_of(record: &Value) -> Vec<String> {
    let mut paths: Vec<String> = record["attachments"].as_array().unwrap().iter().map(|a| a["path"].as_str().unwrap().to_string()).collect();
    paths.sort();
    paths
}

/// evidence.add.90 (0.21 T7): a suite-wide batch that names no screenshot
/// carries the scenario's earlier attachments into its new record
/// unchanged, and `status` shows no dropped bindings.
#[test]
fn evidence_add_90_a_batch_carries_a_scenarios_earlier_attachments_forward() {
    let dir = shots_repo();
    let repo = dir.path();
    let first = latest_of(repo, "shop.cart.01");

    let out = attest_all(repo, "junit:reports/junit.xml", &["--json"]);
    assert_eq!(out.status.code(), Some(0), "{}{}", stdout(&out), stderr(&out));
    let json: Value = serde_json::from_slice(&out.stdout).unwrap();
    let records = json["data"]["records"].as_array().unwrap();
    let carried_of = |id: &str| records.iter().find(|r| r["subject"] == id).unwrap()["carried"].clone();
    assert_eq!(carried_of("shop.cart.01"), serde_json::json!(["shots/a.png", "shots/b.png"]), "{json}");
    assert_eq!(carried_of("shop.cart.02"), serde_json::json!([]), "{json}");

    let latest = latest_of(repo, "shop.cart.01");
    assert_eq!(paths_of(&latest), ["reports/junit.xml", "shots/a.png", "shots/b.png"], "{latest}");
    assert!(latest.get("attachment_drop").is_none(), "{latest}");
    let digest = |record: &Value, path: &str| record["attachments"].as_array().unwrap().iter().find(|a| a["path"] == path).unwrap()["sha256"].clone();
    assert_eq!(digest(&latest, "shots/a.png"), digest(&first, "shots/a.png"), "carried unchanged");

    let status: Value = serde_json::from_str(&ok(repo, &["status", "--json"])).unwrap();
    assert!(status["data"]["drops"].as_array().map_or(true, |d| d.is_empty()), "{status}");
    assert!(!ok(repo, &["status"]).contains("dropped bindings"));
}

/// evidence.add.91: `--drop-attachment` is the only way to lose a binding
/// in a batch; it applies to each scenario that binds the path and is
/// recorded as a drop.
#[test]
fn evidence_add_91_a_batch_drop_is_explicit_and_recorded() {
    let dir = shots_repo();
    let repo = dir.path();

    let out = attest_all(repo, "junit:reports/junit.xml", &["--drop-attachment", "shots/a.png", "--drop-reason", "frame a was retaken elsewhere"]);
    assert_eq!(out.status.code(), Some(0), "{}{}", stdout(&out), stderr(&out));
    let latest = latest_of(repo, "shop.cart.01");
    assert_eq!(paths_of(&latest), ["reports/junit.xml", "shots/b.png"], "{latest}");
    assert_eq!(latest["attachment_drop"], serde_json::json!({"paths": ["shots/a.png"], "reason": "frame a was retaken elsewhere"}), "{latest}");
    assert!(latest_of(repo, "shop.cart.02").get("attachment_drop").is_none());
    assert!(ok(repo, &["status"]).contains("dropped bindings (1):"));

    let before = evidence_files(repo);
    let out = attest_all(repo, "junit:reports/junit.xml", &["--drop-attachment", "shots/zzz.png", "--drop-reason", "nothing binds it"]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(stderr(&out).contains("EVIDENCE_BATCH_REFUSED") && stderr(&out).contains("`shots/zzz.png`"), "{}", stderr(&out));
    assert_eq!(evidence_files(repo), before, "a drop naming nothing writes nothing");
}

/// evidence.add.92: a batch that names a file at a bound path replaces
/// that binding (new digest) and still carries the others.
#[test]
fn evidence_add_92_a_batch_file_at_the_same_path_replaces_the_earlier_binding() {
    let dir = shots_repo();
    let repo = dir.path();
    let first = latest_of(repo, "shop.cart.01");
    write(repo, "shots/a.png", "frame a, retaken");

    let out = attest_all(repo, "junit:reports/junit.xml", &["--artifact", "shots/a.png", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}{}", stdout(&out), stderr(&out));
    let json: Value = serde_json::from_slice(&out.stdout).unwrap();
    let entry = json["data"]["records"].as_array().unwrap().iter().find(|r| r["subject"] == "shop.cart.01").unwrap().clone();
    assert_eq!(entry["carried"], serde_json::json!(["shots/b.png"]), "{json}");

    let latest = latest_of(repo, "shop.cart.01");
    assert_eq!(paths_of(&latest), ["reports/junit.xml", "shots/a.png", "shots/b.png"], "{latest}");
    let digest = |record: &Value, path: &str| record["attachments"].as_array().unwrap().iter().find(|a| a["path"] == path).unwrap()["sha256"].clone();
    assert_ne!(digest(&latest, "shots/a.png"), digest(&first, "shots/a.png"), "new bytes, new digest");
    assert_eq!(digest(&latest, "shots/b.png"), digest(&first, "shots/b.png"));
    assert!(latest.get("attachment_drop").is_none(), "{latest}");
}

/// evidence.add.93: a carried attachment whose blob the store lacks is
/// refused with its own code, not carried silently, and nothing is written.
#[test]
fn evidence_add_93_a_carried_blob_missing_from_the_store_is_refused() {
    let dir = shots_repo();
    let repo = dir.path();
    let first = latest_of(repo, "shop.cart.01");
    let sha = first["attachments"].as_array().unwrap().iter().find(|a| a["path"] == "shots/b.png").unwrap()["sha256"].as_str().unwrap().to_string();
    std::fs::remove_file(repo.join(".canon/artifacts/sha256").join(&sha)).unwrap();
    let before = evidence_files(repo);

    let out = attest_all(repo, "junit:reports/junit.xml", &[]);
    assert_eq!(out.status.code(), Some(2), "{}{}", stdout(&out), stderr(&out));
    assert!(stderr(&out).contains("EVIDENCE_CARRY_BLOB_MISSING") && stderr(&out).contains("`shots/b.png`") && stderr(&out).contains(&sha), "{}", stderr(&out));
    assert_eq!(evidence_files(repo), before, "nothing written");

    let single = canon(repo, &["evidence", "add", "--project-id", "root", "--scenario-id", "shop.cart.01", "--kind", "test-run", "--ref", "npm test", "--role", "implementer"]);
    assert_eq!(single.status.code(), Some(2), "{}", stderr(&single));
    assert!(stderr(&single).contains("EVIDENCE_CARRY_BLOB_MISSING"), "{}", stderr(&single));

    // Dropping it on purpose is the way out.
    ok(repo, &["evidence", "add", "--project-id", "root", "--scenario-id", "shop.cart.01", "--kind", "test-run", "--ref", "npm test", "--role", "implementer", "--drop-attachment", "shots/b.png", "--drop-reason", "blob lost"]);
    assert_eq!(paths_of(&latest_of(repo, "shop.cart.01")), ["shots/a.png"]);
}

/// evidence.add.84: `--task --report` with no scenario id binds every
/// case, and a faithful verdict needs all of them to pass.
#[test]
fn evidence_add_84_a_task_report_without_a_scenario_binds_every_case() {
    let dir = shop_repo("");
    let repo = dir.path();
    write(repo, ".canon/changes/c-demo/brief.md", "## Why\n\nA demo change.\n");
    write(repo, ".canon/changes/c-demo/tasks.md", "## 1. Build\n\n- [ ] 1.1 Build the demo\n");
    write(repo, "reports/failing.xml", r#"<testsuite><testcase name="helper adds"/><testcase name="helper removes"><failure/></testcase></testsuite>"#);
    write(repo, "reports/passing.xml", r#"<testsuite><testcase name="helper adds"/><testcase name="helper removes"/></testsuite>"#);
    let task = |report: &str| canon(repo, &["evidence", "add", "--task", "c-demo#1.1", "--report", report, "--kind", "test-run", "--ref", "npm test", "--role", "implementer"]);

    let out = task("junit:reports/failing.xml");
    assert_eq!(out.status.code(), Some(1), "{}", stderr(&out));
    assert!(stderr(&out).contains("records case `helper removes` as failed"), "{}", stderr(&out));
    assert!(evidence_files(repo).is_empty());

    let out = task("junit:reports/passing.xml");
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let records = committed_evidence(repo);
    let cases: Vec<&str> = records[0]["attachments"].as_array().unwrap().iter().map(|a| a["case"].as_str().unwrap()).collect();
    assert_eq!(cases, ["helper adds", "helper removes"]);
}

/// evidence.add.85: the checked-in stream of a real `swift test` run
/// binds through `--all-scenarios`, by display name then function name.
#[test]
fn evidence_add_85_a_swift_testing_stream_binds_every_scenario() {
    let dir = shop_repo("");
    let repo = dir.path();
    let fixture = Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/swift-testing/events.jsonl");
    write(repo, "reports/events.jsonl", &std::fs::read_to_string(fixture).unwrap());

    let out = attest_all(repo, "swift-testing:reports/events.jsonl", &[]);
    assert_eq!(out.status.code(), Some(0), "{}{}", stdout(&out), stderr(&out));
    let text = stdout(&out);
    for (id, verdict) in [
        ("shop.cart.01", "faithful"),
        ("shop.cart.02", "faithful"),
        ("shop.cart.03", "faithful"),
        ("shop.cart.04", "faithful"),
        ("shop.cart.05", "divergent"),
        ("shop.cart.06", "not-applicable"),
    ] {
        assert!(text.contains(&format!(" for {id} (verdict {verdict})")), "{id}: {text}");
    }
    let four = committed_evidence(repo).into_iter().find(|r| r["scenario_id"] == "shop.cart.04").unwrap();
    assert_eq!(four["attachments"][0]["case"], "shop_cart_04_count_never_negative()");
    assert_eq!(four["attachments"][0]["format"], "swift-testing");

    write(repo, "reports/bad.jsonl", "<testsuite/>\n");
    let out = attest_all(repo, "swift-testing:reports/bad.jsonl", &[]);
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("is not a readable Swift Testing event stream report: line 1:"), "{}", stderr(&out));
}

/// The words of a printed shell command (bare or single-quoted words).
fn shell_words(line: &str) -> Vec<String> {
    let mut words = Vec::new();
    let mut word = String::new();
    let mut quoted = false;
    let mut started = false;
    for c in line.chars() {
        match c {
            '\'' => {
                quoted = !quoted;
                started = true;
            }
            ' ' if !quoted => {
                if started {
                    words.push(std::mem::take(&mut word));
                    started = false;
                }
            }
            c => {
                word.push(c);
                started = true;
            }
        }
    }
    if started {
        words.push(word);
    }
    words
}

/// evidence.add.86: new evidence for a scenario whose current pinned
/// review saw the old evidence names the review and the command that
/// re-pins it; running that command clears the gap. A batch says so
/// too; a scenario with no review prints nothing.
#[test]
fn evidence_add_86_a_write_that_stales_a_pinned_review_names_the_re_pin() {
    let dir = shop_repo("");
    let repo = dir.path();
    std::fs::write(repo.join(".canon/policy.yaml"), "spec_coverage:\n  require_review:\n    scope: []\n").unwrap();
    let attest = |id: &str| canon(repo, &["evidence", "add", "--project-id", "root", "--scenario-id", id, "--kind", "test-run", "--ref", "npm test", "--role", "implementer", "--actor-id", "impl-agent"]);
    assert!(attest("shop.cart.01").status.success());
    ok(
        repo,
        &["review", "add", "--project-id", "root", "--scenario-id", "shop.cart.01", "--reviewer", "reviewer-2", "--actor-id", "reviewer-2", "--role", "reviewer", "--session-id", "sess-review", "--pin", "r1", "--upstream-ref", "tests/cart.rs#shop_cart_01"],
    );
    let git = |args: &[&str]| {
        let out = Command::new("git").args(args).current_dir(repo).env("GIT_AUTHOR_NAME", "t").env("GIT_AUTHOR_EMAIL", "t@t").env("GIT_COMMITTER_NAME", "t").env("GIT_COMMITTER_EMAIL", "t@t").output().unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    };
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "reviewed"]);
    let head = git(&["rev-parse", "HEAD"]);
    let gate = |repo: &Path| stdout(&canon(repo, &["gate", "check"]));
    assert!(!gate(repo).contains("unreviewed-promotion shop.cart.01"), "the review is current");

    let out = attest("shop.cart.01");
    assert!(out.status.success(), "{}", stderr(&out));
    let expected = format!(
        "canon evidence add: review by `reviewer-2` of shop.cart.01 is now stale (it pinned the evidence this record supersedes); re-pin it: canon review add --project-id root --scenario-id shop.cart.01 --reviewer reviewer-2 --actor-id reviewer-2 --role reviewer --pin {head} --session-id sess-review --upstream-ref 'tests/cart.rs#shop_cart_01'"
    );
    let text = stdout(&out);
    let line = text.lines().find(|l| l.contains("is now stale")).unwrap_or_else(|| panic!("no stale-review line: {text}"));
    assert_eq!(line, expected);
    assert!(gate(repo).contains("unreviewed-promotion shop.cart.01 — stale review by `reviewer-2`"), "the gate agrees");

    let command = line.split_once("re-pin it: canon ").unwrap().1;
    ok(repo, &shell_words(command).iter().map(String::as_str).collect::<Vec<_>>());
    assert!(!gate(repo).contains("unreviewed-promotion shop.cart.01"), "the re-pin makes the review current again");

    write(repo, "reports/junit.xml", r#"<testsuite><testcase name="shop.cart.01 adds"/><testcase name="shop.cart.02 counts"/></testsuite>"#);
    let out = attest_all(repo, "junit:reports/junit.xml", &[]);
    assert!(out.status.success(), "{}", stderr(&out));
    let text = stdout(&out);
    assert_eq!(text.lines().filter(|l| l.contains("is now stale")).count(), 1, "one line per stale review, none for shop.cart.02: {text}");
    assert!(text.contains("review by `reviewer-2` of shop.cart.01 is now stale"), "{text}");

    let out = attest("shop.cart.02");
    assert!(!stdout(&out).contains("stale"), "no review, no notice: {}", stdout(&out));
}

fn review_args<'a>(extra: &[&'a str]) -> Vec<&'a str> {
    let mut args = vec!["review", "add", "--project-id", "root", "--reviewer", "reviewer-2", "--actor-id", "reviewer-2", "--role", "reviewer", "--pin", "r1", "--original-spec-ref", FEATURE];
    args.extend_from_slice(extra);
    args
}

/// review.add.80: one invocation, one review per scenario, the same
/// reviewer, pin and artifacts, each pinning its own scenario.
#[test]
fn review_add_80_one_review_add_signs_off_several_scenarios() {
    let dir = shop_repo("");
    let repo = dir.path();
    write(repo, "ids.txt", "# the batch\nshop.cart.03  # third\n\n");
    write(repo, "shot.png", "png");
    let out = ok(repo, &review_args(&["--scenario-id", "shop.cart.01", "--scenario-id", "shop.cart.02", "--scenario-ids-from", "ids.txt", "--artifact", "shot.png"]));
    assert_eq!(out.lines().filter(|l| l.starts_with("canon review add: pinned shop.cart.0")).count(), 3, "{out}");
    assert_eq!(out.lines().filter(|l| l.starts_with("canon review add: wrote kind=review/")).count(), 3, "{out}");

    let written = reviews(repo);
    let mut ids: Vec<&str> = written.iter().map(|r| r["scenario_id"].as_str().unwrap()).collect();
    ids.sort();
    assert_eq!(ids, ["shop.cart.01", "shop.cart.02", "shop.cart.03"]);
    assert!(written.iter().all(|r| r["reviewer"] == "reviewer-2" && r["pin"] == "r1" && r["artifacts"][0]["path"] == "shot.png"), "{written:?}");
    let digests: std::collections::BTreeSet<&str> = written.iter().map(|r| r["pins"]["scenario_digest"].as_str().unwrap()).collect();
    assert_eq!(digests.len(), 3, "each review pins its own scenario");
    assert_eq!(stored_blobs(repo), 1, "the artifact is stored once");

    let mut child = Command::new(env!("CARGO_BIN_EXE_canon"))
        .args(review_args(&["--scenario-ids-from", "-"]))
        .current_dir(repo)
        .stdin(Stdio::piped())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
        .unwrap();
    child.stdin.take().unwrap().write_all(b"shop.cart.04\n").unwrap();
    let out = child.wait_with_output().unwrap();
    assert!(out.status.success(), "{}", stderr(&out));
    assert_eq!(reviews(repo).len(), 4, "stdin names one more scenario");
}

/// review.add.81: one id without a scenario record refuses the batch,
/// and an ids file with a line that is not an id is refused.
#[test]
fn review_add_81_a_batch_with_an_unindexed_scenario_writes_nothing() {
    let dir = shop_repo("");
    let repo = dir.path();
    let out = canon(repo, &review_args(&["--scenario-id", "shop.cart.01", "--scenario-id", "shop.cart.02", "--scenario-id", "shop.cart.77"]));
    assert_eq!(out.status.code(), Some(2));
    assert_eq!(
        stderr(&out),
        "canon review add: REVIEW_SCENARIO_NOT_INDEXED — no scenario record for `shop.cart.77` in spec root `root`; a review pins the scenario it saw, so index it first (`canon inventory sync`) (nothing is written for any of the scenarios)\n"
    );
    assert!(reviews(repo).is_empty());

    write(repo, "ids.txt", "shop.cart.01\nnot an id\n");
    let out = canon(repo, &review_args(&["--scenario-ids-from", "ids.txt"]));
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("--scenario-ids-from ids.txt: line 2: `not an id` is not a scenario id"), "{}", stderr(&out));
    let none = canon(repo, &review_args(&[]));
    assert_eq!(none.status.code(), Some(2));
    assert!(stderr(&none).contains("no scenario to review"), "{}", stderr(&none));
    assert!(reviews(repo).is_empty());
}
