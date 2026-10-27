//! `canon migrate`'s brief rewrite (0.31 `brief-format`, F5 and F6): a change
//! brief in the format canon 0.30 and earlier wrote becomes a brief in the
//! 0.31 format, and an approved brief stays approved.
//!
//! # What is rewritten
//! Per brief under `.canon/changes/` and `.canon/changes/archive/` that
//! [`canon_ingest::brief`] reads as old-format:
//! - the `canon:section`, `canon:hint` and `canon:kind` comments and the
//!   approval note comment (English, Korean and the pre-0.19 `--by` form)
//!   are removed;
//! - each `canon:waive` comment becomes an entry of the frontmatter `waive`
//!   list, `reason` and `match` kept, one entry for the same rule, section,
//!   reason and match;
//! - `canon:kind chore` becomes `kind: chore` in the frontmatter;
//! - a shipped Korean heading becomes the English heading of the same key.
//!
//! Every other byte of the prose stays. A brief this cannot rewrite safely (a
//! `canon:` comment sharing a line with text, a waiver comment that does not
//! parse, a brief that already has a frontmatter) is named and left as it is.
//!
//! # What is re-bound
//! The approval and the review pins are the human's. A brief's `Approved:`
//! digest is re-bound to the new text, and a chore review's pin on the old
//! brief digest to the new one, only when the old digest matches the old text
//! (the brief was not edited after its approval) and the rewrite changes
//! nothing but the above: the sections read (key, role and body) and the set
//! the rules judge are the same before and after ([`equivalent`]). A brief
//! edited after its approval, one whose marker role differs from the table and
//! one with a marker of a key canon does not know is rewritten but not
//! re-bound; the plan names it. The `Approved:` line keeps its approver, time
//! and words; only the digest changes. A review in the committed ledger gets a
//! new version with every field but the pin as before, the old version moves
//! to the archive; a review in a hot or cold store is named, not changed.
//!
//! # Applying, stopping, reverting
//! [`apply`] writes the journal (old and new digests, old and new bytes
//! hashes) first, the reviews before the briefs, and marks the journal
//! complete last, so a stopped run is finished by the next ([`resume_backup`]).
//! The whole-tree snapshot of `canon migrate` holds the old files;
//! [`check_revert`] refuses a revert while a rewritten file changed since the
//! migrate (a later approval, edit or review version), naming it.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};

use canon_ingest::brief::{self as reader, Brief, Kind, Role};
use canon_model::{paths, PinnedSpec, RecordKind, Review};
use canon_store::git_tier::GitTier;
use canon_store::partition::expected_relative_path;
use canon_store::tier::{StoredRecord, Tier, TierQuery};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::safety::Effect;

/// The `HUMAN_ONLY` refusal of `command` for a declared agent role (a plan that
/// re-binds an `Approved:` line or a review pin is the human's), `None` in the
/// human's own shell.
pub fn human_only(command: &str, human_call: &str) -> Option<crate::cli_error::CliError> {
    let role = crate::role::Role::current(None)?;
    Some(
        crate::cli_error::CliError::refused("HUMAN_ONLY", format!("`{command}` re-binds an `Approved:` line or a review pin, a human's decision, and is not available to the {role} role"))
            .field("--as")
            .hint(format!(
                "report this exact call to the human and wait; the human runs it in their own shell: `{human_call}`; never remove your role (`env -u CANON_ROLE`, dropping --as) to run it yourself"
            )),
    )
}

/// The journal in the backup directory: what [`check_revert`] and the next run read.
pub const JOURNAL: &str = "briefs-journal.json";

fn sha256_hex(bytes: &[u8]) -> String {
    Sha256::digest(bytes).iter().map(|b| format!("{b:02x}")).collect()
}

/// A shipped approval note comment, in any of its forms.
fn is_approval_note(body: &str) -> bool {
    body.contains("canon brief approve") && (body.contains("--approver") || body.contains("--by")) && !body.starts_with("canon:")
}

/// One waiver comment, as the old format read it.
#[derive(Debug, Clone, PartialEq, Eq)]
struct OldWaiver {
    rule: String,
    section: String,
    reason: String,
    match_text: Option<String>,
}

/// Splits `body` (the text after a waiver's opening quote) into the reason
/// and what follows its closing quote: the closing quote is the last one that
/// ends the body or is followed by whitespace and `match="`.
fn split_reason(body: &str) -> Option<(&str, &str)> {
    if let Some(at) = body.rfind("\" match=\"") {
        let tail = &body[at + 1..];
        if tail.trim_start().starts_with("match=\"") {
            return Some((&body[..at], tail));
        }
    }
    let inner = body.trim_end().strip_suffix('"')?;
    Some((inner, ""))
}

