//! Integration tests for `canon gate check/task/promote/install-hooks/
//! selftest` (S5 wave-2-part2), invoking the actually-built `canon`
//! binary (`env!("CARGO_BIN_EXE_canon")`) — never `canon_cli::gate`'s
//! library functions in-process, matching `tests/context.rs`'s own
//! discipline (that file's own module doc: subprocess-level behavior,
//! exit codes, and real-filesystem side effects need the real binary
//! boundary; pure logic is already unit-tested inside `canon-gate`
//! itself and `crates/canon-cli/src/gate.rs`'s own formatting helpers).

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use canon_model::{Actor, Envelope, EvidenceRecord, EvidenceVerdict, RawRecord, RecordKind, RoleId, TaskId};
use canon_store::git_tier::GitTier;
use canon_store::tier::{RawWrite, Tier};
use chrono::Utc;

fn run_canon(args: &[&str], cwd: &Path) -> Output {
    Command::new(env!("CARGO_BIN_EXE_canon")).args(args).current_dir(cwd).output().expect("spawn canon binary")
}

fn stdout(output: &Output) -> String {
    String::from_utf8_lossy(&output.stdout).into_owned()
}

fn stderr(output: &Output) -> String {
    String::from_utf8_lossy(&output.stderr).into_owned()
}

fn write_evidence(ledger_root: &Path, task_id: &str, role: &str, verdict: EvidenceVerdict) {
    let envelope = Envelope::new(1, RecordKind::EvidenceRecord, Utc::now(), Actor::new("it-agent", RoleId::parse(role).unwrap()));
    let record = EvidenceRecord::new(envelope, Some(TaskId::parse(task_id).unwrap()), None, None, verdict);
    GitTier::new(ledger_root).write(&record).expect("write evidence record");
}

// ── canon gate check ──

#[test]
fn gate_check_exits_clean_on_an_empty_repo() {
    let dir = tempfile::tempdir().unwrap();
    let output = run_canon(&["gate", "check", "--repo", "."], dir.path());
    assert!(output.status.success(), "empty repo must gate green; stderr: {}", stderr(&output));
    assert!(stdout(&output).contains("clean"), "{}", stdout(&output));
}

#[test]
fn gate_check_exits_gate_red_on_a_seeded_uncovered_cell_violation() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".canon")).unwrap();
    std::fs::write(dir.path().join(".canon/policy.yaml"), "risk_routing:\n  reviewer: true\n").unwrap();
    write_evidence(&dir.path().join(".canon/ledger"), "seed-change#1", "implementer", EvidenceVerdict::Faithful);

    let output = run_canon(&["gate", "check", "--repo", "."], dir.path());
    assert_eq!(output.status.code(), Some(1), "an uncovered-cell violation must gate-red (exit 1); stdout: {}", stdout(&output));
    let text = stdout(&output);
    assert!(text.contains("uncovered-cell"), "{text}");
    assert!(text.contains("seed-change#1"), "{text}");
}

#[test]
fn gate_check_release_flag_engages_release_trust_check_but_ordinary_run_stays_silent_on_it() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".canon")).unwrap();
    std::fs::write(dir.path().join(".canon/policy.yaml"), "trust_required:\n  p1: human\n").unwrap();

    // A `reviewed` record tagged class `p1` with no matching review
    // record: `TrustLadderCheck` reports `unreviewed-promotion` either
    // way; `trust-below-required` needs the release-scoped check.
    // `lifecycle`/`flagged` are native `EvidenceRecord` fields (s15
    // P3b); `class` stays a raw companion key (never migrated).
    let ledger_root = dir.path().join(".canon/ledger");
    std::fs::create_dir_all(&ledger_root).unwrap();
    let envelope = Envelope::new(1, RecordKind::EvidenceRecord, Utc::now(), Actor::new("it-agent", RoleId::parse("implementer").unwrap()));
    let record = EvidenceRecord::new(envelope, Some(TaskId::parse("seed-change#1").unwrap()), None, None, EvidenceVerdict::Faithful).with_lifecycle(canon_model::TrustLifecycle::Reviewed);
    let mut body = serde_json::to_value(&record).unwrap();
    body.as_object_mut().unwrap().insert("class".to_string(), serde_json::json!("p1"));
    GitTier::new(&ledger_root).write(&canon_store::tier::RawWrite(canon_model::RawRecord(body))).unwrap();

    let ordinary = run_canon(&["gate", "check", "--repo", "."], dir.path());
    assert_eq!(ordinary.status.code(), Some(1), "unreviewed-promotion alone still gate-reds an ordinary run");
    assert!(stdout(&ordinary).contains("unreviewed-promotion"));
    assert!(!stdout(&ordinary).contains("trust-below-required"), "ordinary (non-release) evaluation must never surface trust-below-required:\n{}", stdout(&ordinary));

    let release = run_canon(&["gate", "check", "--repo", ".", "--release"], dir.path());
    assert_eq!(release.status.code(), Some(1));
    assert!(stdout(&release).contains("unreviewed-promotion"), "TrustLadderCheck must still be present under --release");
}

