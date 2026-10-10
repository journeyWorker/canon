//! `canon change new <slug> --subject <id> --title <t>` (0.14 D2): the
//! official path to a plan change. It scaffolds an openspec change dir
//! (`proposal.md` + `tasks.md`) under the repo's configured `openspec`
//! plans source, imports it through the same `canon ingest plans` pass
//! ([`crate::plans::run`], one-shot over that one source), and adopts
//! it into the subject through the same write `canon subject adopt`
//! performs ([`crate::subject::adopt_change`]). Nothing in `canon.yaml`
//! needs a hand edit: `canon init` configures the source.
//!
//! # Refusals (exit `2`, nothing written)
//! - no `openspec` source in `canon.yaml`'s `plans.sources`;
//! - an unknown subject;
//! - a slug that already has a change dir (active or archived) or a
//!   `change` record.

use std::fs;
use std::path::{Path, PathBuf};

use canon_model::{ChangeId, SubjectId};

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

/// `canon change new` (module doc). Exit `0` written + imported +
/// adopted, `2` refused or failed.
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

    match crate::subject::subject_and_change_exist(&repo, subject_id, slug) {
        Ok((false, _)) => {
            eprintln!("canon change new: refused — subject `{}` does not exist (author it with `canon subject new` first)", subject_id.as_str());
            return EXIT_REFUSED;
        }
        Ok((true, true)) => {
            eprintln!("canon change new: refused — change `{}` already exists; pick another slug", slug.as_str());
            return EXIT_REFUSED;
        }
        Ok((true, false)) => {}
        Err(e) => {
            eprintln!("canon change new: {e}");
            return EXIT_REFUSED;
        }
    }

    let changes = changes_dir(source.root());
    let dir = changes.join(slug.as_str());
    let archived = changes.join("archive").join(slug.as_str());
    if dir.exists() || archived.exists() {
        let existing = if dir.exists() { &dir } else { &archived };
        eprintln!("canon change new: refused — `{}` already exists; pick another slug", display_rel(existing, &repo));
        return EXIT_REFUSED;
    }

    if let Err(e) = fs::create_dir_all(&dir) {
        eprintln!("canon change new: failed to create `{}`: {e}", display_rel(&dir, &repo));
        return EXIT_REFUSED;
    }
    for (name, content) in [("proposal.md", proposal_md(title)), ("tasks.md", tasks_md(title))] {
        let path = dir.join(name);
        if let Err(e) = fs::write(&path, content) {
            eprintln!("canon change new: failed to write `{}`: {e}", display_rel(&path, &repo));
            return EXIT_REFUSED;
        }
        println!("canon change new: wrote {}", display_rel(&path, &repo));
    }

    if let Err(e) = crate::plans::run(&repo, Some(DIALECT), Some(source.root())) {
        eprintln!("canon change new: wrote the change dir but `canon ingest plans` failed: {e}");
        return EXIT_REFUSED;
    }
    match crate::subject::adopt_change(&repo, slug, subject_id) {
        Ok(_) => {
            println!("canon change new: imported change `{}` and adopted it into subject `{}`", slug.as_str(), subject_id.as_str());
            0
        }
        Err(e) => {
            eprintln!("canon change new: wrote the change dir but could not adopt it: {e}");
            EXIT_REFUSED
        }
    }
}
