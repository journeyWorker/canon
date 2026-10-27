//! Integration tests for `canon finding add` (s43
//! `findings-are-records`, task group 2), invoking the actually-built
//! `canon` binary (`env!("CARGO_BIN_EXE_canon")`) — the same discipline
//! `tests/evidence.rs` and `tests/gate.rs` state for themselves:
//! subprocess-level behavior, exit codes, and real-filesystem side
//! effects need the real binary boundary.
//! `crates/canon-cli/src/finding.rs`'s own unit tests already cover the
//! library decisions in-process; what only THIS boundary can prove is
//! that the clap surface exists and parses, that clap's own
//! `value_parser` refusals land where the module doc says they do, and
//! that the help an operator actually reads says what it claims to.
//!
//! The headline test is
//! [`the_two_command_loop_commits_a_finding_nobody_hand_wrote`]:
//! `canon finding add` → `canon gate promote` → a committed record.
//! Before s43 that sequence was unrunnable, and a review round's
//! findings evaporated with its transcript.

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn run_canon(args: &[&str], cwd: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(cwd).output().expect("spawn canon binary")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// A `--help` invocation's stdout, or WHY it may not be read.
///
/// A help assertion that is entirely NEGATIVE (`!help.contains(...)`)
/// is satisfied by the empty string, so a help branch that never
/// checks the invocation itself reports success having checked
/// nothing — the vacuous-pass shape `panel-copy.test.ts` was rewritten
/// twice to remove, and the shape s43 round 3 (finding 4) found here.
/// Every caller goes through this: exit status and one stable POSITIVE
/// marker first, negative assertions only afterwards.
fn checked_help(output: &Output, marker: &str) -> Result<String, String> {
    if !output.status.success() {
        return Err(format!("--help exited {:?}: {}", output.status.code(), stderr(output)));
    }
    let help = stdout(output);
    if !help.contains(marker) {
        return Err(format!("--help printed no `{marker}` ({} bytes of stdout)", help.len()));
    }
    Ok(help)
}

fn help_stdout(args: &[&str], marker: &str, cwd: &Path) -> String {
    checked_help(&run_canon(args, cwd), marker).unwrap_or_else(|why| panic!("`canon {}`: {why}", args.join(" ")))
}

/// The stable positive markers, one per help surface: clap's own usage
/// line, which is present iff the command parsed and printed help at
/// all, and which no `after_help` wording change can quietly remove.
const ADD_HELP_MARKER: &str = "Usage: canon finding add";
const PARENT_HELP_MARKER: &str = "Usage: canon finding [OPTIONS] <COMMAND>";

/// A repo with nothing but a `canon.yaml`.
///
/// Deliberately barer than `tests/evidence.rs`'s fixture, and the
/// asymmetry is the point: `canon evidence add` admits its `--task`
/// against the live plan corpus, so it needs one. A finding binds to a
/// `ChangeId` by grammar alone and asserts no `Task` record, so
/// requiring a plan corpus here would be a coupling the record does not
/// have.
fn repo() -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a temp dir");
    std::fs::write(dir.path().join("canon.yaml"), "routing:\n  finding: local\n").expect("writing canon.yaml");
    // Every finding command binds a git revision, so the fixture is a
    // work tree with one commit.
    git(dir.path(), &["init", "-q", "-b", "main"]);
    git(dir.path(), &["add", "-A"]);
    git(dir.path(), &["commit", "-q", "-m", "initial"]);
    dir
}

/// Files in a tier subtree, by directory listing rather than a library
/// read — this file deliberately owns no in-process knowledge of
/// canon's layout beyond the one path the CLI documents.
fn files(root: PathBuf) -> Vec<PathBuf> {
    let Ok(entries) = std::fs::read_dir(&root) else { return Vec::new() };
    let mut paths: Vec<PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    // Total, data-derived order so an assertion never depends on
    // filesystem walk order.
    paths.sort();
    paths
}

fn staged_findings(repo: &Path) -> Vec<PathBuf> {
    files(repo.join(".canon/ledger/_staging/kind=finding"))
}

fn committed_findings(repo: &Path) -> Vec<PathBuf> {
    { let mut all = files(repo.join(".canon/ledger/kind=finding")); all.extend(files(repo.join(".canon/archive/kind=finding"))); all }
}

fn body_at(path: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(path).expect("reading a record")).expect("valid JSON")
}

/// The base flags every well-formed add needs, so a test naming a
/// refusal shows only the flag it is about.
const BASE: [&str; 9] = ["finding", "add", "--change", "s43-findings-are-records", "--severity", "blocker", "--introduced-unknown", "--repo", "."];

fn add(repo: &Path, extra: &[&str]) -> Output {
    let mut args: Vec<&str> = BASE.to_vec();
    if extra.contains(&"--introduced-rev") {
        args.retain(|arg| *arg != "--introduced-unknown");
    }
    args.extend_from_slice(extra);
    run_canon(&args, repo)
}

/// s43 task 2.1 end to end, through the real binary: a review finding
/// becomes a committed record, and every field round-trips as asked
/// for. Each step is asserted for its EFFECT — staging populated,
/// staging drained, committed body correct — so a step that silently
/// no-ops cannot be masked by the next one succeeding.
///
/// The `--rev`-less case is deliberate and load-bearing: the round
/// binds HEAD and the working tree as it stands, so a round that
/// reviewed uncommitted code is recorded with its code tree rather than
/// refused.
#[test]
fn the_two_command_loop_commits_a_finding_nobody_hand_wrote() {
    let (dir, head) = git_repo();
    let repo = dir.path();

    let added = add(
        repo,
        &["--round", "8", "--seq", "3", "--disposition", "open", "--actor", "review-voice", "--summary", "round 8 reviewed an uncommitted worktree"],
    );
    assert!(added.status.success(), "finding add must succeed; stderr: {}", stderr(&added));
    assert!(stdout(&added).contains("staged"), "{}", stdout(&added));
    assert_eq!(staged_findings(repo).len(), 1, "the record must land in _staging");
    assert!(committed_findings(repo).is_empty(), "a staged record must not be committed yet");

    let promoted = run_canon(&["gate", "promote", "--repo", "."], repo);
    assert!(promoted.status.success(), "promotion must be clean; stdout: {} stderr: {}", stdout(&promoted), stderr(&promoted));
    assert!(staged_findings(repo).is_empty(), "promotion drains staging");
    assert_eq!(committed_findings(repo).len(), 1, "the record is now committed");

    let body = body_at(&committed_findings(repo)[0]);
    assert_eq!(body["kind"], "finding", "{body}");
    assert_eq!(body["change_id"], "s43-findings-are-records", "{body}");
    assert_eq!(body["round"], 8, "{body}");
    assert_eq!(body["seq"], 3, "{body}");
    assert_eq!(body["severity"], "blocker", "{body}");
    assert_eq!(body["disposition"], "open", "{body}");
    assert_eq!(body["reviewer"], "review-voice", "{body}");
    assert_eq!(body["summary"], "round 8 reviewed an uncommitted worktree", "{body}");

    // The reviewed commit is always recorded now; the optionals stay ABSENT,
    // never an explicit `null` — a `null` fails `Finding`'s own
    // deserialize, so a record written that way would be unreadable.
    assert_eq!(body["reviewed_sha"], head.as_str(), "{body}");
    assert!(body["code_tree"].is_string(), "{body}");
    assert!(body["worktree_dirty"].is_boolean(), "{body}");
    for absent in ["resolution_sha", "introduced_by", "file_ref"] {
        assert!(body.get(absent).is_none(), "`{absent}` must be omitted, never null: {body}");
    }

    // Said on the success path, not only in `--help`: an unsourced
    // finding is one of the two reasons the derived count UNDER-counts,
    // and this is the moment the author still knows whether it was
    // sourced.
    assert!(stdout(&added).contains("UNSOURCED"), "an add with no --introduced-by must say so: {}", stdout(&added));
}

/// D6: `--session` on `finding add` and `finding close` fills each
/// record's own `actor.session_id`; the close records the session that
/// authored the TRANSITION, never the raiser's.
#[test]
fn session_ids_on_add_and_close_land_on_each_records_actor() {
    let dir = repo();
    let repo = dir.path();

    let added = add(repo, &["--round", "1", "--seq", "1", "--actor", "impl-agent", "--summary", "found it myself", "--session", "sess-raise"]);
    assert!(added.status.success(), "stderr: {}", stderr(&added));
    assert!(stdout(&added).contains("— run `canon gate promote` to commit it"), "{}", stdout(&added));
    assert!(run_canon(&["gate", "promote"], repo).status.success());

    let closed = run_canon(
        &["finding", "close", "s43-findings-are-records", "--round", "1", "--seq", "1", "--disposition", "rejected", "--reason", "by design", "--session", "sess-close"],
        repo,
    );
    assert!(closed.status.success(), "stderr: {}", stderr(&closed));
    assert!(stdout(&closed).contains("— run `canon gate promote` to commit it"), "{}", stdout(&closed));
    assert!(run_canon(&["gate", "promote"], repo).status.success());

    let mut sessions: Vec<(String, String)> = committed_findings(repo)
        .iter()
        .map(|path| body_at(path))
        .map(|b| (b["disposition"].as_str().unwrap().to_string(), b["actor"]["session_id"].as_str().unwrap().to_string()))
        .collect();
    sessions.sort();
    assert_eq!(sessions, [("open".to_string(), "sess-raise".to_string()), ("rejected".to_string(), "sess-close".to_string())]);
}

#[test]
fn finding_add_refuses_a_session_id_outside_the_grammar() {
    let dir = repo();
    let out = add(dir.path(), &["--round", "1", "--seq", "1", "--actor", "r", "--summary", "s", "--session", " padded"]);
    assert_eq!(out.status.code(), Some(2), "stderr: {}", stderr(&out));
    assert!(stderr(&out).contains("--session") && stderr(&out).contains("SessionId"), "{}", stderr(&out));
    assert!(staged_findings(dir.path()).is_empty(), "a refused add writes nothing");
}

/// `--introduced-rev` names the commit that introduced a defect, so
/// `self` (who FOUND it) is not a revision and is refused.
#[test]
fn finding_add_refuses_introduced_rev_self() {
    let dir = repo();
    let out = add(dir.path(), &["--round", "1", "--seq", "1", "--actor", "impl-agent", "--summary", "s", "--introduced-rev", "self"]);
    assert_eq!(out.status.code(), Some(2), "stderr: {}", stderr(&out));
    assert!(staged_findings(dir.path()).is_empty(), "a refused add writes nothing");
}