/// `canon:waive <rule> <section> "<reason>" [match="<text>"]`, as canon 0.30 wrote it.
fn parse_waive(body: &str) -> Option<OldWaiver> {
    let rest = body.trim_start_matches("canon:waive").trim();
    let (rule, rest) = rest.split_once(char::is_whitespace)?;
    let (section, rest) = rest.trim_start().split_once(char::is_whitespace)?;
    let quoted = rest.trim().strip_prefix('"')?;
    let (reason, tail) = split_reason(quoted)?;
    let match_text = match tail.trim() {
        "" => None,
        t => {
            let m = t.strip_prefix("match=\"")?.strip_suffix('"')?;
            (!m.trim().is_empty() && !m.contains('"')).then(|| m.to_string())?.into()
        }
    };
    (!reason.trim().is_empty() && !reason.contains('"')).then(|| OldWaiver { rule: rule.to_string(), section: section.to_string(), reason: reason.trim().to_string(), match_text })
}

/// What reading the old text by its markers (the way canon 0.30 did) found in one section.
type ReadSection = (String, Role, Vec<String>);

/// The sections of an old-format brief by their `canon:section` markers: key,
/// effective role (the marker's, else the table's) and the non-blank lines
/// after the section's heading.
fn read_by_markers(brief: &Brief) -> Vec<ReadSection> {
    let mut markers: Vec<(usize, String, Option<Role>)> = Vec::new();
    for comment in &brief.comments {
        let mut tokens = comment.body.split_whitespace();
        if tokens.next() != Some("canon:section") {
            continue;
        }
        let Some(key) = tokens.next() else { continue };
        let role = tokens.find_map(|t| t.strip_prefix("role=")).and_then(Role::parse);
        markers.push((comment.line, key.to_string(), role));
    }
    markers.sort_by_key(|m| m.0);
    markers
        .iter()
        .enumerate()
        .map(|(i, (line, key, role))| {
            let end = markers.get(i + 1).map_or(usize::MAX, |m| m.0);
            let effective = role.or_else(|| reader::canonical_role(key)).unwrap_or(Role::Context);
            let mut heading_dropped = false;
            let body = brief
                .lines
                .iter()
                .filter(|l| l.no > *line && l.no < end && !l.text.trim().is_empty())
                .filter(|l| {
                    if !heading_dropped && l.text.trim_start().starts_with('#') {
                        heading_dropped = true;
                        return false;
                    }
                    true
                })
                .map(|l| l.text.clone())
                .collect();
            (key.clone(), effective, body)
        })
        .collect()
}

/// The sections of a 0.31 brief by their headings, in the same shape.
fn read_by_headings(brief: &Brief) -> Vec<ReadSection> {
    brief.sections.iter().map(|s| (s.key.to_string(), s.role, brief.section_lines(s).iter().skip(1).filter(|l| !l.text.trim().is_empty()).map(|l| l.text.clone()).collect())).collect()
}

/// The keys the rules judge: every section of a feature brief, the chore set's of a chore.
fn judged(kind: Kind, sections: &[ReadSection]) -> Vec<String> {
    sections.iter().filter(|(key, _, _)| kind == Kind::Feature || kind.sections().iter().any(|def| def.key == key)).map(|(key, _, _)| key.clone()).collect()
}

/// Whether the rewrite keeps the brief's reading: the same kind, the same
/// sections (key, role, body) and the same judged set. `Err` says what differs.
fn equivalent(old: &Brief, new: &Brief) -> Result<(), String> {
    let (before, after) = (read_by_markers(old), read_by_headings(new));
    if old.kind != new.kind {
        return Err("the brief would read as a different kind".to_string());
    }
    let describe = |sections: &[ReadSection]| sections.iter().map(|(key, _, _)| key.as_str()).collect::<Vec<_>>().join(", ");
    if before.iter().map(|s| &s.0).ne(after.iter().map(|s| &s.0)) {
        let unknown: Vec<&str> = before.iter().filter(|(key, _, _)| reader::canonical_role(key).is_none()).map(|(key, _, _)| key.as_str()).collect();
        return Err(if unknown.is_empty() {
            format!("its sections ({}) would read as ({}) by heading: a marker and its heading name different sections, or a heading is not the shipped one", describe(&before), describe(&after))
        } else {
            format!("it has a marker of a key canon does not know ({})", unknown.join(", "))
        });
    }
    for ((key, role_before, body_before), (_, role_after, body_after)) in before.iter().zip(&after) {
        if role_before != role_after {
            return Err(format!("the marker of `{key}` says role={} and the table says {}, so the judgement would change", role_before.as_str(), role_after.as_str()));
        }
        if body_before != body_after {
            return Err(format!("the body of `{key}` would read differently"));
        }
    }
    if judged(old.kind, &before) != judged(new.kind, &after) {
        return Err("the set of judged sections would change".to_string());
    }
    Ok(())
}

/// A brief rewritten.
#[derive(Debug, Clone)]
struct Rewrite {
    text: String,
    comments: usize,
    waivers: Vec<OldWaiver>,
    chore: bool,
    titles: Vec<(String, String)>,
}

