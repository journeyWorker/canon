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
const PARENT_HELP_MARKER: &str = "Usage: canon finding <COMMAND>";

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
    // finding is one of the two reasons the derived count UNDER-counts,
    // and this is the moment the author still knows whether it was
    // sourced.
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
    let help = help_stdout(&["finding", "add", "--help"], ADD_HELP_MARKER, dir.path());

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
    // clap re-wraps `after_help` to the terminal width, so the sentence
    // reaches a reader as several lines. Whitespace-collapse both sides
    // and compare the prose: the pin is on the words, not the wrapping.
    let flowed = help.split_whitespace().collect::<Vec<_>>().join(" ");
    let canonical = canon_report::render::FIX_OF_FIX_MEANING.split_whitespace().collect::<Vec<_>>().join(" ");
    assert!(flowed.contains(&canonical), "the help must carry the one canonical fix-of-fix sentence verbatim: {help}");

    // Fix-of-fix stays derived: no flag, and the help says why.
    assert!(help.contains("Fix-of-fix is DERIVED, never recorded"), "{help}");
    assert!(!help.contains("--fix-of-fix"), "there must be no fix-of-fix flag: {help}");

    // And it never claims proof of anything.
    assert!(!help.contains("proof of"), "the help must never claim proof: {help}");
    assert!(!help.contains("verifies that"), "{help}");
}

/// The nouns whose COUNT is the banned claim, in the plural — a number
/// before one of these is "how many", which is the claim.
const METRIC_PLURALS: &[&str] = &["findings", "rounds", "issues", "fix-of-fixes"];

/// The same nouns either way round, for the `<noun> count is <n>`
/// shape, where the noun's number does not matter.
const METRIC_NOUNS: &[&str] = &["finding", "findings", "round", "rounds", "issue", "issues", "fix-of-fix", "fix-of-fixes"];

/// The words that turn an adjacent noun into a tally.
const COUNTING_WORDS: &[&str] = &["count", "counts", "total", "totals", "tally", "tallies", "number"];

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
/// cardinal (`three`, `eleven`), or a hyphenated compound of those
/// (`twenty-one`, whose `one` is part of a number rather than a word
/// on its own). `1-based`, `zero-padded`, `v0.4.0` and `120-134` are
/// NOT quantities — a number welded into a larger word is not an
/// assertion of how many.
fn quantity(word: &str) -> bool {
    !word.is_empty()
        && !SMALL_SPELLED.contains(&word)
        && word.split('-').all(|part| !part.is_empty() && (part.chars().all(|c| c.is_ascii_digit()) || SPELLED_COUNTS.contains(&part) || SMALL_SPELLED.contains(&part)))
}

/// `true` when a raw token closes its clause — it ends the line, or
/// carries clause-ending punctuation once trailing markup is stripped.
/// `2.` and `two,` close; `one` in "one sentence" does not.
fn clause_final(raw: &str, is_last: bool) -> bool {
    is_last || raw.trim_end_matches(|c: char| c.is_alphanumeric()).contains(['.', ',', ';', ':', '!', '?'])
}