/// Every optional flag, set, surviving promotion unmodified — the
/// committed body of a `Finding` is the staged body byte for byte
/// (`canon_gate::StagedAssignment::Nothing`), so this also pins that
/// promotion stamps nothing onto it.
#[test]
fn a_fixed_and_sourced_finding_round_trips_every_optional_through_promotion() {
    let dir = repo();
    let repo = dir.path();
    git(repo, &["commit", "-q", "--allow-empty", "-m", "second"]);
    git(repo, &["commit", "-q", "--allow-empty", "-m", "third"]);
    let resolution = git(repo, &["rev-parse", "HEAD"]);
    let introduced = git(repo, &["rev-parse", "HEAD~1"]);
    let reviewed = git(repo, &["rev-parse", "HEAD~2"]);

    let added = add(
        repo,
        &[
            "--round",
            "11",
            "--seq",
            "1",
            "--disposition",
            "fixed",
            "--resolution-rev",
            &resolution,
            "--introduced-rev",
            &introduced,
            "--rev",
            &reviewed,
            "--file-ref",
            "crates/canon-cli/src/finding.rs:120-134",
            "--actor",
            "review-voice",
            "--summary",
            "a defect in the previous round's fix",
        ],
    );
    assert!(added.status.success(), "stderr: {}", stderr(&added));
    assert!(!stdout(&added).contains("UNSOURCED"), "a sourced finding must not be labelled unsourced: {}", stdout(&added));
    let staged = body_at(&staged_findings(repo)[0]);

    assert!(run_canon(&["gate", "promote", "--repo", "."], repo).status.success());
    let committed = body_at(&committed_findings(repo)[0]);

    assert_eq!(committed, staged, "promotion must commit the staged body unchanged");
    assert_eq!(committed["disposition"], "fixed", "{committed}");
    assert_eq!(committed["resolution_sha"], resolution, "{committed}");
    assert_eq!(committed["introduced_by"], introduced, "{committed}");
    assert_eq!(committed["reviewed_sha"], reviewed, "{committed}");
    assert_eq!(committed["file_ref"], "crates/canon-cli/src/finding.rs:120-134", "{committed}");
}

/// `fixed` with no `--resolution-rev` resolves HEAD.
#[test]
fn finding_add_fixed_defaults_the_resolution_to_head() {
    let (dir, head) = git_repo();
    let repo = dir.path();

    let output = add(repo, &["--round", "1", "--seq", "1", "--disposition", "fixed", "--actor", "r", "--summary", "s"]);
    assert_eq!(output.status.code(), Some(0), "stderr: {}", stderr(&output));
    let staged = body_at(&staged_findings(repo)[0]);
    assert_eq!(staged["resolution_sha"], head.as_str(), "{staged}");
}

/// A resolution revision is refused on EVERY non-`fixed` disposition,
/// not just `open`: `Finding::rejected`/`deferred` clear the resolution,
/// so a CLI that only refused `open` would silently drop it.
#[test]
fn finding_add_04_refuses_a_resolution_rev_on_every_non_fixed_disposition() {
    for disposition in ["open", "rejected", "deferred"] {
        let dir = repo();
        let repo = dir.path();

        let output = add(repo, &["--round", "1", "--seq", "1", "--disposition", disposition, "--resolution-rev", "HEAD", "--actor", "r", "--summary", "s"]);
        assert_eq!(output.status.code(), Some(2), "{disposition} accepted a resolution rev; stdout: {}", stdout(&output));
        assert!(staged_findings(repo).is_empty(), "{disposition}: a refused add must stage nothing");
    }
}

/// Only a `fixed` finding carries a resolution, and a `fixed` one always does:
/// with no `--resolution-rev` it resolves HEAD, and no other disposition has one.
#[test]
fn finding_add_04_only_a_fixed_finding_carries_a_resolution_and_a_fixed_one_always_does() {
    let (dir, head) = git_repo();
    let repo = dir.path();
    for (seq, disposition) in ["open", "rejected", "deferred", "fixed"].into_iter().enumerate() {
        let seq = (seq + 1).to_string();
        let extra = ["--round", "1", "--seq", &seq, "--disposition", disposition, "--actor", "r", "--summary", "s"];
        let output = add(repo, &extra);
        assert_eq!(output.status.code(), Some(0), "{disposition}: {}{}", stdout(&output), stderr(&output));
    }
    let staged: Vec<serde_json::Value> = staged_findings(repo).iter().map(|path| body_at(path)).collect();
    assert_eq!(staged.len(), 4);
    for body in &staged {
        let resolved = body.get("resolution_sha").is_some_and(|sha| !sha.is_null());
        if body["disposition"] == "fixed" {
            assert_eq!(body["resolution_sha"], head.as_str(), "a fixed finding with no --resolution-rev resolves HEAD: {body}");
        } else {
            assert!(!resolved, "only a fixed finding carries a resolution: {body}");
        }
    }
}

/// Runs `git` in `repo` with a fixed identity and clock and no commit
/// signing, so the commits a test makes have the same ids on every
/// machine.
fn git(repo: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .arg("-C")
        .arg(repo)
        .args(["-c", "commit.gpgsign=false", "-c", "tag.gpgsign=false"])
        .args(args)
        .env("GIT_AUTHOR_NAME", "a")
        .env("GIT_AUTHOR_EMAIL", "a@a")
        .env("GIT_COMMITTER_NAME", "a")
        .env("GIT_COMMITTER_EMAIL", "a@a")
        .env("GIT_AUTHOR_DATE", "2020-01-01T00:00:00Z")
        .env("GIT_COMMITTER_DATE", "2020-01-01T00:00:00Z")
        .output()
        .expect("git must be on PATH for this test");
    assert!(out.status.success(), "git {args:?}: {}", stderr(&out));
    stdout(&out).trim().to_string()
}

/// [`repo`] plus its full commit sha.
fn git_repo() -> (tempfile::TempDir, String) {
    let dir = repo();
    let head = git(dir.path(), &["rev-parse", "HEAD"]);
    (dir, head)
}

/// Every revision flag takes any git revision (here a short sha) and
/// the record stores the full sha — on `finding add` and `finding close`.
#[test]
fn a_short_revision_is_resolved_to_the_full_commit_on_add_and_close() {
    let (dir, head) = git_repo();
    let repo = dir.path();
    let short = &head[..7];

    let added = add(repo, &["--round", "1", "--seq", "1", "--actor", "r", "--summary", "s", "--rev", short, "--introduced-rev", short]);
    assert_eq!(added.status.code(), Some(0), "stderr: {}", stderr(&added));
    let staged = body_at(&staged_findings(repo)[0]);
    assert_eq!(staged["reviewed_sha"], head.as_str(), "the full sha is stored, never the abbreviation: {staged}");
    assert_eq!(staged["introduced_by"], head.as_str(), "{staged}");
    assert!(run_canon(&["gate", "promote"], repo).status.success());

    let closed = run_canon(
        &["finding", "close", "s43-findings-are-records", "--round", "1", "--seq", "1", "--disposition", "fixed", "--resolution-rev", short],
        repo,
    );
    assert_eq!(closed.status.code(), Some(0), "stderr: {}", stderr(&closed));
    let transition = body_at(&staged_findings(repo)[0]);
    assert_eq!(transition["resolution_sha"], head.as_str(), "{transition}");

    let fixed = add(repo, &["--round", "1", "--seq", "2", "--disposition", "fixed", "--resolution-rev", short, "--actor", "r", "--summary", "s"]);
    assert_eq!(fixed.status.code(), Some(0), "stderr: {}", stderr(&fixed));
    let staged_fixed = staged_findings(repo).iter().map(|path| body_at(path)).find(|body| body["seq"] == 2).expect("the fixed finding is staged");
    assert_eq!(staged_fixed["resolution_sha"], head.as_str(), "the full 40-character sha is stored: {staged_fixed}");
    assert_eq!(head.len(), 40);
}

/// Task 2.1, at the boundary the equivalent vector was actually
/// reported against in s42: a shell-quoted line separator reaching the
/// real binary. Every free-text field is checked, because the set of
/// fields is as easy to get wrong as the set of characters.
#[test]
fn finding_add_refuses_a_newline_that_would_forge_a_second_row() {
    // The injected value REPLACES the field's normal value rather than
    // being appended, so each case is a single well-formed invocation
    // differing from a passing one in exactly one field.
    for (flag, reviewer, summary, extra) in [
        ("--summary", "r", "ok\n| forged | row |", Vec::new()),
        ("--actor", "r\n| forged | row |", "s", Vec::new()),
        ("--file-ref", "r", "s", vec!["--file-ref", "src/x.rs:1-2\n| forged | row |"]),
    ] {
        let dir = repo();
        let repo = dir.path();

        let mut flags = vec!["--round", "1", "--seq", "1", "--actor", reviewer, "--summary", summary];
        flags.extend_from_slice(&extra);
        let output = add(repo, &flags);
        assert_eq!(output.status.code(), Some(2), "{flag} accepted a line separator; stdout: {}", stdout(&output));
        let err = stderr(&output);
        assert!(err.contains(flag), "the refusal must name the flag: {err}");
        assert!(err.contains("line separator"), "and what it rejected: {err}");
        assert!(staged_findings(repo).is_empty(), "{flag}: a refused add must stage nothing");
    }
}

/// A non-`\n` member of the imported set, at the binary boundary: the
/// refusal covers `canon_ingest::task_rows::ROW_LINE_BREAKS`, not just
/// the newline anyone would think to test. This is what "imported, not
/// restated" buys — U+2028 is a row break in the grammar, so it is one
/// here too, without this command knowing that on its own.
#[test]
fn finding_add_refuses_a_unicode_line_separator_from_the_shared_set() {
    let dir = repo();
    let repo = dir.path();

    let output = add(repo, &["--round", "1", "--seq", "1", "--actor", "r", "--summary", "ok\u{2028}| forged | row |"]);
    assert_eq!(output.status.code(), Some(2), "stdout: {}", stdout(&output));
    assert!(stderr(&output).contains("line separator"), "{}", stderr(&output));
    assert!(staged_findings(repo).is_empty());
}

