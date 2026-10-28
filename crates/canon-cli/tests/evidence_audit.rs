//! 0.15 D3 and 0.23 D1 through the real binary: a record binds exactly the
//! files its own write names (nothing is carried forward), and an external
//! evaluator's audit (`evidence audit`) is recorded beside the declared
//! verdict, sampled by `spec_coverage.require_audit`, and turns a refuted
//! success into `false-success`.

use std::path::Path;
use std::process::{Command, Output};

use sha2::{Digest, Sha256};

const DIGEST: &str = "0f1e2d3c4b5a69788796a5b4c3d2e1f00f1e2d3c4b5a69788796a5b4c3d2e1f0";

fn canon(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).env("CANON_ACTOR", "canon").current_dir(repo).output().expect("spawn canon")
}

fn text(output: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr))
}

fn ok(repo: &Path, args: &[&str]) -> Output {
    let out = canon(repo, args);
    assert!(out.status.success(), "canon {args:?}: {}", text(&out));
    out
}

fn refused(repo: &Path, args: &[&str]) -> String {
    let out = canon(repo, args);
    assert_eq!(out.status.code(), Some(2), "canon {args:?} must refuse with exit 2: {}", text(&out));
    assert!(out.stdout.is_empty(), "a refusal prints nothing on stdout: {}", text(&out));
    text(&out)
}

/// `git init` + `canon init --no-policy`; a test that needs a policy
/// writes its own.
fn setup() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    assert!(Command::new("git").args(["init", "-q"]).current_dir(repo).status().unwrap().success());
    ok(repo, &["init", "--repo", ".", "--no-policy"]);
    commit_all(repo);
    dir
}

fn scenario(repo: &Path, id: &str, subject: Option<&str>) {
    let mut args = vec!["scenario", "new", id, "--title", "A run ends at zero health", "--case", "failure"];
    if let Some(subject) = subject {
        args.extend(["--subject", subject]);
    }
    ok(repo, &args);
    ok(repo, &["inventory", "sync"]);
}

fn subject_at(repo: &Path, id: &str, status: &str) {
    ok(repo, &["subject", "new", id, "--domain", "dev", "--title", "A run", "--actor", "lead"]);
    for step in ["specced", "building", "verifying"] {
        ok(repo, &["subject", "move", id, "--to", step, "--actor", "lead"]);
        if step == status {
            return;
        }
    }
}

/// Record the human's approval of `subject`'s brief, which `verifying →
/// shipped` requires (0.16 D1): a change adopted into it whose
/// `.canon/changes/<change>/brief.md` carries an `Approved:` line.
fn approve_brief(repo: &Path, subject: &str) {
    let change = format!("c-{subject}");
    ok(repo, &["change", "new", &change, "--subject", subject, "--title", "The brief"]);
    let brief = repo.join(".canon/changes").join(&change).join("brief.md");
    // These tests are about evidence, not the brief's format: a pre-0.19 brief is read as before.
    std::fs::write(brief, "# The brief\n\n## Why\n\nThe brief.\n\nApproved: Journey 2026-10-12 — \"승인\"\n").unwrap();
}

fn write(repo: &Path, path: &str, bytes: &[u8]) {
    let path = repo.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

fn attest_args<'a>(id: &'a str, extra: &[&'a str]) -> Vec<&'a str> {
    let mut args = vec!["evidence", "add", "--scenario", id, "--project", "root", "--class", "test-run", "--ref", "npm test", "--role", "implementer", "--actor", "impl-agent"];
    args.extend_from_slice(extra);
    args
}

fn audit_args<'a>(id: &'a str, result: &'a str, extra: &[&'a str]) -> Vec<&'a str> {
    let mut args = vec![
        "evidence", "audit", id, "--project", "root", "--ref", "testjack run 42", "--role", "auditor", "--actor",
        "testjack", "--evaluator", "testjack", "--evaluator-version", "1.4.0", "--evaluator-digest", DIGEST, "--audit-result", result,
    ];
    args.extend_from_slice(extra);
    args
}

