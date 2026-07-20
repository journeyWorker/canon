//! s35 `gate-plan-dialect-seam`: the `PlanWriteBack` seam, exercised
//! per dialect through the SAME `plan_registry` lookup `canon gate task`
//! uses — locate/flip round-trip (openspec), the loud
//! `WriteBackUnsupported` for a dialect that cannot flip (superpowers),
//! and the typed-atoms-path layout resolution.

use std::fs;

use canon_ingest::{find_plan_adapter, PlanWriteBack, WriteBackError};
use canon_model::ids::{ChangeId, TaskId};

fn openspec_wb() -> &'static dyn PlanWriteBack {
    find_plan_adapter("openspec").expect("openspec is registered").write_back.expect("openspec ships a write-back")
}

fn superpowers_wb() -> &'static dyn PlanWriteBack {
    find_plan_adapter("superpowers").expect("superpowers is registered").write_back.expect("superpowers ships a write-back")
}

// ── openspec dialect: locate + flip round-trip ──

#[test]
fn openspec_locates_and_flips_a_task_row_round_trip() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    let dir = root.join("openspec/changes/demo-change");
    fs::create_dir_all(&dir).unwrap();
    fs::write(dir.join("tasks.md"), "- [ ] 1 Do the thing\n- [ ] 2 Do another\n").unwrap();
    let task_id = TaskId::parse("demo-change#1").unwrap();
    let wb = openspec_wb();

    let loc = wb.locate_task(root, &task_id).expect("locates the change's tasks.md");
    assert_eq!(loc.document_path, dir.join("tasks.md"));

    let doc = fs::read_to_string(&loc.document_path).unwrap();
    let out = wb.flip_task(&doc, &task_id, "cargo test: 3 passed").expect("row exists and is open");
    assert!(out.flipped);
    assert_eq!(
        out.document,
        "- [x] 1 Do the thing — ✅ cargo test: 3 passed\n- [ ] 2 Do another\n",
        "only the matched row flips; every other line stays byte-identical"
    );

    // Idempotent: flipping the already-`[x]` row is a byte-identical no-op.
    let again = wb.flip_task(&out.document, &task_id, "ignored second note").expect("row exists");
    assert!(!again.flipped);
    assert_eq!(again.document, out.document);
}

#[test]
fn openspec_flip_reports_row_not_found_for_an_absent_row() {
    let wb = openspec_wb();
    let task_id = TaskId::parse("demo-change#99").unwrap();
    let err = wb.flip_task("- [ ] 1 Only row\n", &task_id, "note").unwrap_err();
    assert_eq!(err, WriteBackError::RowNotFound(task_id));
    // The CLI's stderr contract depends on this substring (pre-s35 compat).
    assert!(err.to_string().contains("no matching row"), "{err}");
}

#[test]
fn openspec_locate_is_none_when_the_change_dir_is_absent() {
    let tmp = tempfile::tempdir().unwrap();
    let wb = openspec_wb();
    let task_id = TaskId::parse("no-such-change#1").unwrap();
    assert!(wb.locate_task(tmp.path(), &task_id).is_none());
}

#[test]
fn openspec_typed_atoms_path_is_the_tasks_vocab_sibling() {
    let tmp = tempfile::tempdir().unwrap();
    let wb = openspec_wb();
    let change_id = ChangeId::parse("demo-change").unwrap();
    let path = wb.typed_atoms_path(tmp.path(), &change_id).expect("openspec has a typed-atoms convention");
    assert_eq!(path, tmp.path().join("openspec/changes/demo-change/tasks.vocab.yaml"));
}

// ── superpowers dialect: locate works, flip is loudly unsupported ──

#[test]
fn superpowers_locates_a_plan_doc_by_slug_but_flip_is_unsupported() {
    let tmp = tempfile::tempdir().unwrap();
    let root = tmp.path();
    fs::write(root.join("demo-change.md"), "# Demo\n\n**Goal:** ship it.\n\n### Task 1: Wire\n- [ ] step\n").unwrap();
    let wb = superpowers_wb();
    let task_id = TaskId::parse("demo-change#1").unwrap();

    let loc = wb.locate_task(root, &task_id).expect("locates the plan doc by slugified stem");
    assert_eq!(loc.document_path, root.join("demo-change.md"));

    // The flip is a loud, typed refusal naming the dialect — never a
    // silent no-op an operator would mistake for a landed flip.
    let err = wb.flip_task("whatever the document is", &task_id, "note").unwrap_err();
    assert_eq!(err, WriteBackError::Unsupported { dialect: "superpowers" });
    assert!(err.to_string().contains("WriteBackUnsupported"), "{err}");
    assert!(err.to_string().contains("superpowers"), "{err}");
}

#[test]
fn superpowers_has_no_typed_atoms_convention() {
    let tmp = tempfile::tempdir().unwrap();
    let wb = superpowers_wb();
    let change_id = ChangeId::parse("demo-change").unwrap();
    assert!(wb.typed_atoms_path(tmp.path(), &change_id).is_none());
}