/// The collision behaviour `--seq` being REQUIRED obliges this command
/// to define: a second record at an occupied `(change_id, round, seq)`
/// is refused, naming the key.
///
/// The clashing add carries a DIFFERENT `--summary` on purpose. That is
/// the case `GitTier::write`'s own duplicate-path check cannot catch —
/// the content digest in the filename differs, so both files would land
/// under one natural key and the finding count would silently inflate,
/// which is the exact number s43 exists to make trustworthy.
#[test]
fn finding_add_refuses_a_second_record_at_an_occupied_seq() {
    let dir = repo();
    let repo = dir.path();

    assert!(add(repo, &["--round", "2", "--seq", "1", "--actor", "r", "--summary", "the first finding"]).status.success());

    let staged_clash = add(repo, &["--round", "2", "--seq", "1", "--actor", "r", "--summary", "a completely different finding"]);
    assert_eq!(staged_clash.status.code(), Some(2), "stdout: {}", stdout(&staged_clash));
    assert!(stderr(&staged_clash).contains("s43-findings-are-records_2_1"), "the refusal must name the key: {}", stderr(&staged_clash));
    assert_eq!(staged_findings(repo).len(), 1, "the clashing add must stage nothing");

    // And after promotion, when the occupant is COMMITTED rather than
    // staged — the tier the check would be easiest to forget.
    assert!(run_canon(&["gate", "promote", "--repo", "."], repo).status.success());
    let committed_clash = add(repo, &["--round", "2", "--seq", "1", "--actor", "r", "--summary", "a third distinct finding"]);
    assert_eq!(committed_clash.status.code(), Some(2), "stdout: {}", stdout(&committed_clash));
    assert!(stderr(&committed_clash).contains("committed"), "the refusal must say which tier holds the occupant: {}", stderr(&committed_clash));
    assert!(staged_findings(repo).is_empty());
    assert_eq!(committed_findings(repo).len(), 1);
}

fn json_of(output: &Output) -> serde_json::Value {
    serde_json::from_slice(&output.stdout).unwrap_or_else(|e| panic!("stdout is not JSON ({e}): {}", stdout(output)))
}

/// Every file under `root`, with its bytes, so a repeat can be shown to write nothing.
fn tree(root: &Path) -> std::collections::BTreeMap<PathBuf, Vec<u8>> {
    fn walk(dir: &Path, out: &mut std::collections::BTreeMap<PathBuf, Vec<u8>>) {
        for entry in std::fs::read_dir(dir).into_iter().flatten().flatten() {
            let path = entry.path();
            // git refreshes its own index when canon reads the work tree; that is not a canon write
            if entry.file_name() == ".git" {
                continue;
            }
            if path.is_dir() {
                walk(&path, out);
            } else {
                out.insert(path.clone(), std::fs::read(&path).unwrap());
            }
        }
    }
    let mut out = std::collections::BTreeMap::new();
    walk(root, &mut out);
    out
}

/// An identical `finding add` repeated, staged or committed, writes nothing and
/// says so; the same key with another body is still refused.
#[test]
fn an_identical_finding_add_is_unchanged_and_writes_nothing() {
    let dir = repo();
    let repo = dir.path();
    let args = ["--round", "4", "--seq", "1", "--actor", "r", "--summary", "the same finding", "--json"];

    let first = add(repo, &args);
    assert!(first.status.success(), "{}", stdout(&first));
    assert!(json_of(&first)["meta"].get("unchanged").is_none(), "the first add writes");

    let before = tree(repo);
    let staged_repeat = add(repo, &args);
    assert_eq!(staged_repeat.status.code(), Some(0), "{}", stderr(&staged_repeat));
    assert_eq!(json_of(&staged_repeat)["meta"]["unchanged"], true);
    assert_eq!(tree(repo), before, "a repeat of a staged finding writes nothing");

    assert!(run_canon(&["gate", "promote", "--repo", "."], repo).status.success());
    let before = tree(repo);
    let committed_repeat = add(repo, &args);
    assert_eq!(committed_repeat.status.code(), Some(0), "{}", stderr(&committed_repeat));
    assert_eq!(json_of(&committed_repeat)["meta"]["unchanged"], true);
    assert_eq!(tree(repo), before, "a repeat of a committed finding writes nothing");

    let other = add(repo, &["--round", "4", "--seq", "1", "--actor", "r", "--summary", "another finding", "--json"]);
    assert_eq!(other.status.code(), Some(2), "{}", stdout(&other));
}

/// Closing a finding to the disposition it already has (`fixed` and `rejected` alike) writes nothing and is `meta.unchanged`.
#[test]
fn closing_a_finding_to_its_current_disposition_is_unchanged() {
    let dir = repo();
    let repo = dir.path();
    for (round, disposition, reason) in [("5", "rejected", Some("works as designed")), ("6", "fixed", None)] {
        assert!(add(repo, &["--round", round, "--seq", "1", "--actor", "r", "--summary", "to close"]).status.success());
        assert!(run_canon(&["gate", "promote", "--repo", "."], repo).status.success());
        let mut close = vec!["finding", "close", "s43-findings-are-records", "--round", round, "--seq", "1", "--disposition", disposition, "--repo", ".", "--json"];
        if let Some(reason) = reason {
            close.extend(["--reason", reason]);
        }

        let first = run_canon(&close, repo);
        assert!(first.status.success(), "{disposition}: {}", stdout(&first));
        assert!(json_of(&first)["meta"].get("unchanged").is_none(), "{disposition}: the first close writes");
        assert!(run_canon(&["gate", "promote", "--repo", "."], repo).status.success());

        let before = tree(repo);
        let again = run_canon(&close, repo);
        assert_eq!(again.status.code(), Some(0), "{disposition}: {}", stderr(&again));
        assert_eq!(json_of(&again)["meta"]["unchanged"], true, "{disposition}");
        assert_eq!(tree(repo), before, "{disposition}: a repeated close writes nothing");
    }
}

/// A neighbouring `seq` in the same round is a different finding: the
/// collision check must not degrade into a per-round refusal.
#[test]
fn finding_add_accepts_the_next_seq_in_the_same_round() {
    let dir = repo();
    let repo = dir.path();

    assert!(add(repo, &["--round", "3", "--seq", "1", "--actor", "r", "--summary", "first"]).status.success());
    assert!(add(repo, &["--round", "3", "--seq", "2", "--actor", "r", "--summary", "second"]).status.success());
    assert_eq!(staged_findings(repo).len(), 2);
}

/// `canon gate promote` must be able to REPORT what it drained per
/// kind, and must say plainly when it drained nothing.
///
/// This is the regression that made the whole `STAGED_KINDS` registry
/// necessary: before it, `canon_gate::promote` read one hardcoded kind,
/// so a staged finding was invisible — `canon finding add` printed "run
/// `canon gate promote`", promote printed "nothing staged", and the
/// record sat in `_staging/` forever, unreadable by every
/// committed-tier query.
#[test]
fn gate_promote_tallies_per_kind_and_names_an_empty_staging_area() {
    let dir = repo();
    let repo = dir.path();

    let empty = run_canon(&["gate", "promote", "--repo", "."], repo);
    assert!(empty.status.success(), "stderr: {}", stderr(&empty));
    let empty_out = stdout(&empty);
    assert!(empty_out.contains("nothing staged"), "{empty_out}");
    assert!(empty_out.contains("finding"), "an empty promote must name the kinds it CAN drain: {empty_out}");
    assert!(empty_out.contains("evidence"), "{empty_out}");

    assert!(add(repo, &["--round", "4", "--seq", "1", "--actor", "r", "--summary", "first"]).status.success());
    assert!(add(repo, &["--round", "4", "--seq", "2", "--actor", "r", "--summary", "second"]).status.success());

    let promoted = run_canon(&["gate", "promote", "--repo", "."], repo);
    assert!(promoted.status.success(), "stderr: {}", stderr(&promoted));
    let out = stdout(&promoted);
    assert!(out.contains("finding=2"), "the tally must be readable per kind: {out}");
    assert!(out.contains("evidence=0"), "including the kinds that drained nothing: {out}");
}

/// context.capability.23: the staged kinds are named as `--kind` takes them
/// (`evidence`), never by the storage name `evidence_record`, in the tally
/// and in the line that lists what could be promoted.
#[test]
fn context_capability_23_gate_promote_tallies_the_staged_kinds_by_the_names_kind_takes() {
    let dir = repo();
    let repo = dir.path();

    let empty = stdout(&run_canon(&["gate", "promote", "--repo", "."], repo));
    assert!(empty.contains("nothing staged") && empty.contains("(evidence, finding"), "{empty}");
    assert!(!empty.contains("evidence_record"), "{empty}");

    assert!(add(repo, &["--round", "5", "--seq", "1", "--actor", "r", "--summary", "first"]).status.success());
    let promoted = stdout(&run_canon(&["gate", "promote", "--repo", "."], repo));
    assert!(promoted.contains("evidence=0, finding=1"), "{promoted}");
    assert!(!promoted.contains("evidence_record"), "{promoted}");

    // A staged evidence record and a staged finding together: both are named as `--kind` takes them.
    let evidence = run_canon(&["evidence", "add", "--project", "root", "--scenario", "a.b.01", "--class", "test-run", "--ref", "npm test", "--role", "implementer", "--repo", "."], repo);
    assert!(evidence.status.success(), "stderr: {}", stderr(&evidence));
    assert!(add(repo, &["--round", "5", "--seq", "2", "--actor", "r", "--summary", "second"]).status.success());
    let tally = stdout(&run_canon(&["gate", "promote", "--repo", "."], repo));
    let tally_line = tally.lines().find(|l| l.starts_with("canon gate promote:")).expect("a tally line");
    assert!(tally_line.contains("evidence=1, finding=1"), "{tally}");
    assert!(!tally_line.contains("evidence_record"), "{tally}");
}

/// The nouns whose COUNT is the banned claim, in the plural. A value
/// within two words BEFORE one of these is "how many", which is the
/// claim. Only before: see [`hand_typed_count`]'s retirement section
/// for why the guard stopped reading the other side.
const METRIC_PLURALS: &[&str] = &["findings", "rounds", "issues", "records", "fix-of-fixes"];

/// The same nouns in the singular. A cardinal DIRECTLY after one of
/// these is an INDEX — `--round 1`, "finding 4" — never a count; see
/// [`hand_typed_count`] for what that exclusion buys and what it costs.
const METRIC_SINGULARS: &[&str] = &["finding", "round", "issue", "record", "fix-of-fix"];

