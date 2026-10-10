//! 0.15 D2 and D3 through the real binary: re-attesting a scenario never
//! silently drops a bound file (`--drop-attachment`), and an external
//! evaluator's audit (`--kind audit`) is recorded beside the declared
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
    ok(repo, &["subject", "new", id, "--domain", "dev", "--title", "A run", "--actor-id", "lead"]);
    for step in ["specced", "building", "verifying"] {
        ok(repo, &["subject", "status", id, step, "--actor-id", "lead"]);
        if step == status {
            return;
        }
    }
}

fn write(repo: &Path, path: &str, bytes: &[u8]) {
    let path = repo.join(path);
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, bytes).unwrap();
}

fn attest_args<'a>(id: &'a str, extra: &[&'a str]) -> Vec<&'a str> {
    let mut args = vec!["evidence", "add", "--scenario-id", id, "--project-id", "root", "--kind", "test-run", "--ref", "npm test", "--role", "implementer", "--actor-id", "impl-agent"];
    args.extend_from_slice(extra);
    args
}

fn audit_args<'a>(id: &'a str, result: &'a str, extra: &[&'a str]) -> Vec<&'a str> {
    let mut args = vec![
        "evidence", "add", "--scenario-id", id, "--project-id", "root", "--kind", "audit", "--ref", "testjack run 42", "--role", "auditor", "--actor-id",
        "testjack", "--evaluator-id", "testjack", "--evaluator-version", "1.4.0", "--evaluator-digest", DIGEST, "--audit-result", result,
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
    std::fs::read_dir(repo.join(".canon/ledger/_staging/kind=evidence_record")).map(|d| d.count()).unwrap_or(0)
}

