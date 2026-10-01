//! Integration tests for `canon evidence add` (s42
//! `close-the-open-loops`, task group 4), invoking the actually-built
//! `canon` binary (`env!("CARGO_BIN_EXE_canon")`) — the same discipline
//! `tests/gate.rs` states for itself: subprocess-level behavior, exit
//! codes, and real-filesystem side effects need the real binary
//! boundary. `crates/canon-cli/src/evidence.rs`'s own unit tests already
//! cover the library decisions in-process; what only THIS boundary can
//! prove is that the clap surface exists and parses — which is exactly
//! what the dogfood in task 4.3 depends on.
//!
//! The headline test is [`the_three_command_loop_flips_a_checkbox_on_authored_evidence`]:
//! `canon evidence add` → `canon gate promote` → `canon gate task`, no
//! `--force`, no hand-written `EvidenceRecord`, no hand edit of
//! `tasks.md`. Before s42 that sequence was unrunnable — the gate
//! demanded evidence no command could author.

use std::path::Path;
use std::process::{Command, Output};

fn run_canon_as(args: &[&str], cwd: &Path, actor: &str) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).env("CANON_ACTOR", actor).current_dir(cwd).output().expect("spawn canon binary")
}

fn run_canon(args: &[&str], cwd: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(cwd).output().expect("spawn canon binary")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

/// A repo with ONE configured openspec plan source carrying one change.
///
/// `proposal.md` is not decoration: `canon evidence add` admits a task
/// through `crate::dispatch::validate_task_binding`, which decides by
/// the IMPORTER's rule (`PlanAdapter::parse`), and the openspec dialect
/// recognizes no change dir that lacks one. `tests/gate.rs`'s own
/// fixtures omit it because `canon gate task` only needs the document to
/// EXIST — that asymmetry is deliberate on both sides, and
/// [`evidence_add_refuses_a_task_the_plan_corpus_does_not_carry`] pins
/// this side of it.
fn repo_with_plan_corpus(change_id: &str, rows: &str) -> tempfile::TempDir {
    let dir = tempfile::tempdir().expect("a temp dir");
    std::fs::write(dir.path().join("canon.yaml"), "plans:\n  sources:\n    - dialect: openspec\n      root: openspec/changes\n").expect("writing canon.yaml");
    let change_dir = dir.path().join("openspec/changes").join(change_id);
    std::fs::create_dir_all(&change_dir).expect("creating the change dir");
    std::fs::write(change_dir.join("proposal.md"), format!("# {change_id}\n\n## Why\n\nTo exercise the authored-evidence loop.\n")).expect("writing proposal.md");
    std::fs::write(change_dir.join("tasks.md"), rows).expect("writing tasks.md");
    dir
}

fn tasks_md(repo: &Path, change_id: &str) -> String {
    std::fs::read_to_string(repo.join("openspec/changes").join(change_id).join("tasks.md")).expect("reading tasks.md")
}

/// Whether anything is sitting in the evidence staging tier
/// (`<ledger_root>/_staging`), by directory presence rather than a
/// library read — this file deliberately owns no in-process knowledge of
/// canon's layout beyond the one path the CLI documents.
fn staging_is_empty(repo: &Path) -> bool {
    let staged = repo.join(".canon/ledger/_staging/kind=evidence_record");
    std::fs::read_dir(&staged).map(|mut entries| entries.next().is_none()).unwrap_or(true)
}

fn committed_evidence_files(repo: &Path) -> Vec<std::path::PathBuf> {
    let committed = repo.join(".canon/ledger/kind=evidence_record");
    let Ok(entries) = std::fs::read_dir(&committed) else { return Vec::new() };
    let mut paths: Vec<std::path::PathBuf> = entries.filter_map(Result::ok).map(|e| e.path()).collect();
    // Total, data-derived order so an assertion never depends on
    // filesystem walk order.
    paths.sort();
    paths
}
#[test]
fn approval_pair_is_recorded_and_half_pairs_write_nothing() {
    let dir = repo_with_plan_corpus("it-approval", "- [ ] 1.1 Approve the change\n");
    let base = ["evidence", "add", "--task", "it-approval#1.1", "--kind", "test-run", "--ref", "report.txt", "--surface-ref", "effect:secret-access", "--role", "implementer"];
    for flags in [vec!["--approval-by", "alice"], vec!["--approval-role", "human"], vec!["--approval-by", " ", "--approval-role", "human"]] {
        let args: Vec<&str> = base.iter().copied().chain(flags).collect();
        let output = run_canon(&args, dir.path());
        assert_eq!(output.status.code(), Some(2), "{}", stderr(&output));
        assert!(staging_is_empty(dir.path()));
        assert!(committed_evidence_files(dir.path()).is_empty());
    }
    let args: Vec<&str> = base.iter().copied().chain(["--approval-by", "alice", "--approval-role", "human"]).collect();
    let output = run_canon_as(&args, dir.path(), "alice");
    assert!(output.status.success(), "{}", stderr(&output));
    let spoofed = run_canon_as(&args, dir.path(), "mallory");
    assert_eq!(spoofed.status.code(), Some(2), "a caller cannot spoof a different approval identity: {}", stderr(&spoofed));
    let output = run_canon(&["gate", "promote"], dir.path());
    assert!(output.status.success(), "{}", stderr(&output));
    let files = committed_evidence_files(dir.path());
    assert_eq!(files.len(), 1);
    let record: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&files[0]).unwrap()).unwrap();
    assert_eq!(record["approval"]["approver"], "alice");
    assert_eq!(record["approval"]["role"], "human");
    assert_eq!(record["approval"]["verified"], true);
    assert_eq!(record["surface_ref"], serde_json::json!(["effect:secret-access"]));
    assert!(record["approval"]["at"].as_str().unwrap().parse::<chrono::DateTime<chrono::Utc>>().is_ok());
}