/// Spelled cardinals, deliberately starting at `three`: see
/// [`hand_typed_count`]'s doc for the line and why it sits there.
const SPELLED_COUNTS: &[&str] = &[
    "zero",
    "three",
    "four",
    "five",
    "six",
    "seven",
    "eight",
    "nine",
    "ten",
    "eleven",
    "twelve",
    "thirteen",
    "fourteen",
    "fifteen",
    "sixteen",
    "seventeen",
    "eighteen",
    "nineteen",
    "twenty",
    "thirty",
    "forty",
    "fifty",
    "sixty",
    "seventy",
    "eighty",
    "ninety",
    "hundred",
    "thousand",
    "dozen",
    "dozens",
];

/// One whitespace-separated token, lowercased with surrounding
/// punctuation and markup stripped. Hyphens and underscores survive
/// INSIDE a token on purpose: `1-based`, `zero-padded` and
/// `{change_id}__{round:04}__{seq:04}` must stay distinguishable from
/// the bare numbers they contain, and `--round` must stay
/// distinguishable from the noun `round`.
fn word(raw: &str) -> String {
    raw.to_ascii_lowercase().trim_matches(|c: char| !c.is_alphanumeric() && c != '-' && c != '_').to_string()
}

/// The two spelled cardinals mechanism prose is written in — "two
/// concurrent adds", "one finding's identity". See [`hand_typed_count`]
/// for where they are admitted and where they are deliberately not.
const SMALL_SPELLED: &[&str] = &["one", "two"];

/// `true` for a token that is entirely a cardinal quantity, EXCEPT a
/// bare [`SMALL_SPELLED`] one: all digits (`3`, `19`), a spelled
/// cardinal (`three`, `eleven`), a hyphenated compound of SPELLED ones
/// (`twenty-one`, whose `one` is part of a number rather than a word
/// on its own), or a hyphenated run of DIGITS (`120-134`). `1-based`,
/// `zero-padded`, `6-64-char` and `v0.4.0` are NOT quantities — a
/// number welded into a larger word is not an assertion of how many.
///
/// Round 6 read a hyphenated run of digits as a range rather than a
/// quantity, so `120-134 findings` — a hand-typed range count with no
/// path anywhere in the sentence — passed (round 7, finding 2). A
/// range and a count are the same characters; what separates them is
/// POSITION, and [`hand_typed_count`]'s range section is where that
/// discrimination lives. `quantity` answers only "is this a cardinal",
/// and `120-134` is one.
///
/// Each part of a hyphenated run must be NON-EMPTY digits. A bare `-`
/// splits into two empty parts, and `all` over an empty part is
/// vacuously true, so without that requirement a markdown bullet
/// reading "- findings" would be a value directly before a plural
/// metric noun.
fn quantity(word: &str) -> bool {
    let spelled = |part: &str| SPELLED_COUNTS.contains(&part) || SMALL_SPELLED.contains(&part);
    let digits = |part: &str| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit());
    !word.is_empty()
        && !SMALL_SPELLED.contains(&word)
        && match word.contains('-') {
            true => word.split('-').all(spelled) || word.split('-').all(digits),
            false => digits(word) || spelled(word),
        }
}

/// `true` when a raw token closes its clause — it ends the line, or
/// carries clause-ending punctuation once trailing markup is stripped.
/// `2.` and `two,` close; `one` in "one sentence" does not.
fn clause_final(raw: &str, is_last: bool) -> bool {
    is_last || raw.trim_end_matches(|c: char| c.is_alphanumeric()).contains(['.', ',', ';', ':', '!', '?'])
}

/// The offending span when `text` asserts a hand-typed count of
/// findings, rounds, issues, records or fix-of-fixes; `None` otherwise.
///
/// # The rule
/// ONE shape. A VALUE is a cardinal quantity that is not an index
/// (below), and a value within two words BEFORE a plural metric noun
/// is a count claim: "3 findings", "3 review rounds", "eleven
/// code-review rounds", "19 real issues", "44 authored records",
/// "twenty-one open findings", "120-134 findings".
///
/// # What round 8 retired, and what that cost
/// Two further shapes stood here until round 8. Shape 2 read a plural
/// metric noun wearing LABEL punctuation whose next word is a value
/// ("Findings: 3", "findings = 3", "Findings =3"). Shape 3 read a
/// counting word or partitive BOUND to a nearby metric noun and
/// carrying a value in a token window ("the fix-of-fix count is 2",
/// "the records total 44", "the number of findings is three"). Both
/// are gone, along with their vocabulary and their windows.
///
/// The reason is the record rather than taste. Rounds 4, 5, 6 and 7
/// each reported a defect in one of those two shapes; each retune
/// fixed the case it was reported for and introduced another. Every
/// one of those regressions was in shape 2 or shape 3 — the label
/// gap's token boundary, the marker window's anchor, the punctuation
/// evidence firing on ordinary parentheses, the budget outrun by table
/// markup. Shape 1 was never edited and never regressed. Two shapes
/// that cost a review round every time they are touched are worth less
/// than the class they cover.
///
/// The cost is real, and stating it here is the point of stating it at
/// all:
///
/// - This guard NO LONGER GENERALIZES over `<counting word> … <value>`
///   constructions. There is no counting-word vocabulary left, no
///   `number of`, no `in all`, no binding window and no marker budget.
///   "the round count is 7", "the records total 44", "the number of
///   findings is three" and "Findings: 19 in all." all pass.
/// - It no longer reads a value that FOLLOWS its noun, in any
///   spelling. "Findings: 3", "rounds: 11" and "| Findings | 3 |" all
///   pass.
/// - The sentences that ACTUALLY SHIPPED WRONG divide here, and the
///   reject table's two groups are that division. Most carry no
///   counting word — they are bare `<value> <plural noun>` and shape 1
///   still holds them. The two that do carry one, "The true fix-of-fix
///   count is 2." and "the fix-of-fix count is **2**", are now held by
///   literal rows in [`SHIPPED_VERBATIM`] and by nothing else. They
///   are pinned as HISTORY, not as a rule: a NEW sentence of the same
///   form — "the round count is 7" — is caught by neither belt and
///   will ship.
///
/// That last bullet is the whole trade. It is written down because a
/// guard that quietly stops covering something is the failure this
/// file has spent four rounds learning to avoid, and because the
/// alternative — a rule that has to be retuned every round to keep
/// covering it — was measured and found more expensive. Shape 1 is the
/// shape that stayed because it is the one that holds the rest of the
/// historical class outright, and the one no round has had to retune.
///
/// # A digit range counts when it MODIFIES the noun
/// `120-134` standing alone as a token is a cardinal, so "120-134
/// findings" is an assertion of how many findings there are and is
/// caught. A SOURCE SPAN is the same characters in a different
/// position, and position is the entire discrimination: a span never
/// PRECEDES what it spans. It is welded to its path
/// ("path/to/file.rs:120-134"), which arrives as one token that
/// normalizes to no quantity at all, or it FOLLOWS the thing it
/// locates ("Findings: 120-134 in src/finding.rs", "see
/// src/finding.rs:120-134 for the findings"), and the one shape here
/// only ever reads forward from a value to a noun. So the source-span
/// rows and "120-134 findings" are separated by CONTEXT, not by an
/// exemption for ranges. Round 6 reached for the exemption — every
/// all-digit range refused outright — and round 7 (finding 2) is what
/// that cost: with no path in sight, a hand-typed range count passed.
/// The exemption is now unnecessary, because the surviving shape
/// cannot see a range that sits after its subject.
///
/// # INDEX, not count
/// A cardinal DIRECTLY after a SINGULAR metric noun is an index, and
/// every such number on these surfaces is legitimate: `--round 1 --seq
/// 1` in the flag example, "s43 round 2 (finding 4)" citing which round
/// raised what, "(1-based)" in a flag's own help. An index is excluded
/// from being a value AT ALL — not merely from quantifying a following
/// plural — which is what lets "The finding count excludes round 3."
/// and "Round 3 review findings are recorded here." through. Round 4
/// (finding 2) caught both flagged: honest prose, and a guard that
/// forces a reword of honest prose earns its deletion instead.
///
/// Spelled `one` and `two` are the register mechanism prose is written
/// in — "landing two records under one finding's identity", "two
/// distinct findings", "MEANS is one sentence" — and banning them
/// outright would force exactly that reword. So they are a value only
/// where they cannot be quantifying the next word: CLAUSE-FINAL, as in
/// "the fix-of-fix count is two." A digit is a value at any magnitude,
/// because prose describing a mechanism spells its small numbers out
/// and a metric claim reaches for a digit.
///
/// # Residual holes, stated rather than papered over
///
/// Every sentence quoted below as PASSING is a row in
/// [`the_documented_holes_are_the_holes_the_guard_actually_has`],
/// which is what keeps this section from drifting into fiction. Round
/// 6 wrote that test after finding four claims here already false, and
/// it earned itself again in round 8: retiring two shapes turned a
/// long run of reject rows into holes, and the table is where they
/// landed rather than a paragraph nobody can fail.
///
/// - A value that FOLLOWS its noun is not read at all, whatever
///   punctuation sits between them: "Findings: 3", "findings = 3",
///   "rounds: 11", "Findings =3", "| Findings | 3 |", "Records: 44",
///   "findings, 3 of them" and "findings, as of this round, 3" all
///   pass. This is the retired shape 2, and it is also why "Findings:
///   120-134" passes where "120-134 findings" does not.
/// - No counting word, partitive or tally marker is read at all: "the
///   finding count is 19", "the count of findings is three", "the
///   record count is 44", "the records total 44", "fix-of-fix total:
///   3", "the fix-of-fix count is two.", "the number of findings is
///   three", "Findings: 19 in all.", "findings, all told 12", bare "3
///   total", bare "19 in all", "3 apiece" and "the finding number is
///   19" all pass. This is the retired shape 3. "the round count is
///   7" is the row that matters most: it is the exact form that
///   shipped wrong twice, in a wording the literal belt does not
///   carry.
/// - "there are two findings" passes. "there are 2 findings", "there
///   are three findings" and "120-134 findings" all fail.
/// - The index rule is unconditional, so a count written as `<singular
///   noun> <number>` — "Finding 44", meaning forty-four of them —
///   passes. That is the price of `--round 1 --seq 1`, and it is the
///   same rule s43 round 3 already relied on.
/// - The vocabulary is those five nouns. "four of them defects",
///   "three blockers", "two verdicts" pass the shape scan; the literal
///   belt in
///   [`no_shipped_finding_help_or_doc_asserts_a_hand_typed_count`] is
///   what pins the one of those that actually shipped.
/// - The window is two words, so a value pushed further from its noun
///   — "3 of the reported findings" — passes.
/// - Tokens are whitespace-separated, so a label WELDED to its value
///   with no space — "Findings:3", "rounds=11" — arrives as ONE token
///   that normalizes to neither a metric noun nor a quantity, and
///   passes. It is the same welding rule that keeps "v0.4.0",
///   "1-based" and `{round:04}` from reading as quantities, and
///   splitting tokens on inner punctuation to close it would turn all
///   three into numbers this guard then has an opinion about.
/// - The guard is not applied to THIS file, whose tables are by
///   construction full of the shapes it bans.
fn hand_typed_count(text: &str) -> Option<String> {
    let raw: Vec<&str> = text.split_whitespace().collect();
    let words: Vec<String> = raw.iter().map(|token| word(token)).collect();
    if words.is_empty() {
        return None;
    }
    let last = words.len() - 1;
    let at = |i: usize| words.get(i).map_or("", String::as_str);
    // Punctuation-only tokens drop out of the reported span, so the
    // message names the claim rather than the markup carrying it.
    let span = |from: usize, to: usize| words[from..=to.min(last)].iter().filter(|w| !w.is_empty()).cloned().collect::<Vec<String>>().join(" ");
    let cardinal = |i: usize| match words.get(i) {
        Some(word) => quantity(word) || (SMALL_SPELLED.contains(&word.as_str()) && clause_final(raw[i], i == last)),
        None => false,
    };
    let index = |i: usize| i > 0 && METRIC_SINGULARS.contains(&at(i - 1));
    let value = |i: usize| cardinal(i) && !index(i);

    for i in 0..words.len() {
        if value(i) {
            if let Some(j) = (i + 1..=i + 2).find(|&j| METRIC_PLURALS.contains(&at(j))) {
                return Some(span(i, j));
            }
        }
    }
    None
}

