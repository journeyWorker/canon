//! `canon brief approve <slug> --approver <human> --note "<words>"`
//! (0.19 D3, #46): the one call that records a human's answer to a brief.
//!
//! It writes `Approved: <who> <when> — "<words>" (brief sha256:<digest>)`
//! into the change's `brief.md` and records the policy approval in the
//! same call ([`crate::policy_approval`]; skipped when the repo has no
//! policy file). `<when>` is the UTC time to the second, RFC 3339: the
//! chore re-verification rule (0.22 D2) reads it. The digest covers the brief with every `Approved:` line
//! removed ([`crate::brief::brief_digest`]), so any later edit to the
//! brief makes every reader say `BRIEF_APPROVAL_STALE`.
//!
//! The line also records the code tree of the work tree at that moment (0.37,
//! `(code tree <id>)` before the digest, outside it, [`crate::brief::approval_tree`]):
//! the tree an evidence record binds, uncommitted code included. Outside a git
//! work tree, or with no commit, the line is written without it. Re-approving
//! records the tree of that moment; a repeat of the same approval over the same
//! code writes nothing.
//!
//! Refused (exit 2) while `canon brief check` has blocker findings, and
//! for an empty `--approver`/`--note`; exit 1 for an operational error. It takes
//! one or more change ids (0.31): one name and answer for all, every id checked
//! before any is written ([`approve_all`]), and one refused id refuses the call
//! (the first refused id's code, with `details` for each). Existing
//! `Approved:` lines are replaced, so re-approving leaves exactly one.
//! Canon cannot tell a human from an agent running this command: the
//! skill makes it a human stop, and `--note` carries their words.

use std::collections::BTreeMap;
use std::path::Path;

use canon_model::{ApprovalRelay, ChangeId};
use chrono::Utc;
use schemars::JsonSchema;
use serde::Serialize;
use sha2::{Digest, Sha256};

use crate::json_out::Report;
use crate::brief::brief_digest;
use crate::brief_check::{check, Severity};
use crate::cli_error::{CliError, CliErrorExt as _};
use crate::context::resolve_repo_root;
use crate::policy_approval::ApprovalState;
use canon_ingest::markdown::{field, without_field};

/// A brief that cannot carry an `Approved:` line.
#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum ApprovedTextError {
    /// The brief ends inside an open code fence or HTML comment.
    #[error("the brief ends inside an open code fence or HTML comment, so an `Approved:` line appended there would not be read; close it first")]
    UnreadableLine,
}

/// `text` approved: its `Approved:` lines replaced by one that binds the
/// result's remaining text. A `relay` (0.32) puts its marker before the
/// digest, outside it; a `tree` (0.37, the code tree of the work tree at the
/// approval) puts `(code tree <id>)` after the relay marker, before the digest
/// and outside it, so it never changes what the approval binds. `Err` when the
/// line would not be read back (the brief ends inside an open code fence or HTML comment).
pub fn approved_text(text: &str, by: &str, when: &str, note: &str, relay: Option<&ApprovalRelay>, tree: Option<&str>) -> Result<String, ApprovedTextError> {
    let body = format!("{}\n\n", without_field(text, "Approved:").trim_end());
    let value = format!("{by} {when} — \"{note}\"{}{} (brief {})", relay_part(relay), tree_part(tree), brief_digest(&body));
    let approved = format!("{body}Approved: {value}\n");
    if field(&approved, "Approved:") != Some(value.as_str()) || brief_digest(&approved) != brief_digest(&body) {
        return Err(ApprovedTextError::UnreadableLine);
    }
    Ok(approved)
}

/// ` (relayed by …)` for a relayed approval, nothing for the human's own.
fn relay_part(relay: Option<&ApprovalRelay>) -> String {
    relay.map(|r| format!(" {}", crate::relay::marker(r))).unwrap_or_default()
}

/// ` (code tree <id>)` when the approval recorded the code tree, nothing otherwise.
fn tree_part(tree: Option<&str>) -> String {
    tree.map(|tree| format!(" {}", crate::brief::tree_marker(tree))).unwrap_or_default()
}

