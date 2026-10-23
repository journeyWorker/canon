//! `canon change new <slug> --subject-id <id> --title <t>` (0.14 D2, 0.17
//! D2): the official path to a plan change. It writes canon's own
//! change dir, `.canon/changes/<slug>/` (`brief.md` only, 0.19 D4: flow coverage
//! replaces the task checklist; an existing `tasks.md` is still read), reads
//! it back with the native `canon` plan adapter `canon ingest plans`
//! reads that directory with, and records it adopted into the subject
//! through the same write `canon subject adopt` performs
//! ([`crate::subject::persist_adoption`]). Nothing in `canon.yaml` is
//! involved: every plan reader reads `.canon/changes/` with no `plans:`
//! entry, and `plans.sources` lists only external plans.
//!
//! # Provenance
//! A change this command writes is canon's by construction: it lives
//! under `.canon/changes/`, and its `Change`/`Task` records carry the
//! actor `canon-plan-import-canon`, which no external plan's records do
//! (an openspec change's carry `canon-plan-import-openspec`).
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
//! `canon ingest plans && canon subject adopt <slug> --subject-id <id>`,
//! which is safe to rerun.
//!
//! # Refusals (exit `2`, nothing written)
//! - an unknown subject;
//! - a slug that already has a directory under `.canon/changes/`
//!   (active or archived) or a `change` record.

use std::fs;
use std::path::{Path, PathBuf};

use canon_ingest::{native_plan_adapter, PlanSourceHandle};
use canon_model::{paths, Change, ChangeId, SubjectId, SubjectStatus};

use crate::cli_error::CliError;
use crate::context::resolve_repo_root;
use crate::brief_template::ChangeKind;
use crate::language::Language;