fn committed(repo: &Path) -> Vec<serde_json::Value> {
    let dir = repo.join(".canon/ledger/kind=evidence_record");
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

/// evidence.add.40 / .41: a re-attest that leaves out a bound file is
/// refused, naming the file and both ways out; re-binding new bytes at
/// the same path is fine; `--drop-attachment` records the drop, which
/// `canon status` lists.
#[test]
fn a_reattest_that_drops_a_binding_is_refused_until_rebound_or_dropped_on_purpose() {
    let dir = setup();
    let repo = dir.path();
    scenario(repo, "game.run.01", None);
    write(repo, "reports/shot.png", b"first frame");
    write(repo, "reports/run.log", b"ok");
    record(repo, &attest_args("game.run.01", &["--artifact", "reports/shot.png", "--artifact", "reports/run.log"]));

    let out = refused(repo, &attest_args("game.run.01", &["--artifact", "reports/run.log"]));
    assert!(out.contains("binds file(s) this record does not: `reports/shot.png`"), "{out}");
    assert!(out.contains("re-bind them") && out.contains("--drop-attachment <path>") && out.contains("--drop-reason <text>"), "names both ways out: {out}");
    assert_eq!(staged(repo), 0, "a refused add stages nothing");

    write(repo, "reports/shot.png", b"second frame");
    record(repo, &attest_args("game.run.01", &["--artifact", "reports/shot.png", "--artifact", "reports/run.log"]));

    record(
        repo,
        &attest_args("game.run.01", &["--artifact", "reports/run.log", "--drop-attachment", "reports/shot.png", "--drop-reason", "screenshots moved to the e2e lane"]),
    );
    let latest = committed(repo).pop().unwrap();
    assert_eq!(latest["attachment_drop"], serde_json::json!({"paths": ["reports/shot.png"], "reason": "screenshots moved to the e2e lane"}), "{latest}");

    // The drop is not inherited: the latest record now binds only the log.
    record(repo, &attest_args("game.run.01", &["--artifact", "reports/run.log"]));

    let report = status_json(repo);
    let drops = report["drops"].as_array().expect("status --json lists drops");
    assert_eq!(drops.len(), 1, "{report}");
    assert_eq!(drops[0]["scenario"], "game.run.01");
    assert_eq!(drops[0]["paths"], serde_json::json!(["reports/shot.png"]));
    assert_eq!(drops[0]["reason"], "screenshots moved to the e2e lane");
    assert_eq!(drops[0]["by"], "impl-agent");
    let human = text(&ok(repo, &["status"]));
    assert!(human.contains("dropped bindings (1):") && human.contains("game.run.01: reports/shot.png — \"screenshots moved to the e2e lane\" (by impl-agent at "), "{human}");
}

/// evidence.add.42: the latest record includes one still in staging.
#[test]
fn an_unpromoted_record_is_the_latest_the_guard_compares_with() {
    let dir = setup();
    let repo = dir.path();
    scenario(repo, "game.run.01", None);
    write(repo, "reports/a.txt", b"a");
    write(repo, "reports/b.txt", b"b");
    ok(repo, &attest_args("game.run.01", &["--artifact", "reports/a.txt", "--artifact", "reports/b.txt"]));
    let out = refused(repo, &attest_args("game.run.01", &["--artifact", "reports/a.txt"]));
    assert!(out.contains("`reports/b.txt`"), "{out}");
    assert_eq!(staged(repo), 1);
}

/// evidence.add.43: malformed drop flags are refused before anything is read.
#[test]
fn malformed_drop_flags_are_refused() {
    let dir = setup();
    let repo = dir.path();
    scenario(repo, "game.run.01", None);
    write(repo, "reports/a.txt", b"a");
    record(repo, &attest_args("game.run.01", &["--artifact", "reports/a.txt"]));

    let out = refused(repo, &attest_args("game.run.01", &["--drop-attachment", "reports/a.txt"]));
    assert!(out.contains("--drop-attachment needs --drop-reason"), "{out}");
    let out = refused(repo, &attest_args("game.run.01", &["--artifact", "reports/a.txt", "--drop-reason", "why"]));
    assert!(out.contains("--drop-reason explains a --drop-attachment; none was given"), "{out}");
    let out = refused(repo, &attest_args("game.run.01", &["--artifact", "reports/a.txt", "--drop-attachment", "reports/a.txt", "--drop-reason", "why"]));
    assert!(out.contains("--drop-attachment `reports/a.txt` is not a file the latest evidence record for game.run.01 binds and this record leaves out (it leaves out: nothing)"), "{out}");
    let out = refused(repo, &attest_args("game.run.01", &["--drop-attachment", "reports/a.txt", "--drop-reason", "line\nbreak"]));
    assert!(out.contains("--drop-reason must be non-empty, single-line text"), "{out}");
    let out = refused(repo, &["evidence", "add", "--task", "cats#1", "--kind", "test-run", "--ref", "x", "--role", "implementer", "--drop-attachment", "a", "--drop-reason", "r"]);
    assert!(out.contains("task-only evidence carries no drop guard"), "{out}");
    assert_eq!(staged(repo), 0);
}

/// evidence.add.44: the audit flags belong to `--kind audit`, which needs
/// all of them and refuses the declared-only flags.
#[test]
fn audit_flags_are_required_with_kind_audit_and_refused_elsewhere() {
    let dir = setup();
    let repo = dir.path();
    scenario(repo, "game.run.01", None);

    let out = refused(repo, &["evidence", "add", "--scenario-id", "game.run.01", "--project-id", "root", "--kind", "audit", "--ref", "x", "--role", "auditor"]);
    assert!(out.contains("needs --evaluator-id, --evaluator-version, --evaluator-digest, --audit-result") && out.contains("canon never generates or runs the audit"), "{out}");
    let out = refused(repo, &attest_args("game.run.01", &["--evaluator-id", "testjack", "--audit-result", "confirmed"]));
    assert!(out.contains("--evaluator-id, --audit-result belong(s) to `--kind audit` only; this record is `--kind test-run`"), "{out}");
    let out = refused(repo, &audit_args("game.run.01", "confirmed", &["--verdict", "faithful"]));
    assert!(out.contains("--verdict do(es) not apply to `--kind audit`"), "{out}");
    let out = refused(repo, &audit_args("game.run.01", "confirmed", &["--task", "cats#1"]));
    assert!(out.contains("--task do(es) not apply to `--kind audit`"), "{out}");
    let out = refused(repo, &audit_args("game.run.01", "maybe", &[]));
    assert!(out.contains("--audit-result `maybe` is not an audit result"), "{out}");
    let mut bad_digest = audit_args("game.run.01", "confirmed", &[]);
    let at = bad_digest.iter().position(|a| *a == DIGEST).unwrap();
    bad_digest[at] = "ABC";
    let out = refused(repo, &bad_digest);
    assert!(out.contains("--evaluator-digest `ABC` is not a sha256"), "{out}");
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
    let run = report["subjects"].as_array().unwrap().iter().find(|s| s["id"] == "run").unwrap();
    assert_eq!((run["evidenced"].as_u64(), run["divergent"].as_u64()), (Some(1), Some(0)), "an audit never becomes the declared verdict: {report}");
    assert_eq!(report["audits"]["refuted"][0]["scenario"], "game.run.01", "{report}");
    assert!(report["next"].as_array().unwrap().iter().any(|n| n["command"].as_str().unwrap().contains("--kind audit --scenario-id game.run.01")), "{report}");

    let ship = canon(repo, &["subject", "status", "run", "shipped", "--actor-id", "lead"]);
    assert_eq!(ship.status.code(), Some(1), "{}", text(&ship));
    assert!(text(&ship).contains("false-success game.run.01 — verifying → shipped: latest audit refuted"), "{}", text(&ship));

    record(repo, &attest_args("game.run.01", &[]));
    assert_eq!(canon(repo, &["gate", "check"]).status.code(), Some(1), "a declared faithful re-attest does not override a refuted audit");

    // The audit fold has its own drop guard: the counterexample report is
    // left out on purpose, with why.
    let out = refused(repo, &audit_args("game.run.01", "confirmed", &[]));
    assert!(out.contains("the latest audit record for game.run.01") && out.contains("`reports/counter.xml`"), "{out}");
    record(repo, &audit_args("game.run.01", "confirmed", &["--drop-attachment", "reports/counter.xml", "--drop-reason", "the counterexample is fixed"]));
    ok(repo, &["gate", "check"]);
    ok(repo, &["subject", "status", "run", "shipped", "--actor-id", "lead"]);
}

/// gate.check.65: the false-success fix names the counterexamples a
/// re-audit must drop on purpose. The gate line and `canon status`'s next
/// command both carry `--drop-attachment <path>`, shell-quoted, and
/// `--drop-reason`; the printed command, placeholders filled in, runs
/// through a shell and the real binary, is accepted, and clears the gate.
#[test]
fn the_false_success_fix_names_the_counterexample_drop() {
    let dir = setup();
    let repo = dir.path();
    subject_at(repo, "run", "verifying");
    scenario(repo, "game.run.01", Some("run"));
    record(repo, &attest_args("game.run.01", &[]));
    let path = "reports/counter example's.xml";
    write(repo, path, br#"<testsuite><testcase name="game.run.01: health below zero keeps the run alive"><failure/></testcase></testsuite>"#);
    let report_arg = format!("junit:{path}");
    record(repo, &audit_args("game.run.01", "refuted", &["--report", &report_arg]));

    let quoted = r#"--drop-attachment 'reports/counter example'\''s.xml' --drop-reason "<why the counterexample no longer applies>""#;
    let gate = text(&canon(repo, &["gate", "check"]));
    assert!(gate.contains(&format!("a new audit that no longer binds the counterexamples records why: {quoted}")), "{gate}");

    let report = status_json(repo);
    let command = report["next"]
        .as_array()
        .unwrap()
        .iter()
        .filter_map(|n| n["command"].as_str())
        .find(|c| c.contains("--kind audit --scenario-id game.run.01"))
        .unwrap_or_else(|| panic!("a re-audit next step: {report}"))
        .to_string();
    assert!(command.ends_with(&format!(" {quoted}")), "{command}");

    // The command without the drop is what the guard refuses; the printed
    // one, run as a shell would, is recorded and false-success clears.
    refused(repo, &audit_args("game.run.01", "confirmed", &[]));
    let runnable = command
        .strip_prefix("canon ")
        .expect("the next step is a canon command")
        .replace("<evaluator>", "testjack")
        .replace("<version>", "1.4.1")
        .replace("<sha256>", DIGEST)
        .replace("<confirmed|refuted|indeterminate>", "confirmed")
        .replace("<evaluator run>", "testjack run 43")
        .replace("<why the counterexample no longer applies>", "the counterexample is fixed");
    let shell = Command::new("sh")
        .arg("-c")
        .arg(format!("\"$CANON_BIN\" {runnable}"))
        .env("CANON_BIN", env!("CARGO_BIN_EXE_canon"))
        .env("CANON_ACTOR", "canon")
        .current_dir(repo)
        .output()
        .expect("spawn sh");
    assert!(shell.status.success(), "the printed command must run as printed: {runnable}\n{}", text(&shell));
    ok(repo, &["gate", "promote"]);
    let dropped = committed(repo).pop().unwrap();
    assert_eq!(dropped["attachment_drop"]["paths"][0], path, "{dropped}");
    let gate = canon(repo, &["gate", "check"]);
    assert_eq!(gate.status.code(), Some(0), "{}", text(&gate));
}

/// gate.check.65: a refutation with no counterexample bound has nothing
/// to drop, so neither the gate line nor the next command names a drop.
#[test]
fn a_refutation_without_counterexamples_names_no_drop() {
    let dir = setup();
    let repo = dir.path();
    subject_at(repo, "run", "verifying");
    scenario(repo, "game.run.01", Some("run"));
    record(repo, &attest_args("game.run.01", &[]));
    record(repo, &audit_args("game.run.01", "refuted", &[]));
    let gate = text(&canon(repo, &["gate", "check"]));
    assert!(gate.contains("false-success game.run.01") && !gate.contains("--drop-attachment"), "{gate}");
    let report = status_json(repo);
    assert!(report["next"].as_array().unwrap().iter().all(|n| !n["command"].as_str().unwrap_or_default().contains("--drop-attachment")), "{report}");
}

/// The scenarios a `require_audit` policy with `seed` and `rate` samples,
/// computed here independently of canon: sha256(seed 0x00 project 0x00
/// scenario), first 8 bytes big-endian, below rate * 2^64.
fn expected_sample(seed: &str, rate: f64, ids: &[&str]) -> Vec<String> {
    let threshold = (rate * 18_446_744_073_709_551_616.0).ceil() as u128;
    ids.iter()
        .filter(|id| {
            let digest = Sha256::digest([seed.as_bytes(), b"\0", b"root", b"\0", id.as_bytes()].concat());
            u128::from(u64::from_be_bytes(digest[..8].try_into().unwrap())) < threshold
        })
        .map(|id| id.to_string())
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
    let owed: Vec<String> = report["audits"]["owed"].as_array().unwrap().iter().map(|e| e["scenario"].as_str().unwrap().to_string()).collect();
    assert_eq!(owed, expected, "{report}");
    assert_eq!(report["audits"]["sampled"].as_u64(), Some(expected.len() as u64));
    assert_eq!(report["policy"]["spec_coverage"]["require_audit"], serde_json::json!({"rate": 0.5, "seed": "canon-0.15", "scope": []}));

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
    assert_eq!(report["audits"]["confirmed"].as_u64(), Some(expected.len() as u64), "{report}");
    assert!(report["audits"]["owed"].as_array().unwrap().is_empty());
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

    ok(repo, &["subject", "status", "run", "verifying", "--actor-id", "lead"]);
    let out = text(&canon(repo, &["gate", "check"]));
    assert!(out.contains("uncovered-cell game.run.01 — sampled for audit"), "{out}");
    let ship = canon(repo, &["subject", "status", "run", "shipped", "--actor-id", "lead"]);
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
