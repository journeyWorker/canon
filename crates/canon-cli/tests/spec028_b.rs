//! Spec-corpus bindings for 0.28 slice B (authoring, skills install, subject
//! lifecycle): behaviour through the built `canon` binary (exit codes, JSON
//! fields, what is and is not written). Each test names the scenario it proves.

use std::collections::BTreeMap;
use std::path::Path;
use std::process::{Command, Output};

use serde_json::Value;
use tempfile::TempDir;

fn canon(repo: &Path, args: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(repo).env_remove("CANON_ROLE").output().expect("spawn canon binary")
}

fn text(out: &Output) -> String {
    format!("{}{}", String::from_utf8_lossy(&out.stdout), String::from_utf8_lossy(&out.stderr))
}

fn ok(repo: &Path, args: &[&str]) -> Output {
    let out = canon(repo, args);
    assert!(out.status.success(), "{args:?}: {}", text(&out));
    out
}

fn json(out: &Output) -> Value {
    serde_json::from_slice(&out.stdout).unwrap_or_else(|e| panic!("stdout is not one JSON document ({e}): {}", text(out)))
}

fn git(repo: &Path, args: &[&str]) {
    let out = Command::new("git").args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false"]).args(args).current_dir(repo).output().expect("spawn git");
    assert!(out.status.success(), "git {args:?}: {}", String::from_utf8_lossy(&out.stderr));
}

/// A git work tree with one commit, set up by `canon init` (no skills, no AGENTS.md block).
fn repo() -> TempDir {
    let dir = TempDir::new().unwrap();
    git(dir.path(), &["init", "-q", "-b", "main"]);
    git(dir.path(), &["commit", "-q", "--allow-empty", "-m", "seed"]);
    ok(dir.path(), &["init", "--providers", "none", "--no-agents-md"]);
    dir
}

fn tree(root: &Path) -> BTreeMap<String, Vec<u8>> {
    let mut out = BTreeMap::new();
    let mut stack = vec![root.to_path_buf()];
    while let Some(dir) = stack.pop() {
        for entry in std::fs::read_dir(&dir).unwrap().flatten() {
            let path = entry.path();
            if path.file_name().is_some_and(|name| name == ".git") {
                continue;
            }
            if path.is_dir() {
                stack.push(path);
            } else {
                out.insert(path.strip_prefix(root).unwrap().to_string_lossy().into_owned(), std::fs::read(&path).unwrap());
            }
        }
    }
    out
}

/// A complete English brief with no blocker (flow steps `F1`, `F2`).
fn clean_brief() -> String {
    "# Add login\n\n\
<!-- canon:section intent role=context -->\n## Intent\n\nAdd login\n\nVisitors lose their cart when they switch devices.\n\n\
<!-- canon:section outcome role=normative -->\n## Desired outcome\n\nA visitor who signs in sees the saved cart. ==**The cart follows the account.**==\n\n\
<!-- canon:section flow role=normative -->\n## User flow\n\n1. F1 The visitor sees the form → submits credentials → lands on the catalog. ==**Only a valid sign-in opens a session.**==\n2. F2 The visitor submits a wrong password → sees an error → can retry.\n\n\
<!-- canon:section detail role=normative -->\n## Step detail\n\n### F1 Submit\n\nThe server issues the session. ==**The password is never logged.**==\n\n### F2 Failed submit\n\nFive failures lock the form for thirty seconds.\n\n\
<!-- canon:section acceptance role=normative -->\n## Acceptance\n\n- F1: valid credentials open the catalog and set a session cookie.\n- F2: a wrong password shows the error and sets no cookie. ==**F2 covers the failure path.**==\n\n\
<!-- canon:section questions role=question -->\n## Questions and assumptions\n\nNone.\n\n\
<!-- canon:section out-of-scope role=context -->\n## Out of scope\n\nSocial sign-in.\n"
        .to_string()
}

