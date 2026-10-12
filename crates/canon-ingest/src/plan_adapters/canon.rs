//! The `PlanAdapter` for canon's own plan source, `.canon/changes/`
//! (0.17 D2): one `<slug>/` directory per change, holding `brief.md`
//! (the openspec proposal's sections; the human's `Approved:` line
//! lives there) and `tasks.md` (the openspec row grammar). `canon change
//! new` writes it, and every plan reader reads it with no `plans:`
//! entry ([`crate::plan_registry::native`]).
//!
//! The change-dir grammar is the openspec dialect's
//! ([`super::openspec::ChangeDirLayout`]), with three differences: the
//! brief is `brief.md`, the source root is always the changes dir
//! itself, and a `brief.md` carrying a `Source: <path>` line is not a
//! change. That file is the canon brief of a change imported from an
//! external plan (0.17 D3): it points at the external document and
//! holds the approval, while the change itself, its summary and its
//! tasks come from that external source. Such a directory is invisible
//! here, so the external source keeps owning the change id.
//!
//! Every record this dialect emits carries the actor
//! `canon-plan-import-canon`: a `Change` with that actor was written by
//! canon (`canon change new`), never by an external plan.

use std::path::{Path, PathBuf};

use canon_model::ids::{ChangeId, TaskId};

use super::openspec::{self, ChangeDirLayout};
use crate::plan_adapter::{PlanAdapter, PlanParseOutcome, PlanSourceConfig, PlanSourceHandle, resolve_path_source};
use crate::plan_registry::NATIVE_DIALECT;
use crate::plan_writeback::{FlipDocOutcome, PlanTaskLocation, PlanWriteBack, WriteBackError};

/// Canon's own change-dir layout (module doc).
const CANON_LAYOUT: ChangeDirLayout = ChangeDirLayout {
    brief: "brief.md",
    missing_brief: "missing-brief-md",
    actor: "canon-plan-import-canon",
    nested: false,
    pointer_briefs: true,
};

pub struct CanonPlanAdapter;

impl PlanAdapter for CanonPlanAdapter {
    fn dialect_id(&self) -> &'static str {
        NATIVE_DIALECT
    }

    fn parse_version(&self) -> u32 {
        1
    }

    fn resolve_source(&self, config: &PlanSourceConfig) -> Option<PlanSourceHandle> {
        resolve_path_source(&config.root)
    }

    fn parse(&self, source: &PlanSourceHandle) -> PlanParseOutcome {
        let PlanSourceHandle::Path(root) = source;
        openspec::parse_root(&CANON_LAYOUT, root)
    }

    fn source_files(&self, source: &PlanSourceHandle) -> Vec<PathBuf> {
        let PlanSourceHandle::Path(root) = source;
        openspec::layout_source_files(&CANON_LAYOUT, root)
    }

    fn parse_change(&self, source: &PlanSourceHandle, change_id: &ChangeId) -> PlanParseOutcome {
        let PlanSourceHandle::Path(root) = source;
        openspec::parse_one(&CANON_LAYOUT, root, change_id)
    }
}

/// `canon gate task` flips a row of `.canon/changes/<change>/tasks.md`
/// exactly as it flips an openspec `tasks.md` row.
impl PlanWriteBack for CanonPlanAdapter {
    fn locate_task(&self, root: &Path, task_id: &TaskId) -> Option<PlanTaskLocation> {
        openspec::locate(&CANON_LAYOUT, root, task_id)
    }

    fn flip_task(&self, document: &str, task_id: &TaskId, evidence_note: &str) -> Result<FlipDocOutcome, WriteBackError> {
        openspec::flip(document, task_id, evidence_note)
    }

    fn typed_atoms_path(&self, root: &Path, change_id: &ChangeId) -> Option<PathBuf> {
        Some(openspec::change_dir(&CANON_LAYOUT, root, change_id).join("tasks.vocab.yaml"))
    }
}

#[cfg(test)]
mod tests {
    use std::fs;

    use canon_model::records::{ChangeStatus, TaskStatus};

    use super::*;

    fn id(s: &str) -> ChangeId {
        ChangeId::parse(s).expect("valid change id")
    }

    fn write(root: &Path, rel: &str, text: &str) {
        let path = root.join(rel);
        fs::create_dir_all(path.parent().unwrap()).unwrap();
        fs::write(path, text).unwrap();
    }

    fn parse(root: &Path) -> PlanParseOutcome {
        CanonPlanAdapter.parse(&PlanSourceHandle::Path(root.to_path_buf()))
    }