/// `brief.md`: the PRD brief or, for a chore, the chore brief
/// ([`crate::brief_template`], 0.19 D1/D5) in the repo's `language:`.
/// The Intent section's first paragraph is the title, which imports as
/// `Change.summary`.
fn brief_md(kind: ChangeKind, title: &str, language: &Language) -> String {
    crate::brief_template::brief(kind, title, language)
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

/// Write `brief.md` into `staging/<slug>/` and read them back with
/// the native `canon` plan adapter: the one `Change` it yields is
/// exactly what `canon ingest plans` would import from the published
/// dir.
fn stage_change(staging: &Path, slug: &ChangeId, kind: ChangeKind, title: &str, language: &Language) -> Result<Change, String> {
    let dir = staging.join(slug.as_str());
    fs::create_dir_all(&dir).map_err(|e| format!("failed to create the staging dir `{}`: {e}", dir.display()))?;
    fs::write(dir.join("brief.md"), brief_md(kind, title, language)).map_err(|e| format!("failed to stage `brief.md`: {e}"))?;
    let entry = native_plan_adapter();
    let outcome = entry.adapter.parse(&PlanSourceHandle::Path(staging.to_path_buf()));
    if !outcome.malformed.is_empty() {
        return Err(format!("the written change did not parse: {:?}", outcome.malformed));
    }
    outcome
        .changes
        .into_iter()
        .find(|change| &change.change_id == slug)
        .ok_or_else(|| format!("the written change `{}` did not parse", slug.as_str()))
}

/// `canon change new` (module doc). Exit `0` written + adopted, `2`
/// refused or failed with nothing left behind.
pub fn run_new(repo: &Path, slug: &ChangeId, subject_id: &SubjectId, kind: ChangeKind, title: &str, json: bool) -> i32 {
    let repo = resolve_repo_root(repo);
    let language = match crate::language::configured(&repo) {
        Ok(language) => language,
        Err(e) => {
            return CliError::refused("CONFIG_UNREADABLE", e.to_string()).hint("fix canon.yaml so it parses (`canon init --check-config` reports what is wrong), then run the command again").emit("canon change new", json);
        }
    };

    let subject = match crate::subject::lookup_adoption(&repo, subject_id, slug) {
        Ok((None, _)) => {
            return CliError::refused("SUBJECT_NOT_FOUND", format!("subject `{}` does not exist (author it with `canon subject new` first)", subject_id.as_str())).hint("author it with `canon subject new <id> --domain <domain> --title <title>`, or check the id against the subjects `canon status` lists").emit("canon change new", json);
        }
        Ok((Some(_), Some(_))) => {
            return CliError::refused("CHANGE_EXISTS", format!("change `{}` already exists; pick another slug", slug.as_str())).hint("pick another slug for `canon change new`, or keep working in the existing change").emit("canon change new", json);
        }
        Ok((Some(subject), None)) => subject,
        Err(e) => {
            return CliError::refused("REPO_UNREADABLE", e.to_string()).hint("check that the repository is readable from here, or pass --repo <path> for its root").emit("canon change new", json);
        }
    };

    let changes = repo.join(paths::CHANGES_DIR);
    let dir = changes.join(slug.as_str());
    let archived = changes.join("archive").join(slug.as_str());
    if dir.exists() || archived.exists() {
        let existing = if dir.exists() { &dir } else { &archived };
        return CliError::refused("CHANGE_EXISTS", format!("`{}` already exists; pick another slug", display_rel(existing, &repo))).hint("pick another slug for `canon change new`, or keep working in the existing change").emit("canon change new", json);
    }

    let staging_path = repo.join(paths::CANON_DIR).join(format!("change-new-{}-{}", slug.as_str(), std::process::id()));
    let staging = Created(Some(first_missing_ancestor(&staging_path)));
    let change = match stage_change(&staging_path, slug, kind, title, &language) {
        Ok(change) => change,
        Err(e) => {
            return CliError::refused("CHANGE_INVALID_ARGUMENT", e.to_string()).hint("`canon schema change new` lists the accepted arguments and their forms").emit("canon change new", json);
        }
    };

    let published = Created(Some(first_missing_ancestor(&dir)));
    let publish = fs::create_dir_all(&changes).and_then(|()| fs::rename(staging_path.join(slug.as_str()), &dir));
    if let Err(e) = publish {
        return CliError::refused("WRITE_FAILED", format!("failed to create `{}`: {e}", display_rel(&dir, &repo))).hint("check that the path is writable and the disk has room, then run the command again").emit("canon change new", json);
    }
    drop(staging);

    let shipped = subject.status == SubjectStatus::Shipped;
    let warnings: Vec<String> = crate::brief::adoption_warning(&repo, &subject, slug)
        .into_iter()
        .collect();
    if let Err(e) = crate::subject::persist_adoption(&repo, change, subject) {
        let repair = format!("canon ingest plans && {}", crate::subject::adopt_command(slug, subject_id));
        if e.subject_written {
            // The subject record exists and cannot be taken back; the
            // change dir stays so the repair can import it.
            published.keep();
        }
        let message = e.describe(slug, subject_id, &repair);
        let error = if e.subject_written {
            CliError::refused("CHANGE_ADOPTION_INCOMPLETE", message).hint(format!("finish the adoption: run `{repair}`"))
        } else {
            CliError::refused("WRITE_FAILED", message).hint("check that the path is writable and the disk has room, then run the command again")
        };
        return error.emit("canon change new", json);
    }
    published.keep();

    let brief = display_rel(&dir.join("brief.md"), &repo);
    crate::say!(json, "canon change new: wrote {brief}");
    crate::say!(
        json,
        "canon change new: recorded change `{}` adopted into subject `{}` — {}",
        slug.as_str(),
        subject_id.as_str(),
        crate::write_mode::DIRECT
    );
    for warning in &warnings {
        eprintln!("canon change new: warning: {warning}");
    }
    // A shipped subject stays shipped until reopened: say how, once the
    // brief is approved (the reopen refuses an unapproved one).
    let reopen = shipped.then(|| crate::status::reopen_command(subject_id.as_str()));
    if let Some(command) = &reopen {
        crate::say!(json, "canon change new: subject `{}` is shipped and stays so until reopened; once the brief is approved, run `{command}`", subject_id.as_str());
    }
    let extra = reopen
        .map(|command| crate::status::NextStep {
            actor: crate::status::StepActor::Agent,
            role: Some(crate::role::Role::Orchestrator),
            command,
            why: format!(
                "subject `{}` is shipped, so the new change `{}` is not worked under it until it reopens; reopen once the brief is approved (an unapproved brief refuses the reopen), then the work, the suite again with `canon evidence add --all-scenarios`, and the re-review",
                subject_id.as_str(),
                slug.as_str()
            ),
        })
        .into_iter()
        .collect();
    crate::next::finish_with_steps(
        &repo,
        "change new",
        &crate::next::Scope::Change(slug.as_str().to_string()),
        false,
        json,
        &crate::json_out::untyped(serde_json::json!({ "changeId": slug.as_str(), "subjectId": subject_id.as_str(), "brief": brief })),
        crate::json_out::warnings_meta(&warnings),
        extra);
    0
}