/// The exact sentences that shipped wrong, matched as lowercased
/// substrings. The second belt, and since round 8 the ONLY belt for
/// the two of them that wear a counting word — see
/// [`hand_typed_count`]'s retirement section. It also carries "of them
/// defects", whose noun the shape guard's vocabulary does not know.
///
/// A literal list catches the sentences that have shipped and nothing
/// else; that limit is exactly why round 3 stopped relying on one
/// alone, and it is why the entries here are labelled history rather
/// than coverage.
const SHIPPED_VERBATIM: &[&str] = &[
    "real issues",
    "of them defects",
    "fix-of-fix count is",
    "count is 2",
    "count is **2**",
    "eleven code-review rounds",
    "eleven rounds",
    "thirty findings",
    "the true fix-of-fix",
];

/// The [`SHIPPED_VERBATIM`] phrase `text` reproduces, if any.
fn shipped_verbatim(text: &str) -> Option<&'static str> {
    let haystack = text.to_ascii_lowercase();
    SHIPPED_VERBATIM.iter().find(|phrase| haystack.contains(&phrase.to_ascii_lowercase())).copied()
}

/// s43 round 2, finding 4. The help shipped "The true fix-of-fix count
/// is 2." — a hand-typed release metric, on the command built to stop
/// hand-typed release metrics, and the number was WRONG. `tasks.md`
/// 4.2 records that both the published `four` and the correction `two`
/// were guesses. The module doc carried the same claim, plus "eleven
/// code-review rounds" and "thirty findings", neither of which this
/// corpus can supply — the panel these very docs point at says the
/// rounds-RUN count is not derivable at all.
///
/// So: no count of findings, rounds, issues or fix-of-fixes may be
/// asserted in the shipped help or in the module docs behind it. s43
/// round 3 (finding 3) replaced the substring blacklist this started
/// as with [`hand_typed_count`], which catches the SHAPE, and kept the
/// list as a second belt. Round 8 retired two of that matcher's three
/// shapes, which moved the two counting-word sentences back onto the
/// literal belt alone; [`hand_typed_count`] says so, and
/// [`the_count_guard_catches_the_class_and_spares_the_indices`] asserts
/// which belt holds each of them. A number belongs in
/// `mart_review_rounds`; prose gets to point at it and nothing else.
#[test]
fn no_shipped_finding_help_or_doc_asserts_a_hand_typed_count() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let add_help = help_stdout(&["finding", "add", "--help"], ADD_HELP_MARKER, dir.path());
    let finding_help = help_stdout(&["finding", "--help"], PARENT_HELP_MARKER, dir.path());
    // The module docs BEHIND that help: the wrong number lived in both,
    // and correcting only the string a user sees would leave the next
    // author copying it back out of the source.
    let module_doc = include_str!("../src/finding.rs");
    let cli_surface = concat!(include_str!("../src/cli/mod.rs"), include_str!("../src/cli/top.rs"), include_str!("../src/cli/finding.rs"), include_str!("../src/cli/review.rs"));

    for (where_, text) in [
        ("`canon finding add --help`", add_help.as_str()),
        ("`canon finding --help`", finding_help.as_str()),
        ("crates/canon-cli/src/finding.rs", module_doc),
        ("crates/canon-cli/src/cli", cli_surface),
    ] {
        if let Some(phrase) = shipped_verbatim(text) {
            panic!("{where_} asserts a hand-typed count via {phrase:?}");
        }
        // Per LINE, so the span a failure names is the sentence to fix
        // rather than a window spanning half a paragraph.
        for (n, line) in text.lines().enumerate() {
            if let Some(span) = hand_typed_count(line) {
                panic!("{where_} line {} asserts a hand-typed count: {span:?}\n  {line}", n + 1);
            }
        }
    }

    // The positive half: having removed the number, the help must say
    // where the real one lives, or it has simply dropped the subject.
    assert!(add_help.contains("Review rounds"), "the help must point at the derived panel: {add_help}");
    assert!(add_help.contains(".canon/REPORT.md"), "the help must name where that panel is rendered: {add_help}");
}