    #[test]
    fn a_change_dir_imports_from_brief_md_with_canon_provenance() {
        let tmp = tempfile::tempdir().unwrap();
        write(tmp.path(), "add-login/brief.md", "# Add login\n\n## Why\n\nUsers sign in.\n");
        write(tmp.path(), "add-login/tasks.md", "# Tasks\n\n- [x] 1 Write the scenarios\n- [ ] 2 Implement\n");
        let outcome = parse(tmp.path());
        assert!(outcome.malformed.is_empty(), "{:?}", outcome.malformed);
        let change = &outcome.changes[0];
        assert_eq!(change.change_id.as_str(), "add-login");
        assert_eq!(change.summary, "Users sign in.");
        assert_eq!(change.status, ChangeStatus::InProgress);
        assert_eq!(change.envelope.actor.agent_id, "canon-plan-import-canon");
        let statuses: Vec<_> = outcome.tasks.iter().map(|t| (t.task_id.as_str().to_string(), t.status)).collect();
        assert_eq!(statuses, vec![("add-login#1".to_string(), TaskStatus::Done), ("add-login#2".to_string(), TaskStatus::Open)]);
        assert!(outcome.tasks.iter().all(|t| t.envelope.actor.agent_id == "canon-plan-import-canon"));
    }

    #[test]
    fn a_pointer_brief_is_not_a_change_and_locates_no_task() {
        let tmp = tempfile::tempdir().unwrap();
        write(tmp.path(), "ext-change/brief.md", "# Brief\n\nSource: openspec/changes/ext-change/proposal.md\n\nApproved: ana 2026-10-12 — \"go\"\n");
        write(tmp.path(), "ext-change/tasks.md", "- [ ] 1 Stray row\n");
        write(tmp.path(), "own/brief.md", "## Why\n\nOwn.\n<!-- Source: not a pointer, it is commented out -->\n");
        let outcome = parse(tmp.path());
        let ids: Vec<&str> = outcome.changes.iter().map(|c| c.change_id.as_str()).collect();
        assert_eq!(ids, vec!["own"], "the pointer brief's dir is invisible; a commented `Source:` is not one");
        assert!(outcome.malformed.is_empty());
        let task = TaskId::parse("ext-change#1").unwrap();
        assert!(CanonPlanAdapter.locate_task(tmp.path(), &task).is_none());
        let files = CanonPlanAdapter.source_files(&PlanSourceHandle::Path(tmp.path().to_path_buf()));
        assert_eq!(files, vec![tmp.path().join("own/brief.md")]);
    }

    #[test]
    fn a_dir_without_brief_md_is_named_malformed() {
        let tmp = tempfile::tempdir().unwrap();
        write(tmp.path(), "half/proposal.md", "## Why\n\nWrong file.\n");
        let outcome = parse(tmp.path());
        assert!(outcome.changes.is_empty());
        assert_eq!(outcome.malformed[0].reason, "missing-brief-md");
        assert_eq!(outcome.malformed[0].path, "half");
    }

    #[test]
    fn the_root_is_the_changes_dir_even_when_it_holds_openspec_changes() {
        let tmp = tempfile::tempdir().unwrap();
        write(tmp.path(), "openspec/changes/x/proposal.md", "## Why\n\nx\n");
        write(tmp.path(), "mine/brief.md", "## Why\n\nmine\n");
        let outcome = CanonPlanAdapter.parse_change(&PlanSourceHandle::Path(tmp.path().to_path_buf()), &id("mine"));
        assert_eq!(outcome.changes.len(), 1);
        assert_eq!(openspec::change_dir(&CANON_LAYOUT, tmp.path(), &id("x")), tmp.path().join("x"));
    }

    #[test]
    fn gate_task_locates_and_flips_a_canon_tasks_md_row() {
        let tmp = tempfile::tempdir().unwrap();
        write(tmp.path(), "add-login/brief.md", "## Why\n\nx\n");
        write(tmp.path(), "add-login/tasks.md", "- [ ] 1 Implement\n");
        let task = TaskId::parse("add-login#1").unwrap();
        let located = CanonPlanAdapter.locate_task(tmp.path(), &task).expect("located");
        assert_eq!(located.document_path, tmp.path().join("add-login/tasks.md"));
        let flipped = CanonPlanAdapter.flip_task("- [ ] 1 Implement\n", &task, "evidence ok").unwrap();
        assert!(flipped.flipped);
        assert!(flipped.document.starts_with("- [x] 1 Implement"), "{}", flipped.document);
    }
}
