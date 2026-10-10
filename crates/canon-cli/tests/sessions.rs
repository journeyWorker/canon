//! 0.14 D6 through the real binary: units and reviews carry a session
//! (`--session-id`), and every write command says whether it staged or
//! wrote directly.

use std::path::Path;
use std::process::{Command, Output};

fn canon(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).env("CANON_ACTOR", "canon").current_dir(repo).output().expect("spawn canon")
}

fn text(output: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr))
}

fn ok(repo: &Path, args: &[&str]) -> String {
    let out = canon(repo, args);
    assert!(out.status.success(), "canon {args:?}: {}", text(&out));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// `git init` + `canon init` + an openspec plan source holding change
/// `cats`, imported.
fn setup() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    assert!(Command::new("git").args(["init", "-q"]).current_dir(repo).status().unwrap().success());
    ok(repo, &["init", "--repo", "."]);
    let yaml = std::fs::read_to_string(repo.join("canon.yaml")).unwrap();
    let yaml = yaml.replace("plans:\n  sources: []", "plans:\n  sources:\n    - dialect: openspec\n      root: openspec/changes");
    assert!(yaml.contains("root: openspec/changes"), "the plan source must be configured:\n{yaml}");
    std::fs::write(repo.join("canon.yaml"), yaml).unwrap();
    let change = repo.join("openspec/changes/cats");
    std::fs::create_dir_all(&change).unwrap();
    std::fs::write(change.join("proposal.md"), "# cats\n\n## Why\n\nA cat-themed run.\n").unwrap();
    std::fs::write(change.join("tasks.md"), "## 1. Run\n\n- [ ] 1 Ship the run\n").unwrap();
    ok(repo, &["ingest", "plans"]);
    dir
}

/// Every JSON record under `.canon/ledger/<subdir>`, recursively.
fn records_under(repo: &Path, subdir: &str) -> Vec<serde_json::Value> {
    let mut files = Vec::new();
    let mut stack = vec![repo.join(".canon/ledger").join(subdir)];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).into_iter().flatten().flatten() {
            let path = entry.path();
            if path.is_dir() {
                stack.push(path);
            } else {
                files.push(path);
            }
        }
    }
    files.sort();
    files.iter().map(|p| serde_json::from_str(&std::fs::read_to_string(p).unwrap()).unwrap()).collect()
}

fn committed(repo: &Path, kind: &str) -> Vec<serde_json::Value> {
    records_under(repo, &format!("kind={kind}"))
}

const STAGED: &str = "— run `canon gate promote` to commit it";
const DIRECT: &str = "— written directly; nothing to promote";

/// `--session-id` on `evidence add` and `review add` fills each record's
/// `actor.session_id`, and survives promotion.
#[test]
fn session_ids_land_on_evidence_and_review_actors() {
    let dir = setup();
    let repo = dir.path();
    ok(repo, &["scenario", "new", "game.run.01", "--title", "A run ends at zero health", "--case", "failure"]);
    ok(repo, &["inventory", "sync"]);

    let evidence = [
        "evidence", "add", "--scenario-id", "game.run.01", "--project-id", "root", "--kind", "test-run", "--ref", "npm run smoke", "--role", "implementer",
        "--actor-id", "impl-unit-1", "--session-id", "sess-impl",
    ];
    ok(repo, &evidence);
    ok(repo, &["gate", "promote"]);
    let records = committed(repo, "evidence_record");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["actor"], serde_json::json!({"agent_id": "impl-unit-1", "role": "implementer", "session_id": "sess-impl"}));

    ok(repo, &[
        "review", "add", "--project-id", "root", "--scenario-id", "game.run.01", "--reviewer", "reviewer-2", "--pin", "r1", "--original-spec-ref",
        "specs/x.feature", "--actor-id", "reviewer-2", "--role", "reviewer", "--session-id", "sess-review",
    ]);
    let reviews = committed(repo, "review");
    assert_eq!(reviews.len(), 1);
    assert_eq!(reviews[0]["actor"]["session_id"], "sess-review");
}