/// Stage one record and promote it.
fn record(repo: &Path, args: &[&str]) {
    ok(repo, args);
    ok(repo, &["gate", "promote"]);
}

fn staged(repo: &Path) -> usize {
    std::fs::read_dir(repo.join(".canon/ledger/_staging/kind=evidence")).map_or(0, std::iter::Iterator::count)
}

fn committed(repo: &Path) -> Vec<serde_json::Value> {
    let dir = repo.join(".canon/ledger/kind=evidence");
    let mut records: Vec<serde_json::Value> =
        std::fs::read_dir(dir).unwrap().filter_map(Result::ok).map(|e| serde_json::from_str(&std::fs::read_to_string(e.path()).unwrap()).unwrap()).collect();
    records.sort_by(|a, b| a["at"].as_str().cmp(&b["at"].as_str()));
    records
}

fn status_json(repo: &Path) -> serde_json::Value {
    serde_json::from_slice(&ok(repo, &["status", "--json"]).stdout).unwrap()
}

fn write_policy(repo: &Path, yaml: &str) {
    write(repo, ".canon/policy.yaml", yaml.as_bytes());
}

/// evidence.add.40 / .41 (0.23 D1): a re-attest binds exactly the files its
/// own write names. Nothing is carried forward from the earlier record, and
/// nothing is recorded as dropped.
#[test]
fn a_reattest_binds_exactly_the_files_it_names() {
    let dir = setup();
    let repo = dir.path();
    scenario(repo, "game.run.01", None);
    write(repo, "reports/shot.png", b"first frame");
    write(repo, "reports/run.log", b"ok");
    record(repo, &attest_args("game.run.01", &["--artifact", "reports/shot.png", "--artifact", "reports/run.log"]));

    let out = ok(repo, &attest_args("game.run.01", &["--artifact", "reports/run.log", "--json"]));
    let json: serde_json::Value = serde_json::from_slice(&out.stdout).unwrap();
    assert!(json["data"]["records"][0].get("carried").is_none(), "{json}");
    ok(repo, &["gate", "promote"]);
    let latest = committed(repo).pop().unwrap();
    let paths: Vec<&str> = latest["attachments"].as_array().unwrap().iter().map(|a| a["path"].as_str().unwrap()).collect();
    assert_eq!(paths, ["reports/run.log"], "the screenshot is not carried: {latest}");
    assert!(latest.get("attachment_drop").is_none(), "{latest}");

    let report = status_json(repo);
    assert!(report.get("drops").is_none(), "status reports no drops: {report}");
}

/// evidence.add.41 (0.23 D1): binding a screenshot again with new bytes on
/// the single-scenario path binds it at its new digest, and nothing else:
/// the log the earlier record bound is not carried.
#[test]
fn evidence_add_41_a_rebound_screenshot_binds_its_new_digest_and_nothing_else() {
    let dir = setup();
    let repo = dir.path();
    scenario(repo, "game.run.01", None);
    write(repo, "reports/shot.png", b"first frame");
    write(repo, "reports/run.log", b"ok");
    record(repo, &attest_args("game.run.01", &["--artifact", "reports/shot.png", "--artifact", "reports/run.log"]));
    let first = committed(repo).pop().unwrap();
    let sha_of = |record: &serde_json::Value, path: &str| record["attachments"].as_array().unwrap().iter().find(|a| a["path"] == path).map(|a| a["sha256"].clone());
    let old_digest = sha_of(&first, "reports/shot.png").expect("the first record binds the screenshot");
    assert!(sha_of(&first, "reports/run.log").is_some(), "{first}");

    write(repo, "reports/shot.png", b"second frame, retaken");
    record(repo, &attest_args("game.run.01", &["--artifact", "reports/shot.png"]));
    let latest = committed(repo).pop().unwrap();
    assert_ne!(latest["at"], first["at"], "the later record is the latest: {latest}");
    let paths: Vec<&str> = latest["attachments"].as_array().unwrap().iter().map(|a| a["path"].as_str().unwrap()).collect();
    assert_eq!(paths, ["reports/shot.png"], "the screenshot alone, the log is not carried: {latest}");
    let expected = format!("{:x}", Sha256::digest(b"second frame, retaken"));
    let new_digest = sha_of(&latest, "reports/shot.png").unwrap();
    assert_eq!(new_digest, serde_json::Value::String(expected), "{latest}");
    assert_ne!(new_digest, old_digest, "new bytes, new digest");
}

