//! A 0.30 brief, seen by canon 0.31 (0.31 `brief-format`, F4).
//!
//! A brief that still carries `<!-- canon:section|hint|waive|kind -->`
//! comments or a shipped Korean heading is in the format canon 0.30 and
//! earlier wrote ([`canon_ingest::brief::OldFormat`]). `canon migrate`
//! rewrites it. Until then:
//!
//! - a command that reads (`status`, `gate check`, `context`, `brief show`)
//!   warns ([`announce`]), with no config error, and puts `canon migrate
//!   --json` first in `meta.next`;
//! - a command that writes and judges such a brief is refused with the same
//!   hint and writes nothing ([`refusal`]): `evidence add` and `subject
//!   move` for the changes the subject answers for, `scenario new --flow`
//!   for the change whose flow it reads, `review change` for the chore it
//!   pins, `subject adopt` for the change it adopts. A brief the command does
//!   not judge never blocks it.
//!
//! An old brief whose `Approved:` digest still matches reads as approved:
//! the detection does not change an approval.

use std::path::Path;

use canon_model::{paths, ChangeId};

use crate::cli_error::CliError;
use crate::status::{NextStep, StepActor};

/// The code of the refusal.
pub const CODE: &str = "OLD_BRIEF_FORMAT";

/// One old-format brief.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OldBrief {
    pub change: String,
    /// The brief, relative to the repo root.
    pub path: String,
    /// What was found in it, in words.
    pub what: String,
}

/// The warning of a read command and the reason of a refused write, for `(change, what)` pairs.
pub fn message(found: &[(&str, &str)]) -> String {
    let each = found.iter().map(|(change, what)| format!("the brief of change `{change}` ({what})")).collect::<Vec<_>>().join("; ");
    format!(
        "{each} is in the format canon 0.30 and earlier wrote; canon 0.31 finds sections by their English `##` heading and keeps waivers in the frontmatter. Hint: run `canon migrate --json` to preview the rewrite (a bare run only previews, and `--commit --plan-digest <digest>` applies it; the human runs it when it re-binds an `Approved:` line)"
    )
}

fn read(repo: &Path, change: &ChangeId) -> Option<OldBrief> {
    let path = crate::brief::canon_brief(repo, change)?;
    let text = std::fs::read_to_string(&path).ok()?;
    let old = canon_ingest::brief::parse(&text).old_format?;
    Some(OldBrief { change: change.as_str().to_string(), path: path.strip_prefix(repo).unwrap_or(&path).display().to_string(), what: old.describe() })
}

/// The old-format briefs among `changes`' (a change with no brief has none).
pub fn among(repo: &Path, changes: &[ChangeId]) -> Vec<OldBrief> {
    changes.iter().filter_map(|change| read(repo, change)).collect()
}

/// Every old-format brief under `.canon/changes/` and its `archive/`, by change id.
pub fn all(repo: &Path) -> Vec<OldBrief> {
    let mut found = Vec::new();
    for base in [repo.join(paths::CHANGES_DIR), repo.join(paths::CHANGES_DIR).join("archive")] {
        let Ok(entries) = std::fs::read_dir(&base) else {
            continue;
        };
        let mut dirs: Vec<_> = entries.filter_map(Result::ok).filter(|e| e.path().join("brief.md").is_file()).collect();
        dirs.sort_by_key(std::fs::DirEntry::file_name);
        for entry in dirs {
            let Some(change) = entry.file_name().to_str().and_then(|name| ChangeId::parse(name).ok()) else {
                continue;
            };
            if let Some(brief) = read(repo, &change) {
                found.push(brief);
            }
        }
    }
    found
}

/// The refusal of a write that judges `found`, or `None` when none is old-format.
pub fn refusal(found: &[OldBrief]) -> Option<CliError> {
    if found.is_empty() {
        return None;
    }
    let pairs: Vec<(&str, &str)> = found.iter().map(|b| (b.change.as_str(), b.what.as_str())).collect();
    Some(
        CliError::refused(CODE, message(&pairs))
            .hint("run `canon migrate --json` to preview the rewrite, then the human runs `canon migrate --commit --plan-digest <digest>` when it re-binds an approval; nothing was written"),
    )
}

/// [`refusal`] for the briefs of `changes`.
pub fn refusal_among(repo: &Path, changes: &[ChangeId]) -> Option<CliError> {
    refusal(&among(repo, changes))
}

/// The `canon migrate --json` step that comes first in `meta.next`.
pub fn migrate_step(found: &[OldBrief]) -> NextStep {
    NextStep {
        command: "canon migrate --json".into(),
        why: format!("{} brief(s) are in the format canon 0.30 wrote; the preview lists what it rewrites, whether it re-binds an `Approved:` line (then the human runs the commit) and the exact command that applies it", found.len()),
        actor: StepActor::Agent,
        role: Some(crate::role::Role::Orchestrator),
    }
}

/// For `status`, `gate check`, `context` and `brief show`: warns when a brief
/// is old-format and puts `canon migrate --json` first in `meta.next`.
pub fn announce(repo: &Path) {
    let found = all(repo);
    if found.is_empty() {
        return;
    }
    let names = found.iter().take(3).map(|b| format!("`{}`", b.change)).collect::<Vec<_>>().join(", ");
    let more = if found.len() > 3 { format!(" and {} more", found.len() - 3) } else { String::new() };
    crate::json_out::warn(format!(
        "{} brief(s) are in the format canon 0.30 and earlier wrote ({names}{more}); commands that write and judge them are refused until `canon migrate` rewrites them. Hint: run `canon migrate --json` to preview the rewrite",
        found.len()
    ));
    crate::json_out::prepend_next(migrate_step(&found));
}

#[cfg(test)]
mod tests {
    use super::*;

    fn brief(root: &Path, change: &str, text: &str) {
        let dir = root.join(paths::CHANGES_DIR).join(change);
        std::fs::create_dir_all(&dir).unwrap();
        std::fs::write(dir.join("brief.md"), text).unwrap();
    }

    /// brief-format F4: only a brief with a `canon:` marker or a shipped Korean heading is old; the
    /// approval note comment is not a marker.
    #[test]
    fn only_markers_and_shipped_korean_headings_make_a_brief_old() {
        let dir = tempfile::tempdir().unwrap();
        brief(dir.path(), "new", "# t\n\n## Intent\n\nx\n\n## Out of scope\n\ny\n\n<!-- The human records their answer -->\n");
        brief(dir.path(), "marked", "# t\n\n<!-- canon:section intent role=context -->\n## Intent\n\nx\n");
        brief(dir.path(), "korean", "# t\n\n## 의도\n\nx\n");
        let found = all(dir.path());
        assert_eq!(found.iter().map(|b| b.change.as_str()).collect::<Vec<_>>(), vec!["korean", "marked"]);
        assert!(found[0].path.ends_with("korean/brief.md"));
        let refused = refusal_among(dir.path(), &[ChangeId::parse("new").unwrap()]);
        assert!(refused.is_none(), "a change the command does not judge, or a new brief, never blocks");
        let refused = refusal_among(dir.path(), &[ChangeId::parse("marked").unwrap(), ChangeId::parse("new").unwrap()]).unwrap();
        assert_eq!(refused.code, CODE);
        assert!(refused.message.contains("canon migrate"), "{}", refused.message);
    }
}
