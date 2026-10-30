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
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use serde_json::Value;

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
#[derive(Debug)]
pub struct Reply {
    pub status: u16,
    pub body: Value,
}

impl Reply {
    fn ok(body: Value) -> Self {
        Reply { status: 200, body }
    }

    pub fn error(status: u16, error: &CliError) -> Self {
        Reply { status, body: error.envelope("dashboard briefs") }
    }
}

/// A request the server cannot make sense of: a 400.
pub fn bad_request(message: impl Into<String>) -> Reply {
    Reply::error(400, &CliError::refused("DASHBOARD_BAD_REQUEST", message).hint("send a JSON body `{\"changeId\", \"by\", \"note\", \"sha256\"}` (or `briefs` for several) with `Content-Type: application/json`, as the briefs page does"))
}

/// The HTTP status of a refusal from the approval core.
fn status_of(code: &str) -> u16 {
    match code {
        "APPROVAL_INCOMPLETE" => 400,
        "BRIEF_NOT_FOUND" => 404,
        "BRIEF_APPROVE_REFUSED" | "BRIEF_UNAPPROVABLE" | CHANGED_SINCE_PAGE_LOAD => 409,
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
    let mut relayed = Vec::new();
    let mut unreadable = Vec::new();
    for (change, path) in live {
        let bytes = match std::fs::read(&path) {
            Ok(bytes) => bytes,
            Err(e) => {
                unreadable.push(UnreadableBrief { change_id: change.as_str().to_string(), message: format!("cannot read the brief: {e}") });
                continue;
            }
        };
        let Ok(text) = String::from_utf8(bytes.clone()) else {
            unreadable.push(UnreadableBrief { change_id: change.as_str().to_string(), message: "the brief is not UTF-8 text".to_string() });
            continue;
        };
        let approval = approval_of(&text);
        let line = match &approval {
            crate::brief::Approval::Bound { line } | crate::brief::Approval::Unbound { line } | crate::brief::Approval::Changed { line, .. } => Some(line.as_str()),
            crate::brief::Approval::Missing => None,
        };
        let relay = line.and_then(crate::relay::parse_marker);
        if approval.approved() {
            if let (Some(relay), Some(line)) = (relay, line) {
                relayed.push(RelayedBrief { change_id: change.as_str().to_string(), brief: path.strip_prefix(repo).unwrap_or(&path).to_string_lossy().replace(std::path::MAIN_SEPARATOR, "/"), approval: line.to_string(), relay });
            }
            continue;
        }
        let checked = match check_text(repo, change.clone(), &path, &text) {
            Ok(checked) => checked,
            Err(e) => return Reply::error(500, &e),
        };
        let counts = checked.evaluation.counts();
        let blocking = approval_blockers(&checked.evaluation.findings);
        let findings: Vec<BriefFinding> = checked
            .evaluation
            .findings
            .iter()
            .map(|f| BriefFinding {
                severity: f.severity,
                rule: f.rule.clone(),
                line: f.line,
                message: f.excerpt.clone(),
                hint: f.hint.clone(),
                section: f.section.clone(),
                blocks_approval: blocking.iter().any(|b| std::ptr::eq(*b, f)),
            })
            .collect();
        let sections = parse_sections(&text);
        briefs.push(PendingBrief {
            change_id: change.as_str().to_string(),
            subjects: subjects.get(change.as_str()).cloned().unwrap_or_default(),
            brief: checked.brief,
            sha256: sha256_hex(&bytes),
            language: checked.language.tag,
            language_source: checked.language_source.as_str(),
            language_note: checked.language_note,
            changed_since_approval: matches!(approval, crate::brief::Approval::Changed { .. }),
            previous_relay: relay,
            legacy: checked.evaluation.legacy,
            sections,
            findings,
            counts,
            approvable: blocking.is_empty(),
            text,
        });
    }
    Reply::ok(crate::json_out::envelope("dashboard briefs", &DashboardBriefsReport { briefs, relayed, unreadable }))
}

/// A brief the page could not read.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct UnreadableBrief {
    pub change_id: String,
    pub message: String,
}

/// One finding of a pending brief, as the page shows it.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BriefFinding {
    pub severity: crate::brief_check::Severity,
    pub rule: String,
    pub line: usize,
    /// The excerpt the finding points at.
    pub message: String,
    pub hint: String,
    pub section: Option<String>,
    /// Whether this finding stops an approval.
    pub blocks_approval: bool,
}

/// An approved brief whose approval an agent recorded for the human (0.32).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RelayedBrief {
    pub change_id: String,
    /// The brief file, relative to the repo root.
    pub brief: String,
    /// The `Approved:` line's text.
    pub approval: String,
    pub relay: canon_model::ApprovalRelay,
}

/// One brief still waiting for the human's answer.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PendingBrief {
    pub change_id: String,
    /// The subjects that adopted the change.
    pub subjects: Vec<String>,
    /// The brief file, relative to the repo root.
    pub brief: String,
    pub text: String,
    pub sha256: String,
    pub language: String,
    pub language_source: &'static str,
    pub language_note: Option<String>,
    pub changed_since_approval: bool,
    /// The relay of the approval this brief was edited after (0.32), when an agent recorded it.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub previous_relay: Option<canon_model::ApprovalRelay>,
    pub legacy: bool,
    pub sections: Vec<crate::brief_check::BriefSection>,
    pub findings: Vec<BriefFinding>,
    pub counts: crate::brief_check::Counts,
    pub approvable: bool,
}

/// `GET /api/briefs` (`DashboardBriefsReport`).
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct DashboardBriefsReport {
    pub briefs: Vec<PendingBrief>,
    /// The approved briefs an agent recorded the approval of for the human (0.32), with who relayed it and through which tool.
    pub relayed: Vec<RelayedBrief>,
    pub unreadable: Vec<UnreadableBrief>,
}