/// The guard's own contract, as tables — it must catch the class it
/// names, and it must spare every legitimate number these surfaces
/// actually carry. Tables rather than prose so the next hole is a ROW
/// rather than a rewrite, which is what s43 round 4 (findings 1 and 2),
/// round 5 (findings 5, 6 and 7), round 6 (findings 3 and 4) and round
/// 7 (findings 2, 3 and 4) cost when it was neither.
///
/// The reject half is now split by BELT, because round 8 retired shapes
/// 2 and 3 and the split is the only way to state honestly what holds
/// each sentence. A row in the second group asserts BOTH that the shape
/// misses it and that the literal list has it — so the day someone
/// re-generalizes the matcher, or drops a phrase from
/// [`SHIPPED_VERBATIM`], a row says which.
///
/// The accept table is the CURATED half: every line of all four
/// surfaces is scanned live in
/// [`no_shipped_finding_help_or_doc_asserts_a_hand_typed_count`], so
/// the exhaustive accept corpus cannot drift out of date. What the rows
/// here pin is the SHAPES within it — one row per way a legitimate
/// number sits next to a metric noun — so that rewording a surface
/// cannot silently retire a shape's only witness.
#[test]
fn the_count_guard_catches_the_class_and_spares_the_indices() {
    // Belt one: the shape. Every sentence here that shipped wrong is a
    // bare `<value> <plural noun>`, which is the form shape 1 reads and
    // the reason shape 1 is the shape that stayed.
    for (provenance, claim) in [
        // v0.4.0's release tag — the claim this whole command exists on
        // account of.
        ("v0.4.0 tag", "49 real issues, four of them defects in the previous round's fix"),
        // s43's own status prose, before the backfill it describes.
        ("s43 report", "44 authored records sat untracked"),
        ("s43 module doc", "eleven code-review rounds"),
        ("s43 module doc", "thirty findings"),
        // The shapes round 3 caught, kept as rows so a tightening
        // cannot quietly drop one.
        ("shape 1", "there are 3 findings"),
        ("shape 1", "3 review rounds"),
        ("shape 1", "three findings"),
        ("shape 1", "there are three findings on this change"),
        ("shape 1", "twenty-one open findings"),
        ("shape 1", "we closed 7 findings in 2 rounds"),
        // Round 7, finding 2: an all-digit range DIRECTLY modifying a
        // metric plural is a hand-typed count. Round 6 exempted every
        // range to protect source spans, and this passed. The spans it
        // was protecting are accept rows below, separated by position
        // rather than by an exemption.
        ("round 7 finding 2", "120-134 findings"),
        ("round 7 finding 2", "1-4 rounds"),
    ] {
        assert!(hand_typed_count(claim).is_some(), "{provenance}: the guard must catch {claim:?}");
    }

    // Belt two: the literal list, alone. These two shipped wrong and
    // carry a counting word, which the matcher stopped reading in round
    // 8. Asserting the MISS as well as the catch is the point — it is
    // what makes [`hand_typed_count`]'s stated loss falsifiable instead
    // of a paragraph.
    for (provenance, claim) in [("round 2 help", "The true fix-of-fix count is 2."), ("round 2 help", "the fix-of-fix count is **2**")] {
        assert_eq!(
            hand_typed_count(claim),
            None,
            "{provenance}: {claim:?} is caught by the literal belt, not the shape. If the shape now catches it, round 8's retirement has been reverted or generalized and the doc's loss statement is stale."
        );
        assert!(shipped_verbatim(claim).is_some(), "{provenance}: nothing holds {claim:?} any more — the shape retired in round 8 and SHIPPED_VERBATIM has lost its phrase");
    }

    for (provenance, legitimate) in [
        // Round 4, finding 2: the two honest sentences the previous
        // matcher flagged. Both read a ROUND INDEX as a count value.
        ("round 4 finding 2", "The finding count excludes round 3."),
        ("round 4 finding 2", "Round 3 review findings are recorded here."),
        // Indices, from the surfaces that carry them.
        ("finding --help example", "canon finding add --change s43-findings-are-records --round 1 --seq 1"),
        ("src/finding.rs", "a finding's natural key is `{change_id}__{round:04}__{seq:04}`"),
        ("--round help", "Which review round on this change raised it (1-based)"),
        ("--seq help", "This finding's index within the round (1-based)"),
        ("src/finding.rs", "s43 round 2 (finding 4) found this doc and the --help text below"),
        ("src/finding.rs", "round 1 caught the earlier claim that it was: the occupancy scan"),
        ("src/finding.rs", "claiming to be round `r` finding `N+1` -- one of which every reader"),
        ("src/finding.rs", "own round 2 proved that is not enough: findings were authored with"),
        ("src/finding.rs", "s43 round 3, finding 2. All three flags are commit-only by"),
        ("src/finding.rs", "s43 round 2, seq 10 -- the input that got past authoring: a"),
        // Mechanism prose in the spelled-small register — the exact
        // adjacency policing `records` puts at risk.
        ("add --help", "same highest seq and both pick the next one, landing two records under one"),
        ("src/finding.rs", "natural key, and the corpus then carries two distinct findings each"),
        ("src/finding.rs", "two concurrent adds land two distinct findings under one finding's identity"),
        // Counting words that count nothing, and pointers to the panel
        // that does. Since round 8 no counting word is read at all, so
        // these pass for a simpler reason than they used to — they stay
        // as rows because a future re-generalization must not reword
        // them.
        ("add --help", "a DERIVED count that stops being typed from memory"),
        ("add --help", "hand-typed its issue counts into a published tag and got them wrong; that"),
        ("src/finding.rs", "and a **derived count** that stops being typed from memory. Both are"),
        ("src/finding.rs", "would sit under one finding's identity and inflate the count s43"),
        ("src/finding.rs", "the number this change exists to make trustworthy -- which would be"),
        ("src/finding.rs", "numbers its findings, so `--seq` is transcription rather than"),
        ("src/finding.rs", "the counts live in mart_review_rounds, off the 'Review rounds' panel"),
        // Numbers that are neither index nor count: source spans, sha
        // widths, argument literals. The three span rows are round 7
        // finding 2's other half — `quantity` now admits `120-134`, so
        // what spares these is POSITION. The span is welded to its path
        // in the first, and FOLLOWS the noun that names it in the next
        // two; the surviving shape only reads forward from a value.
        ("--file-ref help", "Where in the tree, as path/to/file.rs:120-134 (line breaks refused)"),
        ("quantity doc", "Findings: 120-134 in src/finding.rs"),
        ("quantity doc", "see src/finding.rs:120-134 for the findings"),
        // The empty-part requirement in `quantity`'s hyphen branch. A
        // bare `-` splits into empty parts, and `all` over an empty
        // part is vacuously true, so without it every markdown bullet
        // introducing a list of findings would be a count claim.
        ("markdown bullet", "- findings"),
        ("markdown bullet", "| - | findings |"),
        // Witnesses for the tokens `quantity`'s doc names as NOT
        // quantities. Each is that token placed directly before a
        // plural metric noun — the one position where the surviving
        // shape would read it — so the doc's claim fails as a row the
        // moment the token starts normalizing to a number. These are
        // constructed rather than quoted, which the rows above are
        // not; the provenance says so.
        ("quantity doc", "v0.4.0 findings"),
        ("quantity doc", "1-based rounds"),
        ("quantity doc", "zero-padded records"),
        ("src/main.rs", "The <hash> segment (6-64-char lowercase hex; passed through, never re-hashed)"),
        ("src/finding.rs", "Check the sha (a short sha padded to 40 characters is the way this has actually happened), or omit the flag."),
        ("src/finding.rs", "--round and --seq are 1-based (got --round 0 --seq 1); a 0 would sort ahead of the first real finding in a key whose whole job is ordering rounds"),
        ("src/finding.rs", "assert_eq!(read_back.records.len(), 4);"),
        ("src/main.rs", "Only records with at >= <since> (RFC3339/ISO-8601)"),
        // Round 5, findings 5 and 6: the two honest sentences round 4's
        // vocabulary rejected. `numbers` describes NUMBERING, not a
        // tally; `records` — like `issues` and `rounds` — is a verb as
        // readily as a noun, and shape 2 spent three rounds failing to
        // tell the verb reading from a label before it was retired.
        ("round 5 finding 5", "Finding numbers start at 1."),
        ("round 5 finding 5", "Round numbers and seq numbers both start at 1."),
        ("round 5 finding 6", "The importer records 3 commits."),
        ("round 5 finding 6", "The adapter issues 3 warnings and stops."),
        ("round 5 finding 6", "canon rounds 2 timestamps per run."),
        // Round 7, finding 3: shape 2 read the parenthesis as label
        // evidence and rejected this honest sentence. Retiring the
        // shape is what removed the defect; the row stays so the same
        // sentence cannot be re-broken.
        ("round 7 finding 3", "The importer records (3 commits)."),
        // Round 7, finding 4: shape 3's marker budget was an index over
        // raw whitespace tokens, so table markup pushed the value past
        // the window and the claim passed anyway. Retiring the shape
        // removed the contradiction; what remains is an ordinary accept
        // row, because no shape reads a counting word now.
        ("round 7 finding 4", "| Number of rounds | | 3 |"),
        ("round 6 finding 4", "Number of rounds needs only 3 bytes."),
        ("round 6 finding 4", "the number of rounds is not derivable"),
        // The knife edges in the surfaces' own prose: plural metric
        // nouns in the spelled-small register, and a clause-final
        // `one.` beside one.
        ("src/finding.rs", "What a derived fix-of-fix count MEANS is one sentence, stated here"),
        ("src/finding.rs", "record is one of the two reasons the derived count"),
        ("src/finding.rs", "Two attestations at two times are two pieces of evidence, but two records claiming to be"),
        ("src/finding.rs", "occupied key is REFUSED, not staged. Without this, two records"),
        ("add --help", "Fix-of-fix is DERIVED, never recorded: a finding is one when its"),
        ("src/finding.rs", "There is no `--fix-of-fix` flag and there will not be one. A finding"),
        ("src/main.rs", "Write one attributed Review record (exactly one provenance ref required)"),
        ("src/finding.rs", "is not a finding severity — expected one of: blocker, should-fix, note"),
        ("add --help", "a finding is rendered as ONE row, so a separator appends a second row that"),
        ("src/finding.rs", "recorded none of their findings — `canon_ingest::artifact_adapter`'s"),
    ] {
        assert_eq!(hand_typed_count(legitimate), None, "{provenance}: the guard must spare {legitimate:?}");
    }
}

/// The third table, the one round 6 added: every hole
/// [`hand_typed_count`]'s doc admits to, as a row that MUST pass.
///
/// The tables above pin what the guard does. Nothing pinned what its
/// doc says it does NOT do, so those claims drifted — round 6 found
/// four of them false at once. A hole stated in prose cannot fail; a
/// hole stated as a row fails the moment a retune closes it or a rule
/// change quietly widens somewhere else.
///
/// Round 8 is the table's largest bill and its clearest justification:
/// retiring shapes 2 and 3 moved the whole of both shapes' reject rows
/// into this one. Every row below is a class the guard used to cover
/// and now does not, and none of them could have been given up
/// silently while this test exists.
///
/// Round 6's two counter-example assertions are gone from the foot of
/// this test because both flipped and both had somewhere better to go:
/// "findings, 3 of them" was caught by shape 2 and is now a row here,
/// and "120-134 findings" was a hole and is now a reject row.
///
/// Which direction a failure here points matters. A row that starts
/// getting CAUGHT is not automatically a bug — a hole may be closed on
/// purpose — but it does mean the doc now overstates the guard's
/// blindness, so the bullet goes and the row moves to the reject table.
/// A row that cannot be written honestly at all is the signal that the
/// hole was never real.
#[test]
fn the_documented_holes_are_the_holes_the_guard_actually_has() {
    for (bullet, hole) in [
        // Retired shape 2: a value that FOLLOWS its noun, in every
        // spelling rounds 4, 5 and 6 taught the guard to read.
        ("value after noun", "Findings: 3"),
        ("value after noun", "Findings: 3 total."),
        ("value after noun", "findings = 3"),
        ("value after noun", "findings — 3"),
        ("value after noun", "rounds: 11"),
        ("value after noun", "Findings : — 3"),
        ("value after noun", "Findings =3"),
        ("value after noun", "Findings :3"),
        ("value after noun", "| Findings | 3 |"),
        ("value after noun", "Records: 44"),
        ("value after noun", "Issues: 49"),
        ("value after noun", "findings, 3 of them"),
        ("value after noun", "findings, as of this round, 3"),
        // The same rule seen from the range side: a digit range is a
        // count only where it MODIFIES the noun, so the mirror image of
        // the caught "120-134 findings" is a hole.
        ("value after noun", "Findings: 120-134"),
        // Retired shape 3: no counting word, partitive or tally marker
        // is read at all. The first row is the one that costs — the
        // exact form that shipped wrong twice, in a wording
        // SHIPPED_VERBATIM does not carry, so nothing holds it.
        ("counting word", "the round count is 7"),
        ("counting word", "the finding count is 19"),
        ("counting word", "the count of findings is three"),
        ("counting word", "the record count is 44"),
        ("counting word", "the records total 44"),
        ("counting word", "fix-of-fix total: 3"),
        ("counting word", "the fix-of-fix count is two."),
        ("counting word", "the number of findings is three"),
        ("counting word", "the number of rounds is 11"),
        ("counting word", "Findings: 19 in all."),
        ("counting word", "findings, all told 12"),
        ("counting word", "19 in all"),
        ("counting word", "3 total"),
        ("counting word", "3 apiece"),
        ("counting word", "the finding number is 19"),
        // "there are two findings" — the spelled-small register.
        ("spelled small", "there are two findings"),
        // The index rule is unconditional, so a count wearing a
        // singular noun reads as an index.
        ("index", "Finding 44"),
        // The vocabulary is five nouns.
        ("vocabulary", "four of them defects"),
        ("vocabulary", "three blockers"),
        ("vocabulary", "two verdicts"),
        // The window is two words.
        ("window", "3 of the reported findings"),
        // Whitespace tokenization welds a label to its value.
        ("welding", "Findings:3"),
        ("welding", "rounds=11"),
    ] {
        assert_eq!(hand_typed_count(hole), None, "the {bullet:?} hole is no longer a hole: the guard now catches {hole:?}, so its doc bullet overstates what the guard misses");
    }
}

