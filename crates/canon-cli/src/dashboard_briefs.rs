//! The briefs page's two endpoints (0.25 D3, #103): the list of briefs
//! that await the human, and the approval that records the human's answer.
//!
//! `GET /api/briefs` ([`list`]) lists every live change
//! (`.canon/changes/<id>/brief.md`, never the archive) whose brief is not
//! approved: no `Approved:` line, or one the brief changed after. Each
//! entry carries the brief's raw text, the sha256 of the file's bytes the
//! text came from, the sections and the findings read from that same text
//! by `canon brief check`'s own parser ([`crate::brief_check`]), and
//! whether the brief can be approved.
//!
//! `POST /api/briefs/approve` ([`approve`]) calls
//! [`crate::brief_approve::approve`], the function `canon brief approve`
//! calls, with the sha256 the page rendered, so the page records exactly
//! what the CLI records and only for the bytes the human read. The HTTP
//! guards (method, content type, `Host`, `Origin`, body size) sit in
//! [`crate::dashboard`] and run before any of this reads the brief.
//! Approvals are serialized by the caller.

use std::collections::BTreeMap;
use std::path::Path;

use canon_model::{paths, ChangeId};
use serde::Deserialize;
use serde_json::{json, Value};

use crate::brief::approval_of;
use crate::brief_approve::{approval_blockers, sha256_hex};
use crate::brief_check::{check_text, parse_sections};
use crate::cli_error::CliError;

/// The code for an approval whose brief differs from the page's copy.
pub const CHANGED_SINCE_PAGE_LOAD: &str = "BRIEF_CHANGED_SINCE_PAGE_LOAD";

/// The refusal [`crate::brief_approve::approve`] raises when the brief's
/// bytes are not the ones the caller showed the human.
pub fn changed_since_page_load(brief: &str) -> CliError {
    CliError::refused(CHANGED_SINCE_PAGE_LOAD, format!("{brief} changed after this page loaded it, so the answer given here is not about the brief as it is now; nothing was recorded"))
        .hint("reload the page, read the brief as it is now, then approve it again")
        .field("sha256")
}

/// An HTTP status and the JSON document that answers with it.
pub struct Reply {
    pub status: u16,
    pub body: Value,
}

impl Reply {
    fn ok(body: Value) -> Self {
        Reply { status: 200, body }
    }

    pub fn error(status: u16, error: &CliError) -> Self {
        Reply { status, body: error.to_json() }
    }
}

/// A request the server cannot make sense of: a 400.
pub fn bad_request(message: impl Into<String>) -> Reply {
    Reply::error(400, &CliError::refused("DASHBOARD_BAD_REQUEST", message).hint("send a JSON body `{\"changeId\", \"by\", \"note\", \"sha256\"}` with `Content-Type: application/json`, as the briefs page does"))
}

/// The HTTP status of a refusal from the approval core.
fn status_of(code: &str) -> u16 {
    match code {
        "APPROVAL_INCOMPLETE" => 400,
        "BRIEF_NOT_FOUND" => 404,
        "BRIEF_HAS_BLOCKERS" | "BRIEF_UNAPPROVABLE" | CHANGED_SINCE_PAGE_LOAD => 409,
        _ => 500,
    }
}

/// The subjects that adopt each change, by change id.
fn adopting_subjects(repo: &Path) -> BTreeMap<String, Vec<String>> {
    use canon_gate::spec_coverage::{latest_subjects, subject_change_ids};
    use canon_gate::{GateContext, GateCtx};
    let registry = canon_policy::SchemaRegistry::load();
    let Ok(ctx) = GateCtx::from_repo(repo).map_err(|e| e.to_string()).and_then(|ctx| GateContext::load(ctx, &registry, chrono::Utc::now()).map_err(|e| e.to_string())) else {
        return BTreeMap::new();
    };
    let mut by_change: BTreeMap<String, Vec<String>> = BTreeMap::new();
    for subject in latest_subjects(&ctx) {
        for change in subject_change_ids(&ctx, subject) {
            by_change.entry(change.as_str().to_string()).or_default().push(subject.subject_id.as_str().to_string());
        }
    }
    by_change
}

