//! 0.18 D1/D2 through the real binary: one report attests many
//! scenarios (`canon evidence add --all-scenarios`), a task's report with
//! no scenario id binds every case, a Swift Testing event stream binds,
//! one `canon review scenario` signs off many scenarios, and a write that
//! makes a pinned review stale names the review and its re-pin command.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

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
    commit_all(repo);
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
    files_under(&repo.join(".canon"), "evidence")
}

/// The committed evidence records, by scenario id.
fn committed_evidence(repo: &Path) -> Vec<Value> {
    files_under(&repo.join(".canon/ledger/kind=evidence"), ".json")
        .into_iter()
        .map(|p| serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap())
        .collect()
}

fn reviews(repo: &Path) -> Vec<Value> {
    files_under(&repo.join(".canon/ledger/kind=review"), ".json").into_iter().map(|p| serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()).collect()
}

fn stored_blobs(repo: &Path) -> usize {
    std::fs::read_dir(repo.join(".canon/artifacts/sha256")).map_or(0, std::iter::Iterator::count)
}

/// `canon evidence add --all-scenarios` over `report`, plus `extra`.
fn attest_all(repo: &Path, report: &str, extra: &[&str]) -> Output {
    let mut args = vec!["evidence", "add", "--project-id", "root", "--all-scenarios", "--report", report, "--class", "test-run", "--ref", "npm test", "--role", "implementer", "--actor-id", "impl-agent"];
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
        (vec!["--task-id", "c-demo#1.1"], "--task-id"),
        (vec!["--report-case", "x"], "--report-case"),
        (vec!["--approver", "alice", "--approval-role", "human"], "--approver, --approval-role"),
    ] {
        let out = attest_all(repo, "junit:reports/junit.xml", &extra);
        assert_eq!(out.status.code(), Some(2), "{flag}: {}", stderr(&out));
        assert!(stderr(&out).contains("EVIDENCE_INVALID_ARGUMENT") && stderr(&out).contains(flag.split(", ").next().unwrap()), "{flag}: {}", stderr(&out));
    }
    let bare = canon(repo, &["evidence", "add", "--project-id", "root", "--all-scenarios", "--class", "test-run", "--ref", "x", "--role", "implementer"]);
    assert_eq!(bare.status.code(), Some(2));
    assert!(stderr(&bare).contains("none was given"), "{}", stderr(&bare));
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
    ok(repo, &["evidence", "add", "--project-id", "root", "--scenario-id", "shop.cart.01", "--class", "test-run", "--ref", "npm test", "--role", "implementer", "--artifact", "shots/a.png", "--artifact", "shots/b.png"]);
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

/// evidence.add.90 (0.23 D1): a suite-wide batch binds exactly the files
/// its own write names; the scenario's earlier attachments are not carried
/// into the new record, and nothing reports a drop.
#[test]
fn evidence_add_90_a_batch_binds_only_what_it_names() {
    let dir = shots_repo();
    let repo = dir.path();

    let out = attest_all(repo, "junit:reports/junit.xml", &["--json"]);
    assert_eq!(out.status.code(), Some(0), "{}{}", stdout(&out), stderr(&out));
    let json: Value = serde_json::from_slice(&out.stdout).unwrap();
    let records = json["data"]["records"].as_array().unwrap();
    assert!(records.iter().all(|r| r.get("carried").is_none()), "no carried list: {json}");

    let latest = latest_of(repo, "shop.cart.01");
    assert_eq!(paths_of(&latest), ["reports/junit.xml"], "{latest}");
    assert!(latest.get("attachment_drop").is_none(), "{latest}");

    let status: Value = serde_json::from_str(&ok(repo, &["status", "--json"])).unwrap();
    assert!(status["data"].get("drops").is_none(), "{status}");
    assert!(!ok(repo, &["status"]).contains("dropped bindings"));
}

/// evidence.add.91 (0.23 D1): `--drop-attachment` and `--drop-reason` no
/// longer exist; nothing is written.
#[test]
fn evidence_add_91_the_drop_flags_are_gone() {
    let dir = shots_repo();
    let repo = dir.path();
    let before = evidence_files(repo);

    let out = attest_all(repo, "junit:reports/junit.xml", &["--drop-attachment", "shots/a.png", "--drop-reason", "frame a was retaken elsewhere"]);
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(stderr(&out).contains("--drop-attachment"), "{}", stderr(&out));
    assert_eq!(evidence_files(repo), before, "a refused flag writes nothing");
}