/// The digest `brief approve` records for the brief `text`: what `canon brief
/// show --body` prints and a relayed approval names. It is the digest of the
/// text without its `Approved:` lines, normalised the way the approval writes
/// it (one blank line at the end), so trailing whitespace does not change it.
pub fn approval_digest(text: &str) -> String {
    brief_digest(&format!("{}\n\n", without_field(text, "Approved:").trim_end()))
}

/// What one approval recorded: the `brief approve --json` data of one id.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct Approved {
    pub change: String,
    pub brief: String,
    pub digest: String,
    pub approved_by: String,
    pub policy_approved: bool,
    /// Set when an agent recorded the approval for the human (0.32).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relay: Option<ApprovalRelay>,
}

impl Report for Approved {}

/// What happened to one id of a call with several ([`ApprovedMany`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, JsonSchema)]
#[serde(rename_all = "lowercase")]
pub enum Outcome {
    /// The brief and the policy now carry the approval.
    Written,
    /// They already carried this very approval: nothing was written.
    Unchanged,
    /// Writing this id failed (a disk error after the checks passed).
    Failed,
    /// An earlier id failed to write, so this one was not tried.
    Skipped,
}

/// One id of an [`ApprovedMany`].
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApprovalEntry {
    pub change: String,
    pub outcome: Outcome,
    /// The brief file; omitted for a `skipped` id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub brief: Option<String>,
    /// The digest the approval binds; omitted for a `failed` or `skipped` id.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub digest: Option<String>,
    /// Why a `failed` id failed.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub error: Option<String>,
}

/// `brief approve --json` data for several ids: one answer, one entry per id.
#[derive(Debug, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ApprovedMany {
    pub approved_by: String,
    pub policy_approved: bool,
    /// Set when an agent recorded the approvals for the human (0.32).
    #[serde(skip_serializing_if = "Option::is_none")]
    pub relay: Option<ApprovalRelay>,
    pub approvals: Vec<ApprovalEntry>,
}

impl Report for ApprovedMany {}

/// The next step for each code [`approve`] refuses with.
fn hint_for(code: &str) -> &'static str {
    match code {
        "APPROVAL_INVALID" => "pass --approver <name> and --note <the human's answer>, each one non-empty line",
        "BRIEF_INVALID" => "fix the brief as the message says (`canon brief check` re-reads it), then run `canon brief approve` again",
        "BRIEF_DENIED" => "make brief.md readable by this user, then run `canon brief approve` again",
        "BRIEF_UNAVAILABLE" => "the system failed the read of brief.md in a way a repeat can pass: wait a moment, then run `canon brief approve` again",
        "BRIEF_NOT_FOUND" => "check that the change has a brief.md, then run `canon brief approve` again",
        "RELAY_DIGEST_STALE" => "run `canon brief show <change> --body` again, put the brief and its digest in front of the human, ask again, and relay the digest of what they saw",
        "POLICY_APPROVAL_FAILED" => "check that .canon/policy.yaml is readable and writable, then run `canon brief approve` again",
        _ => "check that the change's brief.md is writable, then run `canon brief approve` again",
    }
}

fn refuse(code: &'static str, message: &str) -> CliError {
    CliError::failed(code, message).hint(hint_for(code))
}

/// `sha256` of `bytes` as lowercase hex: what a page that rendered a brief
/// sends back to say which file it showed.
pub fn sha256_hex(bytes: &[u8]) -> String {
    format!("{:x}", Sha256::digest(bytes))
}

/// How a call records its approval beyond the human's own: the relay (0.32)
/// and, for a relay, the digest of each brief the human was shown.
#[derive(Debug, Clone, Copy, Default)]
pub struct Opts<'a> {
    pub relay: Option<&'a ApprovalRelay>,
    /// `change id -> shown digest`; required for every id of a relayed call.
    pub shown: Option<&'a BTreeMap<String, String>>,
}

/// One id of a call, and the sha256 of the brief file the caller showed the
/// human, when it did (the dashboard's briefs page).
#[derive(Debug, Clone, Copy)]
pub struct Item<'a> {
    pub change: Option<&'a ChangeId>,
    pub expected_sha256: Option<&'a str>,
}