fn json_text(value: &str) -> String {
    serde_json::to_string(value).unwrap_or_else(|_| format!("\"{value}\""))
}

fn frontmatter(chore: bool, waivers: &[OldWaiver]) -> String {
    if !chore && waivers.is_empty() {
        return String::new();
    }
    let mut out = String::from("---\n");
    if chore {
        out.push_str("kind: chore\n");
    }
    if !waivers.is_empty() {
        out.push_str("waive:\n");
        for waiver in waivers {
            out.push_str(&format!("  - rule: {}\n    section: {}\n    reason: {}\n", json_text(&waiver.rule), json_text(&waiver.section), json_text(&waiver.reason)));
            if let Some(m) = &waiver.match_text {
                out.push_str(&format!("    match: {}\n", json_text(m)));
            }
        }
    }
    out.push_str("---\n\n");
    out
}

/// The 0.31 text of the old-format `old` (module doc), or why it is left alone.
fn rewrite(old: &str, brief: &Brief) -> Result<Rewrite, String> {
    if brief.frontmatter.is_some() {
        return Err("it already has a frontmatter; move its `canon:` comments by hand".to_string());
    }
    let raw: Vec<&str> = old.split_inclusive('\n').collect();
    let mut removed = vec![false; raw.len() + 1];
    let mut comments = 0;
    let mut waivers: Vec<OldWaiver> = Vec::new();
    let mut chore = false;
    for comment in &brief.comments {
        let first = comment.body.split_whitespace().next().unwrap_or_default();
        let canon = ["canon:section", "canon:hint", "canon:waive", "canon:kind"].contains(&first);
        if !canon && !(is_approval_note(&comment.body) && comment.whole_line) {
            continue;
        }
        if !comment.whole_line {
            return Err(format!("the `{first}` comment on line {} shares its line with text; remove it by hand", comment.line));
        }
        if first == "canon:waive" {
            let waiver = parse_waive(&comment.body).ok_or_else(|| format!("the waiver comment on line {} does not parse; fix or remove it by hand", comment.line))?;
            if !waivers.contains(&waiver) {
                waivers.push(waiver);
            }
        }
        if comment.body.split_whitespace().collect::<Vec<_>>() == ["canon:kind", "chore"] {
            chore = true;
        }
        for line in comment.line..=comment.end_line {
            if let Some(slot) = removed.get_mut(line) {
                *slot = true;
            }
        }
        comments += 1;
    }
    let mut titles: Vec<(String, String)> = Vec::new();
    let mut retitled: Vec<Option<String>> = vec![None; raw.len() + 1];
    for section in &brief.sections {
        let Some(def) = reader::def_of_shipped_ko_heading(&section.heading) else {
            continue;
        };
        let line = raw.get(section.line - 1).copied().unwrap_or_default();
        let ending = &line[line.trim_end_matches(['\r', '\n']).len()..];
        retitled[section.line] = Some(format!("## {}{ending}", def.heading));
        titles.push((section.heading.clone(), def.heading.to_string()));
    }

    let mut body = String::new();
    let (mut previous_blank, mut dropped, mut trailing_dropped) = (false, false, false);
    for (index, line) in raw.iter().enumerate() {
        let no = index + 1;
        if removed[no] {
            dropped = true;
            trailing_dropped = true;
            continue;
        }
        let blank = line.trim().is_empty();
        if blank && dropped && (previous_blank || body.is_empty()) {
            continue;
        }
        trailing_dropped = false;
        body.push_str(retitled[no].as_deref().unwrap_or(line));
        previous_blank = blank;
        if !blank {
            dropped = false;
        }
    }
    if trailing_dropped {
        body.truncate(body.trim_end_matches(['\r', '\n', ' ']).len());
        body.push('\n');
    }
    let text = format!("{}{body}", frontmatter(chore, &waivers));
    Ok(Rewrite { text, comments, waivers, chore, titles })
}

/// One brief to rewrite.
#[derive(Debug, Clone)]
pub struct BriefEntry {
    /// Repo-relative, `/`-separated.
    pub path: String,
    pub change: String,
    old_text: String,
    new_text: String,
    old_digest: String,
    new_digest: String,
    /// The `Approved:` digest is re-bound.
    rebind_approval: bool,
    comments: usize,
    waivers: usize,
    chore: bool,
    titles: Vec<(String, String)>,
    /// Why the approval is not re-bound, when it could have been.
    not_rebound: Option<String>,
}

/// One chore review whose pin moves to the new brief digest.
#[derive(Debug, Clone)]
pub struct ReviewEntry {
    /// Ledger-relative path of the version written and of the one replaced.
    from: String,
    to: String,
    /// Repo-relative, for the effects.
    from_path: String,
    to_path: String,
    old: Review,
    new: Review,
    change: String,
}

/// A brief or review named and left as it is.
#[derive(Debug, Clone)]
struct Kept {
    effect: Effect,
}