/// evidence.add.92 (0.23 D1): a batch that names a file at a bound path
/// binds that file's new bytes (new digest), and only that file.
#[test]
fn evidence_add_92_a_batch_file_at_the_same_path_binds_the_new_bytes() {
    let dir = shots_repo();
    let repo = dir.path();
    let first = latest_of(repo, "shop.cart.01");
    write(repo, "shots/a.png", "frame a, retaken");

    let out = attest_all(repo, "junit:reports/junit.xml", &["--artifact", "shop.cart.01=shots/a.png"]);
    assert_eq!(out.status.code(), Some(0), "{}{}", stdout(&out), stderr(&out));

    let latest = latest_of(repo, "shop.cart.01");
    assert_eq!(paths_of(&latest), ["reports/junit.xml", "shots/a.png"], "{latest}");
    let digest = |record: &Value, path: &str| record["attachments"].as_array().unwrap().iter().find(|a| a["path"] == path).unwrap()["sha256"].clone();
    assert_ne!(digest(&latest, "shots/a.png"), digest(&first, "shots/a.png"), "new bytes, new digest");
}

/// A Playwright-style JUnit report (0.24 D4): per case, the
/// `[[ATTACHMENT|<path>]]` lines of its `<system-out>` CDATA, paths
/// relative to the report's directory.
fn playwright_report(cases: &[(&str, &[&str])]) -> String {
    let mut xml = String::from("<testsuites><testsuite name=\"chromium\">\n");
    for (name, attachments) in cases {
        xml.push_str(&format!("<testcase name=\"{name}\" classname=\"tests/shop.spec.ts\" time=\"0.3\">\n<system-out>\n<![CDATA[\n"));
        for path in *attachments {
            xml.push_str(&format!("[[ATTACHMENT|{path}]]\n\n"));
        }
        xml.push_str("]]>\n</system-out>\n</testcase>\n");
    }
    xml.push_str("</testsuite></testsuites>\n");
    xml
}

/// Cases 01..03, each with its own screenshot (03 names its twice, and a
/// second file with the same bytes), under `test-results/`.
fn playwright_repo() -> tempfile::TempDir {
    let dir = shop_repo("");
    let repo = dir.path();
    for n in 1..=3 {
        write(repo, &format!("test-results/c{n}/shot.png"), &format!("screenshot of case {n}"));
    }
    write(repo, "test-results/c3/again.png", "screenshot of case 3");
    write(
        repo,
        "reports/junit.xml",
        &playwright_report(&[
            ("shop.cart.01 adds", &["../test-results/c1/shot.png"]),
            ("shop.cart.02 counts", &["../test-results/c2/shot.png"]),
            ("shop.cart.03 keeps", &["../test-results/c3/shot.png", "../test-results/c3/again.png"]),
        ]),
    );
    dir
}

fn digest_of(path: &Path) -> String {
    use sha2::{Digest, Sha256};
    format!("{:x}", Sha256::digest(std::fs::read(path).unwrap()))
}

/// evidence.add.93 (0.24 D4, #83): each record of a batch binds the
/// report and the files its own cases name, never another scenario's.
#[test]
fn evidence_add_93_a_batch_binds_each_tests_own_attachments() {
    let dir = playwright_repo();
    let repo = dir.path();

    let out = attest_all(repo, "junit:reports/junit.xml", &[]);
    assert_eq!(out.status.code(), Some(0), "{}{}", stdout(&out), stderr(&out));

    assert_eq!(paths_of(&latest_of(repo, "shop.cart.01")), ["reports/junit.xml", "test-results/c1/shot.png"]);
    assert_eq!(paths_of(&latest_of(repo, "shop.cart.02")), ["reports/junit.xml", "test-results/c2/shot.png"]);
    assert_eq!(
        paths_of(&latest_of(repo, "shop.cart.03")),
        ["reports/junit.xml", "test-results/c3/shot.png"],
        "two files with one digest are bound once"
    );
    for n in 1..=3 {
        let blob = repo.join(".canon/artifacts/sha256").join(digest_of(&repo.join(format!("test-results/c{n}/shot.png"))));
        assert!(blob.is_file(), "case {n}'s screenshot is stored by digest");
    }
    assert_eq!(stored_blobs(repo), 4, "the report and three distinct screenshots");
}