/// s42 task 4.3, through the real binary: a checkbox flips on evidence
/// this repo's own CLI authored. Each step is asserted for its EFFECT,
/// not merely its exit code — staging populated, staging drained,
/// document mutated — so a step that silently no-ops cannot be masked
/// by the next one succeeding.
#[test]
fn the_three_command_loop_flips_a_checkbox_on_authored_evidence() {
    let change_id = "it-evidence-loop";
    let dir = repo_with_plan_corpus(change_id, "# it-evidence-loop — tasks\n\n- [ ] 1.1 Author evidence for a real flip\n");
    let repo = dir.path();
    let summary = "canon evidence add authored this row's proof";

    let added = run_canon(
        &[
            "evidence",
            "add",
            "--task",
            "it-evidence-loop#1.1",
            "--kind",
            "test-run",
            "--ref",
            "cargo test -p canon-cli --test evidence",
            "--role",
            "implementer",
            "--summary",
            summary,
            "--repo",
            ".",
        ],
        repo,
    );
    assert!(added.status.success(), "evidence add must succeed; stderr: {}", stderr(&added));
    assert!(stdout(&added).contains("staged"), "{}", stdout(&added));
    assert!(!staging_is_empty(repo), "the record must land in _staging");
    assert!(committed_evidence_files(repo).is_empty(), "a staged record must not be committed yet");

    // The gate refuses while the evidence is only staged — this is what
    // makes the flip below attributable to the promotion rather than to
    // the gate quietly reading the staging directory.
    let premature = run_canon(&["gate", "task", "it-evidence-loop#1.1", "--repo", "."], repo);
    assert_eq!(premature.status.code(), Some(1), "an unpromoted record must not satisfy the gate; stdout: {}", stdout(&premature));
    assert!(tasks_md(repo, change_id).contains("- [ ] 1.1"), "a refused flip must leave the document untouched");

    let promoted = run_canon(&["gate", "promote", "--repo", "."], repo);
    assert!(promoted.status.success(), "promotion must be clean; stdout: {} stderr: {}", stdout(&promoted), stderr(&promoted));
    assert!(staging_is_empty(repo), "promotion drains staging");
    assert_eq!(committed_evidence_files(repo).len(), 1, "the record is now committed");

    let flip = run_canon(&["gate", "task", "it-evidence-loop#1.1", "--repo", "."], repo);
    assert!(flip.status.success(), "the promoted record must satisfy the gate; stdout: {} stderr: {}", stdout(&flip), stderr(&flip));

    let document = tasks_md(repo, change_id);
    assert!(document.contains("- [x] 1.1"), "the row must be checked: {document}");
    assert!(document.contains(summary), "the authored summary must become the row's evidence note: {document}");

    // The committed record carries the `evidence: {kind, ref}` companion
    // the typed path narrows on — promotion rewrites the body to stamp
    // `run_seq`, so this asserts the companion survives that rewrite.
    let body: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(&committed_evidence_files(repo)[0]).expect("reading the committed record")).expect("valid JSON");
    assert_eq!(body["evidence"]["kind"], "test-run", "{body}");
    assert_eq!(body["evidence"]["ref"], "cargo test -p canon-cli --test evidence", "{body}");
    assert_eq!(body["run_seq"], 1, "promotion assigns the run_seq a staged record has none of: {body}");
    assert_eq!(body["task_id"], "it-evidence-loop#1.1", "{body}");
}