/// The brief rewrite of one `canon migrate` run.
#[derive(Debug, Clone, Default)]
pub struct Plan {
    pub briefs: Vec<BriefEntry>,
    pub reviews: Vec<ReviewEntry>,
    kept: Vec<Kept>,
}

impl BriefEntry {
    /// Whether the plan re-binds this brief's `Approved:` line.
    pub fn rebinds_approval(&self) -> bool {
        self.rebind_approval
    }
}

impl Plan {
    /// Nothing to rewrite or name.
    pub fn is_empty(&self) -> bool {
        self.briefs.is_empty() && self.reviews.is_empty() && self.kept.is_empty()
    }

    /// Whether anything is written.
    pub fn writes(&self) -> bool {
        !self.briefs.is_empty() || !self.reviews.is_empty()
    }

    /// Whether the plan re-binds an `Approved:` line or a review pin: then the
    /// commit is the human's.
    pub fn rebinds(&self) -> bool {
        self.briefs.iter().any(|b| b.rebind_approval) || !self.reviews.is_empty()
    }

    /// The repo-relative paths a commit rewrites, for the uncommitted-changes check.
    pub fn paths(&self) -> Vec<PathBuf> {
        self.briefs.iter().map(|b| PathBuf::from(&b.path)).chain(self.reviews.iter().map(|r| PathBuf::from(&r.from_path))).collect()
    }

    /// The plan as effects, in the order [`apply`] acts: reviews, then briefs, then what is named.
    pub fn effects(&self) -> Vec<Effect> {
        let mut out = Vec::new();
        for review in &self.reviews {
            out.push(Effect::path("rebind", review.from_path.clone()).detail(format!(
                "the review of chore `{}` is pinned to brief digest {}: a new version of the same review at {}, pinned to {}; the old version moves to the archive",
                review.change,
                pin_of(&review.old),
                review.to_path,
                pin_of(&review.new)
            )));
        }
        for brief in &self.briefs {
            let mut parts = vec![format!("removes {} comment(s)", brief.comments)];
            if brief.waivers > 0 {
                parts.push(format!("moves {} waiver(s) into the frontmatter", brief.waivers));
            }
            if brief.chore {
                parts.push("moves the chore marker to `kind: chore`".to_string());
            }
            if !brief.titles.is_empty() {
                parts.push(format!("translates {} heading(s) ({})", brief.titles.len(), brief.titles.iter().map(|(from, to)| format!("`{from}` to `{to}`")).collect::<Vec<_>>().join(", ")));
            }
            out.push(Effect::path("rewrite", brief.path.clone()).detail(parts.join("; ")));
            if brief.rebind_approval {
                out.push(Effect::path("rebind", brief.path.clone()).detail(format!("`Approved:` digest {} to {}; approver, time and words stay", brief.old_digest, brief.new_digest)));
            }
        }
        out.extend(self.kept.iter().map(|k| k.effect.clone()));
        out
    }

    /// The human lines of a preview: whether it re-binds, the condition, then each brief and review.
    pub fn lines(&self) -> Vec<String> {
        if self.is_empty() {
            return Vec::new();
        }
        let approvals = self.briefs.iter().filter(|b| b.rebind_approval).count();
        let mut out = vec![if self.rebinds() {
            format!(
                "canon migrate: briefs — this plan re-binds {approvals} `Approved:` line(s) and {} review pin(s); `--commit` is the human's (HUMAN_ONLY for every agent role), a preview is open to all",
                self.reviews.len()
            )
        } else {
            "canon migrate: briefs — this plan re-binds no `Approved:` line and no review pin; an orchestrator may run `--commit`".to_string()
        }];
        out.push("  a digest is re-bound only when the old digest matches the old text and the rewrite changes nothing but comments, hints, approval notes, waiver comments, the chore marker and shipped Korean headings: the sections read (key, role and body) and the judged set are the same before and after".to_string());
        for brief in &self.briefs {
            out.push(crate::safety::effect_line(&Effect::path("rewrite", brief.path.clone()).detail(format!(
                "{} comment(s), {} waiver(s) to the frontmatter{}{}",
                brief.comments,
                brief.waivers,
                if brief.chore { ", `kind: chore`" } else { "" },
                if brief.titles.is_empty() { String::new() } else { format!(", {} heading(s) to English", brief.titles.len()) }
            ))));
            if brief.rebind_approval {
                out.push(format!("    Approved: {} -> {}", brief.old_digest, brief.new_digest));
            }
            if let Some(why) = &brief.not_rebound {
                out.push(format!("    not re-bound — {why}; it reads as changed since its approval until the human approves it again"));
            }
        }
        for review in &self.reviews {
            out.push(format!("  review of chore `{}` ({}): pin {} -> {}", review.change, review.from_path, pin_of(&review.old), pin_of(&review.new)));
        }
        out.extend(self.kept.iter().map(|k| crate::safety::effect_line(&k.effect)));
        out
    }
}