/// D7/task 1.4-equivalent for `canon gate`: run from a SUBDIRECTORY of a
/// fixture repo with no `--repo` flag — clap's `.` default applies —
/// resolves the nearest ANCESTOR `canon.yaml` as the repo root (matching
/// `canon context`'s own `context_from_a_subdirectory_resolves_the_
/// ancestor_repo_root_policy` test) and surfaces THAT root's real
/// `.canon/policy.yaml` + `.canon/ledger`, never a subdirectory-relative
/// default.
#[test]
fn gate_check_from_a_subdirectory_resolves_the_ancestor_repo_root() {
    let repo = tempfile::tempdir().unwrap();
    std::fs::write(repo.path().join("canon.yaml"), "tiers:\n  git: { root: .canon/ledger }\n").unwrap();
    std::fs::create_dir_all(repo.path().join(".canon")).unwrap();
    std::fs::write(repo.path().join(".canon/policy.yaml"), "risk_routing:\n  reviewer: true\n").unwrap();
    write_evidence(&repo.path().join(".canon/ledger"), "seed-change#1", "implementer", EvidenceVerdict::Faithful);

    let subdir = repo.path().join("nested").join("deep");
    std::fs::create_dir_all(&subdir).unwrap();

    let output = run_canon(&["gate", "check"], &subdir);
    assert_eq!(
        output.status.code(),
        Some(1),
        "canon gate check run from a subdirectory must still surface the repo ROOT's uncovered-cell violation; stdout: {}",
        stdout(&output)
    );
    assert!(stdout(&output).contains("uncovered-cell"), "{}", stdout(&output));
}

// ── canon gate task ──

fn write_tasks_md(repo: &Path, change_id: &str, body: &str) -> PathBuf {
    let dir = repo.join("openspec/changes").join(change_id);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("tasks.md");
    std::fs::write(&path, body).unwrap();
    path
}

#[test]
fn gate_task_flip_is_blocked_with_no_evidence_record() {
    let dir = tempfile::tempdir().unwrap();
    let tasks_path = write_tasks_md(dir.path(), "it-task-flip", "- [ ] 1 Do the thing\n");

    let output = run_canon(&["gate", "task", "it-task-flip#1", "--repo", "."], dir.path());
    assert_eq!(output.status.code(), Some(1), "no evidence must block the flip; stdout: {}", stdout(&output));
    assert!(stderr(&output).contains("unevidenced-flip"), "{}", stderr(&output));
    assert_eq!(std::fs::read_to_string(&tasks_path).unwrap(), "- [ ] 1 Do the thing\n", "the row must stay byte-unchanged when the flip is blocked");
}