/// A brief that passed every check and waits to be written.
struct Checked {
    change: ChangeId,
    brief: String,
    path: std::path::PathBuf,
    text: String,
    /// The brief and the policy already carry this very approval.
    unchanged: bool,
    /// The code tree the approval records (0.37), when there is one.
    tree: Option<String>,
}

/// The checks of one approval, none of which writes: the answer is one line,
/// the brief exists and reads, `canon brief check` has no blocker, the page's
/// copy (when there is one) is the file, and an `Approved:` line can be
/// appended where it will be read.
fn check_one(repo: &Path, item: Item<'_>, by: &str, note: &str, policy_approved: bool, opts: Opts<'_>, tree: Option<&str>) -> Result<Checked, CliError> {
    let checked = check(repo, item.change)?;
    let path = repo.join(&checked.brief);
    if let Some(shown) = opts.shown {
        verify_shown(&checked.change, &checked.brief, &path, shown)?;
    }
    if let Some(expected) = item.expected_sha256 {
        verify_bytes(&checked.brief, std::fs::read(&path).ok().as_deref(), expected)?;
    }
    // Coverage rules (a flow step with no scenario or no failure case) gate
    // shipping, not approval: the human approves the scope before its
    // scenarios exist, and re-approves a changed scope mid-build (0.19 D3).
    let blockers = approval_blockers(&checked.evaluation.findings);
    if !blockers.is_empty() {
        let listed = blockers.iter().map(|f| format!("{} line {}", f.rule, f.line)).collect::<Vec<_>>().join(", ");
        return Err(refuse(
            "BRIEF_INVALID",
            &format!("{} has {} blocker finding(s): {listed}; `canon brief check` shows them, and the human approves a brief only once they are cleared", checked.brief, blockers.len()),
        )
        .reason("BLOCKERS")
        .hint("run `canon brief check`, clear the blockers it lists, then ask the human to approve"));
    }
    let text = canon_store::eio::read_to_string(&path).map_err(|e| crate::read_cause::brief_unreadable(refuse(crate::read_cause::brief_code(&e), &format!("cannot read `{}`: {e}", checked.brief))))?;
    if let Some(expected) = item.expected_sha256 {
        verify_bytes(&checked.brief, Some(text.as_bytes()), expected)?;
    }
    // The same approval again, by the same human, on a brief and a policy
    // that already carry it: nothing to write.
    let unchanged = already_approved_so(&text, by, note, opts.relay, tree) && (!policy_approved || crate::policy_approval::matching_approval(repo, by, Some(note), opts.relay).is_some());
    if !unchanged {
        approved_text(&text, by, "1970-01-01T00:00:00Z", note, opts.relay, tree).map_err(|e| refuse("BRIEF_INVALID", &format!("{}: {e}", checked.brief)).reason("UNAPPROVABLE"))?;
        // What the approval will write is checked here, with the rest, so an unwritable brief or approval file refuses before any id is written.
        let approval_file = repo.join(canon_model::paths::POLICY_APPROVAL_FILE);
        let targets: Vec<&Path> = if policy_approved { vec![approval_file.as_path(), path.as_path()] } else { vec![path.as_path()] };
        for target in targets {
            crate::write_mode::check_writable(target).map_err(|e| crate::write_mode::unwritable(target, &e))?;
        }
    }
    Ok(Checked { change: checked.change, brief: checked.brief, path, text, unchanged, tree: tree.map(str::to_string) })
}

/// `Err` unless the digest the human was shown for `change` (`shown`) is the
/// digest of the brief file now: a relayed approval is of what they read.
fn verify_shown(change: &ChangeId, brief: &str, path: &Path, shown: &BTreeMap<String, String>) -> Result<(), CliError> {
    let Some(digest) = shown.get(change.as_str()) else {
        return Err(CliError::refused("USAGE", format!("a relayed approval of `{0}` needs the digest the human was shown: pass --digest {0}=<the digest `canon brief show {0} --body` printed>", change.as_str())).field("--digest").hint(hint_for("RELAY_DIGEST_STALE")));
    };
    let text = canon_store::eio::read_to_string(path).map_err(|e| crate::read_cause::brief_unreadable(refuse(crate::read_cause::brief_code(&e), &format!("cannot read `{brief}`: {e}"))))?;
    let now = approval_digest(&text);
    if digest.eq_ignore_ascii_case(&now) {
        return Ok(());
    }
    Err(CliError::refused("RELAY_DIGEST_STALE", format!("{brief} is {now} now, but the human was shown {digest}; the human must answer for the brief as it is, so nothing was recorded")).field("--digest").hint(hint_for("RELAY_DIGEST_STALE")))
}