/// Task 4.1's shared-admission requirement: an id the plan corpus does
/// not carry is refused loudly (exit `2`) naming the id — the same
/// decision `canon dispatch begin --task` makes, through the same
/// helper, never a second copy of it.
#[test]
fn evidence_add_refuses_a_task_the_plan_corpus_does_not_carry() {
    let dir = repo_with_plan_corpus("it-evidence-unknown", "- [ ] 1.1 Do the thing\n");
    let repo = dir.path();

    let output = run_canon(
        &["evidence", "add", "--task", "it-evidence-unknown#9.9", "--kind", "test-run", "--ref", "some-command", "--role", "implementer", "--repo", "."],
        repo,
    );
    assert_eq!(output.status.code(), Some(2), "an unknown task is a fixable invocation; stdout: {}", stdout(&output));
    assert!(stderr(&output).contains("it-evidence-unknown#9.9"), "the refusal must name the rejected id: {}", stderr(&output));
    assert!(staging_is_empty(repo), "a refused add must stage nothing");
}

/// The other half of the shared admission: `canon evidence add` takes no
/// compat default. A repo configuring no `plans:` section has nowhere
/// for any id to be right, and says so distinctly from "your id is
/// wrong" — `canon gate task`'s openspec@repo fallback deliberately does
/// NOT apply here, because binding evidence asserts a `Task` RECORD
/// while flipping a row only edits a document.
#[test]
fn evidence_add_refuses_a_repo_that_configures_no_plan_sources() {
    let dir = tempfile::tempdir().expect("a temp dir");
    // The same shape `crate::dispatch`'s own `repo_without_plan_sources`
    // fixture uses: a real `canon.yaml` carrying no `plans:` section at
    // all, so the corpus is CONSULTABLE and simply empty.
    std::fs::write(dir.path().join("canon.yaml"), "project: demo\n").expect("writing canon.yaml");

    let output = run_canon(
        &["evidence", "add", "--task", "it-no-corpus#1.1", "--kind", "test-run", "--ref", "some-command", "--role", "implementer", "--repo", "."],
        dir.path(),
    );
    assert_eq!(output.status.code(), Some(2));
    assert!(stderr(&output).contains("configures no plan sources"), "{}", stderr(&output));
}

/// The fabrication scan runs at AUTHORING time, graded `1` — the same
/// code `canon gate task` returns for `fabricated-evidence`. Refusing
/// here rather than at flip time is the whole point: `canon gate
/// promote` would already have committed the note into the append-only
/// ledger, leaving the task permanently gate-red.
#[test]
fn evidence_add_refuses_a_fabricated_summary_before_staging_anything() {
    let dir = repo_with_plan_corpus("it-evidence-fake", "- [ ] 1.1 Do the thing\n");
    let repo = dir.path();

    let output = run_canon(
        &[
            "evidence",
            "add",
            "--task",
            "it-evidence-fake#1.1",
            "--kind",
            "test-run",
            "--ref",
            "some-command",
            "--role",
            "implementer",
            "--summary",
            "this would pass once the suite is wired up",
            "--repo",
            ".",
        ],
        repo,
    );
    assert_eq!(output.status.code(), Some(1), "a fabrication marker is a gate-red refusal; stdout: {}", stdout(&output));
    assert!(stderr(&output).contains("fabricated-evidence"), "{}", stderr(&output));
    assert!(staging_is_empty(repo), "a refused add must stage nothing");
}

