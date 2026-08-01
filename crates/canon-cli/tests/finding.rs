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
//! Before s43 that sequence was unrunnable, and eleven review rounds'
//! findings evaporated with their transcripts.

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
    files(repo.join(".canon/ledger/kind=finding"))
}

fn body_at(path: &Path) -> serde_json::Value {
    serde_json::from_str(&std::fs::read_to_string(path).expect("reading a record")).expect("valid JSON")
}

/// The base flags every well-formed add needs, so a test naming a
/// refusal shows only the flag it is about.
const BASE: [&str; 8] = ["finding", "add", "--change-id", "s43-findings-are-records", "--severity", "blocker", "--repo", "."];

fn add(repo: &Path, extra: &[&str]) -> Output {
    let mut args: Vec<&str> = BASE.to_vec();
    args.extend_from_slice(extra);
    run_canon(&args, repo)
}

/// s43 task 2.1 end to end, through the real binary: a review finding
/// becomes a committed record, and every field round-trips as asked
/// for. Each step is asserted for its EFFECT — staging populated,
/// staging drained, committed body correct — so a step that silently
/// no-ops cannot be masked by the next one succeeding.
///
/// The `--reviewed-sha`-less case is deliberate and load-bearing: round
/// 8 of v0.4.0 reviewed an uncommitted working tree, so a kind that
/// required a reviewed commit could not have recorded a third of the
/// backfill this change exists to produce.
#[test]
fn the_two_command_loop_commits_a_finding_nobody_hand_wrote() {
    let dir = repo();
    let repo = dir.path();

    let added = add(
        repo,
        &["--round", "8", "--seq", "3", "--disposition", "open", "--reviewer", "review-voice", "--summary", "round 8 reviewed an uncommitted worktree"],
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

    // An absent optional is ABSENT, never an explicit `null` — a `null`
    // fails `Finding`'s own deserialize, so a record written that way
    // would be unreadable by every consumer.
    for absent in ["reviewed_sha", "resolution_sha", "introduced_by", "file_ref"] {
        assert!(body.get(absent).is_none(), "`{absent}` must be omitted, never null: {body}");
    }

    // Said on the success path, not only in `--help`: an unsourced
    // finding is why the derived count is a floor, and this is the
    // moment the author still knows whether it was sourced.
    assert!(stdout(&added).contains("UNSOURCED"), "an add with no --introduced-by must say so: {}", stdout(&added));
}

/// Every optional flag, set, surviving promotion unmodified — the
/// committed body of a `Finding` is the staged body byte for byte
/// (`canon_gate::StagedAssignment::Nothing`), so this also pins that
/// promotion stamps nothing onto it.
#[test]
fn a_fixed_and_sourced_finding_round_trips_every_optional_through_promotion() {
    let dir = repo();
    let repo = dir.path();
    let resolution = "a".repeat(40);
    let introduced = "b".repeat(40);
    let reviewed = "c".repeat(40);

    let added = add(
        repo,
        &[
            "--round",
            "11",
            "--seq",
            "1",
            "--disposition",
            "fixed",
            "--resolution-sha",
            &resolution,
            "--introduced-by",
            &introduced,
            "--reviewed-sha",
            &reviewed,
            "--file-ref",
            "crates/canon-cli/src/finding.rs:120-134",
            "--reviewer",
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

/// Task 2.2, first half: `fixed` naming no closing commit. Graded `2`
/// (a fixable invocation), naming BOTH flags, and staging nothing.
#[test]
fn finding_add_refuses_fixed_with_no_resolution_sha() {
    let dir = repo();
    let repo = dir.path();

    let output = add(repo, &["--round", "1", "--seq", "1", "--disposition", "fixed", "--reviewer", "r", "--summary", "s"]);
    assert_eq!(output.status.code(), Some(2), "stdout: {}", stdout(&output));
    let err = stderr(&output);
    assert!(err.contains("--disposition fixed"), "the refusal must name the disposition: {err}");
    assert!(err.contains("--resolution-sha"), "and the missing flag: {err}");
    assert!(staged_findings(repo).is_empty(), "a refused add must stage nothing");
}

/// Task 2.2, second half — over EVERY non-`fixed` disposition, not just
/// `open`. `Finding::rejected`/`deferred` CLEAR `resolution_sha`, so a
/// CLI that only refused the `open` case would silently drop a sha the
/// caller supplied on the other two.
#[test]
fn finding_add_refuses_a_resolution_sha_on_every_non_fixed_disposition() {
    let resolution = "a".repeat(40);
    for disposition in ["open", "rejected", "deferred"] {
        let dir = repo();
        let repo = dir.path();

        let output = add(repo, &["--round", "1", "--seq", "1", "--disposition", disposition, "--resolution-sha", &resolution, "--reviewer", "r", "--summary", "s"]);
        assert_eq!(output.status.code(), Some(2), "{disposition} accepted a resolution sha; stdout: {}", stdout(&output));
        let err = stderr(&output);
        assert!(err.contains("--resolution-sha"), "the refusal must name the flag: {err}");
        assert!(err.contains(disposition), "and the disposition it conflicts with: {err}");
        assert!(staged_findings(repo).is_empty(), "{disposition}: a refused add must stage nothing");
    }
}

/// Task 2.2, third: a malformed SHA, on each of the three sha-shaped
/// flags. Refused by clap's `value_parser` — so it is impossible for
/// one to reach `run_add` at all — naming the flag, the value, and the
/// grammar it failed.
#[test]
fn finding_add_refuses_a_malformed_sha_naming_the_flag_and_the_grammar() {
    for flag in ["--reviewed-sha", "--introduced-by"] {
        let dir = repo();
        let repo = dir.path();

        let output = add(repo, &["--round", "1", "--seq", "1", "--reviewer", "r", "--summary", "s", flag, "deadbeef"]);
        assert_eq!(output.status.code(), Some(2), "{flag}; stdout: {}", stdout(&output));
        let err = stderr(&output);
        assert!(err.contains(flag), "the refusal must name the flag: {err}");
        assert!(err.contains("deadbeef"), "and the offending value: {err}");
        assert!(err.contains("40 lowercase hex"), "and the grammar it failed: {err}");
        assert!(staged_findings(repo).is_empty(), "a refused add must stage nothing");
    }

    // `--resolution-sha` needs a coherent disposition to get past the
    // pairing check, so its malformed-value refusal is exercised on the
    // one invocation that would otherwise succeed.
    let dir = repo();
    let repo = dir.path();
    let output = add(repo, &["--round", "1", "--seq", "1", "--disposition", "fixed", "--resolution-sha", "NOTASHA", "--reviewer", "r", "--summary", "s"]);
    assert_eq!(output.status.code(), Some(2), "stdout: {}", stdout(&output));
    assert!(stderr(&output).contains("--resolution-sha"), "{}", stderr(&output));
    assert!(staged_findings(repo).is_empty());
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
        ("--reviewer", "r\n| forged | row |", "s", Vec::new()),
        ("--file-ref", "r", "s", vec!["--file-ref", "src/x.rs:1-2\n| forged | row |"]),
        ("--actor-id", "r", "s", vec!["--actor-id", "canon\n| forged | row |"]),
    ] {
        let dir = repo();
        let repo = dir.path();

        let mut flags = vec!["--round", "1", "--seq", "1", "--reviewer", reviewer, "--summary", summary];
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

    let output = add(repo, &["--round", "1", "--seq", "1", "--reviewer", "r", "--summary", "ok\u{2028}| forged | row |"]);
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

    assert!(add(repo, &["--round", "2", "--seq", "1", "--reviewer", "r", "--summary", "the first finding"]).status.success());

    let staged_clash = add(repo, &["--round", "2", "--seq", "1", "--reviewer", "r", "--summary", "a completely different finding"]);
    assert_eq!(staged_clash.status.code(), Some(2), "stdout: {}", stdout(&staged_clash));
    assert!(stderr(&staged_clash).contains("s43-findings-are-records__0002__0001"), "the refusal must name the key: {}", stderr(&staged_clash));
    assert_eq!(staged_findings(repo).len(), 1, "the clashing add must stage nothing");

    // And after promotion, when the occupant is COMMITTED rather than
    // staged — the tier the check would be easiest to forget.
    assert!(run_canon(&["gate", "promote", "--repo", "."], repo).status.success());
    let committed_clash = add(repo, &["--round", "2", "--seq", "1", "--reviewer", "r", "--summary", "a third distinct finding"]);
    assert_eq!(committed_clash.status.code(), Some(2), "stdout: {}", stdout(&committed_clash));
    assert!(stderr(&committed_clash).contains("committed"), "the refusal must say which tier holds the occupant: {}", stderr(&committed_clash));
    assert!(staged_findings(repo).is_empty());
    assert_eq!(committed_findings(repo).len(), 1);
}

/// A neighbouring `seq` in the same round is a different finding: the
/// collision check must not degrade into a per-round refusal.
#[test]
fn finding_add_accepts_the_next_seq_in_the_same_round() {
    let dir = repo();
    let repo = dir.path();

    assert!(add(repo, &["--round", "3", "--seq", "1", "--reviewer", "r", "--summary", "first"]).status.success());
    assert!(add(repo, &["--round", "3", "--seq", "2", "--reviewer", "r", "--summary", "second"]).status.success());
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
    assert!(empty_out.contains("evidence_record"), "{empty_out}");

    assert!(add(repo, &["--round", "4", "--seq", "1", "--reviewer", "r", "--summary", "first"]).status.success());
    assert!(add(repo, &["--round", "4", "--seq", "2", "--reviewer", "r", "--summary", "second"]).status.success());

    let promoted = run_canon(&["gate", "promote", "--repo", "."], repo);
    assert!(promoted.status.success(), "stderr: {}", stderr(&promoted));
    let out = stdout(&promoted);
    assert!(out.contains("finding=2"), "the tally must be readable per kind: {out}");
    assert!(out.contains("evidence_record=0"), "including the kinds that drained nothing: {out}");
}

/// Task 2.3. Asserted at the binary boundary because this is the text
/// an operator or an agent actually reads before trusting a finding —
/// and because the command exists on account of an unsupported claim,
/// its own help overstating what it establishes would be the same
/// mistake in the same release.
#[test]
fn finding_add_help_states_it_is_an_observation_and_names_what_canon_skips() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let output = run_canon(&["finding", "add", "--help"], dir.path());
    assert!(output.status.success(), "--help must exit 0: {}", stderr(&output));
    let help = stdout(&output);

    assert!(help.contains("RECORDED OBSERVATION, NOT PROOF"), "{help}");
    assert!(help.contains("does not verify that the defect"), "{help}");
    assert!(help.contains("can record its own review round"), "{help}");

    // What it does NOT establish, each named.
    for skipped in ["never resolved, never read", "never verified, and never guessed", "nothing here is gated"] {
        assert!(help.contains(skipped), "the help must name `{skipped}`: {help}");
    }

    // What it IS good for, so the help is not merely a disclaimer.
    assert!(help.contains("attribution"), "{help}");
    assert!(help.contains("stops being typed from memory"), "{help}");

    // The `--introduced-by` guidance, which is the decision a user makes
    // at this command and nowhere else.
    assert!(help.contains("must be SOURCED"), "{help}");
    assert!(help.contains("Leaving it UNSET is the correct record"), "{help}");
    assert!(help.contains("FLOOR"), "the help must state that a derived count is a floor: {help}");

    // Fix-of-fix stays derived: no flag, and the help says why.
    assert!(help.contains("Fix-of-fix is DERIVED, never recorded"), "{help}");
    assert!(!help.contains("--fix-of-fix"), "there must be no fix-of-fix flag: {help}");

    // And it never claims proof of anything.
    assert!(!help.contains("proof of"), "the help must never claim proof: {help}");
    assert!(!help.contains("verifies that"), "{help}");
}