/// The offending span when `text` asserts a hand-typed count of
/// findings, rounds, issues or fix-of-fixes; `None` otherwise.
///
/// Two shapes, and the guard claims exactly these two:
///
/// 1. a [`quantity`] within two words BEFORE a plural metric noun —
///    "3 findings", "3 review rounds", "eleven code-review rounds",
///    "thirty findings", "19 real issues";
/// 2. a counting word bound to a metric noun (within three words,
///    either side) carrying a value within four words after —
///    "fix-of-fix count is 2", "fix-of-fix total: 3", "the count of
///    findings is three".
///
/// # Where the line is drawn, and what is deliberately allowed
/// A number AFTER a singular noun is an INDEX, not a count, and every
/// such number on these surfaces is legitimate: `--round 1 --seq 1` in
/// the flag example, "s43 round 2 (finding 4)" citing which round
/// raised what, "(1-based)" in a flag's own help. Shape 1 therefore
/// requires the noun to be PLURAL and the number to come BEFORE it,
/// which is where English puts a quantifier and nowhere else.
///
/// Spelled `one` and `two` are the register mechanism prose is written
/// in — "two concurrent adds", "two distinct findings under one
/// finding's identity", "MEANS is one sentence" — and banning them
/// outright would force a rewording of correct text, which is worse
/// than the disease. So they count as a value only where they cannot
/// be quantifying the next word: as the CLAUSE-FINAL value of a
/// counting word, "the fix-of-fix count is two." A digit is a value at
/// any magnitude and in either shape, because prose describing a
/// mechanism spells its small numbers out and a metric claim reaches
/// for a digit.
///
/// The hole this leaves, stated rather than papered over: "there are
/// two findings" passes. "there are 2 findings", "there are three
/// findings" and "the finding count is two." all fail. The guard also
/// does not police the word `record`, which is too common in this
/// command's own prose to quantify safely.
fn hand_typed_count(text: &str) -> Option<String> {
    let raw: Vec<&str> = text.split_whitespace().collect();
    let words: Vec<String> = raw.iter().map(|token| word(token)).collect();
    let at = |i: usize| words.get(i).map(String::as_str).unwrap_or("");
    let span = |from: usize, to: usize| words[from..=to.min(words.len() - 1)].join(" ");
    let value = |i: usize| match words.get(i) {
        Some(word) => quantity(word) || (SMALL_SPELLED.contains(&word.as_str()) && clause_final(raw[i], i + 1 == raw.len())),
        None => false,
    };

    for i in 0..words.len() {
        if quantity(at(i)) {
            if let Some(j) = (i + 1..=i + 2).find(|&j| METRIC_PLURALS.contains(&at(j))) {
                return Some(span(i, j));
            }
        }
        if COUNTING_WORDS.contains(&at(i)) {
            let bound_to_a_metric = (i.saturating_sub(3)..=i + 3).any(|k| k != i && METRIC_NOUNS.contains(&at(k)));
            if bound_to_a_metric {
                if let Some(k) = (i + 1..=i + 4).find(|&k| value(k)) {
                    return Some(span(i.saturating_sub(3), k));
                }
            }
        }
    }
    None
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
/// as — which caught the nine sentences that had shipped and nothing
/// else — with [`hand_typed_count`], which catches the SHAPE. The
/// literal list stays as a second belt: it pins the exact strings that
/// did ship, including the two ("real issues", "of them defects") whose
/// nouns are outside the shape guard's vocabulary. A number belongs in
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
    let cli_surface = include_str!("../src/main.rs");

    // The exact sentences that shipped. Lowercased substring match.
    const BANNED: &[&str] = &[
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
    for (where_, text) in [
        ("`canon finding add --help`", add_help.as_str()),
        ("`canon finding --help`", finding_help.as_str()),
        ("crates/canon-cli/src/finding.rs", module_doc),
        ("crates/canon-cli/src/main.rs", cli_surface),
    ] {
        let haystack = text.to_ascii_lowercase();
        for phrase in BANNED {
            assert!(!haystack.contains(&phrase.to_ascii_lowercase()), "{where_} asserts a hand-typed count via {phrase:?}");
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

/// The guard's own contract, both halves — it must catch the class it
/// names, and it must spare every legitimate number these surfaces
/// actually carry. Without the second half the guard would be one
/// tightening away from forcing a rewording of correct help, which is
/// the failure mode a shape guard has and a substring list does not.
#[test]
fn the_count_guard_catches_the_class_and_spares_the_indices() {
    for claim in [
        "there are 3 findings",
        "3 review rounds",
        "fix-of-fix total: 3",
        "three findings",
        "there are three findings on this change",
        "the true fix-of-fix count is 2",
        "the fix-of-fix count is **2**",
        "the finding count is 19",
        "the count of findings is three",
        "eleven code-review rounds",
        "thirty findings",
        "19 real issues",
        "twenty-one open findings",
        "we closed 7 findings in 2 rounds",
    ] {
        assert!(hand_typed_count(claim).is_some(), "the guard must catch {claim:?}");
    }

    for legitimate in [
        "canon finding add --change-id s43-findings-are-records --round 1 --seq 1",
        "a finding's natural key is `{change_id}__{round:04}__{seq:04}`",
        "Which review round on this change raised it (1-based)",
        "This finding's index within the round (1-based)",
        "s43 round 2 (finding 4) found this doc and the --help text below",
        "v0.4.0 shipped after a long run of code-review rounds",
        "two concurrent adds land two distinct findings under one finding's identity",
        "a DERIVED count that stops being typed from memory",
        "v0.4.0's release note hand-typed its issue counts into a published tag",
        "Where in the tree, as path/to/file.rs:120-134 (line breaks refused)",
        "a short sha zero-padded to 40, and all of them were accepted",
        "the counts live in mart_review_rounds, off the 'Review rounds' panel",
    ] {
        assert_eq!(hand_typed_count(legitimate), None, "the guard must spare {legitimate:?}");
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
    assert!(checked_help(&run_canon(&["finding", "--help"], dir.path()), PARENT_HELP_MARKER).is_ok());
}