/// evidence.add.43 (0.23 D1): the drop flags no longer exist.
#[test]
fn the_drop_flags_are_refused_as_unknown() {
    let dir = setup();
    let repo = dir.path();
    scenario(repo, "game.run.01", None);
    for flag in ["--drop-attachment", "--drop-reason"] {
        let out = refused(repo, &attest_args("game.run.01", &[flag, "reports/a.txt"]));
        assert!(out.contains(flag), "{out}");
    }
    assert_eq!(staged(repo), 0);
}

/// The audit flags belong to `evidence audit`, which needs all of them;
/// `evidence add` no longer takes them and `--kind audit` points at the new leaf.
#[test]
fn audit_flags_are_required_on_evidence_audit_and_refused_on_evidence_add() {
    let dir = setup();
    let repo = dir.path();
    scenario(repo, "game.run.01", None);

    refused(repo, &["evidence", "audit", "game.run.01", "--project", "root", "--ref", "x", "--role", "auditor"]);
    let out = refused(repo, &attest_args("game.run.01", &["--evaluator", "testjack", "--audit-result", "confirmed"]));
    assert!(out.contains("evidence audit"), "{out}");
    let out = refused(repo, &["evidence", "add", "--scenario", "game.run.01", "--project", "root", "--class", "audit", "--ref", "x", "--role", "auditor"]);
    assert!(out.contains("evidence audit"), "{out}");
    refused(repo, &audit_args("game.run.01", "confirmed", &["--verdict", "faithful"]));
    refused(repo, &audit_args("game.run.01", "confirmed", &["--task", "cats#1"]));
    refused(repo, &audit_args("game.run.01", "maybe", &[]));
    let mut bad_digest = audit_args("game.run.01", "confirmed", &[]);
    let at = bad_digest.iter().position(|a| *a == DIGEST).unwrap();
    bad_digest[at] = "ABC";
    refused(repo, &bad_digest);
    assert_eq!(staged(repo), 0);
}