/// evidence.add.94: Cucumber embeddings are decoded, stored by digest and
/// bound to the scenario they belong to.
#[test]
fn evidence_add_94_a_cucumber_embedding_is_stored_by_digest() {
    use base64::Engine;
    let dir = shop_repo("");
    let repo = dir.path();
    let encode = |bytes: &[u8]| base64::engine::general_purpose::STANDARD.encode(bytes);
    let report = serde_json::json!([{
        "name": "Cart",
        "elements": [
            {"type": "scenario", "name": "adds", "tags": [{"name": "@shop.cart.01"}],
             "steps": [{"result": {"status": "passed"}, "embeddings": [{"data": encode(b"first frame"), "mime_type": "image/png"}]}]},
            {"type": "scenario", "name": "counts", "tags": [{"name": "@shop.cart.02"}],
             "steps": [{"result": {"status": "passed"}, "embeddings": [{"data": encode(b"second frame"), "mime_type": "image/png"}, {"data": encode(b"{}"), "mime_type": "application/json"}]}]}
        ]
    }]);
    write(repo, "reports/cucumber.json", &report.to_string());

    let out = attest_all(repo, "cucumber:reports/cucumber.json", &[]);
    assert_eq!(out.status.code(), Some(0), "{}{}", stdout(&out), stderr(&out));

    let first = latest_of(repo, "shop.cart.01");
    assert_eq!(paths_of(&first), ["reports/cucumber.json", "reports/cucumber.json.attachments/adds/1.png"], "{first}");
    let second = latest_of(repo, "shop.cart.02");
    assert_eq!(paths_of(&second), ["reports/cucumber.json", "reports/cucumber.json.attachments/counts/1.png", "reports/cucumber.json.attachments/counts/2.json"], "{second}");
    let sha = |record: &Value, path: &str| record["attachments"].as_array().unwrap().iter().find(|a| a["path"] == path).unwrap()["sha256"].as_str().unwrap().to_string();
    use sha2::{Digest, Sha256};
    let digest = format!("{:x}", Sha256::digest(b"first frame"));
    assert_eq!(sha(&first, "reports/cucumber.json.attachments/adds/1.png"), digest);
    assert_eq!(std::fs::read(repo.join(".canon/artifacts/sha256").join(&digest)).unwrap(), b"first frame", "the decoded bytes are stored");
    assert_eq!(stored_blobs(repo), 4, "the report and three embeddings");
}