/// The `ReviewEvidence` BLOCKER, at the boundary it was actually
/// reported against: a shell-quoted `--summary $'ok\n- [x] 9.9 Forged
/// task'` reaching the real binary. Graded `2` (a fixable invocation,
/// not gate-red evidence), and the plan document is byte-identical
/// afterwards — the forged row never existed to be cleaned up.
#[test]
fn evidence_add_refuses_a_newline_that_would_forge_a_second_checked_row() {
    let rows = "- [ ] 1.1 Do the thing\n";
    let dir = repo_with_plan_corpus("it-evidence-inject", rows);
    let repo = dir.path();

    let output = run_canon(
        &[
            "evidence",
            "add",
            "--task",
            "it-evidence-inject#1.1",
            "--kind",
            "test-run",
            "--ref",
            "some-command",
            "--role",
            "implementer",
            "--summary",
            "ok\n- [x] 9.9 Forged task",
            "--repo",
            ".",
        ],
        repo,
    );
    assert_eq!(output.status.code(), Some(2), "a line separator is a usage refusal; stdout: {}", stdout(&output));
    assert!(stderr(&output).contains("line separator"), "the refusal must name what it rejected: {}", stderr(&output));
    assert!(staging_is_empty(repo), "a refused add must stage nothing");
    assert_eq!(tasks_md(repo, "it-evidence-inject"), rows, "a refused add must leave the plan document byte-identical");
}

/// The same vector without `--summary` at all: with no note,
/// `canon gate task`'s fallback suffix embeds the record's own
/// `actor.agent_id`, so `--actor-id` is the second injection point.
#[test]
fn evidence_add_refuses_a_newline_in_the_actor_id() {
    let rows = "- [ ] 1.1 Do the thing\n";
    let dir = repo_with_plan_corpus("it-evidence-actor", rows);
    let repo = dir.path();

    let output = run_canon(
        &[
            "evidence",
            "add",
            "--task",
            "it-evidence-actor#1.1",
            "--kind",
            "test-run",
            "--ref",
            "some-command",
            "--role",
            "implementer",
            "--actor-id",
            "canon\n- [x] 9.9 Forged task",
            "--repo",
            ".",
        ],
        repo,
    );
    assert_eq!(output.status.code(), Some(2), "stdout: {}", stdout(&output));
    assert!(stderr(&output).contains("--actor-id"), "the refusal must name the flag: {}", stderr(&output));
    assert!(staging_is_empty(repo), "a refused add must stage nothing");
    assert_eq!(tasks_md(repo, "it-evidence-actor"), rows);
}

/// The command's own help must not overstate what it proves
/// (`ReviewEvidence` finding 1). Asserted at the binary boundary because
/// this is the text an operator or an agent actually reads before
/// trusting a flip.
#[test]
fn evidence_add_help_states_it_is_an_attestation_and_names_what_the_gate_skips() {
    let dir = tempfile::tempdir().expect("a temp dir");
    let output = run_canon(&["evidence", "add", "--help"], dir.path());
    assert!(output.status.success(), "--help must exit 0: {}", stderr(&output));
    let help = stdout(&output);

    assert!(help.contains("ATTESTATION, NOT PROOF"), "{help}");
    assert!(help.contains("never runs, resolves, or checks --ref"), "{help}");
    assert!(help.contains("can authorize its own checkbox"), "{help}");
    for skipped in ["staleness", "trust-ladder", "release-trust", "divergence"] {
        assert!(help.contains(skipped), "the help must name `{skipped}` among what the gate does NOT check: {help}");
    }
    assert!(!help.contains("proof of"), "the help must never claim proof: {help}");
}