#[test]
fn gate_task_flip_succeeds_with_clean_evidence() {
    let dir = tempfile::tempdir().unwrap();
    let tasks_path = write_tasks_md(dir.path(), "it-task-flip-ok", "- [ ] 1 Do the thing\n");
    write_evidence(&dir.path().join(".canon/ledger"), "it-task-flip-ok#1", "implementer", EvidenceVerdict::Faithful);

    let output = run_canon(&["gate", "task", "it-task-flip-ok#1", "--repo", "."], dir.path());
    assert!(output.status.success(), "clean evidence must flip the row; stderr: {}", stderr(&output));
    let flipped = std::fs::read_to_string(&tasks_path).unwrap();
    assert!(flipped.starts_with("- [x] 1 Do the thing"), "{flipped}");

    // Idempotent on a second run.
    let second = run_canon(&["gate", "task", "it-task-flip-ok#1", "--repo", "."], dir.path());
    assert!(second.status.success());
    assert_eq!(std::fs::read_to_string(&tasks_path).unwrap(), flipped, "a second gate_task call on an already-flipped row must be a byte-identical no-op");
}

#[test]
fn gate_task_flip_is_blocked_on_a_divergent_verdict() {
    let dir = tempfile::tempdir().unwrap();
    let tasks_path = write_tasks_md(dir.path(), "it-task-flip-divergent", "- [ ] 1 Do the thing\n");
    write_evidence(&dir.path().join(".canon/ledger"), "it-task-flip-divergent#1", "implementer", EvidenceVerdict::Divergent);

    let output = run_canon(&["gate", "task", "it-task-flip-divergent#1", "--repo", "."], dir.path());
    assert_eq!(output.status.code(), Some(1), "a Divergent verdict is no evidence; stdout: {}", stdout(&output));
    assert_eq!(std::fs::read_to_string(&tasks_path).unwrap(), "- [ ] 1 Do the thing\n");
}

#[test]
fn gate_task_reports_an_unknown_task_id_as_a_usage_failure() {
    let dir = tempfile::tempdir().unwrap();
    write_tasks_md(dir.path(), "it-task-unknown", "- [ ] 1 Do the thing\n");

    let output = run_canon(&["gate", "task", "it-task-unknown#99", "--repo", "."], dir.path());
    assert_eq!(output.status.code(), Some(1));
    assert!(stderr(&output).contains("no matching row"), "{}", stderr(&output));
}

// ── canon gate task: s35 dialect-seam plan-source resolution ──

fn write_tasks_under(root: &Path, change_id: &str, body: &str) -> PathBuf {
    let dir = root.join("openspec/changes").join(change_id);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("tasks.md");
    std::fs::write(&path, body).unwrap();
    path
}

/// s35 compat default: a repo whose `canon.yaml` EXISTS but has no
/// `plans:` section still resolves the flip through the documented
/// `[{ dialect: openspec, root: <repo> }]` default — the dependence
/// moved from hardcoded to configured-default, never removed.
#[test]
fn gate_task_compat_default_resolves_openspec_at_repo_when_plans_is_absent() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(dir.path().join("canon.yaml"), "tiers:\n  git:\n    root: .canon/ledger\n").unwrap();
    let tasks_path = write_tasks_md(dir.path(), "it-compat-default", "- [ ] 1 Do the thing\n");
    write_evidence(&dir.path().join(".canon/ledger"), "it-compat-default#1", "implementer", EvidenceVerdict::Faithful);

    let output = run_canon(&["gate", "task", "it-compat-default#1", "--repo", "."], dir.path());
    assert!(output.status.success(), "compat default must flip via openspec@repo; stderr: {}", stderr(&output));
    assert!(std::fs::read_to_string(&tasks_path).unwrap().starts_with("- [x] 1 Do the thing"));
}

/// s35 multi-source resolution: sources are consulted in config order,
/// first-hit-wins. The change lives only in the SECOND source, proving
/// the driver keeps looking past a source that does not locate it.
#[test]
fn gate_task_resolves_the_task_in_a_later_configured_source() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("canon.yaml"),
        "tiers:\n  git:\n    root: .canon/ledger\nplans:\n  sources:\n    - dialect: openspec\n      root: plansA\n    - dialect: openspec\n      root: plansB\n",
    )
    .unwrap();
    let tasks_b = write_tasks_under(&dir.path().join("plansB"), "it-multi", "- [ ] 1 Do the thing\n");
    write_evidence(&dir.path().join(".canon/ledger"), "it-multi#1", "implementer", EvidenceVerdict::Faithful);

    let output = run_canon(&["gate", "task", "it-multi#1", "--repo", "."], dir.path());
    assert!(output.status.success(), "the second source must be consulted; stderr: {}", stderr(&output));
    assert!(std::fs::read_to_string(&tasks_b).unwrap().starts_with("- [x] 1 Do the thing"), "the plansB copy must flip");
}

