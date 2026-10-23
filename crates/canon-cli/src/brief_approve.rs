//! `canon brief approve <slug> --approver <human> --note "<words>"`
//! (0.19 D3, #46): the one call that records a human's answer to a brief.
//!
//! It writes `Approved: <who> <when> — "<words>" (brief sha256:<digest>)`
//! into the change's `brief.md` and records the policy approval in the
//! same call ([`crate::policy_approval`]; skipped when the repo has no
//! policy file). `<when>` is the UTC time to the second, RFC 3339: the
//! chore re-verification rule (0.22 D2) reads it. The digest covers the brief with every `Approved:` line
//! removed ([`crate::brief::brief_digest`]), so any later edit to the
//! brief makes every reader say `BRIEF_CHANGED_SINCE_APPROVAL`.
//!
//! Refused (exit 2) while `canon brief check` has blocker findings, and
//! for an empty `--approver`/`--note`; exit 1 for an operational error. Existing
//! `Approved:` lines are replaced, so re-approving leaves exactly one.
//! Canon cannot tell a human from an agent running this command: the
//! skill makes it a human stop, and `--note` carries their words.

use std::path::Path;

use canon_model::ChangeId;
use chrono::Utc;
use schemars::JsonSchema;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::json_out::Report;
use crate::brief::brief_digest;
use crate::brief_check::{check, Severity};
use crate::cli_error::CliError;
use crate::context::resolve_repo_root;
use crate::policy_approval::ApprovalState;
use canon_ingest::markdown::{field, without_field};

const EXIT_REFUSED: i32 = 2;
const EXIT_ERROR: i32 = 1;

/// `text` approved: its `Approved:` lines replaced by one that binds the
/// result's remaining text. `Err` when the line would not be read back
/// (the brief ends inside an open code fence or HTML comment).
pub fn approved_text(text: &str, by: &str, when: &str, note: &str) -> Result<String, String> {
    let body = format!("{}\n\n", without_field(text, "Approved:").trim_end());
    let value = format!("{by} {when} — \"{note}\" (brief {})", brief_digest(&body));
    let approved = format!("{body}Approved: {value}\n");
    if field(&approved, "Approved:") != Some(value.as_str()) || brief_digest(&approved) != brief_digest(&body) {
        return Err("the brief ends inside an open code fence or HTML comment, so an `Approved:` line appended there would not be read; close it first".to_string());
    }
    Ok(approved)
}

/// What one approval recorded: the `brief approve --json` data.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Approved {
    pub change: String,
    pub brief: String,
    pub digest: String,
    pub approved_by: String,
    pub policy_approved: bool,
}

impl Report for Approved {}

/// The next step for each code [`approve`] refuses with.
fn hint_for(code: &str) -> &'static str {
    match code {
        "APPROVAL_INCOMPLETE" => "pass --approver <name> and --note <the human's answer>, each one non-empty line",
        "BRIEF_APPROVE_REFUSED" => "run `canon brief check`, clear the blockers it lists, then ask the human to approve",
        "BRIEF_UNAPPROVABLE" => "fix the brief as the message says (`canon brief check` re-reads it), then run `canon brief approve` again",
        "BRIEF_UNREADABLE" => "check that brief.md is a readable UTF-8 file, then run `canon brief approve` again",
        "POLICY_APPROVAL_FAILED" => "check that .canon/policy.yaml is readable and writable, then run `canon brief approve` again",
        _ => "check that the change's brief.md is writable, then run `canon brief approve` again",
    }
}

fn refuse(code: &'static str, message: &str, exit: i32) -> CliError {
    CliError::failed(code, message).hint(hint_for(code)).with_exit(exit)
}

/// `sha256` of `bytes` as lowercase hex: what a page that rendered a brief
/// sends back to say which file it showed.
pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// The one approval: used by `canon brief approve` and by the dashboard's
/// briefs page, so both record exactly the same. `repo` is a resolved repo
/// root. With `expected_sha256` (the hex digest of the brief file's bytes
/// the caller showed the human), the approval is refused with
/// [`crate::dashboard_briefs::CHANGED_SINCE_PAGE_LOAD`] when the file's
/// bytes differ, and nothing is written.
pub fn approve(repo: &Path, change: Option<&ChangeId>, by: &str, note: &str, expected_sha256: Option<&str>) -> Result<Approved, CliError> {
    approve_checked(repo, change, by, note, expected_sha256).map(|(approved, _)| approved)
}