impl crate::json_out::Report for DashboardBriefsReport {}

/// The `POST /api/briefs/approve` body: one name and answer, and either the
/// one brief of the single form (`changeId` + `sha256`) or the `briefs` of
/// the multi form (0.31), each with the sha256 of the file the page read.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ApproveRequest {
    change_id: Option<String>,
    sha256: Option<String>,
    briefs: Option<Vec<ApproveBrief>>,
    by: String,
    note: String,
}

/// One brief of a multi-brief approval request.
#[derive(Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
struct ApproveBrief {
    change_id: String,
    sha256: String,
}

fn bad_change_id(id: &str) -> Reply {
    Reply::error(400, &CliError::refused("DASHBOARD_BAD_REQUEST", format!("`{id}` is not a change id")).field("changeId").hint("send the changeId the briefs list gave"))
}

fn no_live_brief(ids: &[&str]) -> Reply {
    let named = ids.iter().map(|id| format!("`{id}`")).collect::<Vec<_>>().join(", ");
    Reply::error(404, &CliError::failed("BRIEF_NOT_FOUND", format!("no live change {named} has a brief")).hint("reload the page: the change may have been archived or removed"))
}

/// `POST /api/briefs/approve` (the guards have passed): the single form's
/// answer is `canon brief approve --json`'s document, or its error; the
/// multi form's answer is its several-id document ([`ApprovedMany`]), and a
/// refused brief refuses them all with `BRIEF_APPROVE_REFUSED` naming each.
pub fn approve(repo: &Path, body: &[u8]) -> Reply {
    let request: ApproveRequest = match serde_json::from_slice(body) {
        Ok(request) => request,
        Err(e) => return bad_request(format!("the body is not the approval request: {e}")),
    };
    match (&request.change_id, &request.sha256, &request.briefs) {
        (Some(id), Some(sha256), None) => approve_one(repo, id, sha256, &request),
        (None, None, Some(briefs)) if !briefs.is_empty() => approve_many(repo, briefs, &request),
        _ => bad_request("the body names either one brief (`changeId` and `sha256`) or `briefs`, a non-empty list of `{changeId, sha256}`"),
    }
}

fn approve_one(repo: &Path, id: &str, sha256: &str, request: &ApproveRequest) -> Reply {
    let Ok(change) = ChangeId::parse(id) else { return bad_change_id(id) };
    if !repo.join(paths::CHANGES_DIR).join(change.as_str()).join("brief.md").is_file() {
        return no_live_brief(&[change.as_str()]);
    }
    match crate::brief_approve::approve(repo, Some(&change), &request.by, &request.note, Some(sha256)) {
        Ok(approved) => {
            crate::human_err!("canon dashboard: {} approved by `{}` ({})", approved.brief, approved.approved_by, approved.digest);
            let after = crate::next::after_with(repo, &crate::next::Scope::Change(approved.change.clone()), false, Vec::new());
            let body = crate::json_out::envelope_with_next("brief approve", &approved, &after, Value::Null);
            Reply::ok(body)
        }
        Err(e) => refused_reply(e, None),
    }
}

fn approve_many(repo: &Path, briefs: &[ApproveBrief], request: &ApproveRequest) -> Reply {
    let mut changes: Vec<(ChangeId, &str)> = Vec::new();
    for brief in briefs {
        let Ok(change) = ChangeId::parse(&brief.change_id) else { return bad_change_id(&brief.change_id) };
        changes.push((change, &brief.sha256));
    }
    let missing: Vec<&str> = changes.iter().filter(|(change, _)| !repo.join(paths::CHANGES_DIR).join(change.as_str()).join("brief.md").is_file()).map(|(change, _)| change.as_str()).collect();
    if !missing.is_empty() {
        return no_live_brief(&missing);
    }
    let items: Vec<crate::brief_approve::Item<'_>> = changes.iter().map(|(change, sha256)| crate::brief_approve::Item { change: Some(change), expected_sha256: Some(sha256) }).collect();
    match crate::brief_approve::approve_all(repo, &items, &request.by, &request.note) {
        Ok(many) => {
            for (entry, _) in &many.entries {
                crate::human_err!("canon dashboard: {} {:?} by `{}`", entry.change, entry.outcome, many.approved_by);
            }
            let report = crate::brief_approve::ApprovedMany { approved_by: many.approved_by, policy_approved: many.policy_approved, relay: None, approvals: many.entries.into_iter().map(|(entry, _)| entry).collect() };
            if let Some(e) = many.failure {
                return Reply { status: 500, body: crate::json_out::failure_value("dashboard briefs", &e, Some(&report)) };
            }
            let after = crate::next::after_with(repo, &crate::next::Scope::Repo, false, Vec::new());
            Reply::ok(crate::json_out::envelope_with_next("brief approve", &report, &after, Value::Null))
        }
        Err((e, data)) => refused_reply(e, data),
    }
}

/// The reply of a refused approval: the code's status, and the refused ids as `data`
/// when the call named several.
fn refused_reply(e: CliError, data: Option<crate::brief_approve::Refused>) -> Reply {
    let status = status_of(e.code);
    let e = match (e.code, data.is_some()) {
        ("BRIEF_APPROVE_REFUSED", true) => e.hint("nothing was recorded: reload this page to read the briefs named as they are now, clear any blockers (`canon brief check` lists them), then select and approve again"),
        ("BRIEF_APPROVE_REFUSED", false) => e.hint("clear the blockers in the brief (`canon brief check` lists them), then reload this page"),
        _ => e,
    };
    match data {
        Some(data) => Reply { status, body: crate::json_out::failure_value("dashboard briefs", &e, Some(&data)) },
        None => Reply::error(status, &e),
    }
}