/// s35 first-source-wins: when the change lives in BOTH configured
/// sources, the FIRST one wins and the later one is never touched.
#[test]
fn gate_task_first_configured_source_wins_when_both_locate_the_task() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("canon.yaml"),
        "tiers:\n  git:\n    root: .canon/ledger\nplans:\n  sources:\n    - dialect: openspec\n      root: plansA\n    - dialect: openspec\n      root: plansB\n",
    )
    .unwrap();
    let tasks_a = write_tasks_under(&dir.path().join("plansA"), "it-both", "- [ ] 1 Do the thing\n");
    let tasks_b = write_tasks_under(&dir.path().join("plansB"), "it-both", "- [ ] 1 Do the thing\n");
    write_evidence(&dir.path().join(".canon/ledger"), "it-both#1", "implementer", EvidenceVerdict::Faithful);

    let output = run_canon(&["gate", "task", "it-both#1", "--repo", "."], dir.path());
    assert!(output.status.success(), "stderr: {}", stderr(&output));
    assert!(std::fs::read_to_string(&tasks_a).unwrap().starts_with("- [x] 1 Do the thing"), "the first source must flip");
    assert_eq!(std::fs::read_to_string(&tasks_b).unwrap(), "- [ ] 1 Do the thing\n", "the later source must stay untouched");
}

/// s35: no configured source locates the task -> loud usage failure
/// (exit 2) naming that the sources were consulted, never a silent
/// success or a misleading gate-red.
#[test]
fn gate_task_reports_when_no_plan_source_locates_the_task() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::write(
        dir.path().join("canon.yaml"),
        "tiers:\n  git:\n    root: .canon/ledger\nplans:\n  sources:\n    - dialect: openspec\n      root: plansA\n",
    )
    .unwrap();

    let output = run_canon(&["gate", "task", "it-ghost#1", "--repo", "."], dir.path());
    assert_eq!(output.status.code(), Some(2), "no source located it — a usage failure; stdout: {}", stdout(&output));
    assert!(stderr(&output).contains("no plan source locates"), "{}", stderr(&output));
}

// ── canon gate task: typed evidence path (S10 part2, design.md D4) ──

fn write_tasks_vocab(repo: &Path, change_id: &str, yaml: &str) -> PathBuf {
    let dir = repo.join("openspec/changes").join(change_id);
    std::fs::create_dir_all(&dir).unwrap();
    let path = dir.join("tasks.vocab.yaml");
    std::fs::write(&path, yaml).unwrap();
    path
}

/// A minimal, self-contained `canon.core` vocabulary plugin — one `task`
/// directive, one `task-status` enum — independent of this repo's real
/// checked-in `.canon/vocab/canon.core/` (mirrors `src/context.rs`'s own
/// `seed_vocab` test fixture).
fn seed_vocab_core(repo: &Path) {
    let core = repo.join(".canon/vocab/canon.core");
    std::fs::create_dir_all(core.join("directives")).unwrap();
    std::fs::write(core.join("plugin.yaml"), "id: canon.core\nversion: \"0.1.0\"\nkind: core\nexports:\n  directives: directives/\n  enums: enums.yaml\n").unwrap();
    std::fs::write(
        core.join("directives/task.yaml"),
        "directives:\n  - name: task\n    attrs:\n      - name: desc\n        type: string\n        required: true\n      - name: status\n        type: { domain: task-status }\n        required: true\n      - name: evidence\n        type: evidence\n        required: true\n",
    )
    .unwrap();
    std::fs::write(core.join("enums.yaml"), "enums:\n  task-status:\n    - open\n    - done\n").unwrap();
}

