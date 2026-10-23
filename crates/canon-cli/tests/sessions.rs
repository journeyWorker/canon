//! 0.14 D6 through the real binary: units and reviews carry a session
//! (`--session-id`), and every write command says whether it staged or
//! wrote directly.

use std::path::Path;
use std::process::{Command, Output};

fn canon(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).env("CANON_ACTOR", "canon").current_dir(repo).output().expect("spawn canon")
}

/// Makes `repo` a git work tree with one commit, which every revision-bound
/// write needs.
fn git_seed(repo: &Path) {
    for args in [&["init", "-q", "-b", "main"][..], &["config", "user.name", "t"], &["config", "user.email", "t@example.com"], &["commit", "-q", "--allow-empty", "-m", "x"]] {
        let out = Command::new("git").args(args).current_dir(repo).output().expect("spawn git");
        assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
    }
}

fn text(output: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr))
}

fn ok(repo: &Path, args: &[&str]) -> String {
    let out = canon(repo, args);
    assert!(out.status.success(), "canon {args:?}: {}", text(&out));
    String::from_utf8_lossy(&out.stdout).into_owned()
}

/// `git init` + `canon init --no-policy` + change `cats` in canon's own
/// `.canon/changes/`, read with no `plans:` entry, imported.
/// The write-mode test moves a subject, so it opts out of the starter
/// policy (0.14 D2) whose require_review would gate that move.
fn setup() -> tempfile::TempDir {
    let dir = tempfile::tempdir().unwrap();
    let repo = dir.path();
    git_seed(repo);
    ok(repo, &["init", "--repo", ".", "--no-policy"]);
    let change = repo.join(".canon/changes/cats");
    std::fs::create_dir_all(&change).unwrap();
    std::fs::write(change.join("brief.md"), "# cats\n\n## Why\n\nA cat-themed run.\n").unwrap();
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

/// Where a write left its record: kept staged (`--no-promote`), or in the committed ledger (promoted in the run, or written directly).
#[derive(Clone, Copy, PartialEq, Eq, Debug)]
enum Landed {
    Staged,
    Committed,
}
const STAGED: Landed = Landed::Staged;
const PROMOTED: Landed = Landed::Committed;

/// `--session-id` on `evidence add` and `review add` fills each record's
/// `actor.session_id`, and survives promotion.
#[test]
fn session_ids_land_on_evidence_and_review_actors() {
    let dir = setup();
    let repo = dir.path();
    ok(repo, &["scenario", "new", "game.run.01", "--title", "A run ends at zero health", "--case", "failure"]);
    ok(repo, &["inventory", "sync"]);

    let evidence = [
        "evidence", "add", "--scenario-id", "game.run.01", "--project-id", "root", "--class", "test-run", "--ref", "npm run smoke", "--role", "implementer",
        "--actor-id", "impl-unit-1", "--session-id", "sess-impl",
    ];
    ok(repo, &evidence);
    ok(repo, &["gate", "promote"]);
    let records = committed(repo, "evidence_record");
    assert_eq!(records.len(), 1);
    assert_eq!(records[0]["actor"], serde_json::json!({"agent_id": "impl-unit-1", "role": "implementer", "session_id": "sess-impl"}));

    ok(repo, &[
        "review", "scenario", "game.run.01", "--project-id", "root", "--original-spec-ref",
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
            &["evidence", "add", "--scenario-id", "game.run.01", "--project-id", "root", "--class", "test-run", "--ref", "r", "--role", "implementer", "--session-id", bad],
        );
        let review = canon(
            repo,
            &[
                "review", "scenario", "game.run.01", "--project-id", "root", "--actor-id", "r", "--original-spec-ref", "s", "--role",
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

/// D6 (F16): every write command leaves its record where its write mode says,
/// so an operator knows whether a promote is due. `canon init` writes
/// `write: { promote: auto }` (0.16 D2), so a staged write promotes itself
/// unless `--no-promote` keeps it staged; the direct writers never stage.
#[test]
fn every_write_command_stages_or_writes_directly_as_its_mode_says() {
    let dir = setup();
    let repo = dir.path();
    let direct = |args: &[&str]| {
        ok(repo, args);
        assert!(records_under(repo, "_staging").is_empty(), "canon {args:?} writes directly and must stage nothing");
    };
    let staged = |args: &[&str], landed: Landed| {
        let before = records_under(repo, "_staging").len();
        ok(repo, args);
        let after = records_under(repo, "_staging").len();
        match landed {
            Landed::Staged => assert!(after > before, "canon {args:?} with --no-promote must leave its record staged"),
            Landed::Committed => assert_eq!(after, before, "canon {args:?} must promote its record in the run"),
        }
    };

    direct(&["feature", "new", "game.run", "--title", "Runs"]);
    direct(&["scenario", "new", "game.run.01", "--title", "A run ends at zero health", "--case", "failure"]);
    ok(repo, &["inventory", "sync"]);
    direct(&["subject", "new", "runs", "--domain", "dev", "--title", "Runs"]);
    direct(&["subject", "adopt", "runs", "--change-id", "cats"]);
    direct(&["subject", "move", "runs", "--to", "specced"]);
    direct(&["change", "new", "runs-boss", "--subject-id", "runs", "--title", "A boss wave"]);

    staged(
        &["evidence", "add", "--scenario-id", "game.run.01", "--project-id", "root", "--class", "test-run", "--ref", "npm run smoke", "--role", "implementer", "--no-promote"],
        STAGED,
    );
    staged(&["evidence", "add", "--scenario-id", "game.run.01", "--project-id", "root", "--class", "test-run", "--ref", "npm run smoke 2", "--role", "implementer"], PROMOTED);
    staged(&["finding", "add", "--change-id", "cats", "--round", "1", "--seq", "1", "--severity", "note", "--actor-id", "r", "--summary", "s", "--no-promote"], STAGED);
    ok(repo, &["gate", "promote"]);
    staged(&["finding", "close", "cats", "--round", "1", "--seq", "1", "--disposition", "rejected", "--reason", "by design"], PROMOTED);
    assert!(records_under(repo, "_staging").is_empty(), "nothing is left to promote");

    direct(&[
        "review", "scenario", "game.run.01", "--project-id", "root", "--actor-id", "r", "--original-spec-ref", "specs/x.feature",
        "--role", "reviewer",
    ]);

    staged(
        &["divergence", "add", "game.run.01", "--project-id", "root", "--actor-id", "r", "--role", "reviewer", "--no-promote"],
        STAGED,
    );
    staged(&["divergence", "add", "game.run.01", "--project-id", "root", "--round", "2", "--actor-id", "r", "--role", "reviewer"], PROMOTED);
    staged(&["divergence", "resolve", "game.run.01", "--project-id", "root", "--round", "3", "--actor-id", "r", "--role", "reviewer"], PROMOTED);
}