/// s43 round 3, finding 4: the branch above used to read
/// `stdout(&run_canon(...))` and apply only `!contains(...)`
/// assertions, so a help invocation that FAILED — printing nothing to
/// stdout — satisfied every one of them and the guard reported success
/// having inspected an empty string. This pins that the empty case is
/// genuinely vacuous, and that [`checked_help`] refuses it.
#[test]
fn an_empty_help_invocation_is_refused_rather_than_passing_vacuously() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let broken = run_canon(&["finding", "--no-such-flag"], dir.path());

    assert!(!broken.status.success(), "the premise: clap refuses an unknown flag");
    assert!(stdout(&broken).is_empty(), "the premise: a clap error prints nothing to stdout: {}", stdout(&broken));
    // Every negative assertion the old branch made passes on it, which
    // is precisely why the positive check has to come first.
    assert!(hand_typed_count(&stdout(&broken)).is_none());
    assert!(!stdout(&broken).contains("--fix-of-fix"));

    let refused = checked_help(&broken, PARENT_HELP_MARKER).expect_err("a failed --help must not be readable as help");
    assert!(refused.contains("exited"), "the refusal must name the exit status: {refused}");

    // And the ordinary invocation still reads, so this is not a blanket
    // refusal: the marker is present in real help.
    checked_help(&run_canon(&["finding", "--help"], dir.path()), PARENT_HELP_MARKER).unwrap();
}

/// Three findings on one change, two committed and one still staged:
/// (1,1) blocker raised open then closed `rejected`, (1,2) should-fix
/// left open, and a staged (2,1). Plus one committed finding on another
/// change, which no list of this change may show.
fn listed_repo() -> tempfile::TempDir {
    let dir = repo();
    let repo = dir.path();
    let raise = |change: &str, severity: &str, round: &str, seq: &str, reviewer: &str, summary: &str| {
        let out = run_canon(
            &["finding", "add", "--change", change, "--severity", severity, "--round", round, "--seq", seq, "--actor", reviewer, "--summary", summary, "--introduced-unknown"],
            repo,
        );
        assert!(out.status.success(), "{}", stderr(&out));
    };
    raise("s43-findings-are-records", "blocker", "1", "1", "rev-a", "teardown races HMR");
    raise("s43-findings-are-records", "should-fix", "1", "2", "rev-b", "the count was typed by hand");
    raise("other-change", "note", "1", "1", "rev-c", "elsewhere");
    raise("closed-only", "blocker", "1", "1", "rev-d", "fixed long ago");
    assert!(run_canon(&["gate", "promote"], repo).status.success());
    let closed = run_canon(&["finding", "close", "s43-findings-are-records", "--round", "1", "--seq", "1", "--disposition", "rejected", "--reason", "by design"], repo);
    assert!(closed.status.success(), "{}", stderr(&closed));
    let closed = run_canon(&["finding", "close", "closed-only", "--round", "1", "--seq", "1", "--disposition", "deferred", "--reason", "by design"], repo);
    assert!(closed.status.success(), "{}", stderr(&closed));
    assert!(run_canon(&["gate", "promote"], repo).status.success());
    assert!(add(repo, &["--round", "2", "--seq", "1", "--actor", "rev-a", "--summary", "still staged"]).status.success());
    dir
}

/// finding.list.01: every committed finding of the change, one row each
/// at its latest version, ordered by round and seq; a staged one is
/// named on stderr, not listed.
#[test]
fn finding_list_prints_each_committed_finding_at_its_latest_version() {
    let dir = listed_repo();
    let out = run_canon(&["finding", "list", "--change", "s43-findings-are-records"], dir.path());
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(
        stdout(&out),
        "round  seq  severity    disposition  reviewer  summary\n\
         1      1    blocker     rejected     rev-a     teardown races HMR [reason: by design]\n\
         1      2    should-fix  open         rev-b     the count was typed by hand\n"
    );
    assert_eq!(
        stderr(&out),
        "canon finding list: 1 staged finding(s) on change `s43-findings-are-records` are not listed until committed — run `canon gate promote`\n"
    );
}

/// finding.list.02: `--open` keeps only the findings whose CURRENT
/// disposition is open — a closed one stays out though it was raised open.
#[test]
fn finding_list_open_keeps_only_currently_open_findings() {
    let dir = listed_repo();
    let out = run_canon(&["finding", "list", "--change", "s43-findings-are-records", "--open"], dir.path());
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    assert_eq!(stdout(&out), "round  seq  severity    disposition  reviewer  summary\n1      2    should-fix  open         rev-b     the count was typed by hand\n");

    let none = run_canon(&["finding", "list", "--change", "other-change", "--open"], dir.path());
    assert_eq!(none.status.code(), Some(0));
    assert_eq!(stdout(&none), "round  seq  severity  disposition  reviewer  summary\n1      1    note      open         rev-c     elsewhere\n");
    let closed = run_canon(&["finding", "list", "--change", "closed-only", "--open"], dir.path());
    assert_eq!(closed.status.code(), Some(0), "{}", stderr(&closed));
    assert_eq!(stdout(&closed), "canon finding list: no committed open findings on change `closed-only`\n", "a change whose findings are all closed has none open");
    let all = run_canon(&["finding", "list", "--change", "closed-only"], dir.path());
    assert!(stdout(&all).contains("deferred") && stdout(&all).contains("fixed long ago"), "without --open the closed finding lists: {}", stdout(&all));
    let empty = run_canon(&["finding", "list", "--change", "no-findings-here", "--open"], dir.path());
    assert_eq!(empty.status.code(), Some(0));
    assert_eq!(stdout(&empty), "canon finding list: no committed open findings on change `no-findings-here`\n");
}

/// finding.list.03: `--json` carries the same rows, every field typed.
#[test]
fn finding_list_json_carries_the_same_rows() {
    let dir = listed_repo();
    let out = run_canon(&["finding", "list", "--change", "s43-findings-are-records", "--json"], dir.path());
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let json: serde_json::Value = serde_json::from_str(&stdout(&out)).expect("stdout is one JSON document");
    assert_eq!(json["ok"], true, "{json}");
    assert_eq!(json["meta"]["returned"], 2);
    assert_eq!(json["meta"]["total"], 2);
    assert_eq!(json["meta"]["hasMore"], false);
    assert!(json["meta"]["nextCursor"].is_null());
    assert_eq!(
        json["data"],
        serde_json::json!({
            "changeId": "s43-findings-are-records",
            "findings": [
                {"round": 1, "seq": 1, "severity": "blocker", "disposition": "rejected", "reviewer": "rev-a", "summary": "teardown races HMR", "resolutionSha": null, "fileRef": null, "reason": "by design"},
                {"round": 1, "seq": 2, "severity": "should-fix", "disposition": "open", "reviewer": "rev-b", "summary": "the count was typed by hand", "resolutionSha": null, "fileRef": null, "reason": null},
            ]
        })
    );
    let open = run_canon(&["finding", "list", "--change", "s43-findings-are-records", "--open", "--json"], dir.path());
    let json: serde_json::Value = serde_json::from_str(&stdout(&open)).unwrap();
    assert_eq!(json["data"]["findings"].as_array().unwrap().len(), 1, "{json}");
    assert_eq!(json["data"]["findings"][0]["seq"], 2, "{json}");
}

/// finding.list.04: a change id outside the grammar is refused by clap,
/// exit 2, and nothing is printed to stdout.
#[test]
fn finding_list_refuses_a_malformed_change_id() {
    let dir = repo();
    let out = run_canon(&["finding", "list", "--change", "Not A Change"], dir.path());
    assert_eq!(out.status.code(), Some(2), "{}", stderr(&out));
    assert!(stdout(&out).is_empty(), "{}", stdout(&out));
    assert!(stderr(&out).contains("--change"), "{}", stderr(&out));
}

const CHANGE: &str = "s43-findings-are-records";

/// Raise round 1 seq 1 (open) and commit it.
fn committed_open(repo: &Path) {
    let added = add(repo, &["--round", "1", "--seq", "1", "--actor", "r", "--summary", "needs a guard"]);
    assert!(added.status.success(), "stderr: {}", stderr(&added));
    assert!(run_canon(&["gate", "promote"], repo).status.success());
}

fn close(repo: &Path, extra: &[&str]) -> Output {
    let mut args = vec!["finding", "close", CHANGE, "--round", "1", "--seq", "1"];
    args.extend_from_slice(extra);
    run_canon(&args, repo)
}

fn error_json(out: &Output) -> serde_json::Value {
    serde_json::from_str::<serde_json::Value>(&stdout(out)).unwrap_or_else(|e| panic!("not JSON ({e}): {}{}", stdout(out), stderr(out)))["error"].clone()
}

/// finding.close.82: rejected and deferred owe a reason; the refusal is
/// exit 2 with a command-shaped hint, and nothing is staged.
#[test]
fn finding_close_82_rejected_or_deferred_without_a_reason_is_refused() {
    let dir = repo();
    let repo = dir.path();
    committed_open(repo);
    for disposition in ["rejected", "deferred"] {
        let out = close(repo, &["--disposition", disposition, "--json"]);
        assert_eq!(out.status.code(), Some(2), "{disposition}: {}{}", stdout(&out), stderr(&out));
        let e = error_json(&out);
        assert_eq!(e["code"], "FINDING_REASON_REQUIRED", "{e}");
        assert_eq!(e["field"], "--reason", "{e}");
        let hint = e["hint"].as_str().unwrap();
        assert!(hint.contains("canon finding close") && hint.contains(&format!("--disposition {disposition}")) && hint.contains("--reason"), "{hint}");
        assert!(staged_findings(repo).is_empty(), "a refused close stages nothing");
    }
    assert_eq!(committed_findings(repo).len(), 1, "the ledger is untouched");
}