/// An `EvidenceRecord` carrying the D4 typed-evidence companion — an extra
/// top-level `evidence: {kind, ref}` key on the raw ledger JSON, mirroring
/// the atom's own `evidence` attr shape (`crate::gate::raw_evidence_kind_ref`'s
/// own doc: silently dropped by `EvidenceRecord`'s strict `Deserialize`,
/// same established companion pattern as `trust_ladder`/`evidence_sha`).
fn write_typed_evidence(ledger_root: &Path, task_id: &str, role: &str, verdict: EvidenceVerdict, kind: &str, evidence_ref: &str) {
    let envelope = Envelope::new(1, RecordKind::EvidenceRecord, Utc::now(), Actor::new("it-agent", RoleId::parse(role).unwrap()));
    let record = EvidenceRecord::new(envelope, Some(TaskId::parse(task_id).unwrap()), None, None, verdict);
    let mut body = serde_json::to_value(&record).unwrap();
    body.as_object_mut().unwrap().insert("evidence".to_string(), serde_json::json!({"kind": kind, "ref": evidence_ref}));
    GitTier::new(ledger_root).write(&RawWrite(RawRecord(body))).unwrap();
}

/// [`write_typed_evidence`] plus an `evidence_note` companion (`summary`,
/// no `command_result`) — the SEPARATE raw-JSON key
/// `canon_gate::evidence_note_of` reads, independent of the `evidence:
/// {kind, ref}` typed-path companion `write_typed_evidence` itself adds.
fn write_typed_evidence_with_note(ledger_root: &Path, task_id: &str, role: &str, verdict: EvidenceVerdict, kind: &str, evidence_ref: &str, note_summary: &str) {
    let envelope = Envelope::new(1, RecordKind::EvidenceRecord, Utc::now(), Actor::new("it-agent", RoleId::parse(role).unwrap()));
    let record = EvidenceRecord::new(envelope, Some(TaskId::parse(task_id).unwrap()), None, None, verdict);
    let mut body = serde_json::to_value(&record).unwrap();
    let obj = body.as_object_mut().unwrap();
    obj.insert("evidence".to_string(), serde_json::json!({"kind": kind, "ref": evidence_ref}));
    obj.insert("evidence_note".to_string(), serde_json::json!({"summary": note_summary}));
    GitTier::new(ledger_root).write(&RawWrite(RawRecord(body))).unwrap();
}

fn task_atom_yaml(id: &str, kind: &str, evidence_ref: &str) -> String {
    format!("- id: {id}\n  tag: task\n  attrs:\n    desc: \"typed pilot task\"\n    status: open\n    evidence:\n      kind: {kind}\n      ref: \"{evidence_ref}\"\n")
}

#[test]
fn gate_task_typed_path_passes_with_a_matching_typed_evidence_record() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".canon")).unwrap();
    std::fs::write(dir.path().join(".canon/policy.yaml"), "trust_required:\n  test-run: agent\n").unwrap();
    seed_vocab_core(dir.path());

    let tasks_path = write_tasks_md(dir.path(), "it-typed-gate", "- [ ] 1 Do the typed thing\n");
    write_tasks_vocab(dir.path(), "it-typed-gate", &task_atom_yaml("it-typed-gate#1", "test-run", "cargo test -p it"));
    write_typed_evidence(&dir.path().join(".canon/ledger"), "it-typed-gate#1", "implementer", EvidenceVerdict::Faithful, "test-run", "cargo test -p it");

    let output = run_canon(&["gate", "task", "it-typed-gate#1", "--repo", "."], dir.path());
    assert!(output.status.success(), "a matching typed evidence.kind/ref must flip the row; stderr: {}", stderr(&output));
    assert!(std::fs::read_to_string(&tasks_path).unwrap().starts_with("- [x] 1 Do the typed thing"));
}