/// The answer's own two lines, trimmed: refused up front, for every id alike.
fn answer(by: &str, note: &str) -> Result<(String, String), CliError> {
    let (by, note) = (by.trim(), note.trim());
    for (flag, value) in [("--approver", by), ("--note", note)] {
        if value.is_empty() {
            return Err(refuse("APPROVAL_INVALID", &format!("{flag} must carry the human's answer, not be empty")).field(flag));
        }
        if value.contains(['\n', '\r']) {
            return Err(refuse("APPROVAL_INVALID", &format!("{flag} must be one line")).field(flag));
        }
    }
    Ok((by.to_string(), note.to_string()))
}

/// Writes one approved brief (and, first, the policy approval) at `when`, recording the code tree `checked` saw.
fn write_one(repo: &Path, checked: Checked, by: &str, note: &str, when: &str, policy_approved: bool, relay: Option<&ApprovalRelay>) -> Result<Approved, CliError> {
    let approved = approved_text(&checked.text, by, when, note, relay, checked.tree.as_deref()).map_err(|e| refuse("BRIEF_INVALID", &e.to_string()).reason("UNAPPROVABLE"))?;
    // Policy first: a failed policy write leaves the brief untouched, and a
    // failed brief write leaves a policy approval that is true on its own.
    if policy_approved {
        crate::policy_approval::record_at(repo, by, Some(note), when, relay).map_err(|e| refuse("POLICY_APPROVAL_FAILED", &e.to_string()))?;
    }
    std::fs::write(&checked.path, &approved).map_err(|e| refuse("BRIEF_WRITE_FAILED", &format!("cannot write `{}`: {e}", checked.brief)))?;
    Ok(Approved { change: checked.change.as_str().to_string(), brief: checked.brief, digest: brief_digest(&approved), approved_by: by.to_string(), policy_approved, relay: relay.cloned() })
}

fn already(checked: Checked, by: &str, policy_approved: bool, relay: Option<&ApprovalRelay>) -> Approved {
    Approved { change: checked.change.as_str().to_string(), brief: checked.brief, digest: brief_digest(&checked.text), approved_by: by.to_string(), policy_approved, relay: relay.cloned() }
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
    let (by, note) = answer(by, note)?;
    let policy_approved = !matches!(crate::policy_approval::approval_state(repo), ApprovalState::NoPolicy);
    // The code tree of the work tree now (0.37), `None` without a git commit: the approval records the code it saw.
    let tree = crate::git_rev::code_tree_if_committed(repo)?;
    let checked = check_one(repo, Item { change, expected_sha256 }, &by, &note, policy_approved, Opts::default(), tree.as_deref())?;
    if checked.unchanged {
        return Ok((already(checked, &by, policy_approved, None), true));
    }
    let when = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    write_one(repo, checked, &by, &note, &when, policy_approved, None).map(|approved| (approved, false))
}

/// The error of a call with several ids when any is refused (0.31): the first refused id's own code and reason, with an
/// `error.details` entry for each refused id (spec 7.2.2); no `data`, and nothing was written.
fn refused_many(refused: Vec<(String, CliError)>) -> CliError {
    let hint = "fix what each refused id names (`canon brief check` lists blocker findings, a missing brief needs `canon change new`, an unclosed fence needs closing), then ask the human to approve again";
    let named = |id: &str, e: &CliError| {
        let error = CliError::refused(e.code, format!("`{id}`: {}", e.message)).hint(hint);
        match e.reason_name() {
            Some(reason) => error.reason(reason),
            None => error,
        }
    };
    let all: Vec<CliError> = refused.iter().map(|(id, e)| named(id, e)).collect();
    let first = all[0].clone();
    first.details(all)
}