/// finding.close.83: the reason lands on the closing
/// version only and `finding list` shows it in the table and in JSON.
#[test]
fn finding_close_83_a_reason_is_recorded_on_the_closing_version() {
    let dir = repo();
    let repo = dir.path();
    committed_open(repo);
    let out = close(repo, &["--disposition", "deferred", "--reason", "  next release  "]);
    assert!(out.status.success(), "{}", stderr(&out));
    let staged = body_at(&staged_findings(repo)[0]);
    assert_eq!(staged["reason"], "next release", "stored trimmed: {staged}");
    assert!(run_canon(&["gate", "promote"], repo).status.success());

    let originals: Vec<_> = committed_findings(repo).iter().map(|p| body_at(p)).filter(|b| b["disposition"] == "open").collect();
    assert!(originals.len() == 1 && originals[0].get("reason").is_none(), "the raised version keeps no reason: {originals:?}");

    let table = run_canon(&["finding", "list", "--change", CHANGE], repo);
    assert!(stdout(&table).contains("deferred") && stdout(&table).contains("needs a guard [reason: next release]"), "{}", stdout(&table));
    let listed: serde_json::Value = serde_json::from_str(&stdout(&run_canon(&["finding", "list", "--change", CHANGE, "--json"], repo))).unwrap();
    assert_eq!(listed["data"]["findings"][0]["reason"], "next release", "{listed}");
    let kept: serde_json::Value =
        serde_json::from_str(&stdout(&run_canon(&["finding", "list", "--change", CHANGE, "--json", "--fields", "disposition,reason"], repo))).unwrap();
    assert_eq!(kept["data"]["findings"][0]["reason"], "next release", "{kept}");
}

/// finding.list.05: the table appends the reason to the summary and the
/// JSON row carries it (null where there is none).
#[test]
fn finding_list_05_shows_the_reason_in_the_table_and_in_json() {
    let (dir, head) = git_repo();
    let repo = dir.path();
    for seq in ["1", "2"] {
        assert!(add(repo, &["--round", "1", "--seq", seq, "--actor", "r", "--summary", &format!("finding {seq}")]).status.success());
    }
    assert!(run_canon(&["gate", "promote"], repo).status.success());
    assert!(close(repo, &["--disposition", "rejected", "--reason", "works as designed"]).status.success());
    let fixed = run_canon(&["finding", "close", CHANGE, "--round", "1", "--seq", "2", "--disposition", "fixed", "--resolution-rev", &head], repo);
    assert!(fixed.status.success(), "{}", stderr(&fixed));
    assert!(run_canon(&["gate", "promote"], repo).status.success());

    let table = stdout(&run_canon(&["finding", "list", "--change", CHANGE], repo));
    assert!(table.contains("finding 1 [reason: works as designed]\n") && table.contains("finding 2\n"), "{table}");
    let json: serde_json::Value = serde_json::from_str(&stdout(&run_canon(&["finding", "list", "--change", CHANGE, "--json"], repo))).unwrap();
    assert_eq!(json["data"]["findings"][0]["reason"], "works as designed");
    assert!(json["data"]["findings"][1]["reason"].is_null(), "{json}");
}

/// finding.close.84: fixed closes with or without a reason.
#[test]
fn finding_close_84_fixed_takes_a_reason_but_does_not_need_one() {
    let (dir, head) = git_repo();
    let repo = dir.path();
    for seq in ["1", "2"] {
        assert!(add(repo, &["--round", "1", "--seq", seq, "--actor", "r", "--summary", "s"]).status.success());
    }
    assert!(run_canon(&["gate", "promote"], repo).status.success());
    let bare = close(repo, &["--disposition", "fixed", "--resolution-rev", &head]);
    assert!(bare.status.success(), "{}", stderr(&bare));
    let with = run_canon(&["finding", "close", CHANGE, "--round", "1", "--seq", "2", "--disposition", "fixed", "--resolution-rev", &head, "--reason", "guard added"], repo);
    assert!(with.status.success(), "{}", stderr(&with));
    let mut staged: Vec<_> = staged_findings(repo).iter().map(|p| body_at(p)).collect();
    staged.sort_by_key(|b| b["seq"].as_u64());
    assert!(staged[0].get("reason").is_none(), "{}", staged[0]);
    assert_eq!(staged[1]["reason"], "guard added");
}

/// finding.close.85: a blank or multi-line reason is the same refusal,
/// for every disposition including fixed.
#[test]
fn finding_close_85_a_blank_or_multi_line_reason_is_refused() {
    let (dir, head) = git_repo();
    let repo = dir.path();
    committed_open(repo);
    for reason in ["", "   ", "line one\nline two"] {
        for disposition in [&["--disposition", "rejected"][..], &["--disposition", "fixed", "--resolution-rev", head.as_str()][..]] {
            let mut args = disposition.to_vec();
            args.extend(["--reason", reason, "--json"]);
            let out = close(repo, &args);
            assert_eq!(out.status.code(), Some(2), "{reason:?} {disposition:?}: {}{}", stdout(&out), stderr(&out));
            assert_eq!(error_json(&out)["code"], "FINDING_REASON_REQUIRED");
            assert!(staged_findings(repo).is_empty());
        }
    }
}

/// finding.close.02: a close changes the disposition, the resolution sha, the
/// optional reason, and the actor and time of the transition; every other
/// field of the body is the previous version's.
#[test]
fn finding_close_02_a_close_adds_only_the_disposition_resolution_sha_reason_actor_and_time() {
    let (dir, head) = git_repo();
    let repo = dir.path();
    let added = add(repo, &["--round", "1", "--seq", "1", "--actor", "r", "--summary", "needs a guard", "--file-ref", "src/a.rs:1-2", "--introduced-rev", &head, "--rev", &head]);
    assert!(added.status.success(), "{}", stderr(&added));
    assert!(run_canon(&["gate", "promote"], repo).status.success());
    let previous = body_at(&committed_findings(repo).remove(0));

    let closed = close(repo, &["--disposition", "rejected", "--reason", "works as designed", "--actor", "closer"]);
    assert!(closed.status.success(), "{}{}", stdout(&closed), stderr(&closed));
    assert!(run_canon(&["gate", "promote"], repo).status.success());
    let next = committed_findings(repo).iter().map(|p| body_at(p)).find(|b| b["disposition"] == "rejected").expect("the close");
    assert_eq!(next["reason"], "works as designed", "{next}");
    assert_eq!(next["actor"]["agent_id"], "closer", "{next}");

    let strip = |mut body: serde_json::Value| {
        let object = body.as_object_mut().unwrap();
        for key in ["disposition", "resolution_sha", "reason", "actor", "at"] {
            object.remove(key);
        }
        body
    };
    assert_eq!(strip(next), strip(previous), "a close adds nothing but those fields");
}

/// finding.close.86: under the default manual promote, a staged close
/// carrying a reason promotes; changing only the reason is still refused.
#[test]
fn finding_close_86_a_staged_close_with_a_reason_promotes_and_a_bare_reason_change_does_not() {
    let dir = repo();
    let repo = dir.path();
    std::fs::write(repo.join("canon.yaml"), "routing:\n  finding: local\nwrite:\n  promote: manual\n").unwrap();
    committed_open(repo);
    let out = close(repo, &["--disposition", "rejected", "--reason", "works as designed"]);
    assert!(out.status.success() && stdout(&out).contains("canon gate promote"), "{}{}", stdout(&out), stderr(&out));
    assert_eq!(staged_findings(repo).len(), 1);
    let promoted = run_canon(&["gate", "promote"], repo);
    assert!(promoted.status.success(), "{}{}", stdout(&promoted), stderr(&promoted));
    assert!(staged_findings(repo).is_empty());
    assert_eq!(committed_findings(repo).len(), 2);

    let promoted_close = committed_findings(repo).iter().map(|p| body_at(p)).find(|b| b["disposition"] == "rejected").unwrap();
    assert_eq!(promoted_close["reason"], "works as designed", "the promoted close carries the given reason: {promoted_close}");

    // The same close with only its reason changed, staged under the file
    // name the layout requires (the digest of the original close's name).
    let promoted_name = committed_findings(repo)
        .iter()
        .find(|p| body_at(p)["disposition"] == "rejected")
        .map(|p| p.file_name().unwrap().to_string_lossy().into_owned())
        .unwrap();
    let mut tampered = promoted_close.clone();
    tampered["reason"] = "a different story".into();
    tampered["at"] = "2030-01-01T00:00:00Z".into();
    let staging = repo.join(".canon/ledger/_staging/kind=finding");
    std::fs::create_dir_all(&staging).unwrap();
    std::fs::write(staging.join(&promoted_name), serde_json::to_string(&tampered).unwrap()).unwrap();
    // The layout names the file by the body's digest; the first refusal says
    // which name the body needs, so the second one is past the layout rule.
    let layout = run_canon(&["gate", "promote"], repo);
    let layout_text = format!("{}{}", stdout(&layout), stderr(&layout));
    let wanted = layout_text.split("expected `kind=finding/").nth(1).and_then(|r| r.split('`').next()).expect("the layout refusal names the file").to_string();
    std::fs::rename(staging.join(&promoted_name), staging.join(&wanted)).unwrap();
    let refused = run_canon(&["gate", "promote"], repo);
    let text = format!("{}{}", stdout(&refused), stderr(&refused));
    assert!(!refused.status.success(), "a reason change outside a close must be refused: {text}");
    assert!(!text.contains("expected kind=finding"), "the refusal must not be the file-layout rule: {text}");
    assert!(text.contains("already-committed record with a DIFFERENT body"), "the refusal is the identity check: {text}");
    assert_eq!(committed_findings(repo).len(), 2, "nothing new committed");
    assert!(!staged_findings(repo).is_empty(), "the refused body stays staged");
}

/// finding.close.87: a record written before reasons existed reads,
/// lists and closes, and its committed bytes never change.
#[test]
fn finding_close_87_an_old_record_without_reason_is_untouched_by_reading_listing_and_closing() {
    let (dir, head) = git_repo();
    let repo = dir.path();
    committed_open(repo);
    let path = committed_findings(repo).remove(0);
    let before = std::fs::read(&path).unwrap();
    assert!(!String::from_utf8_lossy(&before).contains("reason"));

    let listed: serde_json::Value = serde_json::from_str(&stdout(&run_canon(&["finding", "list", "--change", CHANGE, "--json"], repo))).unwrap();
    assert!(listed["data"]["findings"][0]["reason"].is_null(), "{listed}");
    let table = stdout(&run_canon(&["finding", "list", "--change", CHANGE], repo));
    assert!(!table.contains("[reason"), "{table}");

    assert!(close(repo, &["--disposition", "fixed", "--resolution-rev", &head]).status.success());
    assert!(run_canon(&["gate", "promote"], repo).status.success());
    assert_eq!(std::fs::read(&path).unwrap_or_else(|_| std::fs::read(repo.join(".canon/archive").join(path.strip_prefix(repo.join(".canon/ledger")).unwrap())).unwrap()), before, "the untouched record's bytes are unchanged");
}