/// Proves the typed path genuinely NARROWS by kind — not merely "some
/// evidence exists for this task_id" (the free path's own, weaker bar):
/// the only evidence record on the ledger names a DIFFERENT kind than the
/// typed atom declares, so it must be ignored and the flip blocked with
/// the same stable `unevidenced-flip` failure class the free path uses
/// for "no evidence at all".
#[test]
fn gate_task_typed_path_blocks_on_a_wrong_kind_evidence_record() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".canon")).unwrap();
    std::fs::write(dir.path().join(".canon/policy.yaml"), "trust_required:\n  test-run: agent\n  manual-review: human\n").unwrap();
    seed_vocab_core(dir.path());

    let tasks_path = write_tasks_md(dir.path(), "it-typed-gate-wrong-kind", "- [ ] 1 Do the typed thing\n");
    write_tasks_vocab(dir.path(), "it-typed-gate-wrong-kind", &task_atom_yaml("it-typed-gate-wrong-kind#1", "test-run", "cargo test -p it"));
    // Wrong kind: the atom declared `test-run`, this record is `manual-review`.
    write_typed_evidence(&dir.path().join(".canon/ledger"), "it-typed-gate-wrong-kind#1", "implementer", EvidenceVerdict::Faithful, "manual-review", "reviewer sign-off");

    let output = run_canon(&["gate", "task", "it-typed-gate-wrong-kind#1", "--repo", "."], dir.path());
    assert_eq!(output.status.code(), Some(1), "a wrong-kind evidence record must not satisfy the typed requirement; stdout: {}", stdout(&output));
    assert!(stderr(&output).contains("unevidenced-flip"), "{}", stderr(&output));
    assert_eq!(std::fs::read_to_string(&tasks_path).unwrap(), "- [ ] 1 Do the typed thing\n", "the row must stay byte-unchanged when the typed flip is blocked");
}

/// ReviewS10Part2 fix: the typed path's `EvidenceNote` companions must be
/// derived from the SAME raw records the kind/ref filter matched — never
/// from every raw record sharing `task_id`. Seeds a stale/wrong-kind
/// record for this task_id carrying its OWN (different) `evidence_note`,
/// alongside the correctly-typed matching record carrying its OWN note;
/// the flip must succeed using ONLY the matching record's note text, and
/// the wrong-kind record must neither block the flip nor leak its note
/// onto the flipped row.
#[test]
fn gate_task_typed_path_pairs_notes_with_only_the_matching_typed_record() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".canon")).unwrap();
    std::fs::write(dir.path().join(".canon/policy.yaml"), "trust_required:\n  test-run: agent\n  manual-review: human\n").unwrap();
    seed_vocab_core(dir.path());

    let tasks_path = write_tasks_md(dir.path(), "it-typed-gate-note-pairing", "- [ ] 1 Do the typed thing\n");
    write_tasks_vocab(dir.path(), "it-typed-gate-note-pairing", &task_atom_yaml("it-typed-gate-note-pairing#1", "test-run", "cargo test -p it"));
    // Stale/wrong-kind record for the SAME task_id, carrying its OWN
    // note — must be excluded from both evidence AND notes.
    write_typed_evidence_with_note(
        &dir.path().join(".canon/ledger"),
        "it-typed-gate-note-pairing#1",
        "reviewer",
        EvidenceVerdict::Faithful,
        "manual-review",
        "reviewer sign-off",
        "WRONG stale note — must never appear on the flipped row",
    );
    // The correctly-typed matching record, with its own note.
    write_typed_evidence_with_note(
        &dir.path().join(".canon/ledger"),
        "it-typed-gate-note-pairing#1",
        "implementer",
        EvidenceVerdict::Faithful,
        "test-run",
        "cargo test -p it",
        "cargo test -p it: 12 passed",
    );

    let output = run_canon(&["gate", "task", "it-typed-gate-note-pairing#1", "--repo", "."], dir.path());
    assert!(output.status.success(), "the matching typed record must flip the row; stderr: {}", stderr(&output));
    let flipped = std::fs::read_to_string(&tasks_path).unwrap();
    assert!(flipped.contains("cargo test -p it: 12 passed"), "the matching record's own note must supply the evidence text: {flipped}");
    assert!(!flipped.contains("WRONG stale note"), "the wrong-kind record's note must never leak into the flip: {flipped}");
}

