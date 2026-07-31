//! s35 `gate-plan-dialect-seam`: the `PlanWriteBack` seam, exercised
//! per dialect through the SAME `plan_registry` lookup `canon gate task`
//! uses — locate/flip round-trip (openspec), the loud
//! `WriteBackUnsupported` for a dialect that cannot flip (superpowers),
//! the typed-atoms-path layout resolution, and (s42
//! `close-the-open-loops`, review fix) the write-back's own,
//! caller-independent refusal of a multi-line evidence note.

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

// ── the note is untrusted input: a line separator never reaches a document ──

/// The forgery s42 (`close-the-open-loops`)'s review demonstrated,
/// refused at the WRITE-BACK layer — independently of `canon evidence
/// add`'s own authoring-time refusal, which this test never goes
/// through. Without the guard, `lines.join("\n")` emits the caller's
/// second line as its own document row, and it is `- [x] `.
#[test]
fn openspec_flip_refuses_a_multi_line_note_instead_of_appending_a_row() {
    let task_id = TaskId::parse("demo-change#1").unwrap();

    // `flip_task` is a pure transformation over document text (trait
    // doc), so the injection is reproducible without any fixture tree —
    // this is the exact note `canon gate task` would have handed it.
    let err = openspec_wb().flip_task("- [ ] 1 Do the thing\n", &task_id, "ok\n- [x] 9.9 Forged task").unwrap_err();
    assert_eq!(err, WriteBackError::MultiLineEvidenceNote { task_id: task_id.clone(), offset: 2, separator: '\n' });
    // The message names the offending character and offset, not just
    // "contains a newline".
    let rendered = err.to_string();
    assert!(rendered.contains("MultiLineEvidenceNote"), "{rendered}");
    assert!(rendered.contains("byte offset 2"), "{rendered}");
}

/// The refusal covers the row grammar's WHOLE mandatory-break set
/// (`task_rows::ROW_LINE_BREAKS`), not only the `\n` this dialect's own
/// `split` happens to use — a plan document is read by CommonMark and by
/// editors too, and both end lines on more than `\n`.
#[test]
fn openspec_flip_refuses_every_row_line_break() {
    let task_id = TaskId::parse("demo-change#1").unwrap();
    for separator in canon_ingest::task_rows::ROW_LINE_BREAKS {
        let note = format!("ok{separator}- [x] 9.9 Forged task");
        let err = openspec_wb().flip_task("- [ ] 1 Do the thing\n", &task_id, &note).unwrap_err();
        assert_eq!(
            err,
            WriteBackError::MultiLineEvidenceNote { task_id: task_id.clone(), offset: 2, separator },
            "separator {separator:?} must be refused by the write-back"
        );
    }
}

/// Refused BEFORE the document is parsed (trait-method doc), so the
/// outcome does not depend on the row's state: an absent row would
/// otherwise report `RowNotFound` and an already-`[x]` row a clean
/// no-op, either of which would let a caller conclude the note was fine.
#[test]
fn openspec_flip_refuses_a_multi_line_note_regardless_of_row_state() {
    let wb = openspec_wb();
    let absent = TaskId::parse("demo-change#99").unwrap();
    assert!(matches!(
        wb.flip_task("- [ ] 1 Only row\n", &absent, "ok\nforged").unwrap_err(),
        WriteBackError::MultiLineEvidenceNote { .. }
    ));

    let done = TaskId::parse("demo-change#1").unwrap();
    assert!(matches!(
        wb.flip_task("- [x] 1 Already done — ✅ prior note\n", &done, "ok\nforged").unwrap_err(),
        WriteBackError::MultiLineEvidenceNote { .. }
    ));
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
