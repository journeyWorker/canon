//! Integration tests for `canon finding add` (s43
//! `findings-are-records`, task group 2), invoking the actually-built
//! `canon` binary (`env!("CARGO_BIN_EXE_canon")`) — the same discipline
//! `tests/evidence.rs` and `tests/gate.rs` state for themselves:
//! subprocess-level behavior, exit codes, and real-filesystem side
//! effects need the real binary boundary.
//! `crates/canon-cli/src/finding.rs`'s own unit tests already cover the
//! library decisions in-process; what only THIS boundary can prove is
//! that the clap surface exists and parses, and that clap's own
//! `value_parser` refusals land where the module doc says they do.
//!
//! The headline test is
//! [`the_two_command_loop_commits_a_finding_nobody_hand_wrote`]:
//! `canon finding add` → `canon gate promote` → a committed record.
//! Before s43 that sequence was unrunnable, and a review round's
//! findings evaporated with its transcript.

#[path = "support/hermetic.rs"]
mod hermetic;
use std::path::{Path, PathBuf};
use std::process::{Command, Output};

fn run_canon(args: &[&str], cwd: &Path) -> Output {
    hermetic::canon().args(args).current_dir(cwd).output().expect("spawn canon binary")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

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
    assert!(stdout(&added).contains("`canon gate promote`"), "{}", stdout(&added));
    assert!(run_canon(&["gate", "promote"], repo).status.success());

    let closed = run_canon(
        &["finding", "close", "s43-findings-are-records", "--round", "1", "--seq", "1", "--disposition", "rejected", "--reason", "by design", "--session", "sess-close"],
        repo,
    );
    assert!(closed.status.success(), "stderr: {}", stderr(&closed));
    assert!(stdout(&closed).contains("`canon gate promote`"), "{}", stdout(&closed));
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
    assert!(stderr(&out).contains("--session"), "{}", stderr(&out));
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
    assert!(stderr(&output).contains("--summary"), "{}", stderr(&output));
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
    assert!(stderr(&committed_clash).contains("s43-findings-are-records_2_1"), "{}", stderr(&committed_clash));
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
    assert!(json_of(&first)["data"].get("unchanged").is_none(), "the first add writes");

    let before = tree(repo);
    let staged_repeat = add(repo, &args);
    assert_eq!(staged_repeat.status.code(), Some(0), "{}", stderr(&staged_repeat));
    let repeat = json_of(&staged_repeat);
    assert_eq!(repeat["data"]["unchanged"], true, "{repeat}");
    assert!(repeat["meta"].get("unchanged").is_none(), "`unchanged` is a report member, not a flat `meta` one: {repeat}");
    assert_eq!(tree(repo), before, "a repeat of a staged finding writes nothing");

    assert!(run_canon(&["gate", "promote", "--repo", "."], repo).status.success());
    let before = tree(repo);
    let committed_repeat = add(repo, &args);
    assert_eq!(committed_repeat.status.code(), Some(0), "{}", stderr(&committed_repeat));
    assert_eq!(json_of(&committed_repeat)["data"]["unchanged"], true);
    assert_eq!(tree(repo), before, "a repeat of a committed finding writes nothing");

    let other = add(repo, &["--round", "4", "--seq", "1", "--actor", "r", "--summary", "another finding", "--json"]);
    assert_eq!(other.status.code(), Some(2), "{}", stdout(&other));
}

/// Closing a finding to the disposition it already has (`fixed` and `rejected` alike) writes nothing and is `data.unchanged`.
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
        assert!(json_of(&first)["data"].get("unchanged").is_none(), "{disposition}: the first close writes");
        assert!(run_canon(&["gate", "promote", "--repo", "."], repo).status.success());

        let before = tree(repo);
        let again = run_canon(&close, repo);
        assert_eq!(again.status.code(), Some(0), "{disposition}: {}", stderr(&again));
        assert_eq!(json_of(&again)["data"]["unchanged"], true, "{disposition}");
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
    assert!(empty.contains("(evidence, finding"), "{empty}");
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
        "round  seq  severity    disposition  reviewer  closed by  summary\n\
         1      1    blocker     rejected     rev-a     canon      teardown races HMR [reason: by design]\n\
         1      2    should-fix  open         rev-b     -          the count was typed by hand\n"
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
    let table = stdout(&out);
    assert!(table.contains("rev-b") && table.contains("the count was typed by hand"), "{table}");
    assert!(!table.contains("teardown races HMR"), "the closed finding stays out: {table}");

    let none = run_canon(&["finding", "list", "--change", "other-change", "--open"], dir.path());
    assert_eq!(none.status.code(), Some(0));
    let table = stdout(&none);
    assert!(table.contains("rev-c") && table.contains("elsewhere"), "{table}");
    let closed = run_canon(&["finding", "list", "--change", "closed-only", "--open"], dir.path());
    assert_eq!(closed.status.code(), Some(0), "{}", stderr(&closed));
    assert!(stdout(&closed).contains("`closed-only`") && !stdout(&closed).contains("fixed long ago"), "a change whose findings are all closed has none open");
    let all = run_canon(&["finding", "list", "--change", "closed-only"], dir.path());
    assert!(stdout(&all).contains("deferred") && stdout(&all).contains("fixed long ago"), "without --open the closed finding lists: {}", stdout(&all));
    let empty = run_canon(&["finding", "list", "--change", "no-findings-here", "--open"], dir.path());
    assert_eq!(empty.status.code(), Some(0));
    assert!(stdout(&empty).contains("`no-findings-here`"));
}