/// evidence.add.95: a bare `--artifact` would bind to every record.
#[test]
fn evidence_add_95_a_bare_artifact_is_refused_in_a_batch() {
    let dir = playwright_repo();
    let repo = dir.path();
    write(repo, "reports/screens/a.png", "a");
    let before = evidence_files(repo);

    let out = attest_all(repo, "junit:reports/junit.xml", &["--artifact", "reports/screens/a.png", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}{}", stdout(&out), stderr(&out));
    let json: Value = serde_json::from_slice(&out.stderr).or_else(|_| serde_json::from_slice(&out.stdout)).unwrap();
    assert_eq!(json["error"]["code"], "EVIDENCE_INVALID_ARGUMENT", "{json}");
    let hint = json["error"]["hint"].as_str().unwrap();
    assert!(hint.contains("per-test attachments") && hint.contains("--artifact <scenario-id>=<path>"), "{hint}");
    assert_eq!(evidence_files(repo), before, "a refused add writes nothing");
    assert_eq!(stored_blobs(repo), 0);
}

/// evidence.add.96: `--artifact <scenario-id>=<path>` binds one scenario.
#[test]
fn evidence_add_96_a_scoped_artifact_binds_one_scenario() {
    let dir = playwright_repo();
    let repo = dir.path();
    write(repo, "notes/qa.md", "qa notes");

    let out = attest_all(repo, "junit:reports/junit.xml", &["--artifact", "shop.cart.02=notes/qa.md"]);
    assert_eq!(out.status.code(), Some(0), "{}{}", stdout(&out), stderr(&out));
    assert_eq!(paths_of(&latest_of(repo, "shop.cart.02")), ["notes/qa.md", "reports/junit.xml", "test-results/c2/shot.png"]);
    assert_eq!(paths_of(&latest_of(repo, "shop.cart.01")), ["reports/junit.xml", "test-results/c1/shot.png"], "no other record binds it");

    // A scenario with no record in the batch (05 owns no case; 09 is not indexed).
    let before = evidence_files(repo);
    for id in ["shop.cart.05", "shop.cart.09"] {
        let out = attest_all(repo, "junit:reports/junit.xml", &["--artifact", &format!("{id}=notes/qa.md")]);
        assert_eq!(out.status.code(), Some(2), "{id}: {}{}", stdout(&out), stderr(&out));
        assert!(stderr(&out).contains(&format!("`{id}=")) && stderr(&out).contains("no record in this batch"), "{}", stderr(&out));
    }
    assert_eq!(evidence_files(repo), before, "a refused add writes nothing");

    // One scenario on its own: its own id, or a bare path; another id is refused.
    let single = |artifact: &str| {
        canon(repo, &["evidence", "add", "--project-id", "root", "--scenario-id", "shop.cart.01", "--class", "test-run", "--ref", "npm test", "--role", "implementer", "--actor-id", "impl-agent", "--artifact", artifact])
    };
    assert_eq!(single("shop.cart.01=notes/qa.md").status.code(), Some(0));
    assert_eq!(paths_of(&latest_of(repo, "shop.cart.01")), ["notes/qa.md"]);
    let other = single("shop.cart.02=notes/qa.md");
    assert_eq!(other.status.code(), Some(2), "{}", stderr(&other));
    assert!(stderr(&other).contains("scoped to scenario `shop.cart.02`"), "{}", stderr(&other));
    assert_eq!(single("notes/qa.md").status.code(), Some(0), "a bare path is unchanged");
}

/// evidence.add.97: a report attachment that is missing, outside the
/// repository or over the limit refuses the bind.
#[test]
fn evidence_add_97_an_unbindable_report_attachment_refuses() {
    let dir = playwright_repo();
    let repo = dir.path();
    let outside = tempfile::tempdir().unwrap();
    std::fs::write(outside.path().join("o.png"), "outside").unwrap();
    write(repo, "test-results/big.bin", &"x".repeat(2 * 1024 * 1024));
    let before = evidence_files(repo);

    let cases: [(&str, String, &str); 3] = [
        ("missing", "../test-results/gone.png".to_string(), "gone.png"),
        ("outside", outside.path().join("o.png").to_string_lossy().into_owned(), "o.png"),
        ("oversized", "../test-results/big.bin".to_string(), "big.bin"),
    ];
    for (what, attachment, needle) in cases {
        write(repo, "reports/bad.xml", &playwright_report(&[("shop.cart.01 adds", &[attachment.as_str()]), ("shop.cart.02 counts", &["../test-results/c2/shot.png"])]));
        let args: &[&str] = if what == "oversized" { &["--max-artifact-mib", "1"] } else { &[] };
        let batch = attest_all(repo, "junit:reports/bad.xml", args);
        assert_eq!(batch.status.code(), Some(2), "{what}: {}{}", stdout(&batch), stderr(&batch));
        let err = stderr(&batch);
        assert!(err.contains("case `shop.cart.01 adds`") && err.contains(needle), "{what}: {err}");

        let mut single = vec!["evidence", "add", "--project-id", "root", "--scenario-id", "shop.cart.01", "--class", "test-run", "--ref", "npm test", "--role", "implementer", "--actor-id", "impl-agent", "--report", "junit:reports/bad.xml"];
        single.extend_from_slice(args);
        let one = canon(repo, &single);
        assert_eq!(one.status.code(), Some(2), "{what}: {}{}", stdout(&one), stderr(&one));
        assert!(stderr(&one).contains("case `shop.cart.01 adds`") && stderr(&one).contains(needle), "{what}: {}", stderr(&one));
    }
    assert_eq!(evidence_files(repo), before, "a refused add writes nothing");
    assert_eq!(stored_blobs(repo), 0);
}

/// evidence.add.98: one scenario's `--report` binds the files of the
/// cases matched to it.
#[test]
fn evidence_add_98_a_single_scenario_binds_its_cases_attachments() {
    let dir = playwright_repo();
    let repo = dir.path();

    ok(repo, &["evidence", "add", "--project-id", "root", "--scenario-id", "shop.cart.02", "--class", "test-run", "--ref", "npm test", "--role", "implementer", "--actor-id", "impl-agent", "--report", "junit:reports/junit.xml"]);
    assert_eq!(paths_of(&latest_of(repo, "shop.cart.02")), ["reports/junit.xml", "test-results/c2/shot.png"]);
}

/// evidence.add.99 (0.24 D4): review pins hold the report format and the
/// matched case names, not attachment bytes: a re-run with new
/// screenshots leaves the review current.
#[test]
fn evidence_add_99_new_screenshot_bytes_keep_a_review_current() {
    let dir = playwright_repo();
    let repo = dir.path();
    std::fs::write(repo.join(".canon/policy.yaml"), "spec_coverage:\n  require_review:\n    scope: []\n").unwrap();
    assert!(attest_all(repo, "junit:reports/junit.xml", &[]).status.success());
    ok(
        repo,
        &["review", "scenario", "shop.cart.01", "--project-id", "root", "--actor-id", "reviewer-2", "--role", "reviewer", "--session-id", "sess-review", "--upstream-ref", "tests/cart.rs#shop_cart_01"],
    );
    let git = |args: &[&str]| {
        let out = Command::new("git").args(args).current_dir(repo).env("GIT_AUTHOR_NAME", "t").env("GIT_AUTHOR_EMAIL", "t@t").env("GIT_COMMITTER_NAME", "t").env("GIT_COMMITTER_EMAIL", "t@t").output().unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    };
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "reviewed"]);
    let gate = || stdout(&canon(repo, &["gate", "check"]));
    assert!(!gate().contains("unreviewed-promotion shop.cart.01"), "the review is current");
    let first = latest_of(repo, "shop.cart.01");

    write(repo, "test-results/c1/shot.png", "screenshot of case 1, rerun");
    let out = attest_all(repo, "junit:reports/junit.xml", &[]);
    assert_eq!(out.status.code(), Some(0), "{}{}", stdout(&out), stderr(&out));
    assert!(!stdout(&out).contains("stale"), "no stale-review line: {}", stdout(&out));
    let latest = latest_of(repo, "shop.cart.01");
    assert_ne!(latest["attachments"], first["attachments"], "the record binds the new bytes");
    assert!(!gate().contains("unreviewed-promotion shop.cart.01"), "the review is still current: {}", gate());
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
    let task = |report: &str| canon(repo, &["evidence", "add", "--task-id", "c-demo#1.1", "--report", report, "--verdict", "faithful", "--class", "test-run", "--ref", "npm test", "--role", "implementer"]);

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
    let attest = |id: &str| canon(repo, &["evidence", "add", "--project-id", "root", "--scenario-id", id, "--class", "test-run", "--ref", "npm test", "--role", "implementer", "--actor-id", "impl-agent"]);
    assert!(attest("shop.cart.01").status.success());
    ok(
        repo,
        &["review", "scenario", "shop.cart.01", "--project-id", "root", "--actor-id", "reviewer-2", "--role", "reviewer", "--session-id", "sess-review", "--upstream-ref", "tests/cart.rs#shop_cart_01"],
    );
    let git = |args: &[&str]| {
        let out = Command::new("git").args(args).current_dir(repo).env("GIT_AUTHOR_NAME", "t").env("GIT_AUTHOR_EMAIL", "t@t").env("GIT_COMMITTER_NAME", "t").env("GIT_COMMITTER_EMAIL", "t@t").output().unwrap();
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
        String::from_utf8_lossy(&out.stdout).trim().to_string()
    };
    git(&["add", "-A"]);
    git(&["commit", "-q", "-m", "reviewed"]);
    let gate = |repo: &Path| stdout(&canon(repo, &["gate", "check"]));
    assert!(!gate(repo).contains("unreviewed-promotion shop.cart.01"), "the review is current");

    // The same verdict from the same checks says what the review saw (0.23 D5).
    let same = attest("shop.cart.01");
    assert!(same.status.success(), "{}", stderr(&same));
    assert!(!stdout(&same).contains("stale"), "no notice for evidence that says the same: {}", stdout(&same));
    assert!(!gate(repo).contains("unreviewed-promotion shop.cart.01"), "the review is still current");

    let out = canon(repo, &["evidence", "add", "--project-id", "root", "--scenario-id", "shop.cart.01", "--class", "test-run", "--ref", "npm test", "--role", "implementer", "--actor-id", "impl-agent", "--verdict", "not-applicable", "--summary", "skipped in this run"]);
    assert!(out.status.success(), "{}", stderr(&out));
    let expected = "canon evidence add: review by `reviewer-2` of shop.cart.01 is now stale (this record's verdict or checks differ from what it saw); re-pin it: canon review scenario shop.cart.01 --project-id root --actor-id reviewer-2 --role reviewer --session-id sess-review --upstream-ref 'tests/cart.rs#shop_cart_01'".to_string();
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