fn pin_of(review: &Review) -> String {
    review.pins.as_ref().map_or_else(|| "(none)".to_string(), |p| p.scenario_digest.as_str().to_string())
}

fn rel_slash(path: &Path) -> String {
    path.components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/")
}

/// The committed reviews of the ledger at `ledger_root`, with the ledger-relative path each is filed at.
fn committed_reviews(ledger_root: &Path) -> Result<Vec<(Review, String)>, String> {
    let tier = GitTier::new(ledger_root);
    let read = tier.read(&TierQuery::kind(RecordKind::Review)).map_err(|e| format!("cannot read the review ledger: {e}"))?;
    Ok(read.records.iter().filter_map(|raw| Some((serde_json::from_value::<Review>(raw.0.clone()).ok()?, rel_slash(&expected_relative_path(RecordKind::Review, &raw.0).ok()?)))).collect())
}

/// The plan for `repo`: every old-format brief, and the chore reviews pinned
/// to a brief it re-binds. `ledger_root` is the ledger as the commit will find
/// it (after the renaming), `ledger_prefix` its path relative to the repo.
pub fn plan(repo: &Path, ledger_root: &Path, ledger_prefix: &Path) -> Result<Plan, String> {
    let mut plan = Plan::default();
    let old = crate::old_briefs::all(repo);
    if old.is_empty() {
        return Ok(plan);
    }
    let reviews = committed_reviews(ledger_root)?;
    let elsewhere = reviews_elsewhere(repo, ledger_root, &reviews);
    for found in old {
        let path = repo.join(&found.path);
        let text = std::fs::read_to_string(&path).map_err(|e| format!("cannot read {}: {e}", found.path))?;
        let brief = reader::parse(&text);
        let rewritten = match rewrite(&text, &brief) {
            Ok(rewritten) => rewritten,
            Err(why) => {
                plan.kept.push(Kept { effect: Effect::path("keep", found.path.clone()).detail(format!("left in the old format: {why}")) });
                continue;
            }
        };
        let new = reader::parse(&rewritten.text);
        if new.old_format.is_some() || new.frontmatter.as_ref().is_some_and(|f| !f.problems.is_empty()) {
            plan.kept.push(Kept { effect: Effect::path("keep", found.path.clone()).detail("left in the old format: the rewrite would not read as a 0.31 brief; fix it by hand") });
            continue;
        }
        let approval = crate::brief::approval_of(&text);
        let same = equivalent(&brief, &new);
        let old_digest = crate::brief::brief_digest(&text);
        let new_digest = crate::brief::brief_digest(&rewritten.text);
        let mut not_rebound = None;
        let mut rebind_approval = false;
        match (&approval, &same) {
            (crate::brief::Approval::Bound { .. }, Ok(())) => rebind_approval = true,
            (crate::brief::Approval::Bound { .. }, Err(why)) => not_rebound = Some(why.clone()),
            (crate::brief::Approval::Changed { .. }, _) => not_rebound = Some("it was edited after its approval (its `Approved:` digest does not match its text)".to_string()),
            (_, Err(why)) => not_rebound = Some(why.clone()),
            _ => {}
        }
        let mut text_out = rewritten.text.clone();
        if rebind_approval {
            if let Some(index) = canon_ingest::markdown::field_index(&text_out, "Approved:") {
                text_out = text_out.split_inclusive('\n').enumerate().map(|(i, l)| if i == index { l.replacen(old_digest.as_str(), new_digest.as_str(), 1) } else { l.to_string() }).collect();
            }
        }
        // The chore reviews that pin the old brief digest.
        if brief.kind == Kind::Chore {
            let old_pin = crate::brief::brief_spec_digest(&text);
            let new_pin = crate::brief::brief_spec_digest(&text_out);
            let matches = |r: &Review| {
                r.change_id.as_ref().is_some_and(|c| c.as_str() == found.change) && r.pins.as_ref().is_some_and(|p| p.scenario_digest_scope == PinnedSpec::Brief && p.scenario_digest == old_pin)
            };
            for (review, from) in reviews.iter().filter(|(r, _)| matches(r)) {
                if same.is_err() {
                    plan.kept.push(Kept {
                        effect: Effect::path("keep", ledger_prefix.join(from).display().to_string()).detail(format!(
                            "the review of chore `{}` stays pinned to the old brief digest, so the chore is reviewed again: {}",
                            found.change,
                            same.as_ref().err().cloned().unwrap_or_default()
                        )),
                    });
                    continue;
                }
                let mut new_review = review.clone();
                if let Some(pins) = new_review.pins.as_mut() {
                    pins.scenario_digest = new_pin.clone();
                }
                let body = serde_json::to_value(&new_review).map_err(|e| e.to_string())?;
                let to = rel_slash(&expected_relative_path(RecordKind::Review, &body).map_err(|e| e.to_string())?);
                plan.reviews.push(ReviewEntry {
                    from: from.clone(),
                    from_path: rel_slash(&ledger_prefix.join(from)),
                    to_path: rel_slash(&ledger_prefix.join(&to)),
                    to,
                    old: review.clone(),
                    new: new_review,
                    change: found.change.clone(),
                });
            }
            for _ in elsewhere.iter().filter(|r| matches(r)) {
                plan.kept.push(Kept {
                    effect: Effect::record("keep", format!("review of chore {}", found.change)).detail("in a hot or cold store, which migrate does not change; the chore is reviewed again"),
                });
            }
        }
        plan.briefs.push(BriefEntry {
            path: found.path.clone(),
            change: found.change.clone(),
            old_text: text,
            new_text: text_out,
            old_digest,
            new_digest,
            rebind_approval,
            comments: rewritten.comments,
            waivers: rewritten.waivers.len(),
            chore: rewritten.chore,
            titles: rewritten.titles,
            not_rebound,
        });
    }
    for brief in plan.briefs.iter().filter(|b| b.not_rebound.is_some()) {
        plan.kept.push(Kept { effect: Effect::path("keep", brief.path.clone()).detail(format!("`Approved:` line not re-bound: {}", brief.not_rebound.clone().unwrap_or_default())) });
    }
    plan.reviews.sort_by(|a, b| a.from.cmp(&b.from));
    Ok(plan)
}