/// evidence.add.45 / gate.check.60 / gate.check.61: a refuted audit is
/// `false-success` beside a faithful declared verdict, which it never
/// displaces; a later declared re-attest does not clear it; the ship guard
/// refuses it; a newer confirmed audit clears it.
#[test]
fn a_refuted_audit_is_false_success_until_a_newer_audit_confirms() {
    let dir = setup();
    let repo = dir.path();
    subject_at(repo, "run", "verifying");
    scenario(repo, "game.run.01", Some("run"));
    record(repo, &attest_args("game.run.01", &[]));
    ok(repo, &["gate", "check"]);

    write(
        repo,
        "reports/counter.xml",
        br#"<testsuite><testcase name="game.run.01: health below zero keeps the run alive"><failure/></testcase></testsuite>"#,
    );
    let staged_out = text(&ok(repo, &audit_args("game.run.01", "refuted", &["--report", "junit:reports/counter.xml"])));
    assert!(staged_out.contains("(audit refuted)"), "{staged_out}");
    ok(repo, &["gate", "promote"]);
    let audit = committed(repo).pop().unwrap();
    assert_eq!(audit["evidence"]["kind"], "audit");
    assert_eq!(audit["verdict"], "divergent", "a refuted audit carries the failing verdict: {audit}");
    assert_eq!(audit["audit"], serde_json::json!({"evaluator_id": "testjack", "evaluator_version": "1.4.0", "evaluator_digest": DIGEST, "result": "refuted"}));
    assert!(repo.join(".canon/artifacts/sha256").join(format!("{:x}", Sha256::digest(std::fs::read(repo.join("reports/counter.xml")).unwrap()))).is_file());

    let gate = canon(repo, &["gate", "check"]);
    assert_eq!(gate.status.code(), Some(1), "{}", text(&gate));
    let out = text(&gate);
    assert!(out.contains("false-success game.run.01 — latest audit refuted this scenario's declared success (evaluator `testjack` 1.4.0"), "{out}");
    assert!(out.contains("counterexamples: reports/counter.xml"), "{out}");
    assert!(!out.contains("uncovered-cell"), "the declared verdict is still faithful — the audit sits beside it: {out}");

    let report = status_json(repo);
    let run = report["data"]["subjects"].as_array().unwrap().iter().find(|s| s["id"] == "run").unwrap();
    assert_eq!((run["evidenced"].as_u64(), run["divergent"].as_u64()), (Some(1), Some(0)), "an audit never becomes the declared verdict: {report}");
    assert_eq!(report["data"]["audits"]["refuted"][0]["scenario"], "game.run.01", "{report}");
    assert!(report["meta"]["next"].as_array().unwrap().iter().any(|n| n["command"].as_str().unwrap().contains("evidence audit game.run.01")), "{report}");

    let ship = canon(repo, &["subject", "move", "run", "--to", "shipped", "--actor", "lead"]);
    assert_eq!(ship.status.code(), Some(1), "{}", text(&ship));
    assert!(text(&ship).contains("false-success game.run.01 — verifying → shipped: latest audit refuted"), "{}", text(&ship));

    record(repo, &attest_args("game.run.01", &[]));
    assert_eq!(canon(repo, &["gate", "check"]).status.code(), Some(1), "a declared faithful re-attest does not override a refuted audit");

    // A new audit replaces the refutation and binds only what it names.
    record(repo, &audit_args("game.run.01", "confirmed", &[]));
    ok(repo, &["gate", "check"]);
    approve_brief(repo, "run");
    ok(repo, &["subject", "move", "run", "--to", "shipped", "--actor", "lead"]);
}