/// [`approve`], also saying whether the brief and the policy already carried
/// this very approval, in which case nothing was written.
fn approve_checked(repo: &Path, change: Option<&ChangeId>, by: &str, note: &str, expected_sha256: Option<&str>) -> Result<(Approved, bool), CliError> {
    let (by, note) = (by.trim(), note.trim());
    for (flag, value) in [("--approver", by), ("--note", note)] {
        if value.is_empty() {
            return Err(refuse("APPROVAL_INCOMPLETE", &format!("{flag} must carry the human's answer, not be empty"), EXIT_REFUSED));
        }
        if value.contains(['\n', '\r']) {
            return Err(refuse("APPROVAL_INCOMPLETE", &format!("{flag} must be one line"), EXIT_REFUSED));
        }
    }
    let checked = check(repo, change)?;
    let path = repo.join(&checked.brief);
    if let Some(expected) = expected_sha256 {
        verify_bytes(&checked.brief, std::fs::read(&path).ok().as_deref(), expected)?;
    }
    // Coverage rules (a flow step with no scenario or no failure case) gate
    // shipping, not approval: the human approves the scope before its
    // scenarios exist, and re-approves a changed scope mid-build (0.19 D3).
    let blockers = approval_blockers(&checked.evaluation.findings);
    if !blockers.is_empty() {
        let listed = blockers.iter().map(|f| format!("{} line {}", f.rule, f.line)).collect::<Vec<_>>().join(", ");
        return Err(refuse(
            "BRIEF_APPROVE_REFUSED",
            &format!("{} has {} blocker finding(s): {listed}; `canon brief check` shows them, and the human approves a brief only once they are cleared", checked.brief, blockers.len()),
            EXIT_REFUSED,
        ));
    }
    let text = std::fs::read_to_string(&path).map_err(|e| refuse("BRIEF_UNREADABLE", &format!("cannot read `{}`: {e}", checked.brief), EXIT_ERROR))?;
    if let Some(expected) = expected_sha256 {
        verify_bytes(&checked.brief, Some(text.as_bytes()), expected)?;
    }
    // The same approval again, by the same human, on a brief and a policy
    // that already carry it: nothing to write.
    let policy_approved = !matches!(crate::policy_approval::approval_state(repo), ApprovalState::NoPolicy);
    if already_approved_so(&text, by, note) && (!policy_approved || crate::policy_approval::matching_approval(repo, by, Some(note)).is_some()) {
        return Ok((Approved { change: checked.change.as_str().to_string(), brief: checked.brief, digest: brief_digest(&text), approved_by: by.to_string(), policy_approved }, true));
    }
    let when = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let approved = approved_text(&text, by, &when, note).map_err(|e| refuse("BRIEF_UNAPPROVABLE", &e, EXIT_REFUSED))?;
    // Policy first: a failed policy write leaves the brief untouched, and a
    // failed brief write leaves a policy approval that is true on its own.
    if policy_approved {
        crate::policy_approval::record(repo, by, Some(note)).map_err(|e| refuse("POLICY_APPROVAL_FAILED", &e, EXIT_ERROR))?;
    }
    std::fs::write(&path, &approved).map_err(|e| refuse("BRIEF_WRITE_FAILED", &format!("cannot write `{}`: {e}", checked.brief), EXIT_ERROR))?;
    Ok((Approved { change: checked.change.as_str().to_string(), brief: checked.brief, digest: brief_digest(&approved), approved_by: by.to_string(), policy_approved }, false))
}

/// The findings that stop an approval: blockers, coverage rules excluded.
pub fn approval_blockers(findings: &[crate::brief_check::Finding]) -> Vec<&crate::brief_check::Finding> {
    findings.iter().filter(|f| f.severity == Severity::Blocker && !crate::brief_check::COVERAGE_RULES.contains(&f.rule.as_ref())).collect()
}

/// `Err` unless `bytes` (the brief as read now) hash to `expected`.
fn verify_bytes(brief: &str, bytes: Option<&[u8]>, expected: &str) -> Result<(), CliError> {
    if bytes.is_some_and(|b| sha256_hex(b).eq_ignore_ascii_case(expected)) {
        return Ok(());
    }
    Err(crate::dashboard_briefs::changed_since_page_load(brief))
}

/// `canon brief approve`. Returns the process exit code.
pub fn run_approve(repo: &Path, change: Option<&ChangeId>, by: &str, note: &str, json: bool) -> i32 {
    let repo = resolve_repo_root(repo);
    let (approved, unchanged) = match approve_checked(&repo, change, by, note, None) {
        Ok(done) => done,
        Err(e) => return e.emit("canon brief approve", json),
    };
    if unchanged {
        crate::json_out::mark_unchanged();
    }
    if !json {
        let policy = if approved.policy_approved { "; policy approval recorded too" } else { "; no policy file, so no policy approval" };
        let verb = if unchanged { "already approved, unchanged" } else { "approved" };
        println!("canon brief approve: {} {verb} by `{}` ({}){policy}", approved.brief, approved.approved_by, approved.digest);
    }
    crate::next::finish(&repo, "brief approve", &crate::next::Scope::Change(approved.change.clone()), false, json, &approved);
    0
}

/// Whether `text` already carries an `Approved:` line by `by` with `note`
/// that binds the brief's current body: the line a repeat of the same
/// approval would write, up to the time it was written.
fn already_approved_so(text: &str, by: &str, note: &str) -> bool {
    let body = format!("{}\n\n", without_field(text, "Approved:").trim_end());
    let tail = format!(" — \"{note}\" (brief {})", brief_digest(&body));
    field(text, "Approved:")
        .and_then(|value| value.strip_prefix(by))
        .and_then(|value| value.strip_prefix(' '))
        .and_then(|value| value.strip_suffix(tail.as_str()))
        .is_some_and(|when| !when.is_empty() && !when.contains(char::is_whitespace))
}