/// finding.list.03: `--json` carries the same rows, every field typed.
#[test]
fn finding_list_json_carries_the_same_rows() {
    let dir = listed_repo();
    let out = run_canon(&["finding", "list", "--change", "s43-findings-are-records", "--json"], dir.path());
    assert_eq!(out.status.code(), Some(0), "{}", stderr(&out));
    let json: serde_json::Value = serde_json::from_str(&stdout(&out)).expect("stdout is one JSON document");
    assert_eq!(json["ok"], true, "{json}");
    assert_eq!(json["meta"]["page"], serde_json::json!({"returned": 2, "total": 2}), "a last page has no `nextCursor`: {json}");
    for flat in ["returned", "total", "hasMore", "nextCursor"] {
        assert!(json["meta"].get(flat).is_none(), "`{flat}` is not a flat `meta` member any more: {json}");
    }
    assert!(json["meta"].get("next").is_none(), "no next page, no `meta.next`: {json}");
    assert_eq!(
        json["data"],
        serde_json::json!({
            "changeId": "s43-findings-are-records",
            "findings": [
                {"round": 1, "seq": 1, "severity": "blocker", "disposition": "rejected", "reviewer": "rev-a", "summary": "teardown races HMR", "resolutionSha": null, "fileRef": null, "reason": "by design", "closedBy": "canon", "closedRole": null},
                {"round": 1, "seq": 2, "severity": "should-fix", "disposition": "open", "reviewer": "rev-b", "summary": "the count was typed by hand", "resolutionSha": null, "fileRef": null, "reason": null, "closedBy": null, "closedRole": null},
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
    assert!(stdout(&table).contains("deferred") && stdout(&table).contains("next release"), "{}", stdout(&table));
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
    assert!(table.contains("works as designed") && table.matches("works as designed").count() == 1, "{table}");
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

/// finding.close.05 (0.37, `canon-0-37-option-values`): a fix that did not hold is not reopened; `--disposition open` is refused with exit 2 naming the three
/// dispositions that close a finding, and nothing is staged or committed. The defect that came back is a new finding.
#[test]
fn finding_close_05_a_fix_that_did_not_hold_is_a_new_finding_and_closing_to_open_is_refused() {
    let (dir, head) = git_repo();
    let repo = dir.path();
    let added = add(repo, &["--round", "1", "--seq", "1", "--actor", "r", "--summary", "needs a guard", "--introduced-rev", &head, "--rev", &head]);
    assert!(added.status.success(), "{}", stderr(&added));
    assert!(run_canon(&["gate", "promote"], repo).status.success());
    let fixed = close(repo, &["--disposition", "fixed", "--resolution-rev", head.as_str()]);
    assert!(fixed.status.success(), "{}{}", stdout(&fixed), stderr(&fixed));
    assert!(run_canon(&["gate", "promote"], repo).status.success());
    let committed = committed_findings(repo).len();

    let out = close(repo, &["--disposition", "open", "--json"]);
    assert_eq!(out.status.code(), Some(2), "{}{}", stdout(&out), stderr(&out));
    let error = error_json(&out);
    assert_eq!((error["code"].as_str(), error["field"].as_str()), (Some("USAGE"), Some("--disposition")), "{error}");
    assert_eq!(error["expected"], serde_json::json!(["fixed", "rejected", "deferred"]), "{error}");
    assert!(staged_findings(repo).is_empty(), "nothing is staged");
    assert_eq!(committed_findings(repo).len(), committed, "nothing is committed");

    // The defect that came back is the next finding of the round, raised open.
    let again = add(repo, &["--round", "1", "--seq", "2", "--actor", "r", "--summary", "the guard is gone again", "--introduced-rev", &head, "--rev", &head]);
    assert!(again.status.success(), "{}", stderr(&again));
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
    assert!(text.contains(&format!("{CHANGE}_1_1")), "the refusal names the natural key: {text}");
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