/// gate.check.65 (0.23 D1): the false-success line names the
/// counterexamples the refutation bound, and a new confirmed audit replaces
/// it without carrying them; the gate clears.
#[test]
fn a_new_audit_replaces_a_refutation_and_binds_only_what_it_names() {
    let dir = setup();
    let repo = dir.path();
    subject_at(repo, "run", "verifying");
    scenario(repo, "game.run.01", Some("run"));
    record(repo, &attest_args("game.run.01", &[]));
    let path = "reports/counter example's.xml";
    write(repo, path, br#"<testsuite><testcase name="game.run.01: health below zero keeps the run alive"><failure/></testcase></testsuite>"#);
    let report_arg = format!("junit:{path}");
    record(repo, &audit_args("game.run.01", "refuted", &["--report", &report_arg]));

    let gate = text(&canon(repo, &["gate", "check"]));
    assert!(gate.contains("false-success game.run.01") && gate.contains(&format!("counterexamples: {path}")), "{gate}");
    assert!(!gate.contains("--drop-attachment"), "{gate}");

    record(repo, &audit_args("game.run.01", "confirmed", &[]));
    let confirmed = committed(repo).pop().unwrap();
    assert!(confirmed["attachments"].as_array().is_none_or(std::vec::Vec::is_empty), "the counterexample is not carried: {confirmed}");
    let gate = canon(repo, &["gate", "check"]);
    assert_eq!(gate.status.code(), Some(0), "{}", text(&gate));
}

/// The scenarios a `require_audit` policy with `seed` and `rate` samples,
/// computed here independently of canon: sha256(seed 0x00 project 0x00
/// scenario), first 8 bytes big-endian, below rate * 2^64.
fn expected_sample(seed: &str, rate: f64, ids: &[&str]) -> Vec<String> {
    #[expect(clippy::cast_possible_truncation, clippy::cast_sign_loss, reason = "range: rate is a fraction in [0, 1], so rate * 2^64 is non-negative and fits u128")]
    let threshold = (rate * 18_446_744_073_709_551_616.0).ceil() as u128;
    ids.iter()
        .filter(|id| {
            let digest = Sha256::digest([seed.as_bytes(), b"\0", b"root", b"\0", id.as_bytes()].concat());
            u128::from(u64::from_be_bytes(digest[..8].try_into().unwrap())) < threshold
        })
        .map(std::string::ToString::to_string)
        .collect()
}

/// gate.check.62: the sample is a pure function of
/// (seed, project, scenario, rate); a sampled scenario without an audit
/// is `uncovered-cell` with an audit detail, an indeterminate audit still
/// owes one, and a confirmed audit clears it.
#[test]
fn require_audit_samples_deterministically_and_owes_until_confirmed() {
    let dir = setup();
    let repo = dir.path();
    let ids = ["game.run.01", "game.run.02", "game.run.03", "game.run.04", "game.run.05", "game.run.06"];
    for id in ids {
        scenario(repo, id, None);
    }
    write_policy(repo, "spec_coverage:\n  require_audit:\n    rate: 0.5\n    seed: canon-0.15\n    scope: []\n");
    let expected = expected_sample("canon-0.15", 0.5, &ids);
    assert!(!expected.is_empty() && expected.len() < ids.len(), "the fixed seed splits the corpus: {expected:?}");

    let report = status_json(repo);
    let owed: Vec<String> = report["data"]["audits"]["owed"].as_array().unwrap().iter().map(|e| e["scenario"].as_str().unwrap().to_string()).collect();
    assert_eq!(owed, expected, "{report}");
    assert_eq!(report["data"]["audits"]["sampled"].as_u64(), Some(expected.len() as u64));
    assert_eq!(report["data"]["policy"]["spec_coverage"]["require_audit"], serde_json::json!({"rate": 0.5, "seed": "canon-0.15", "scope": []}));

    let gate = canon(repo, &["gate", "check"]);
    assert_eq!(gate.status.code(), Some(1));
    let out = text(&gate);
    for id in ids {
        let line = format!("uncovered-cell {id} — sampled for audit by spec_coverage.require_audit (rate 0.5, seed `canon-0.15`) and has no audit record");
        assert_eq!(out.contains(&line), expected.contains(&id.to_string()), "{id}: {out}");
    }

    let first = expected[0].as_str();
    record(repo, &audit_args(first, "indeterminate", &[]));
    let out = text(&canon(repo, &["gate", "check"]));
    assert!(out.contains(&format!("uncovered-cell {first} — sampled for audit by spec_coverage.require_audit (rate 0.5, seed `canon-0.15`), but its latest audit is indeterminate (evaluator `testjack` 1.4.0)")), "{out}");

    for id in &expected {
        record(repo, &audit_args(id, "confirmed", &[]));
    }
    ok(repo, &["gate", "check"]);
    let report = status_json(repo);
    assert_eq!(report["data"]["audits"]["confirmed"].as_u64(), Some(expected.len() as u64), "{report}");
    assert!(report["data"]["audits"]["owed"].as_array().unwrap().is_empty());
}

/// gate.check.63: `scope` defaults to `[verifying, shipped]`, and `rate`
/// bounds are inclusive — 0 samples nothing, 1 samples every scenario.
#[test]
fn require_audit_scope_defaults_to_claimed_done_subjects_and_rate_is_inclusive() {
    let dir = setup();
    let repo = dir.path();
    subject_at(repo, "run", "building");
    scenario(repo, "game.run.01", Some("run"));
    write_policy(repo, "spec_coverage:\n  require_audit:\n    rate: 1\n");
    ok(repo, &["gate", "check"]);

    ok(repo, &["subject", "move", "run", "--to", "verifying", "--actor", "lead"]);
    let out = text(&canon(repo, &["gate", "check"]));
    assert!(out.contains("uncovered-cell game.run.01 — sampled for audit"), "{out}");
    let ship = canon(repo, &["subject", "move", "run", "--to", "shipped", "--actor", "lead"]);
    assert_eq!(ship.status.code(), Some(1), "the ship guard refuses an owed audit: {}", text(&ship));
    assert!(text(&ship).contains("uncovered-cell game.run.01 — verifying → shipped: sampled for audit"), "{}", text(&ship));

    write_policy(repo, "spec_coverage:\n  require_audit:\n    rate: 0\n");
    ok(repo, &["gate", "check"]);
}

/// gate.check.64: a malformed `require_audit` poisons the
/// whole `spec_coverage` section.
#[test]
fn a_malformed_require_audit_poisons_spec_coverage() {
    let dir = setup();
    let repo = dir.path();
    scenario(repo, "game.run.01", None);
    for (yaml, detail) in [
        ("require_audit:\n    rate: 1.5\n", "`require_audit.rate` must be a number from 0 to 1, got 1.5"),
        ("require_audit:\n    seed: x\n", "missing field `rate`"),
        ("require_audit:\n    rate: 0.5\n    sample: 3\n", "unknown field `sample`"),
        ("require_audit:\n    rate: \"0.5\"\n", "invalid type: string"),
        ("require_audit:\n    rate: 0.5\n    scope: [done]\n", "`done` is not a valid value for `require_audit.scope`"),
        ("require_audit: null\n", "`require_audit` must be a mapping"),
    ] {
        write_policy(repo, &format!("spec_coverage:\n  {yaml}"));
        let gate = canon(repo, &["gate", "check"]);
        assert_eq!(gate.status.code(), Some(1), "{yaml}: {}", text(&gate));
        let out = text(&gate);
        assert!(out.contains("uncovered-cell spec_coverage — policy.yaml's `spec_coverage` section is unusable") && out.contains(detail), "{yaml}: {out}");
    }
}

/// `evidence audit` writes the audit record shape: class audit, a verdict
/// derived from the result, the evaluator fields, scenario and project ids.
#[test]
fn evidence_audit_derives_the_verdict_from_the_result() {
    let dir = setup();
    let repo = dir.path();
    scenario(repo, "game.run.01", None);
    for (result, verdict) in [("confirmed", "faithful"), ("refuted", "divergent"), ("indeterminate", "not_applicable")] {
        record(repo, &audit_args("game.run.01", result, &[]));
        let rec = committed(repo).pop().unwrap();
        assert_eq!(rec["evidence"]["kind"], "audit", "{rec}");
        assert_eq!(rec["verdict"], verdict, "{rec}");
        assert_eq!(rec["audit"]["result"], result, "{rec}");
        assert_eq!(rec["audit"]["evaluator_id"], "testjack", "{rec}");
        assert_eq!(rec["audit"]["evaluator_version"], "1.4.0", "{rec}");
        assert_eq!(rec["audit"]["evaluator_digest"], DIGEST, "{rec}");
        assert_eq!(rec["scenario_id"], "game.run.01", "{rec}");
        assert_eq!(rec["project_id"], "root", "{rec}");
    }
}

/// Commit the fixture so revision-taking commands have a commit to bind.
fn commit_all(repo: &std::path::Path) {
    for args in [&["config", "user.name", "t"][..], &["config", "user.email", "t@example.com"], &["add", "-A"], &["commit", "-q", "-m", "x"]] {
        assert!(std::process::Command::new("git").args(args).current_dir(repo).status().unwrap().success());
    }
}