/// The reviews the gate reads that are not files of the committed ledger (hot and cold stores).
fn reviews_elsewhere(repo: &Path, ledger_root: &Path, committed: &[(Review, String)]) -> Vec<Review> {
    use canon_gate::{GateContext, GateCtx};
    let Ok(mut ctx) = GateCtx::from_repo(repo) else {
        return Vec::new();
    };
    ctx.ledger_root = ledger_root.to_path_buf();
    let Ok(ctx) = GateContext::load(ctx, &canon_policy::SchemaRegistry::load(), chrono::Utc::now()) else {
        return Vec::new();
    };
    ctx.reviews.iter().filter(|r| r.change_id.is_some() && !committed.iter().any(|(c, _)| c == *r)).cloned().collect()
}

/// The journal: what a run rewrote, with the digests and hashes before and after.
#[derive(Debug, Default, Serialize, Deserialize)]
struct Journal {
    complete: bool,
    briefs: Vec<JournalBrief>,
    reviews: Vec<JournalReview>,
}

#[derive(Debug, Serialize, Deserialize)]
struct JournalBrief {
    path: String,
    old_sha256: String,
    new_sha256: String,
    old_digest: String,
    new_digest: String,
    approval_rebound: bool,
}

#[derive(Debug, Serialize, Deserialize)]
struct JournalReview {
    /// Repo-relative path of the version replaced and of the one written.
    from: String,
    to: String,
    old_sha256: String,
    new_sha256: String,
    old_pin: String,
    new_pin: String,
}

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent).map_err(|e| format!("could not create {}: {e}", parent.display()))?;
    }
    let tmp = path.with_extension("canon-tmp");
    std::fs::write(&tmp, bytes).map_err(|e| format!("could not write {}: {e}", tmp.display()))?;
    std::fs::rename(&tmp, path).map_err(|e| format!("could not replace {}: {e}", path.display()))
}

fn read_journal(backup: &Path) -> Option<Journal> {
    serde_json::from_slice(&std::fs::read(backup.join(JOURNAL)).ok()?).ok()
}

/// The newest backup directory whose briefs journal is not complete: a run
/// that stopped partway, which the next run continues in the same directory.
pub fn resume_backup(repo: &Path) -> Option<PathBuf> {
    let base = repo.join(paths::MIGRATE_BACKUP_DIR);
    let mut dirs: Vec<PathBuf> = std::fs::read_dir(base).ok()?.filter_map(Result::ok).map(|e| e.path()).filter(|p| p.join(JOURNAL).is_file()).collect();
    dirs.sort();
    dirs.pop().filter(|dir| read_journal(dir).is_some_and(|j| !j.complete))
}