/// D4's Risks-section mitigation: gate time re-resolves the vocabulary
/// fresh, so an atom whose declared `evidence.kind` is OUTSIDE the
/// policy-derived domain fails closed with the checker's own diagnostic —
/// never a silent pass.
#[test]
fn gate_task_typed_path_rejects_an_evidence_kind_outside_the_policy_domain() {
    let dir = tempfile::tempdir().unwrap();
    std::fs::create_dir_all(dir.path().join(".canon")).unwrap();
    std::fs::write(dir.path().join(".canon/policy.yaml"), "trust_required:\n  test-run: agent\n").unwrap();
    seed_vocab_core(dir.path());

    let tasks_path = write_tasks_md(dir.path(), "it-typed-gate-bad-kind", "- [ ] 1 Do the typed thing\n");
    write_tasks_vocab(dir.path(), "it-typed-gate-bad-kind", &task_atom_yaml("it-typed-gate-bad-kind#1", "not-a-real-kind", "whatever"));
    write_typed_evidence(&dir.path().join(".canon/ledger"), "it-typed-gate-bad-kind#1", "implementer", EvidenceVerdict::Faithful, "not-a-real-kind", "whatever");

    let output = run_canon(&["gate", "task", "it-typed-gate-bad-kind#1", "--repo", "."], dir.path());
    assert_eq!(output.status.code(), Some(1), "an out-of-domain evidence kind must fail closed; stdout: {}", stdout(&output));
    assert!(stderr(&output).contains("E-BAD-EVIDENCE-KIND"), "{}", stderr(&output));
    assert_eq!(std::fs::read_to_string(&tasks_path).unwrap(), "- [ ] 1 Do the typed thing\n");
}

/// Additive (design.md Non-Goals): a change with NO `tasks.vocab.yaml` at
/// all still gates through the untyped free path exactly as before this
/// change — any non-`Divergent` `EvidenceRecord` for the task_id, no
/// `evidence.kind`/`ref` companion required.
#[test]
fn gate_task_falls_back_to_the_free_path_with_no_typed_atom_for_this_task_id() {
    let dir = tempfile::tempdir().unwrap();
    seed_vocab_core(dir.path());
    // A tasks.vocab.yaml exists for this change, but declares a DIFFERENT
    // task_id (`#2`) than the one being gated (`#1`) — `#1` must still
    // fall through to the free path untouched.
    let tasks_path = write_tasks_md(dir.path(), "it-typed-gate-fallback", "- [ ] 1 Do the untyped thing\n");
    write_tasks_vocab(dir.path(), "it-typed-gate-fallback", &task_atom_yaml("it-typed-gate-fallback#2", "test-run", "n/a"));
    write_evidence(&dir.path().join(".canon/ledger"), "it-typed-gate-fallback#1", "implementer", EvidenceVerdict::Faithful);

    let output = run_canon(&["gate", "task", "it-typed-gate-fallback#1", "--repo", "."], dir.path());
    assert!(output.status.success(), "an untyped task_id must still flip via the free path; stderr: {}", stderr(&output));
    assert!(std::fs::read_to_string(&tasks_path).unwrap().starts_with("- [x] 1 Do the untyped thing"));
}

// ── canon gate promote ──

#[test]
fn gate_promote_assigns_a_run_seq_and_lands_the_record_in_the_committed_tier() {
    let dir = tempfile::tempdir().unwrap();
    let ledger_root = dir.path().join(".canon/ledger");
    write_evidence(&ledger_root.join("_staging"), "it-promote#1", "implementer", EvidenceVerdict::Faithful);

    let output = run_canon(&["gate", "promote", "--repo", "."], dir.path());
    assert!(output.status.success(), "a well-formed staging record must promote cleanly; stdout: {}", stdout(&output));
    let text = stdout(&output);
    assert!(text.contains("run_seq=1"), "{text}");

    let committed = GitTier::new(&ledger_root).read(&canon_store::tier::TierQuery::kind(RecordKind::EvidenceRecord)).expect("read committed tier");
    assert_eq!(committed.records.len(), 1, "the promoted record must land in the committed tier");

    let staged_remaining = GitTier::new(ledger_root.join("_staging")).read(&canon_store::tier::TierQuery::kind(RecordKind::EvidenceRecord)).expect("read staging tier");
    assert!(staged_remaining.records.is_empty(), "a promoted staging file must be deleted");
}

