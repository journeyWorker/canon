//! `canon change new <slug> --subject <id> --title <t>` (0.14 D2): the
//! official path to a plan change. It scaffolds an openspec change dir
//! (`proposal.md` + `tasks.md`) under the repo's configured `openspec`
//! plans source, reads it with the same `openspec` plan adapter `canon
//! ingest plans` uses, and records it adopted into the subject through
//! the same write `canon subject adopt` performs
//! ([`crate::subject::persist_adoption`]). Nothing in `canon.yaml` needs
//! a hand edit: `canon init` configures the source.
//!
//! # What a failure leaves behind
//! The files are staged in a private directory under `.canon/` and
//! parsed there; only when that succeeds is the change dir moved into
//! place, and only then are the records written. A failure before any
//! record is written removes the staged files, the published change dir
//! and any directory created for it: nothing is left behind. The two
//! records (subject, then change; see
//! [`crate::subject::persist_adoption`] for the order) cannot be written
//! atomically, so a failure between them keeps the change dir and the
//! subject record and prints the command that completes the link,
//! `canon ingest plans && canon subject adopt <slug> --subject <id>`,
//! which is safe to rerun.
//!
//! # Refusals (exit `2`, nothing written)
//! - no `openspec` source in `canon.yaml`'s `plans.sources`;
//! - an unknown subject;
//! - a slug that already has a change dir (active or archived) or a
//!   `change` record.

use std::fs;
use std::path::{Path, PathBuf};

use canon_ingest::{find_plan_adapter, PlanSourceHandle};
use canon_model::{paths, Change, ChangeId, SubjectId};

use crate::context::{resolve_canon_yaml, resolve_repo_root};

/// Exit code for a refused invocation or a failed write — mirrors
/// `canon subject`'s own `2`.
const EXIT_REFUSED: i32 = 2;

/// The plan dialect this command scaffolds.
const DIALECT: &str = "openspec";

/// The directory the openspec adapter reads change dirs from for a
/// source rooted at `root`, mirroring its discovery order
/// (`canon_ingest::plan_adapters::openspec`): `<root>/openspec/changes`
/// when it exists; otherwise `root` itself when it is a changes dir
/// passed directly (named `changes`, or already holding a change dir
/// with a `proposal.md`); otherwise `<root>/openspec/changes`, created
/// by the write.
fn changes_dir(root: &Path) -> PathBuf {
    let canonical = root.join("openspec").join("changes");
    if canonical.is_dir() {
        return canonical;
    }
    let direct = root.file_name().is_some_and(|name| name == "changes")
        || fs::read_dir(root).is_ok_and(|entries| entries.filter_map(Result::ok).any(|entry| entry.path().join("proposal.md").is_file()));
    if direct { root.to_path_buf() } else { canonical }
}

/// `proposal.md`: the `## Why` paragraph is the title, which the
/// openspec dialect imports as `Change.summary`.
fn proposal_md(title: &str) -> String {
    format!(
        "# {title}\n\n## Why\n\n{title}\n\n## What Changes\n\n<!-- What this change adds or changes, and the scenarios it covers (`@subject` tag, failure paths included). -->\n"
    )
}

/// `tasks.md`: no rows yet, so the change imports as `proposed`. The
/// row grammar is named in a comment, which the row parser never reads.
fn tasks_md(title: &str) -> String {
    format!(
        "# Tasks: {title}\n\n<!-- One checkbox row per task, numbered: `- [ ] 1 Write the failure-path scenarios`. Each row imports as task `<slug>#<n>`; flip it with `canon gate task` once evidence exists, never by hand. -->\n"
    )
}

/// `path` relative to `repo` for output, or as-is when it is not under it.
fn display_rel(path: &Path, repo: &Path) -> String {
    path.strip_prefix(repo).unwrap_or(path).display().to_string()
}

/// A directory this command created and removes again unless
/// [`Created::keep`] is called — the staging dir always, the published
/// change dir (or the first ancestor of it this command created) on
/// every failure path.
struct Created(Option<PathBuf>);

impl Created {
    fn keep(mut self) {
        self.0 = None;
    }
}

impl Drop for Created {
    fn drop(&mut self) {
        if let Some(dir) = self.0.take() {
            let _ = fs::remove_dir_all(dir);
        }
    }
}

/// The outermost missing directory on the way to `dir` (`dir` itself
/// when only it is missing) — what removing undoes a `create_dir_all`.
fn first_missing_ancestor(dir: &Path) -> PathBuf {
    let mut missing = dir.to_path_buf();
    while let Some(parent) = missing.parent() {
        if parent.as_os_str().is_empty() || parent.exists() {
            break;
        }
        missing = parent.to_path_buf();
    }
    missing
}

