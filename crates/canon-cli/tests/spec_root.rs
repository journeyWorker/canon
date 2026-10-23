//! #104 through the real binary: a `--project-id` that names no spec root
//! configured in canon.yaml is refused (exit 2, `UNKNOWN_SPEC_ROOT`) before
//! anything is written, by every write command that takes one. The hint
//! names the configured roots and, with one root, the exact flag to pass.
//! Read-only commands are not checked: a filter on an id no root carries
//! matches nothing and writes nothing.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

const FEATURE_REL: &str = "kind=feature/area=cart/add.feature";
const SHA: &str = "0123456789012345678901234567890123456789";

const FEATURE: &str = "Feature: cart add
  # canon: {\"schema\":1,\"at\":\"2026-10-12T00:00:00Z\",\"actor\":{\"agent_id\":\"canon\"}}

  # canon: {\"schema\":1,\"at\":\"2026-10-12T00:00:00Z\",\"actor\":{\"agent_id\":\"canon\"}}
  @cart.add.01
  Scenario: Behavior cart.add.01
    Given a cart
    When the shopper acts
    Then the cart holds the item
";

const JUNIT: &str = "<testsuite>\n  <testcase classname=\"cart\" name=\"cart.add.01 adds\"/>\n</testsuite>\n";

fn canon(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).env("CANON_ACTOR", "canon").env_remove("CANON_ROLE").current_dir(repo).output().expect("spawn canon")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).to_string()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).to_string()
}

/// A git repo whose canon.yaml configures `roots` (each with `cart.add.01`
/// under `specs-<id>/`), indexed, with one commit.
fn repo(roots: &[&str]) -> TempDir {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    let mut yaml = String::from("tiers:\n  local:\n    backend: git\n    root: .canon/ledger\nspecs:\n  roots:\n");
    for id in roots {
        yaml.push_str(&format!("    - id: {id}\n      root: specs-{id}\n"));
        let feature = repo.join(format!("specs-{id}")).join(FEATURE_REL);
        std::fs::create_dir_all(feature.parent().unwrap()).unwrap();
        std::fs::write(feature, FEATURE).unwrap();
    }
    yaml.push_str("routing:\n  scenario: local\n  evidence_record: local\n  review: local\n  divergence: local\n");
    std::fs::write(repo.join("canon.yaml"), yaml).unwrap();
    std::fs::create_dir_all(repo.join("reports")).unwrap();
    std::fs::write(repo.join("reports/junit.xml"), JUNIT).unwrap();
    for args in [&["init", "-q"][..], &["add", "-A"], &["-c", "user.email=a@b.c", "-c", "user.name=n", "commit", "-qm", "seed"]] {
        assert!(Command::new("git").args(args).current_dir(repo).status().unwrap().success());
    }
    let sync = canon(repo, &["inventory", "sync"]);
    assert!(sync.status.success(), "{}{}", stdout(&sync), stderr(&sync));
    dir
}

/// Every file under `.canon` that is not the derived scenario index or its
/// lock: what a write would add (a staged or committed record, an
/// artifact blob).
fn written(repo: &Path) -> Vec<PathBuf> {
    fn walk(dir: &Path, out: &mut Vec<PathBuf>) {
        let Ok(entries) = std::fs::read_dir(dir) else { return };
        for entry in entries.flatten() {
            let path = entry.path();
            if path.is_dir() {
                walk(&path, out);
            } else {
                out.push(path);
            }
        }
    }
    let mut files = Vec::new();
    walk(&repo.join(".canon"), &mut files);
    files.retain(|p| {
        let s = p.to_string_lossy();
        !s.contains("kind=scenario") && !s.ends_with("scenario-index.lock") && !s.contains("audit")
    });
    files.sort();
    files
}