/// authoring.brief-check.16 (D4): blockers exit 1 and keep the report in `data`; a fixed brief
/// exits 0; a check that is refused (no such change) exits 2 with no report.
#[test]
fn brief_check_16_blockers_exit_1_with_the_report_and_a_refusal_exits_2() {
    let dir = repo();
    let repo = dir.path();
    ok(repo, &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    ok(repo, &["change", "new", "add-login", "--subject-id", "demo", "--title", "Add login"]);
    let brief = repo.join(".canon/changes/add-login/brief.md");

    // The fresh skeleton holds placeholders: structure blockers.
    let out = canon(repo, &["brief", "check", "add-login", "--json"]);
    assert_eq!(out.status.code(), Some(1), "{}", text(&out));
    let body = json(&out);
    assert_eq!(body["ok"], false, "{body}");
    assert_eq!(body["error"]["code"], "BRIEF_HAS_BLOCKERS", "{body}");
    assert!(body["data"]["findings"].is_array() && body["data"]["counts"].is_object(), "the report rides in data: {body}");

    std::fs::write(&brief, clean_brief()).unwrap();
    let out = canon(repo, &["brief", "check", "add-login", "--json"]);
    assert_eq!(out.status.code(), Some(0), "{}", text(&out));
    assert_eq!(json(&out)["ok"], true);

    let out = canon(repo, &["brief", "check", "no-such-change", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    let body = json(&out);
    assert_eq!(body["error"]["code"], "BRIEF_NOT_FOUND", "{body}");
    assert!(body.get("data").is_none(), "a refusal carries no report: {body}");
}

fn digest_of(preview: &Output) -> String {
    json(preview)["data"]["planDigest"].as_str().expect("a preview carries a planDigest").to_string()
}

/// authoring.migrate.16 (D6): a commit with the digest of an earlier plan, after the repo
/// changed so the plan differs, is refused PLAN_CHANGED and writes nothing; a fresh preview
/// gives a digest that commits.
#[test]
fn migrate_16_a_digest_of_a_plan_the_repo_has_since_changed_is_refused() {
    let dir = repo();
    let repo = dir.path();
    let strip = |line: &str| {
        let gitignore = std::fs::read_to_string(repo.join(".gitignore")).unwrap();
        assert!(gitignore.contains(line), "{line} is a managed line: {gitignore}");
        std::fs::write(repo.join(".gitignore"), gitignore.replace(line, "")).unwrap();
        git(repo, &["add", "-A"]);
        git(repo, &["commit", "-q", "-m", "drop a managed gitignore line"]);
    };
    strip(".canon/r2/\n");
    let first = ok(repo, &["migrate", "--json"]);
    let old = digest_of(&first);

    strip(".canon/learn/\n");
    let before = tree(repo);
    let refused = canon(repo, &["migrate", "--commit", "--plan-digest", &old, "--json"]);
    assert_eq!(refused.status.code(), Some(2), "{}", text(&refused));
    assert_eq!(json(&refused)["error"]["code"], "PLAN_CHANGED");
    assert_eq!(tree(repo), before, "a refused commit writes nothing");

    let fresh = digest_of(&ok(repo, &["migrate", "--json"]));
    assert_ne!(fresh, old, "the plan changed, so its digest did");
    let done = ok(repo, &["migrate", "--commit", "--plan-digest", &fresh, "--json"]);
    assert_eq!(json(&done)["data"]["committed"], true);
    let gitignore = std::fs::read_to_string(repo.join(".gitignore")).unwrap();
    assert!(gitignore.contains(".canon/r2/") && gitignore.contains(".canon/learn/"), "{gitignore}");
}

/// authoring.scaffold.15 (D7): a batch is validated whole before the first write; the refusal
/// points at the invalid element's field and nothing, not even the valid first element, is written.
#[test]
fn scaffold_15_a_batch_with_an_invalid_case_writes_nothing_and_points_at_the_element() {
    let dir = repo();
    let repo = dir.path();
    ok(repo, &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    let before = tree(repo);
    let doc = r#"[{"operands":["demo.login.01"],"title":"One","case":"happy"},{"operands":["demo.login.02"],"title":"Two","case":"Not A Slug"}]"#;
    let out = canon(repo, &["scenario", "new", "--input", doc, "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}", text(&out));
    let body = json(&out);
    assert_eq!(body["ok"], false, "{body}");
    assert_eq!(body["error"]["field"], "/items/1/case", "{body}");
    assert_eq!(body["error"]["received"], "Not A Slug", "{body}");
    assert!(!repo.join(".canon/specs/kind=feature/area=demo").exists(), "element 0 was not written");
    assert_eq!(tree(repo), before, "the refused batch changed nothing");

    // The same batch with the case fixed writes both.
    let fixed = doc.replace("Not A Slug", "failure");
    ok(repo, &["scenario", "new", "--input", &fixed, "--json"]);
    let feature = std::fs::read_to_string(repo.join(".canon/specs/kind=feature/area=demo/login.feature")).unwrap();
    assert!(feature.contains("@demo.login.01") && feature.contains("@demo.login.02"), "{feature}");
}

/// skills.install.04: the install lock keys on content: identical bytes hash alike, one differing
/// byte differs, and the hash names its algorithm.
#[test]
fn skills_install_04_the_content_hash_is_stable_and_content_addressed() {
    let a = canon_cli::skills::content_hash(b"the same bytes");
    assert_eq!(a, canon_cli::skills::content_hash(b"the same bytes"));
    assert_ne!(a, canon_cli::skills::content_hash(b"the same bytez"));
    assert!(a.starts_with("sha256:"), "{a}");
}

/// skills.install.07: an unknown provider fails naming it, before any projection or lock is written.
#[test]
fn skills_install_07_an_unknown_provider_is_refused_before_anything_is_written() {
    let dir = repo();
    let repo = dir.path();
    let before = tree(repo);
    let out = canon(repo, &["skills", "install", "--providers", "gemini", "--json"]);
    assert_ne!(out.status.code(), Some(0), "{}", text(&out));
    let body = json(&out);
    assert_eq!(body["ok"], false, "{body}");
    assert!(body["error"]["message"].as_str().unwrap().contains("gemini"), "the provider is named: {body}");
    assert_eq!(tree(repo), before, "no projection and no lock were written");
    assert!(!repo.join(".canon/skills").exists());
}

/// skills.install.21: the leaves open to no agent role are exactly the three human-only acts.
#[test]
fn skills_install_21_only_the_three_human_acts_name_no_agent_role() {
    let dir = repo();
    let listing = json(&ok(dir.path(), &["schema", "--json"]));
    let commands = listing["data"]["commands"].as_array().unwrap();
    assert!(commands.len() > 50, "the listing is the whole tree: {}", commands.len());
    let closed: Vec<&str> = commands.iter().filter(|c| c["roles"].as_array().is_some_and(Vec::is_empty)).map(|c| c["command"].as_str().unwrap()).collect();
    assert_eq!(closed, ["brief approve", "divergence accept", "policy approve"], "every other leaf names an agent role");
}

/// skills.install.22: the write commands do their own bookkeeping: no format, import, index or
/// promote step sits between writing a scenario, the gate seeing it and evidence counting.
#[test]
fn skills_install_22_the_write_commands_do_their_own_bookkeeping() {
    let dir = repo();
    let repo = dir.path();
    ok(repo, &["subject", "new", "demo", "--domain", "dev", "--title", "Demo"]);
    ok(repo, &["scenario", "new", "demo.login.01", "--title", "Logs in", "--subject-id", "demo", "--case", "failure"]);
    ok(repo, &["format", ".canon/specs"]);

    // The gate indexes the new scenario itself and names it as having no evidence.
    let gate = canon(repo, &["gate", "check"]);
    assert_eq!(gate.status.code(), Some(1), "{}", text(&gate));
    assert!(text(&gate).contains("uncovered-cell demo.login.01"), "{}", text(&gate));

    // evidence add promotes in the same run: the gate no longer names the scenario.
    let staged = ["evidence", "add", "--scenario-id", "demo.login.01", "--class", "test-run", "--ref", "cargo test", "--role", "implementer", "--actor-id", "impl"];
    let add = json(&ok(repo, &[&staged[..], &["--json"]].concat()));
    assert!(add["data"]["records"][0]["committed"].is_string(), "promoted in the same run: {add}");
    let gate = canon(repo, &["gate", "check"]);
    assert!(!text(&gate).contains("uncovered-cell demo.login.01"), "{}", text(&gate));

    // --no-promote stages only: the gate still names the scenario and the next step is the promote.
    ok(repo, &["scenario", "new", "demo.login.02", "--title", "Fails", "--subject-id", "demo", "--case", "failure"]);
    let held = json(&ok(
        repo,
        &["evidence", "add", "--scenario-id", "demo.login.02", "--class", "test-run", "--ref", "cargo test 2", "--role", "implementer", "--actor-id", "impl", "--no-promote", "--json"],
    ));
    assert!(held["data"]["records"][0]["committed"].is_null(), "{held}");
    assert_eq!(held["meta"]["next"][0]["argv"], serde_json::json!(["canon", "gate", "promote"]), "{held}");
    assert!(text(&canon(repo, &["gate", "check"])).contains("uncovered-cell demo.login.02"));
    ok(repo, &["gate", "promote"]);
    assert!(!text(&canon(repo, &["gate", "check"])).contains("uncovered-cell demo.login.02"));
}
