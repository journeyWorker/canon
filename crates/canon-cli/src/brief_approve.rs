//! `canon brief approve --change <slug> --by <human> --note "<words>"`
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
//! for an empty `--by`/`--note`; exit 1 for an operational error. Existing
//! `Approved:` lines are replaced, so re-approving leaves exactly one.
//! Canon cannot tell a human from an agent running this command: the
//! skill makes it a human stop, and `--note` carries their words.

use std::path::Path;

use canon_model::ChangeId;
use chrono::Utc;
use serde::Serialize;

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

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
struct ApproveJson<'a> {
    change: &'a str,
    brief: &'a str,
    digest: &'a str,
    approved_by: &'a str,
    policy_approved: bool,
}

/// The next step for each code `run_approve` refuses with.
fn hint_for(code: &str) -> &'static str {
    match code {
        "APPROVAL_INCOMPLETE" => "pass --by <name> and --note <the human's answer>, each one non-empty line",
        "BRIEF_HAS_BLOCKERS" => "run `canon brief check`, clear the blockers it lists, then ask the human to approve",
        "BRIEF_UNAPPROVABLE" => "fix the brief as the message says (`canon brief check` re-reads it), then run `canon brief approve` again",
        "BRIEF_UNREADABLE" => "check that brief.md is a readable UTF-8 file, then run `canon brief approve` again",
        "POLICY_APPROVAL_FAILED" => "check that .canon/policy.yaml is readable and writable, then run `canon brief approve` again",
        _ => "check that the change's brief.md is writable, then run `canon brief approve` again",
    }
}

fn refuse(json: bool, code: &'static str, message: &str, exit: i32) -> i32 {
    CliError::failed(code, message).hint(hint_for(code)).with_exit(exit).emit("canon brief approve", json)
}

/// `canon brief approve`. Returns the process exit code.
pub fn run_approve(repo: &Path, change: Option<&ChangeId>, by: &str, note: &str, json: bool) -> i32 {
    let repo = resolve_repo_root(repo);
    let (by, note) = (by.trim(), note.trim());
    for (flag, value) in [("--by", by), ("--note", note)] {
        if value.is_empty() {
            return refuse(json, "APPROVAL_INCOMPLETE", &format!("{flag} must carry the human's answer, not be empty"), EXIT_REFUSED);
        }
        if value.contains(['\n', '\r']) {
            return refuse(json, "APPROVAL_INCOMPLETE", &format!("{flag} must be one line"), EXIT_REFUSED);
        }
    }
    let checked = match check(&repo, change) {
        Ok(checked) => checked,
        Err(e) => return e.emit("canon brief approve", json),
    };
    // Coverage rules (a flow step with no scenario or no failure case) gate
    // shipping, not approval: the human approves the scope before its
    // scenarios exist, and re-approves a changed scope mid-build (0.19 D3).
    let blockers: Vec<_> = checked
        .evaluation
        .findings
        .iter()
        .filter(|f| f.severity == Severity::Blocker && !crate::brief_check::COVERAGE_RULES.contains(&f.rule.as_ref()))
        .collect();
    if !blockers.is_empty() {
        let listed = blockers.iter().map(|f| format!("{} line {}", f.rule, f.line)).collect::<Vec<_>>().join(", ");
        return refuse(
            json,
            "BRIEF_HAS_BLOCKERS",
            &format!("{} has {} blocker finding(s): {listed}; `canon brief check` shows them, and the human approves a brief only once they are cleared", checked.brief, blockers.len()),
            EXIT_REFUSED,
        );
    }
    let path = repo.join(&checked.brief);
    let text = match std::fs::read_to_string(&path) {
        Ok(text) => text,
        Err(e) => return refuse(json, "BRIEF_UNREADABLE", &format!("cannot read `{}`: {e}", checked.brief), EXIT_ERROR),
    };
    let when = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let approved = match approved_text(&text, by, &when, note) {
        Ok(approved) => approved,
        Err(e) => return refuse(json, "BRIEF_UNAPPROVABLE", &e, EXIT_REFUSED),
    };
    // Policy first: a failed policy write leaves the brief untouched, and a
    // failed brief write leaves a policy approval that is true on its own.
    let policy_approved = !matches!(crate::policy_approval::approval_state(&repo), ApprovalState::NoPolicy);
    if policy_approved {
        if let Err(e) = crate::policy_approval::record(&repo, by, Some(note)) {
            return refuse(json, "POLICY_APPROVAL_FAILED", &e, EXIT_ERROR);
        }
    }
    if let Err(e) = std::fs::write(&path, &approved) {
        return refuse(json, "BRIEF_WRITE_FAILED", &format!("cannot write `{}`: {e}", checked.brief), EXIT_ERROR);
    }
    let digest = brief_digest(&approved);
    if !json {
        let policy = if policy_approved { "; policy approval recorded too" } else { "; no policy file, so no policy approval" };
        println!("canon brief approve: {} approved by `{by}` ({digest}){policy}", checked.brief);
    }
    let body = ApproveJson { change: checked.change.as_str(), brief: &checked.brief, digest: &digest, approved_by: by, policy_approved };
    crate::next::finish(&repo, "brief approve", &crate::next::Scope::Change(checked.change.as_str().to_string()), false, json, serde_json::to_value(&body).unwrap_or_default());
    0
}
