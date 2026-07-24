//! S10 part2 pilot (task 7.1, design.md "one consumer repo authors a real
//! change with it"): proves the typed task-atom mechanism against a REAL,
//! non-canon-vocab-fixture consumer change —
//! `openspec/changes/s10-vocab-pilot/tasks.vocab.yaml`, resolved against
//! THIS repo's own real, checked-in `.canon/vocab/canon.core/` +
//! `.canon/policy.yaml` (not a tempdir copy, unlike
//! `canon_core_selftest.rs`'s own fixture-corpus proof). Read-only —
//! never mutates the real repo; the pilot's own `canon gate task` flip
//! was performed once, manually, against the real binary, and its result
//! (the flipped `tasks.md` row + the ledger `EvidenceRecord`) is already
//! committed (`openspec/changes/s10-vocab-pilot/proposal.md`).

use std::path::{Path, PathBuf};

fn repo_root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).parent().unwrap().parent().unwrap().to_path_buf()
}

fn pilot_atoms_file() -> String {
    std::fs::read_to_string(repo_root().join("openspec/changes/s10-vocab-pilot/tasks.vocab.yaml")).expect("read the real pilot tasks.vocab.yaml")
}

#[test]
fn the_pilot_atom_validates_against_the_real_repo_snapshot() {
    let (snapshot, diags) = canon_vocab::resolve_snapshot(&repo_root(), None);
    assert!(diags.is_empty(), "the real repo's vocabulary must resolve clean: {diags:?}");

    let atoms = canon_vocab::atom::parse_atoms_file(&pilot_atoms_file()).expect("the pilot's tasks.vocab.yaml parses");
    assert_eq!(atoms.len(), 1, "the pilot declares exactly one typed task atom");

    let atom_diags = canon_vocab::atom::validate_atoms(&atoms, &snapshot);
    assert!(atom_diags.is_empty(), "the pilot atom must validate against the real repo: {atom_diags:?}");
}

#[test]
fn the_pilot_atom_compiles_to_the_s1_task_model_and_round_trips() {
    use canon_model::{Actor, Envelope, RecordKind, RoleId};

    let (snapshot, _) = canon_vocab::resolve_snapshot(&repo_root(), None);
    let atoms = canon_vocab::atom::parse_atoms_file(&pilot_atoms_file()).unwrap();
    let atom = &atoms[0];
    assert_eq!(atom.id, "s10-vocab-pilot#1");
    assert_eq!(atom.tag, "task");

    let envelope = Envelope::new(1, RecordKind::Task, chrono::Utc::now(), Actor::new("test-runner", RoleId::parse("implementer").unwrap()));
    let task = canon_vocab::compile_task(atom, &snapshot, envelope.clone()).expect("the pilot atom compiles to an S1 Task");
    assert_eq!(task.task_id.to_string(), "s10-vocab-pilot#1");
    assert_eq!(task.title, "author canon/policy.yaml declaring the evidence-kind domain canon's typed authoring vocabulary resolves Type::Evidence against");

    let decompiled = canon_vocab::decompile_task(&task).expect("decompiles");
    assert_eq!(decompiled.id, atom.id);
    assert_eq!(decompiled.tag, atom.tag);
    assert_eq!(decompiled.attrs, atom.attrs);
    assert!(canon_vocab::atom::validate_atoms(std::slice::from_ref(&decompiled), &snapshot).is_empty(), "the decompiled atom must itself validate");

    let task2 = canon_vocab::compile_task(&decompiled, &snapshot, envelope).expect("recompiles");
    assert_eq!(task.task_id, task2.task_id);
    assert_eq!(task.title, task2.title);
    assert_eq!(task.status, task2.status);
    assert_eq!(task.evidence_note, task2.evidence_note);
}

/// Proves the pilot's evidence requirement (`evidence: {kind: test-run,
/// ref: ...}`) genuinely resolves against the REAL repo's policy-derived
/// domain, not merely "some string" — the whole point of design.md D4.
#[test]
fn the_pilot_atoms_declared_evidence_kind_is_in_the_real_policy_derived_domain() {
    let (snapshot, _) = canon_vocab::resolve_snapshot(&repo_root(), None);
    assert!(snapshot.evidence_kinds.contains(&"test-run".to_string()), "evidence_kinds: {:?}", snapshot.evidence_kinds);
}

/// The pilot's own `tasks.md` checkbox row was gated and flipped for real
/// via `canon gate task s10-vocab-pilot#1 --repo .` (this repo's own
/// committed proof — a fresh evidence-record write/gate run is
/// `crates/canon-cli/tests/gate.rs`'s own territory, never re-performed
/// here against the real repo's ledger).
#[test]
fn the_pilot_tasks_md_row_was_gated_and_flipped_for_real() {
    let text = std::fs::read_to_string(repo_root().join("openspec/changes/s10-vocab-pilot/tasks.md")).unwrap();
    assert!(text.contains("- [x] 1 Author `canon/policy.yaml`"), "{text}");
    assert!(text.contains("✅ Faithful evidence recorded"), "{text}");
}