#[test]
fn gate_promote_dry_run_writes_nothing() {
    let dir = tempfile::tempdir().unwrap();
    let ledger_root = dir.path().join(".canon/ledger");
    write_evidence(&ledger_root.join("_staging"), "it-promote-dry#1", "implementer", EvidenceVerdict::Faithful);

    let output = run_canon(&["gate", "promote", "--repo", ".", "--dry-run"], dir.path());
    assert!(output.status.success());
    assert!(stdout(&output).contains("would promote"), "{}", stdout(&output));

    let committed = GitTier::new(&ledger_root).read(&canon_store::tier::TierQuery::kind(RecordKind::EvidenceRecord)).expect("read committed tier");
    assert!(committed.records.is_empty(), "--dry-run must never write to the committed tier");
    let staged = GitTier::new(ledger_root.join("_staging")).read(&canon_store::tier::TierQuery::kind(RecordKind::EvidenceRecord)).expect("read staging tier");
    assert_eq!(staged.records.len(), 1, "--dry-run must never delete the staging file");
}

// ── canon gate install-hooks ──

#[test]
fn gate_install_hooks_is_idempotent_and_seeds_a_pre_commit_script_for_a_fresh_repo() {
    let dir = tempfile::tempdir().unwrap();

    let first = run_canon(&["gate", "install-hooks", "--repo", "."], dir.path());
    assert!(first.status.success(), "stderr: {}", stderr(&first));
    let settings: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.path().join(".claude/settings.json")).unwrap()).unwrap();
    assert_eq!(settings["hooks"]["PreToolUse"][0]["hooks"][0]["command"], serde_json::json!("canon gate task"), "the default install-hooks command must be the evidence-gated task-flip entry point, not the read-only check");
    let script_path = dir.path().join(".canon/scripts/canon-gate-pre-commit.sh");
    assert!(script_path.is_file(), "a fresh repo with no canon-gate hook entries must get the generic pre-commit script");

    let first_bytes = std::fs::read(&script_path).unwrap();
    let settings_bytes_first = std::fs::read(dir.path().join(".claude/settings.json")).unwrap();

    let second = run_canon(&["gate", "install-hooks", "--repo", "."], dir.path());
    assert!(second.status.success());
    assert!(stdout(&second).contains("no diff"), "{}", stdout(&second));
    assert_eq!(std::fs::read(&script_path).unwrap(), first_bytes, "a second run must not rewrite the pre-commit script");
    assert_eq!(std::fs::read(dir.path().join(".claude/settings.json")).unwrap(), settings_bytes_first, "a second run must not rewrite settings.json");
}

#[test]
fn gate_install_hooks_with_no_command_flag_wires_the_evidence_gated_task_flip() {
    // gated-task-completion spec.md "Hook-seam wiring generation": the
    // installed hook-seam entry MUST invoke `canon gate task` (the
    // evidence-gated flip entry point), never the read-only `canon gate
    // check`, when `--command` is omitted entirely.
    let dir = tempfile::tempdir().unwrap();

    let output = run_canon(&["gate", "install-hooks", "--repo", "."], dir.path());
    assert!(output.status.success(), "stderr: {}", stderr(&output));

    let settings: serde_json::Value = serde_json::from_str(&std::fs::read_to_string(dir.path().join(".claude/settings.json")).unwrap()).unwrap();
    let command = settings["hooks"]["PreToolUse"][0]["hooks"][0]["command"].as_str().unwrap();
    assert_eq!(command, "canon gate task", "the documented install path must wire the evidence-gated flip, not check-only");
}

// ── canon gate selftest ──

#[test]
fn gate_selftest_exits_zero_against_the_shipped_fixture_corpus() {
    let dir = tempfile::tempdir().unwrap();
    let output = run_canon(&["gate", "selftest"], dir.path());
    assert!(output.status.success(), "stdout: {}\nstderr: {}", stdout(&output), stderr(&output));
    let text = stdout(&output);
    for class in ["uncovered-cell", "unreviewed-promotion", "trust-below-required", "stale-evidence", "malformed-evidence", "flagged", "unevidenced-flip", "fabricated-evidence"] {
        assert!(text.contains(&format!("ok    {class}")), "expected `{class}` to report ok:\n{text}");
    }
}
