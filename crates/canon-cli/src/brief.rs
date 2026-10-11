//! Whether a subject's brief is approved (0.16 D1): the one decision
//! `canon status` reports and the `verifying → shipped` guard of
//! `canon subject status` enforces. Both call [`brief_of`], so status
//! never says `ship` for a subject the guard would refuse on its brief,
//! and the guard never refuses a subject status reads as needing no
//! brief.
//!
//! One human answer approves a subject's scope: its brief and the
//! policy. The agent records it, from that answer only, as an
//! `Approved:` line in the change's `proposal.md`, read under the
//! openspec plan sources (no record kind), outside HTML comments and
//! code fences. Fixes live as tasks in the approved change, so the
//! subject's newest adopted change is its current scope.

use std::path::{Path, PathBuf};

use canon_model::{Subject, SubjectStatus};
use serde::Serialize;

/// Whether a subject's newest adopted change records the human's
/// approval of its brief: an `Approved:` line in that change's
/// `proposal.md`, read under the repo's `openspec` plan sources. A fix
/// lives as tasks inside the approved change, so a newer adopted change
/// is a new scope, and an older change's approval does not cover it.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BriefStatus {
    pub approved: bool,
    /// The newest adopted change (the last in `change_ids`, adoption
    /// order), whose `proposal.md` must carry the approval. Omitted when
    /// the subject adopts no change.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub change: Option<String>,
    /// That change's `proposal.md`, relative to the repo root. Omitted
    /// when no `openspec` plan source holds one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub proposal: Option<String>,
    /// The approval line's text after `Approved:`.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub approval: Option<String>,
}

/// The `openspec` plan source roots briefs are read under, resolved the
/// way `canon gate task` resolves plan sources (an absent `plans:`
/// section falls back to the repo root). `Err` names why `canon.yaml`'s
/// `plans:` section cannot be read; then no brief reads as approved.
pub fn plan_roots(repo: &Path) -> Result<Vec<PathBuf>, String> {
    crate::plans::load_plan_sources_for_gate(repo)
        .map(|sources| sources.iter().filter(|source| source.dialect() == "openspec").map(|source| source.root().to_path_buf()).collect())
        .map_err(|e| e.to_string())
}

/// The brief decision for `subject`: `None` for a subject that needs no
/// brief yet (`proposed`, `specced`, `retired`), otherwise whether its
/// newest adopted change (the last in `change_ids`, adoption order)
/// carries an `Approved:` line in its `proposal.md` under `plan_roots`
/// (the first root holding the change wins, as in `canon ingest
/// plans`). A fix lives as tasks inside the approved change, so a newer
/// adopted change is a new scope that an older change's approval does
/// not cover.
pub fn brief_of(repo: &Path, plan_roots: &[PathBuf], subject: &Subject) -> Option<BriefStatus> {
    if !matches!(subject.status, SubjectStatus::Building | SubjectStatus::Verifying | SubjectStatus::Shipped) {
        return None;
    }
    let Some(change) = subject.change_ids.last() else {
        return Some(BriefStatus { approved: false, change: None, proposal: None, approval: None });
    };
    let path = plan_roots.iter().find_map(|root| canon_ingest::plan_adapters::openspec::proposal_path(root, change));
    let text = path.as_ref().and_then(|path| std::fs::read_to_string(path).ok()).unwrap_or_default();
    let approval = approval_line(&text).map(str::to_string);
    Some(BriefStatus {
        approved: approval.is_some(),
        change: Some(change.as_str().to_string()),
        proposal: path.map(|path| path.strip_prefix(repo).unwrap_or(&path).display().to_string()),
        approval,
    })
}

/// The text after `Approved:` on the first line of `proposal` that
/// starts with it (leading whitespace allowed), says something, and sits
/// outside fenced code blocks (the same fence rule `canon init` reads
/// AGENTS.md with) and HTML comments, which may span lines: a quoted or
/// commented-out approval is not one.
fn approval_line(proposal: &str) -> Option<&str> {
    let mut fence: Option<(char, usize)> = None;
    let mut in_comment = false;
    for line in proposal.lines() {
        if !in_comment {
            match (fence, crate::init::fence_of(line)) {
                (None, Some((ch, run, _))) => {
                    fence = Some((ch, run));
                    continue;
                }
                (Some((open_ch, open_run)), Some((ch, run, true))) if ch == open_ch && run >= open_run => {
                    fence = None;
                    continue;
                }
                (Some(_), _) => continue,
                (None, None) => {}
            }
            if let Some(approval) = line.trim_start().strip_prefix("Approved:").map(str::trim).filter(|rest| !rest.is_empty()) {
                return Some(approval);
            }
        }
        in_comment = comment_open_after(line, in_comment);
    }
    None
}

/// Whether an HTML comment is open at the end of `line`, given whether
/// one was open at its start.
fn comment_open_after(line: &str, mut open: bool) -> bool {
    let mut rest = line;
    loop {
        let marker = if open { "-->" } else { "<!--" };
        let Some(at) = rest.find(marker) else {
            return open;
        };
        rest = &rest[at + marker.len()..];
        open = !open;
    }
}