/// The grammar is `SessionId`'s: a control character or surrounding
/// whitespace is a usage error on every command that takes the flag, and
/// nothing is written.
#[test]
fn a_session_id_outside_the_grammar_is_refused_and_nothing_is_written() {
    let dir = setup();
    let repo = dir.path();
    ok(repo, &["scenario", "new", "game.run.01", "--title", "A run ends at zero health", "--case", "failure"]);
    ok(repo, &["inventory", "sync"]);

    for bad in ["sess\timpl", " sess-impl", ""] {
        let evidence = canon(
            repo,
            &["evidence", "add", "--scenario-id", "game.run.01", "--project-id", "root", "--kind", "test-run", "--ref", "r", "--role", "implementer", "--session-id", bad],
        );
        let review = canon(
            repo,
            &[
                "review", "add", "--project-id", "root", "--scenario-id", "game.run.01", "--reviewer", "r", "--pin", "p", "--original-spec-ref", "s", "--role",
                "reviewer", "--session-id", bad,
            ],
        );
        for out in [evidence, review] {
            assert_eq!(out.status.code(), Some(2), "{bad:?}: {}", text(&out));
            assert!(text(&out).contains("--session-id"), "{}", text(&out));
        }
    }
    assert!(records_under(repo, "_staging").is_empty(), "a refused add stages nothing");
    assert!(committed(repo, "evidence_record").is_empty() && committed(repo, "review").is_empty());
}

/// D6 (F16): every write command ends its success line with the same
/// suffix for the same write mode, so an operator knows whether a
/// promote is due.
#[test]
fn every_write_command_says_whether_it_staged_or_wrote_directly() {
    let dir = setup();
    let repo = dir.path();
    let direct = |args: &[&str]| {
        let out = ok(repo, args);
        assert!(out.lines().any(|l| l.ends_with(DIRECT)), "canon {args:?} wrote directly and must say so:\n{out}");
    };
    let staged = |args: &[&str], suffix: &str| {
        let out = ok(repo, args);
        assert!(out.lines().any(|l| l.contains(suffix)), "canon {args:?} staged and must name its promote:\n{out}");
    };

    direct(&["feature", "new", "game.run", "--title", "Runs"]);
    direct(&["scenario", "new", "game.run.01", "--title", "A run ends at zero health", "--case", "failure"]);
    ok(repo, &["inventory", "sync"]);
    direct(&["subject", "new", "runs", "--domain", "dev", "--title", "Runs"]);
    direct(&["subject", "adopt", "cats", "--subject", "runs"]);
    direct(&["subject", "status", "runs", "specced"]);

    staged(
        &["evidence", "add", "--scenario-id", "game.run.01", "--project-id", "root", "--kind", "test-run", "--ref", "npm run smoke", "--role", "implementer"],
        STAGED,
    );
    staged(&["finding", "add", "--change-id", "cats", "--round", "1", "--seq", "1", "--severity", "note", "--reviewer", "r", "--summary", "s"], STAGED);
    ok(repo, &["gate", "promote"]);
    staged(&["finding", "close", "--change-id", "cats", "--round", "1", "--seq", "1", "--disposition", "rejected"], STAGED);
    ok(repo, &["gate", "promote"]);

    direct(&[
        "review", "add", "--project-id", "root", "--scenario-id", "game.run.01", "--reviewer", "r", "--pin", "p", "--original-spec-ref", "specs/x.feature",
        "--role", "reviewer",
    ]);

    let sha = "a".repeat(40);
    staged(
        &["divergence", "stage", "--project-id", "root", "--scenario-id", "game.run.01", "--sha", &sha, "--reviewer", "r", "--role", "reviewer"],
        "— run `canon divergence promote` to commit it",
    );
    ok(repo, &["divergence", "promote"]);
    direct(&["divergence", "resolve", "--project-id", "root", "--scenario-id", "game.run.01", "--sha", &sha, "--round", "2", "--reviewer", "r", "--role", "reviewer"]);
}