/// `GET /api/briefs`.
pub fn list(repo: &Path) -> Reply {
    let subjects = adopting_subjects(repo);
    let mut live: Vec<(ChangeId, std::path::PathBuf)> = std::fs::read_dir(repo.join(paths::CHANGES_DIR))
        .map(|entries| entries.filter_map(Result::ok).collect::<Vec<_>>())
        .unwrap_or_default()
        .into_iter()
        .filter(|e| e.file_name() != "archive" && e.path().join("brief.md").is_file())
        .filter_map(|e| Some((ChangeId::parse(e.file_name().to_str()?).ok()?, e.path().join("brief.md"))))
        .collect();
    live.sort_by(|a, b| a.0.as_str().cmp(b.0.as_str()));

    let mut briefs = Vec::new();
    let mut unreadable = Vec::new();
    for (change, path) in live {
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) => {
                unreadable.push(json!({ "changeId": change.as_str(), "message": format!("cannot read the brief: {e}") }));
                continue;
            }
        };
        let Ok(text) = String::from_utf8(bytes.clone()) else {
            unreadable.push(json!({ "changeId": change.as_str(), "message": "the brief is not UTF-8 text" }));
            continue;
        };
        let approval = approval_of(&text);
        if approval.approved() {
            continue;
        }
        let checked = match check_text(repo, change.clone(), &path, &text) {
            Ok(checked) => checked,
            Err(e) => return Reply::error(500, &e),
        };
        let counts = checked.evaluation.counts();
        let blocking = approval_blockers(&checked.evaluation.findings);
        let findings: Vec<Value> = checked
            .evaluation
            .findings
            .iter()
            .map(|f| {
                json!({
                    "severity": f.severity.as_str(),
                    "rule": f.rule,
                    "line": f.line,
                    "message": f.excerpt,
                    "hint": f.hint,
                    "section": f.section,
                    "blocksApproval": blocking.iter().any(|b| std::ptr::eq(*b, f)),
                })
            })
            .collect();
        briefs.push(json!({
            "changeId": change.as_str(),
            "subjects": subjects.get(change.as_str()).cloned().unwrap_or_default(),
            "brief": checked.brief,
            "text": text,
            "sha256": sha256_hex(&bytes),
            "language": checked.language.tag,
            "changedSinceApproval": matches!(approval, crate::brief::Approval::Changed { .. }),
            "legacy": checked.evaluation.legacy,
            "sections": parse_sections(&text),
            "findings": findings,
            "counts": { "blockers": counts.blockers, "advisories": counts.advisories },
            "approvable": blocking.is_empty(),
        }));
    }
    Reply::ok(crate::json_out::envelope("dashboard briefs", json!({ "briefs": briefs, "unreadable": unreadable })))
}

/// The `POST /api/briefs/approve` body.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ApproveRequest {
    change_id: String,
    by: String,
    note: String,
    sha256: String,
}

/// `POST /api/briefs/approve` (the guards have passed): the answer is
/// `canon brief approve --json`'s document, or its error.
pub fn approve(repo: &Path, body: &[u8]) -> Reply {
    let request: ApproveRequest = match serde_json::from_slice(body) {
        Ok(request) => request,
        Err(e) => return bad_request(format!("the body is not the approval request: {e}")),
    };
    let Ok(change) = ChangeId::parse(&request.change_id) else {
        return Reply::error(400, &CliError::refused("DASHBOARD_BAD_REQUEST", format!("`{}` is not a change id", request.change_id)).field("changeId").hint("send the changeId the briefs list gave"));
    };
    if !repo.join(paths::CHANGES_DIR).join(change.as_str()).join("brief.md").is_file() {
        return Reply::error(404, &CliError::failed("BRIEF_NOT_FOUND", format!("no live change `{}` has a brief", change.as_str())).hint("reload the page: the change may have been archived or removed"));
    }
    match crate::brief_approve::approve(repo, Some(&change), &request.by, &request.note, Some(&request.sha256)) {
        Ok(approved) => {
            eprintln!("canon dashboard: {} approved by `{}` ({})", approved.brief, approved.approved_by, approved.digest);
            let after = crate::next::after_with(repo, &crate::next::Scope::Change(approved.change.clone()), false, Vec::new());
            let body = crate::json_out::envelope_with_next("brief approve", serde_json::to_value(&approved).unwrap_or_default(), &after, Value::Null);
            Reply::ok(body)
        }
        Err(e) => {
            let status = status_of(e.code);
            let e = if e.code == "BRIEF_HAS_BLOCKERS" { e.hint("clear the blockers in the brief (`canon brief check` lists them), then reload this page") } else { e };
            Reply::error(status, &e)
        }
    }
}