/// Write the two files into `staging/<slug>/` and read them back with
/// the `openspec` plan adapter: the one `Change` it yields is exactly
/// what `canon ingest plans` would import from the published dir.
fn stage_change(staging: &Path, slug: &ChangeId, title: &str) -> Result<Change, String> {
    let dir = staging.join(slug.as_str());
    fs::create_dir_all(&dir).map_err(|e| format!("failed to create the staging dir `{}`: {e}", dir.display()))?;
    for (name, content) in [("proposal.md", proposal_md(title)), ("tasks.md", tasks_md(title))] {
        fs::write(dir.join(name), content).map_err(|e| format!("failed to stage `{name}`: {e}"))?;
    }
    let entry = find_plan_adapter(DIALECT).ok_or_else(|| format!("the `{DIALECT}` plan dialect is not registered"))?;
    let outcome = entry.adapter.parse(&PlanSourceHandle::Path(staging.to_path_buf()));
    if !outcome.malformed.is_empty() {
        return Err(format!("the scaffolded change did not parse as `{DIALECT}`: {:?}", outcome.malformed));
    }
    outcome
        .changes
        .into_iter()
        .find(|change| &change.change_id == slug)
        .ok_or_else(|| format!("the scaffolded change `{}` did not parse as `{DIALECT}`", slug.as_str()))
}

/// `canon change new` (module doc). Exit `0` written + adopted, `2`
/// refused or failed with nothing left behind.
pub fn run_new(repo: &Path, slug: &ChangeId, subject_id: &SubjectId, title: &str) -> i32 {
    let repo = resolve_repo_root(repo);
    let canon_yaml_path = resolve_canon_yaml(&repo, None);
    let sources = match crate::plans::load_plan_sources_from_config(&canon_yaml_path, &repo) {
        Ok(sources) => sources,
        Err(e) => {
            eprintln!("canon change new: {e}");
            return EXIT_REFUSED;
        }
    };
    let Some(source) = sources.into_iter().find(|s| s.dialect() == DIALECT) else {
        eprintln!(
            "canon change new: refused — canon.yaml configures no `{DIALECT}` plans source; add `{{ dialect: {DIALECT}, root: . }}` under `plans.sources` (what `canon init` writes)"
        );
        return EXIT_REFUSED;
    };

    let subject = match crate::subject::lookup_adoption(&repo, subject_id, slug) {
        Ok((None, _)) => {
            eprintln!("canon change new: refused — subject `{}` does not exist (author it with `canon subject new` first)", subject_id.as_str());
            return EXIT_REFUSED;
        }
        Ok((Some(_), Some(_))) => {
            eprintln!("canon change new: refused — change `{}` already exists; pick another slug", slug.as_str());
            return EXIT_REFUSED;
        }
        Ok((Some(subject), None)) => subject,
        Err(e) => {
            eprintln!("canon change new: {e}");
            return EXIT_REFUSED;
        }
    };

    let changes = changes_dir(source.root());
    let dir = changes.join(slug.as_str());
    let archived = changes.join("archive").join(slug.as_str());
    if dir.exists() || archived.exists() {
        let existing = if dir.exists() { &dir } else { &archived };
        eprintln!("canon change new: refused — `{}` already exists; pick another slug", display_rel(existing, &repo));
        return EXIT_REFUSED;
    }

    let staging_path = repo.join(paths::CANON_DIR).join(format!("change-new-{}-{}", slug.as_str(), std::process::id()));
    let staging = Created(Some(first_missing_ancestor(&staging_path)));
    let change = match stage_change(&staging_path, slug, title) {
        Ok(change) => change,
        Err(e) => {
            eprintln!("canon change new: {e}");
            return EXIT_REFUSED;
        }
    };

    let published = Created(Some(first_missing_ancestor(&dir)));
    let publish = fs::create_dir_all(&changes).and_then(|()| fs::rename(staging_path.join(slug.as_str()), &dir));
    if let Err(e) = publish {
        eprintln!("canon change new: failed to create `{}`: {e}", display_rel(&dir, &repo));
        return EXIT_REFUSED;
    }
    drop(staging);

    if let Err(e) = crate::subject::persist_adoption(&repo, change, subject) {
        let repair = format!("canon ingest plans && {}", crate::subject::adopt_command(slug, subject_id));
        eprintln!("canon change new: {}", e.describe(slug, subject_id, &repair));
        if e.subject_written {
            // The subject record exists and cannot be taken back; the
            // change dir stays so the repair can import it.
            published.keep();
        }
        return EXIT_REFUSED;
    }
    published.keep();

    for name in ["proposal.md", "tasks.md"] {
        println!("canon change new: wrote {}", display_rel(&dir.join(name), &repo));
    }
    println!(
        "canon change new: recorded change `{}` adopted into subject `{}` — {}",
        slug.as_str(),
        subject_id.as_str(),
        crate::write_mode::DIRECT
    );
    0
}