fn evidence_args<'a>(project: &'a str, extra: &[&'a str]) -> Vec<&'a str> {
    let mut args = vec!["evidence", "add", "--project-id", project, "--class", "test-run", "--ref", "npm test", "--role", "implementer", "--actor-id", "impl-agent", "--json"];
    args.extend_from_slice(extra);
    args
}

/// The refusal's JSON error body, after asserting exit 2.
fn refusal(out: &Output) -> Value {
    assert_eq!(out.status.code(), Some(2), "{}{}", stdout(out), stderr(out));
    let body: Value = serde_json::from_str(&stdout(out)).unwrap_or_else(|e| panic!("JSON on stdout ({e}): {}{}", stdout(out), stderr(out)));
    assert_eq!(body["error"]["code"], "UNKNOWN_SPEC_ROOT", "{body}");
    assert_eq!(body["error"]["field"], "--project-id", "{body}");
    body["error"].clone()
}

/// evidence.add.103: an `evidence add` whose `--project-id` is no
/// configured root, for one scenario and with `--all-scenarios`, is
/// refused before anything is written, and the hint names the one root.
#[test]
fn evidence_add_103_an_unknown_project_id_is_refused_before_anything_is_written() {
    let dir = repo(&["game"]);
    let r = dir.path();
    let before = written(r);

    let single = refusal(&canon(r, &evidence_args("root", &["--scenario-id", "cart.add.01"])));
    assert!(single["hint"].as_str().unwrap().contains("--project-id game"), "{single}");
    assert_eq!(single["expected"], serde_json::json!(["game"]));
    assert!(single["message"].as_str().unwrap().contains("`--project-id root`"), "{single}");

    let batch = refusal(&canon(r, &evidence_args("root", &["--all-scenarios", "--report", "junit:reports/junit.xml"])));
    assert!(batch["hint"].as_str().unwrap().contains("--project-id game"), "{batch}");

    let task_only = refusal(&canon(r, &evidence_args("root", &["--task-id", "c-cart#1.1"])));
    assert!(task_only["hint"].as_str().unwrap().contains("--project-id game"), "{task_only}");
    assert_eq!(written(r), before, "a refused add stages, binds and writes nothing");

    let accepted = canon(r, &evidence_args("game", &["--scenario-id", "cart.add.01"]));
    assert!(accepted.status.success(), "{}{}", stdout(&accepted), stderr(&accepted));
    assert_ne!(written(r), before, "the same add under the configured root writes the record");
}

/// evidence.add.104: with several configured roots the refusal lists them
/// all, and each configured id is accepted.
#[test]
fn evidence_add_104_the_refusal_lists_every_configured_spec_root() {
    let dir = repo(&["game", "ui"]);
    let r = dir.path();
    let before = written(r);

    let error = refusal(&canon(r, &evidence_args("root", &["--scenario-id", "cart.add.01"])));
    assert_eq!(error["expected"], serde_json::json!(["game", "ui"]));
    let hint = error["hint"].as_str().unwrap();
    assert!(hint.contains("`game`") && hint.contains("`ui`"), "{hint}");
    assert!(error["message"].as_str().unwrap().contains("game, ui"), "{error}");
    assert_eq!(written(r), before);

    for root in ["ui", "game"] {
        let out = canon(r, &evidence_args(root, &["--scenario-id", "cart.add.01"]));
        assert!(out.status.success(), "{root}: {}{}", stdout(&out), stderr(&out));
    }
}

/// evidence.add.105: `evidence approval-payload` refuses an unknown
/// project id too, printing no payload; the configured root's payload is
/// printed.
#[test]
fn evidence_add_105_an_approval_payload_is_not_issued_for_an_unknown_project_id() {
    let dir = repo(&["game"]);
    let r = dir.path();
    let sha = String::from_utf8(Command::new("git").args(["rev-parse", "HEAD"]).current_dir(r).output().unwrap().stdout).unwrap().trim().to_string();
    let payload = |project: &str| {
        canon(r, &["evidence", "approval-payload", "--project-id", project, "--scenario-id", "cart.add.01", "--artifact-sha", &sha, "--approver", "ana", "--approval-at", "2026-10-12T00:00:00Z", "--json"])
    };

    let out = payload("root");
    refusal(&out);
    assert!(!stdout(&out).contains("canon-approval-v1"), "no payload bytes: {}", stdout(&out));

    let out = payload("game");
    assert!(out.status.success(), "{}{}", stdout(&out), stderr(&out));
    assert!(stdout(&out).contains("canon-approval-v1"), "{}", stdout(&out));
}

/// review scenario: it with an unknown project id writes no review
/// and names the configured root.
#[test]
fn review_add_92_an_unknown_project_id_is_refused_and_writes_no_review() {
    let dir = repo(&["game"]);
    let r = dir.path();
    let before = written(r);
    let review = |project: &str| {
        canon(
            r,
            &["review", "scenario", "cart.add.01", "--project-id", project, "--pin", "p1", "--original-spec-ref", "specs/cart.md", "--actor-id", "reviewer-2", "--role", "reviewer", "--json"],
        )
    };

    let error = refusal(&review("root"));
    assert!(error["hint"].as_str().unwrap().contains("--project-id game"), "{error}");
    assert_eq!(written(r), before);

    let out = review("game");
    assert!(out.status.success(), "{}{}", stdout(&out), stderr(&out));
    assert_ne!(written(r), before);
}

/// divergence.lifecycle.10: `divergence add`, `resolve`, `defer` and
/// `accept` refuse an unknown project id and stage or commit nothing;
/// `divergence status`, which only reads, still answers.
#[test]
fn divergence_lifecycle_10_an_unknown_project_id_is_refused_by_every_writing_divergence_command() {
    let dir = repo(&["game"]);
    let r = dir.path();
    let before = written(r);
    let common = ["cart.add.01", "--project-id", "root", "--actor-id", "reviewer-2", "--role", "reviewer", "--json"];
    let with = |verb: &str, extra: &[&str]| {
        let mut args = vec!["divergence", verb];
        args.extend_from_slice(&common);
        args.extend_from_slice(extra);
        canon(r, &args)
    };

    for (verb, extra) in [
        ("add", vec!["--sha", SHA]),
        ("resolve", vec!["--sha", SHA]),
        ("defer", vec!["--sha", SHA, "--reason", "later", "--expiry", "2999-01-01T00:00:00Z"]),
        ("accept", vec!["--reason", "known gap", "--tracking", "https://example.com/issues/1"]),
    ] {
        let error = refusal(&with(verb, &extra));
        assert!(error["hint"].as_str().unwrap().contains("--project-id game"), "{verb}: {error}");
    }
    assert_eq!(written(r), before, "nothing staged or committed");

    let status = canon(r, &["divergence", "list", "--json"]);
    assert!(status.status.success(), "{}{}", stdout(&status), stderr(&status));
}

/// authoring.scaffold.14: `scenario new --feature <file under a root>
/// --project-id <unknown>` is refused, though the file itself lies under a
/// configured root; the file is untouched.
#[test]
fn authoring_scaffold_14_an_explicit_feature_file_still_refuses_an_unknown_project_id() {
    let dir = repo(&["game"]);
    let r = dir.path();
    let feature = r.join("specs-game").join(FEATURE_REL);
    let before = std::fs::read(&feature).unwrap();
    let scaffold = |project: &str| {
        canon(r, &["scenario", "new", "cart.add.09", "--title", "Another path", "--feature", &format!("specs-game/{FEATURE_REL}"), "--project-id", project, "--json"])
    };

    let error = refusal(&scaffold("root"));
    assert!(error["hint"].as_str().unwrap().contains("--project-id game"), "{error}");
    assert_eq!(std::fs::read(&feature).unwrap(), before, "a refused scenario new writes nothing");

    let out = scaffold("game");
    assert!(out.status.success(), "{}{}", stdout(&out), stderr(&out));
    assert_ne!(std::fs::read(&feature).unwrap(), before);
}

/// With one configured root `--project-id` can be left out; with several the
/// leaf refuses, names the roots and writes nothing.
#[test]
fn project_id_is_optional_with_one_spec_root_and_required_with_several() {
    let review = |r: &Path| {
        canon(r, &["review", "scenario", "cart.add.01", "--pin", "p1", "--original-spec-ref", "specs/cart.md", "--actor-id", "reviewer-2", "--role", "reviewer", "--json"])
    };
    let audit = |r: &Path| {
        canon(
            r,
            &[
                "evidence", "audit", "cart.add.01", "--ref", "x", "--evaluator-id", "e", "--evaluator-version", "1", "--evaluator-digest",
                "0f1e2d3c4b5a69788796a5b4c3d2e1f00f1e2d3c4b5a69788796a5b4c3d2e1f0", "--audit-result", "confirmed", "--actor-id", "a", "--role", "auditor", "--json",
            ],
        )
    };
    let add = |r: &Path| canon(r, &["evidence", "add", "--scenario-id", "cart.add.01", "--class", "test-run", "--ref", "npm test", "--role", "implementer", "--actor-id", "impl-agent", "--json"]);

    let one = repo(&["game"]);
    let r = one.path();
    for (name, run) in [("evidence add", &add as &dyn Fn(&Path) -> Output), ("evidence audit", &audit), ("review scenario", &review)] {
        let before = written(r);
        let out = run(r);
        assert!(out.status.success(), "{name}: {}{}", stdout(&out), stderr(&out));
        assert_ne!(written(r), before, "{name} wrote a record");
    }

    let two = repo(&["game", "ui"]);
    let r = two.path();
    let before = written(r);
    for (name, run) in [("evidence add", &add as &dyn Fn(&Path) -> Output), ("evidence audit", &audit), ("review scenario", &review)] {
        let out = run(r);
        assert_eq!(out.status.code(), Some(2), "{name}: {}{}", stdout(&out), stderr(&out));
        assert_eq!(written(r), before, "{name} wrote nothing");
    }
}