/// Writes the plan (module doc): the journal first, the reviews, then the
/// briefs, the journal complete last. Each step is skipped when it is
/// already done, so a repeat after a stop finishes the rest.
pub fn apply(repo: &Path, plan: &Plan, backup: &Path) -> Result<(), String> {
    if !plan.writes() {
        return Ok(());
    }
    let mut journal = read_journal(backup).unwrap_or_default();
    journal.complete = false;
    for brief in &plan.briefs {
        if !journal.briefs.iter().any(|j| j.path == brief.path) {
            journal.briefs.push(JournalBrief {
                path: brief.path.clone(),
                old_sha256: sha256_hex(brief.old_text.as_bytes()),
                new_sha256: sha256_hex(brief.new_text.as_bytes()),
                old_digest: brief.old_digest.clone(),
                new_digest: brief.new_digest.clone(),
                approval_rebound: brief.rebind_approval,
            });
        }
    }
    let mut new_review_bytes: Vec<Vec<u8>> = Vec::new();
    for review in &plan.reviews {
        let bytes = serde_json::to_vec_pretty(&review.new.to_raw().0).map_err(|e| e.to_string())?;
        let old_bytes = std::fs::read(repo.join(&review.from_path)).unwrap_or_default();
        if !journal.reviews.iter().any(|j| j.from == review.from_path) {
            journal.reviews.push(JournalReview {
                from: review.from_path.clone(),
                to: review.to_path.clone(),
                old_sha256: sha256_hex(&old_bytes),
                new_sha256: sha256_hex(&bytes),
                old_pin: pin_of(&review.old),
                new_pin: pin_of(&review.new),
            });
        }
        new_review_bytes.push(bytes);
    }
    write_file(&backup.join(JOURNAL), &serde_json::to_vec_pretty(&journal).map_err(|e| e.to_string())?)?;

    let ledger_root = canon_gate::GateCtx::from_repo(repo).map_err(|e| e.to_string())?.ledger_root;
    let tier = GitTier::new(&ledger_root);
    let archive = canon_store::archive::archive_root_of(&ledger_root);
    for (review, bytes) in plan.reviews.iter().zip(&new_review_bytes) {
        let old_file = ledger_root.join(&review.from);
        if old_file.is_file() {
            tier.write_unarchived(&review.new).map_err(|e| format!("cannot write the new version of the review {}: {e}", review.from_path))?;
            if let Some(root) = &archive {
                let kept = root.join(&review.from);
                let old_bytes = std::fs::read(&old_file).map_err(|e| format!("cannot read {}: {e}", review.from_path))?;
                write_file(&kept, &old_bytes)?;
            }
            std::fs::remove_file(&old_file).map_err(|e| format!("cannot replace {}: {e}", review.from_path))?;
        } else if !ledger_root.join(&review.to).is_file() {
            write_file(&ledger_root.join(&review.to), bytes)?;
        }
    }
    for brief in &plan.briefs {
        write_file(&repo.join(&brief.path), brief.new_text.as_bytes())?;
    }
    journal.complete = true;
    write_file(&backup.join(JOURNAL), &serde_json::to_vec_pretty(&journal).map_err(|e| e.to_string())?)
}

/// What a revert of the briefs would change that is the human's.
#[derive(Debug, Default, Clone, Copy, PartialEq, Eq)]
pub struct RevertNote {
    /// Restoring an old file puts back an `Approved:` digest or a review pin.
    pub rebinds: bool,
}

/// Checks the files the migrate rewrote before `canon migrate --revert`: each
/// is still as the migrate left it (or already restored); a file changed
/// since (a later approval, edit or review version) is named and the revert
/// refused, with nothing written.
pub fn check_revert(repo: &Path, backup: &Path) -> Result<RevertNote, String> {
    let Some(journal) = read_journal(backup) else {
        return Ok(RevertNote::default());
    };
    let mut note = RevertNote::default();
    let mut changed: Vec<String> = Vec::new();
    for brief in &journal.briefs {
        let now = std::fs::read(repo.join(&brief.path)).map(|b| sha256_hex(&b)).ok();
        match now {
            Some(sha) if sha == brief.new_sha256 => note.rebinds |= brief.approval_rebound,
            Some(sha) if sha == brief.old_sha256 => {}
            _ => changed.push(brief.path.clone()),
        }
    }
    let ledger = canon_gate::GateCtx::from_repo(repo).ok().map(|ctx| ctx.ledger_root);
    let archive = ledger.as_deref().and_then(canon_store::archive::archive_root_of);
    let prefix = ledger.as_deref().and_then(|root| root.strip_prefix(repo).ok().map(Path::to_path_buf));
    for review in &journal.reviews {
        let now = std::fs::read(repo.join(&review.to)).map(|b| sha256_hex(&b)).ok();
        match now {
            Some(sha) if sha == review.new_sha256 => note.rebinds = true,
            Some(_) => changed.push(review.to.clone()),
            None => {
                // Gone: restored by an earlier revert, or superseded by a later version, which moved it to the archive.
                let in_archive = match (&archive, &prefix) {
                    (Some(root), Some(prefix)) => Path::new(&review.to).strip_prefix(prefix).is_ok_and(|rel| root.join(rel).is_file()),
                    _ => false,
                };
                if in_archive {
                    changed.push(review.to.clone());
                }
            }
        }
    }
    if changed.is_empty() {
        Ok(note)
    } else {
        Err(format!(
            "{} changed since `canon migrate` rewrote {}, and a revert would discard that change: {}; nothing was written",
            if changed.len() == 1 { "a file" } else { "files" },
            if changed.len() == 1 { "it" } else { "them" },
            changed.join(", ")
        ))
    }
}

/// The paths of the briefs and reviews a run rewrote, for a journal-aware preview of a revert.
pub fn journal_paths(backup: &Path) -> BTreeSet<String> {
    read_journal(backup).map(|j| j.briefs.into_iter().map(|b| b.path).chain(j.reviews.into_iter().map(|r| r.to)).collect()).unwrap_or_default()
}