fn review_args<'a>(ids: &[&'a str], extra: &[&'a str]) -> Vec<&'a str> {
    let mut args = vec!["review", "scenario"];
    args.extend_from_slice(ids);
    args.extend(["--project-id", "root", "--actor-id", "reviewer-2", "--role", "reviewer", "--original-spec-ref", FEATURE]);
    args.extend_from_slice(extra);
    args
}

/// One invocation, one review per scenario, the same
/// reviewer, pin and artifacts, each pinning its own scenario.
#[test]
fn review_scenario_one_invocation_signs_off_several_scenarios() {
    let dir = shop_repo("");
    let repo = dir.path();
    write(repo, "shot.png", "png");
    ok(repo, &review_args(&["shop.cart.01", "shop.cart.02", "shop.cart.03"], &["--artifact", "shot.png"]));

    let written = reviews(repo);
    let mut ids: Vec<&str> = written.iter().map(|r| r["scenario_id"].as_str().unwrap()).collect();
    ids.sort();
    assert_eq!(ids, ["shop.cart.01", "shop.cart.02", "shop.cart.03"]);
    assert!(written.iter().all(|r| r["reviewer"] == "reviewer-2" && r["pin"] == written[0]["pin"] && r["artifacts"][0]["path"] == "shot.png"), "{written:?}");
    let digests: std::collections::BTreeSet<&str> = written.iter().map(|r| r["pins"]["scenario_digest"].as_str().unwrap()).collect();
    assert_eq!(digests.len(), 3, "each review pins its own scenario");
    assert_eq!(stored_blobs(repo), 1, "the artifact is stored once");
}

/// One id without a scenario record refuses the batch and writes nothing.
#[test]
fn review_scenario_a_batch_with_an_unindexed_scenario_writes_nothing() {
    let dir = shop_repo("");
    let repo = dir.path();
    let out = canon(repo, &review_args(&["shop.cart.01", "shop.cart.02", "shop.cart.77"], &["--json"]));
    assert_eq!(out.status.code(), Some(2));
    assert!(stderr(&out).contains("REVIEW_SCENARIO_NOT_INDEXED") || stdout(&out).contains("REVIEW_SCENARIO_NOT_INDEXED"), "{}{}", stdout(&out), stderr(&out));
    assert!(reviews(repo).is_empty());

    let none = canon(repo, &review_args(&[], &[]));
    assert_eq!(none.status.code(), Some(2));
    assert!(reviews(repo).is_empty());
}

/// Commit the fixture so revision-taking commands have a commit to bind.
fn commit_all(repo: &std::path::Path) {
    for args in [&["config", "user.name", "t"][..], &["config", "user.email", "t@example.com"], &["add", "-A"], &["commit", "-q", "-m", "x"]] {
        assert!(std::process::Command::new("git").args(args).current_dir(repo).status().unwrap().success());
    }
}