/// What [`approve_all`] recorded, id by id, in the order given.
#[derive(Debug)]
pub struct Many {
    pub approved_by: String,
    pub policy_approved: bool,
    pub relay: Option<ApprovalRelay>,
    pub entries: Vec<(ApprovalEntry, Option<Approved>)>,
    /// The first write failure, when one stopped the call.
    pub failure: Option<CliError>,
}

/// Approves every id of `items` with one name and one answer, at one time,
/// all or nothing on refusals (0.31): every id is checked before any is
/// written, and one refused id refuses the call with the first refused id's code
/// with `details` for each refused id. Then the ids are written in the order given,
/// exactly as one id is; a write failure (exit 1) leaves the ids already
/// written approved and skips the rest. A repeated id counts once.
pub fn approve_all(repo: &Path, items: &[Item<'_>], by: &str, note: &str) -> Result<Many, CliError> {
    approve_all_with(repo, items, by, note, Opts::default())
}

/// [`approve_all`] with [`Opts`]: a relayed call (0.32) writes the marker and
/// refuses an id whose shown digest is missing or differs.
pub fn approve_all_with(repo: &Path, items: &[Item<'_>], by: &str, note: &str, opts: Opts<'_>) -> Result<Many, CliError> {
    let (by, note) = answer(by, note)?;
    let mut distinct: Vec<Item<'_>> = Vec::new();
    for item in items {
        if item.change.is_none() || !distinct.iter().any(|seen| seen.change == item.change) {
            distinct.push(*item);
        }
    }
    let policy_approved = !matches!(crate::policy_approval::approval_state(repo), ApprovalState::NoPolicy);
    let tree = crate::git_rev::code_tree_if_committed(repo)?;
    let mut checked: Vec<Checked> = Vec::new();
    let mut refused: Vec<(String, CliError)> = Vec::new();
    for item in &distinct {
        match check_one(repo, *item, &by, &note, policy_approved, opts, tree.as_deref()) {
            Ok(one) => checked.push(one),
            Err(e) => refused.push((item.change.map_or_else(|| "<the only live change>".to_string(), |c| c.as_str().to_string()), e)),
        }
    }
    if distinct.len() == 1 {
        if let Some((_, e)) = refused.pop() {
            return Err(e);
        }
    } else if !refused.is_empty() {
        return Err(refused_many(refused));
    }
    let when = Utc::now().to_rfc3339_opts(chrono::SecondsFormat::Secs, true);
    let mut entries: Vec<(ApprovalEntry, Option<Approved>)> = Vec::new();
    let mut failure: Option<CliError> = None;
    for one in checked {
        let change = one.change.as_str().to_string();
        if failure.is_some() {
            entries.push((ApprovalEntry { change, outcome: Outcome::Skipped, brief: None, digest: None, error: None }, None));
            continue;
        }
        if one.unchanged {
            let done = already(one, &by, policy_approved, opts.relay);
            entries.push((ApprovalEntry { change, outcome: Outcome::Unchanged, brief: Some(done.brief.clone()), digest: Some(done.digest.clone()), error: None }, Some(done)));
            continue;
        }
        let brief = one.brief.clone();
        match write_one(repo, one, &by, &note, &when, policy_approved, opts.relay) {
            Ok(done) => entries.push((ApprovalEntry { change, outcome: Outcome::Written, brief: Some(done.brief.clone()), digest: Some(done.digest.clone()), error: None }, Some(done))),
            Err(e) => {
                entries.push((ApprovalEntry { change, outcome: Outcome::Failed, brief: Some(brief), digest: None, error: Some(e.message.clone()) }, None));
                failure = Some(e);
            }
        }
    }
    Ok(Many { approved_by: by, policy_approved, relay: opts.relay.cloned(), entries, failure })
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

/// `canon brief approve`: the `--digest` entries are `<change>=sha256:<digest>` and name a change
/// once; a relayed approval names, for every change it approves, the digest the human was shown,
/// and no digest of another change.
pub(crate) fn check_approve(args: &crate::cli::brief::BriefApproveArgs) -> Result<(), CliError> {
    crate::relay::check_arguments("canon brief approve", &crate::relay::Call { via: args.relayed_via.as_deref(), actor: args.actor.as_deref(), approver: Some(&args.approver), note: Some(&args.note) })?;
    // The human's answer: one line each, not blank (a relayed blank answer was refused above, with its own code).
    answer(&args.approver, &args.note)?;
    let shown = shown_digests(&args.digests)?;
    if args.relayed_via.is_some() {
        // Every id names the digest its brief showed, and nothing else.
        let missing = args.change_ids.iter().find(|c| !shown.contains_key(c.as_str()));
        let stray = shown.keys().find(|k| !args.change_ids.is_empty() && !args.change_ids.iter().any(|c| c.as_str() == k.as_str()));
        let refusal = match (missing, stray) {
            (Some(c), _) => Some(format!("a relayed approval of `{0}` needs the digest the human was shown: pass --digest {0}=<the digest `canon brief show {0} --body` printed>", c.as_str())),
            (_, Some(k)) => Some(format!("--digest names `{k}`, which is not one of the changes approved")),
            _ => None,
        };
        if let Some(message) = refusal {
            return Err(CliError::refused("USAGE", message).field("--digest").hint(hint_for("RELAY_DIGEST_STALE")));
        }
    }
    Ok(())
}

/// The digests a relayed `canon brief approve` names, by change. `Err` for an entry that is not
/// `<change>=sha256:<digest>` and for a change named twice with different digests.
pub(crate) fn shown_digests(entries: &[String]) -> Result<BTreeMap<String, String>, CliError> {
    let mut shown: BTreeMap<String, String> = BTreeMap::new();
    for entry in entries {
        let Some((change, digest)) = entry.split_once('=').filter(|(c, d)| !c.is_empty() && d.starts_with("sha256:")) else {
            return Err(CliError::refused("USAGE", format!("--digest takes `<change>=<sha256:digest>`, got `{entry}`")).field("--digest").hint("copy the change id and the `digest:` line `canon brief show <change> --body` printed"));
        };
        if let Some(earlier) = shown.insert(change.to_string(), digest.to_string()).filter(|earlier| earlier != digest) {
            return Err(CliError::refused("USAGE", format!("--digest names `{change}` twice with different digests ({earlier} and {digest})")).field("--digest").hint("pass one digest per change"));
        }
    }
    Ok(shown)
}

/// `canon brief approve`. Returns the process exit code. One id (or none: the
/// only live change) prints what it always printed; several print one entry
/// each.
pub fn run_approve(repo: &Path, changes: &[ChangeId], by: &str, note: &str, relay: Option<&ApprovalRelay>, shown: &BTreeMap<String, String>, json: bool) -> u8 {
    let repo = resolve_repo_root(repo);
    let items: Vec<Item<'_>> = if changes.is_empty() { vec![Item { change: None, expected_sha256: None }] } else { changes.iter().map(|change| Item { change: Some(change), expected_sha256: None }).collect() };
    let opts = Opts { relay, shown: relay.map(|_| shown) };
    let many = match approve_all_with(&repo, &items, by, note, opts) {
        Ok(many) => many,
        Err(e) => return e.emit("canon brief approve", json),
    };
    if let [(_, Some(approved))] = many.entries.as_slice() {
        let unchanged = many.entries[0].0.outcome == Outcome::Unchanged;
        if unchanged {
            crate::json_out::mark_unchanged();
        }
        if !json {
            let policy = if approved.policy_approved { "; policy approval recorded too" } else { "; no policy file, so no policy approval" };
            let verb = if unchanged { "already approved, unchanged" } else { "approved" };
            let via = approved.relay.as_ref().map(|r| format!(" ({})", crate::relay::describe(r))).unwrap_or_default();
            println!("canon brief approve: {} {verb} by `{}`{via} ({}){policy}", approved.brief, approved.approved_by, approved.digest);
        }
        crate::next::finish(&repo, "brief approve", &crate::next::Scope::Change(approved.change.clone()), false, json, approved);
        return 0;
    }
    if let (Some(e), 1) = (&many.failure, many.entries.len()) {
        return e.emit("canon brief approve", json);
    }
    let all_unchanged = many.failure.is_none() && many.entries.iter().all(|(entry, _)| entry.outcome == Outcome::Unchanged);
    if all_unchanged {
        crate::json_out::mark_unchanged();
    }
    let report = ApprovedMany { approved_by: many.approved_by.clone(), policy_approved: many.policy_approved, relay: many.relay.clone(), approvals: many.entries.into_iter().map(|(entry, _)| entry).collect() };
    if !json {
        let policy = match (report.policy_approved, &many.failure) {
            (true, None) => "; policy approval recorded too",
            (true, Some(_)) => "; the policy approval may not be recorded (see the error)",
            (false, _) => "; no policy file, so no policy approval" };
        for entry in &report.approvals {
            let what = match entry.outcome {
                Outcome::Written => "approved",
                Outcome::Unchanged => "already approved, unchanged",
                Outcome::Failed => "failed",
                Outcome::Skipped => "skipped",
            };
            println!("canon brief approve: {} {what} by `{}`{}", entry.change, report.approved_by, entry.error.as_ref().map(|e| format!(" — {e}")).unwrap_or_default());
        }
        let via = report.relay.as_ref().map(|r| format!(" ({})", crate::relay::describe(r))).unwrap_or_default();
        println!("canon brief approve: {} change(s){via}{policy}", report.approvals.len());
    }
    if let Some(e) = many.failure {
        let named = report.approvals.iter().map(|entry| format!("`{}` {}", entry.change, match entry.outcome { Outcome::Written => "written", Outcome::Unchanged => "unchanged", Outcome::Failed => "failed", Outcome::Skipped => "skipped" })).collect::<Vec<_>>().join(", ");
        let message = format!("{}; {named}; the ids written stay approved and the rest were skipped", e.message);
        // A stop after some ids were written is a stopped batch (exit 1, the report in `data`), whatever the stopping id's own code: a refusal carries no `data`.
        let error = if crate::cli_error::code_info(e.code).is_some_and(|info| info.exit == crate::cli_error::EXIT_REFUSED) {
            let at = report.approvals.iter().position(|entry| entry.outcome == Outcome::Failed).unwrap_or(0);
            CliError::failed("BATCH_INCOMPLETE", format!("the batch stopped at element {at}: {}: {message}", e.code)).hint("`data.approvals` says what each id did: fix the failed one, then run the command again (the written ones report `unchanged`)").field(format!("/{at}"))
        } else {
            refuse(e.code, &message)
        };
        return if json { error.emit_with_data("canon brief approve", true, &report) } else { error.emit("canon brief approve", false) };
    }
    crate::next::finish(&repo, "brief approve", &crate::next::Scope::Repo, false, json, &report);
    0
}

/// Whether `text` already carries an `Approved:` line by `by` with `note`
/// that binds the brief's current body and records the code tree `tree`: the
/// line a repeat of the same approval would write, up to the time it was
/// written. The same words over other code are a new approval (0.37): it
/// records the tree it saw.
fn already_approved_so(text: &str, by: &str, note: &str, relay: Option<&ApprovalRelay>, tree: Option<&str>) -> bool {
    let body = format!("{}\n\n", without_field(text, "Approved:").trim_end());
    let tail = format!(" — \"{note}\"{}{} (brief {})", relay_part(relay), tree_part(tree), brief_digest(&body));
    field(text, "Approved:")
        .and_then(|value| value.strip_prefix(by))
        .and_then(|value| value.strip_prefix(' '))
        .and_then(|value| value.strip_suffix(tail.as_str()))
        .is_some_and(|when| !when.is_empty() && !when.contains(char::is_whitespace))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_brief_ending_inside_a_fence_is_refused_in_its_old_words() {
        let error = approved_text("# Brief\n\n```\nunclosed\n", "me", "2026-01-01T00:00:00Z", "ok", None, None).unwrap_err();
        assert_eq!(error.to_string(), "the brief ends inside an open code fence or HTML comment, so an `Approved:` line appended there would not be read; close it first");
    }
}