#[cfg(test)]
mod tests {
    use super::*;

    const OLD: &str = "# 로그인\n\n<!-- canon:section intent role=context -->\n## 의도\n\n로그인을 추가한다.\n\n<!-- canon:hint what -->\n\n<!-- canon:section outcome role=normative -->\n## 원하는 결과\n\n- 사용자가 로그인한다. ==**세션이 열린다.**==\n\n<!-- canon:waive prose.length outcome \"길다\" -->\n<!-- canon:waive prose.length outcome \"길다\" -->\n<!-- canon:waive prose.code-citation brief \"코드 이름\" match=\"X_Y\" -->\n\n<!-- 사람이 답한 뒤, 그 답에서만, 다음으로 기록한다: canon brief approve --approver <누가> --note \"<그 사람의 말>\" -->\n";

    fn migrated(old: &str) -> Rewrite {
        rewrite(old, &reader::parse(old)).unwrap()
    }

    #[test]
    fn comments_go_headings_become_english_and_waivers_move_once() {
        let out = migrated(OLD);
        assert!(out.text.starts_with("---\nwaive:\n  - rule: \"prose.length\"\n    section: \"outcome\"\n    reason: \"길다\"\n  - rule: \"prose.code-citation\"\n    section: \"brief\"\n    reason: \"코드 이름\"\n    match: \"X_Y\"\n---\n\n# 로그인\n\n## Intent\n\n로그인을 추가한다.\n\n## Desired outcome\n"), "{}", out.text);
        assert!(!out.text.contains("<!--"), "{}", out.text);
        assert_eq!((out.comments, out.waivers.len(), out.titles.len()), (7, 2, 2));
        let new = reader::parse(&out.text);
        assert!(new.old_format.is_none());
        assert_eq!(new.frontmatter.as_ref().map(|f| (f.waivers.len(), f.problems.len())), Some((2, 0)));
        equivalent(&reader::parse(OLD), &new).unwrap();
    }

    #[test]
    fn the_prose_is_byte_for_byte_what_it_was() {
        let out = migrated(OLD);
        let strip = |text: &str| text.lines().filter(|l| !l.trim().is_empty() && !l.starts_with("##") && !l.starts_with("<!--")).collect::<Vec<_>>().join("\n");
        let body = out.text.split("---\n\n").nth(1).unwrap_or_default().to_string();
        assert_eq!(strip(&body), strip(OLD));
    }

    #[test]
    fn a_chore_marker_becomes_kind_and_a_role_that_differs_from_the_table_is_not_equivalent() {
        let chore = "# t\n\n<!-- canon:kind chore -->\n\n<!-- canon:section intent role=context -->\n## Intent\n\nx\n\n<!-- canon:section category role=context -->\n## Category\n\nci\n";
        let out = migrated(chore);
        assert!(out.text.starts_with("---\nkind: chore\n---\n\n# t\n\n## Intent"), "{}", out.text);
        equivalent(&reader::parse(chore), &reader::parse(&out.text)).unwrap();
        let odd = "# t\n\n<!-- canon:section intent role=context -->\n## Intent\n\nx\n\n<!-- canon:section outcome role=context -->\n## Desired outcome\n\ny\n";
        let out = migrated(odd);
        let why = equivalent(&reader::parse(odd), &reader::parse(&out.text)).unwrap_err();
        assert!(why.contains("role=context"), "{why}");
        let unknown = "# t\n\n<!-- canon:section intent role=context -->\n## Intent\n\nx\n\n<!-- canon:section mystery role=context -->\n## Mystery\n\ny\n";
        let why = equivalent(&reader::parse(unknown), &reader::parse(&migrated(unknown).text)).unwrap_err();
        assert!(why.contains("mystery"), "{why}");
    }

    #[test]
    fn a_comment_that_shares_a_line_or_a_waiver_that_does_not_parse_is_left_alone() {
        let shared = "# t\n\ntext <!-- canon:hint x -->\n\n## Intent\n\nx\n\n## Out of scope\n\ny\n";
        assert!(rewrite(shared, &reader::parse(shared)).unwrap_err().contains("shares its line"));
        let bad = "# t\n\n<!-- canon:waive prose.length -->\n\n## Intent\n\nx\n\n## Out of scope\n\ny\n";
        assert!(rewrite(bad, &reader::parse(bad)).unwrap_err().contains("does not parse"));
    }

    #[test]
    fn the_old_by_form_of_the_approval_note_is_removed() {
        let old = "# t\n\n## Intent\n\nx\n\n## Out of scope\n\ny\n\n<!-- canon:section x -->\n<!-- The human records the answer: canon brief approve --by <who> -->\n";
        let out = migrated(old);
        assert!(!out.text.contains("<!--") && out.text.ends_with("y\n"), "{:?}", out.text);
    }
}
