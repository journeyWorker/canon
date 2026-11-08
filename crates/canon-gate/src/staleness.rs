//! The staleness check (design decision 4/D4/A3): a GREEN evidence
//! record degrades to `stale-evidence` once the surface it covers has
//! moved out from under it. Direct port of `tools/parity.py`'s
//! two-tier `_is_stale` (the donor parity-harness audit's staleness notes
//! §3.1) — surface-scoped git-diff SHORT-CIRCUITS to
//! stale when the record declares a surface ref; the
//! `max_commits_behind` ceiling ALWAYS applies underneath it as a
//! backstop, never bypassed even when the precise diff comes back
//! clean (staleness.md §4's own "Silent-downgrade risk" warning: "if
//! `surface_scoped` is on and the diff returns `False` ..., execution
//! still falls through to the `max_commits_behind` check below it ...
//! easy to misport as 'if `port_files` exist, skip the ceiling
//! entirely' — canon's reimplementation must preserve the ... control
//! flow").
//!
//! # s15 P3b: native `evidence_sha` / `surface_ref`, read off `ctx.evidence`
//! `evidence_sha`/`surface_ref` used to be an interim, canon-gate-owned
//! raw-JSON companion this module independently re-scanned the ledger
//! for (`crate::trust_ladder`'s own migration note named the identical
//! move for `lifecycle`/`flagged`). s15 P1 moved both onto
//! `canon_model::EvidenceRecord` natively — `evidence_sha:
//! Option<Sha>`, `surface_ref: Vec<String>` (the one `Vec` exception:
//! defaults to empty when absent rather than `None`, design D9/
//! `gate-native-record-fields` spec) — so this module now reads them
//! as plain field accesses on the already-typed `ctx.evidence`, never
//! a second `GitTier` construction. Absent `evidence_sha` (`None`)
//! still means the SAME "unresolvable" third state as before (neither
//! git-diff tier has a starting ref) — this check silently skips such
//! a record (stays at whatever verdict [`crate::ledger`] already
//! reported), never manufacturing a `stale-evidence` violation from an
//! absent signal; a present-malformed `evidence_sha`/`surface_ref`
//! never reaches `ctx.evidence` at all (it already failed
//! [`crate::context::GateContext::load`]'s deserialize and lives in
//! `ctx.violations`, surfaced by `crate::ledger::LedgerCheck`).
//!
//! # Only the LATEST verdict per cell is ever checked
//! Staleness only ever DEGRADES a passing verdict (spec.md "Staleness
//! detection": "degrade a PASSING evidence record to stale") — a
//! `Divergent`/`NotApplicable` cell is already not green;
//! [`crate::ledger::LedgerEntry::is_green`] is the exact predicate
//! this module's own fold mirrors, applied to the WINNER of a
//! last-wins-by-`at` fold ([`fold_latest_green_cells`]'s own doc) —
//! never to any individual historical record in isolation. An older
//! `Faithful` record superseded by a newer non-green record for the
//! SAME (subject, role) cell is not green either: the ledger's
//! current answer for that cell is whatever its LATEST record says,
//! exactly [`crate::ledger::latest_verdicts`]'s own read model.
//!
//! # A declared surface is a path pattern (0.39)
//! A `surface_ref` that is not `effect:...` is a pattern read from the repository root with the glob of
//! `code.exclude` ([`canon_model::path_glob`], [`Anchor::Root`]): `src/`, `src`, `src/**` and `src/*.rs` all
//! go stale when `src/a.rs` changes, `Cargo.toml` is the root manifest and not every crate's, and a value
//! identical to a changed path matches it. The reason names the declarations that matched and the paths they
//! matched (the first three of each, and the totals).
//!
//! # A base commit git could not read (0.39)
//! A cell that reaches git (a green cell with an `evidence_sha` whose code tree is not the current one) has a
//! base commit. When the base is not in the repository (`base_missing`: a shallow clone, a pruned commit) or
//! git cannot answer (`git_unavailable`: no `git`, past its deadline or its output limit, or a failure that is
//! neither yes nor no), the cell adds nothing, as before. The same goes for the later queries: the changed paths
//! (`git diff --name-only -z <base>..HEAD`) and the commits behind that git does not answer in time, or answers with a name that is
//! not UTF-8, make the cell `git_unavailable` (counted once, by `gate check` and `status` alike); a base that is there but not an ancestor of HEAD (`base_not_ancestor`) is computed as
//! `<base>..HEAD`, as before. [`Unresolvable`] counts the cells of each reason so `canon gate check` and
//! `canon status` can say so: the verdicts, and the exit status, do not change.

use std::collections::HashMap;
use std::path::Path;

use canon_model::path_glob::{Anchor, Glob};
use canon_model::{EvidenceRecord, EvidenceVerdict, Sha};
use canon_store::fold_latest_by_key;
use canon_store::git::{GitCall, GitError};
use chrono::{DateTime, Utc};

use crate::context::{GateCheck, GateContext};
use crate::coverage::CellSubject;
use crate::failure_class::{FailureClass, Violation};
use crate::policy::{DEFAULT_MAX_COMMITS_BEHIND, DEFAULT_SURFACE_SCOPED};

/// One (subject, role) cell's LATEST record's native staleness
/// signals, read directly off `ctx.evidence` (module doc) — no longer
/// a separate companion type. `raw` is the record's own
/// `serde_json::to_value` — the exact [`crate::PolicyResolution`] CEL
/// binding shape (`policy.rs` module doc), reused as-is for
/// `surface_scoped`/`max_commits_behind` evaluation. Only ever
/// constructed for a cell whose LATEST record's verdict is `Faithful`
/// — [`fold_latest_green_cells`] filters AFTER, never BEFORE, its
/// last-wins fold.
struct GreenCell {
    subject: String,
    evidence_sha: Option<Sha>,
    /// The code tree the record was written against (0.28 D7b).
    code_tree: Option<String>,
    surface_ref: Vec<String>,
    raw: serde_json::Value,
}

/// One `ctx.evidence` record folded to its (subject, role) cell — the
/// input [`fold_latest_by_key`] (design D11/s21 D3,
/// `s38-evidence-bearing-memory`) folds to the
/// greatest-`(at, schema, digest)` winner per cell, mirroring
/// [`crate::ledger::latest_verdicts`]'s identical shape and total,
/// machine-independent tie-break.
struct Candidate<'a> {
    subject: String,
    role: Option<String>,
    at: DateTime<Utc>,
    /// The record's own `envelope.schema` — the equal-`at` generation
    /// discriminator, read off `record` rather than assumed.
    schema: u32,
    digest: String,
    record: &'a EvidenceRecord,
}

/// Folds `ctx.evidence` to the LATEST record per (subject, role) cell
/// FIRST — the exact last-wins-by-`(at, schema, digest)` discipline
/// [`crate::ledger::latest_verdicts`] applies, via the SAME hoisted
/// [`fold_latest_by_key`] (design D11/s21 D3) — THEN keeps only the
/// cells whose WINNING record's verdict is `Faithful`. Filtering out
/// non-`Faithful` records BEFORE the fold, instead of after, would let
/// an OLDER green record survive here even after a NEWER
/// `Divergent`/`NotApplicable` record for the SAME cell already
/// superseded it in the ledger's own last-wins read model — staleness
/// must only ever degrade the verdict the ledger ACTUALLY reports as
/// current for that cell, never a stale winner of its own that the
/// real ledger no longer agrees with (module doc's "Only the LATEST
/// verdict per cell is ever checked"). `ctx.evidence` already excludes
/// every content-malformed record (module doc) — no separate skip
/// logic needed here.
fn fold_latest_green_cells(ctx: &GateContext) -> Vec<GreenCell> {
    // Every cell the record answers (`CellSubject::all_of`): a
    // task+scenario record's staleness degrades its scenario cell too.
    let stamps = &ctx.evidence_index().stamps;
    let candidates = ctx.evidence.iter().enumerate().flat_map(|(i, record)| {
        let role = record.envelope.actor.role.as_ref().map(|r| r.as_str().to_string());
        // The record's digest is the one the shared stamp already holds.
        let digest = match &stamps[i] {
            Some(stamp) => stamp.digest.clone(),
            None => canon_store::partition::content_digest(&serde_json::to_value(record).unwrap_or_default()),
        };
        CellSubject::all_of(record).map(move |subject| Candidate {
            subject: subject.as_str().to_string(),
            role: role.clone(),
            at: record.envelope.at,
            schema: record.envelope.schema,
            digest: digest.clone(),
            record,
        })
    });

    fold_latest_by_key(candidates, |c| (c.subject.clone(), c.role.clone()), |c| c.at, |c| c.schema, |c| c.digest.as_str())
        .into_values()
        .filter(|c| matches!(c.record.verdict, EvidenceVerdict::Faithful))
        .map(|c| GreenCell {
            subject: c.subject,
            evidence_sha: c.record.evidence_sha.clone(),
            code_tree: c.record.code_tree.clone(),
            surface_ref: c.record.surface_ref.clone(),
            raw: serde_json::to_value(c.record).unwrap_or(serde_json::Value::Null),
        })
        .collect()
}

/// Why the base commit of a cell could not be read (module doc).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Unresolved {
    /// The base commit is not in this repository.
    BaseMissing,
    /// It is there, but not an ancestor of HEAD.
    BaseNotAncestor,
    /// Git did not answer: it did not start, ran past its deadline, or wrote what could not be read.
    GitUnavailable,
}

impl Unresolved {
    /// The name the reports use.
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::BaseMissing => "base_missing",
            Self::BaseNotAncestor => "base_not_ancestor",
            Self::GitUnavailable => "git_unavailable",
        }
    }
}

/// How many subjects a report names before it counts the rest.
const SHOWN: usize = 3;

/// The cells of passing evidence whose base commit git could not read, counted by reason (module doc).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Unresolvable {
    pub base_missing: usize,
    pub base_not_ancestor: usize,
    pub git_unavailable: usize,
    /// How many subjects the cells are of.
    pub subjects: usize,
    /// The first three of them, in subject order.
    pub first: Vec<String>,
    /// The repository is a shallow clone.
    pub shallow: bool,
}

impl Unresolvable {
    /// The cells counted, whatever the reason.
    pub const fn total(&self) -> usize {
        self.base_missing + self.base_not_ancestor + self.git_unavailable
    }

    /// Whether no cell is counted: nothing is said.
    pub const fn is_empty(&self) -> bool {
        self.total() == 0
    }

    /// The cells of each reason, the ones that are counted, in the order of [`Unresolved`].
    pub fn reasons(&self) -> Vec<(Unresolved, usize)> {
        [(Unresolved::BaseMissing, self.base_missing), (Unresolved::BaseNotAncestor, self.base_not_ancestor), (Unresolved::GitUnavailable, self.git_unavailable)]
            .into_iter()
            .filter(|(_, count)| *count > 0)
            .collect()
    }

    /// The lines under the advisory of `canon gate check`: the cells of each reason, then the first subjects.
    pub fn items(&self) -> Vec<String> {
        let mut lines: Vec<String> = self.reasons().iter().map(|(reason, count)| format!("{}: {count} cell(s)", reason.as_str())).collect();
        let more = if self.first.len() < self.subjects { ", ..." } else { "" };
        lines.push(format!("first subjects: {}{more}", self.first.join(", ")));
        lines
    }

    /// The sentence `canon gate check` (its advisory) and `canon status` (its warning) both say.
    pub fn message(&self) -> String {
        let reasons = self.reasons().iter().map(|(reason, count)| format!("{} {count}", reason.as_str())).collect::<Vec<_>>().join(", ");
        let named = self.first.iter().map(|subject| format!("`{subject}`")).collect::<Vec<_>>().join(", ");
        let more = if self.first.len() < self.subjects { ", ..." } else { "" };
        let hint = if self.shallow { " This is a shallow clone: run `git fetch --unshallow` to read the base commits." } else { "" };
        format!(
            "{} (subject, role) cell(s) of passing evidence could not be checked against their base commit ({reasons}); first: {named}{more}.{hint} Their verdicts are unchanged.",
            self.total()
        )
    }
}

/// What the staleness check found: the violations, and what it could not check.
#[derive(Debug, Default)]
pub struct Staleness {
    pub violations: Vec<Violation>,
    pub unresolvable: Unresolvable,
}

/// What git says of a base commit.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Base {
    Ancestor,
    NotAncestor,
    Missing,
    Unavailable,
}

/// A question git answers yes or no with its exit status.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Probe {
    Yes,
    No,
    Unanswered,
}

/// Memoized `git` subprocess boundary (staleness.md §3.3: "5k+ ledger
/// records share ~15 unique `app_sha`s ... one fork per unique query,
/// not per ledger row") — process-lifetime-per-`run()`-call, keyed by
/// the exact `(subcommand, sha)` pair; swallows every failure to
/// `None`, never panics (module doc's "unresolvable" third state).
///
/// A query git did not answer in time, or answered with more than the output limit
/// ([`canon_store::git::GitError::is_stopped`]), is `None`, and so is every later query of the run: one stuck git
/// is not asked again for each commit of the ledger (0.39). The next `gate check` asks afresh.
#[derive(Default)]
struct GitMemo {
    changed: HashMap<String, Option<Vec<String>>>,
    behind: HashMap<String, Option<u32>>,
    base: HashMap<String, Base>,
    stopped: bool,
}

impl GitMemo {
    /// The paths that changed since `sha`, sorted.
    fn changed_files_since(&mut self, repo: &Path, sha: &str) -> Option<&[String]> {
        if !self.changed.contains_key(sha) {
            let answer = self.git_diff_names(repo, sha);
            self.changed.insert(sha.to_string(), answer);
        }
        self.changed[sha].as_deref()
    }

    fn commits_behind(&mut self, repo: &Path, sha: &str) -> Option<u32> {
        if !self.behind.contains_key(sha) {
            let answer = self.git_rev_list_count(repo, sha);
            self.behind.insert(sha.to_string(), answer);
        }
        self.behind[sha]
    }

    /// Where the base commit `sha` stands against HEAD: two questions, asked once for a commit however many cells
    /// have it as their base.
    fn base(&mut self, repo: &Path, sha: &str) -> Base {
        if let Some(&known) = self.base.get(sha) {
            return known;
        }
        let commit = format!("{sha}^{{commit}}");
        let found = match self.probe(repo, &["rev-parse", "--verify", "--quiet", &commit]) {
            Probe::Yes => match self.probe(repo, &["merge-base", "--is-ancestor", sha, "HEAD"]) {
                Probe::Yes => Base::Ancestor,
                Probe::No => Base::NotAncestor,
                Probe::Unanswered => Base::Unavailable,
            },
            Probe::No => Base::Missing,
            Probe::Unanswered => Base::Unavailable,
        };
        self.base.insert(sha.to_string(), found);
        found
    }

    /// A call whose answer is its exit status: zero is yes, one is no, anything else (git did not start, ran out of
    /// time or size, or failed for a reason that is not the question) is no answer.
    fn probe(&mut self, repo: &Path, args: &[&str]) -> Probe {
        if self.stopped {
            return Probe::Unanswered;
        }
        match GitCall::new(repo).args(args).no_lazy_fetch().run() {
            Ok(_) => Probe::Yes,
            Err(GitError::Exit { code: Some(1), .. }) => Probe::No,
            Err(error) => {
                self.stopped = error.is_stopped();
                Probe::Unanswered
            }
        }
    }

    /// Whether the repository is a shallow clone, asked when a report needs it.
    fn shallow(&mut self, repo: &Path) -> bool {
        self.run_git(repo, &["rev-parse", "--is-shallow-repository"]).is_some_and(|out| out.trim() == "true")
    }

    /// The ONE subprocess boundary this module ever crosses — swallows
    /// every failure (missing binary, unresolvable sha, non-zero exit,
    /// invalid UTF-8, past the deadline or the output limit) into `None`,
    /// never raises (staleness.md §3.3/`_git`; module doc's
    /// "unresolvable" third state).
    fn run_git(&mut self, repo: &Path, args: &[&str]) -> Option<String> {
        if self.stopped {
            return None;
        }
        match GitCall::new(repo).args(args).run() {
            Ok(output) => String::from_utf8(output.stdout).ok(),
            Err(error) => {
                self.stopped = error.is_stopped();
                None
            }
        }
    }

    /// The precise tier: `git diff --name-only -z --no-renames <sha>..HEAD`
    /// (staleness.md §3.1/§6, `_changed_files_since` +
    /// `_git_diff_touches`). `-z` names a file as it is, whatever its characters.
    fn git_diff_names(&mut self, repo: &Path, sha: &str) -> Option<Vec<String>> {
        let range = format!("{sha}..HEAD");
        let out = self.run_git(repo, &["diff", "--name-only", "-z", "--no-renames", &range])?;
        let mut names: Vec<String> = out.split('\0').filter(|name| !name.is_empty()).map(str::to_string).collect();
        names.sort_unstable();
        names.dedup();
        Some(names)
    }

    /// The coarse tier: `git rev-list --count <sha>..HEAD` (staleness.md
    /// §3.1, the `max_commits_behind` ceiling's own git query).
    fn git_rev_list_count(&mut self, repo: &Path, sha: &str) -> Option<u32> {
        let range = format!("{sha}..HEAD");
        let out = self.run_git(repo, &["rev-list", "--count", &range])?;
        out.trim().parse().ok()
    }
}

/// What the declarations of a cell matched among the changed paths.
struct Matched<'a> {
    declared: Vec<&'a str>,
    paths: Vec<&'a str>,
}

/// The declarations among `declared` (never an `effect:`) that match a changed path, and the changed paths they
/// match: a declaration is a pattern from the root ([`Anchor::Root`]), and a value identical to a path matches it.
fn match_surface<'a>(declared: &'a [String], changed: &'a [String]) -> Matched<'a> {
    let mut hit = vec![false; changed.len()];
    let mut matched = Matched { declared: Vec::new(), paths: Vec::new() };
    for declaration in declared.iter().filter(|value| !value.starts_with("effect:")) {
        let glob = Glob::new(declaration, Anchor::Root);
        let mut any = false;
        for (i, path) in changed.iter().enumerate() {
            if path == declaration || glob.matches(path) {
                hit[i] = true;
                any = true;
            }
        }
        if any {
            matched.declared.push(declaration);
        }
    }
    matched.paths = changed.iter().zip(&hit).filter(|(_, hit)| **hit).map(|(path, _)| path.as_str()).collect();
    matched
}

/// `items`, the first [`SHOWN`] of them in backticks and how many more.
fn first_of(items: &[&str]) -> String {
    let shown = items.iter().take(SHOWN).map(|item| format!("`{item}`")).collect::<Vec<_>>().join(", ");
    match items.len().checked_sub(SHOWN).filter(|more| *more > 0) {
        Some(more) => format!("{shown} and {more} more"),
        None => shown,
    }
}

/// The staleness [`crate::GateCheck`] (D4) — see module doc for the
/// two-tier decision and the native `evidence_sha`/`surface_ref`
/// fields.
#[derive(Debug)]
pub struct StalenessCheck;

impl GateCheck for StalenessCheck {
    fn name(&self) -> &'static str {
        "staleness"
    }

    fn run(&self, ctx: &GateContext) -> Vec<Violation> {
        ctx.staleness_violations().to_vec()
    }
}

/// What [`StalenessCheck`] reports for `ctx`. Callers go through
/// [`GateContext::staleness_violations`] and [`GateContext::staleness_unresolvable`], which hold the result for the
/// command: the staleness check, the risk check and the reports of what could not be read share it.
pub(crate) fn compute(ctx: &GateContext) -> Staleness {
    evaluate(ctx, ctx.code_tree.as_deref())
}

/// The cells whose base commit git could not read, without the violations: `canon status` says what `canon gate
/// check` does, and asks git two questions per base commit and nothing else. `code_tree` is the code tree now
/// (the one `canon gate check` sets on its context); it is asked for only when a cell is unread without it, since
/// a cell whose record's code tree is the current one never reaches git.
pub fn unresolvable_cells(ctx: &GateContext, code_tree: impl FnOnce() -> Option<String>) -> Unresolvable {
    let without = evaluate(ctx, None).unresolvable;
    if without.is_empty() {
        return without;
    }
    evaluate(ctx, code_tree().as_deref()).unresolvable
}

/// The staleness of every green cell as of `code_tree` (the code now, `None` when unknown), and the cells whose
/// base commit could not be read, or whose changed paths or commits behind git could not answer.
fn evaluate(ctx: &GateContext, code_tree: Option<&str>) -> Staleness {
    let mut memo = GitMemo::default();
    let repo = &ctx.ctx.repo;
    let now = ctx.now;
    let mut violations = Vec::new();
    let mut unread: Vec<(String, Unresolved)> = Vec::new();

    for cell in fold_latest_green_cells(ctx) {
        // No `evidence_sha` at all: neither tier has a starting
        // ref to query. Unresolvable, never assumed stale (module
        // doc).
        let Some(sha) = cell.evidence_sha.as_ref() else { continue };
        // The code is what it was when the evidence was recorded (D7b):
        // commits since then (docs, specs, briefs) did not move it, so no
        // surface changed and the ceiling is not about code.
        if cell.code_tree.is_some() && cell.code_tree.as_deref() == code_tree {
            continue;
        }

        let base = memo.base(repo, sha.as_str());
        match base {
            // Nothing to compare: as it always was, the cell adds nothing.
            Base::Missing => {
                unread.push((cell.subject.clone(), Unresolved::BaseMissing));
                continue;
            }
            Base::Unavailable => {
                unread.push((cell.subject.clone(), Unresolved::GitUnavailable));
                continue;
            }
            // Computed as `<base>..HEAD`, as it always was.
            Base::NotAncestor => unread.push((cell.subject.clone(), Unresolved::BaseNotAncestor)),
            Base::Ancestor => {}
        }
        // A cell is counted once: a base that is not an ancestor and a range git cannot answer is one `git_unavailable` cell.
        let not_ancestor_at = (base == Base::NotAncestor).then(|| unread.len() - 1);
        let mut unanswered = false;

        let mut reasons: Vec<String> = Vec::new();

        let surface_scoped = ctx.policy.surface_scoped(&cell.raw, now).unwrap_or(DEFAULT_SURFACE_SCOPED);
        if surface_scoped && !cell.surface_ref.is_empty() {
            // `None` (unresolvable diff query) contributes nothing
            // to this tier — never assumed stale.
            let changed = memo.changed_files_since(repo, sha.as_str());
            unanswered |= changed.is_none();
            if let Some(changed) = changed {
                let matched = match_surface(&cell.surface_ref, changed);
                if !matched.paths.is_empty() {
                    reasons.push(format!(
                        "a declared surface changed since {sha}: {} declared surface ref(s) matched {} changed path(s) — declared {}; changed {}",
                        matched.declared.len(),
                        matched.paths.len(),
                        first_of(&matched.declared),
                        first_of(&matched.paths)
                    ));
                }
            }
        }

        // The ceiling is an UNCONDITIONAL backstop (module doc) —
        // it always runs, even when the precise diff above came
        // back clean.
        let commits_behind = memo.commits_behind(repo, sha.as_str());
        unanswered |= commits_behind.is_none();
        if let Some(commits_behind) = commits_behind {
            let ceiling = ctx.policy.max_commits_behind(&cell.raw, now).unwrap_or(DEFAULT_MAX_COMMITS_BEHIND);
            if commits_behind > ceiling {
                reasons.push(format!("{commits_behind} commits behind HEAD (max_commits_behind={ceiling})"));
            }
        }

        if unanswered {
            match not_ancestor_at {
                Some(at) => unread[at].1 = Unresolved::GitUnavailable,
                None => unread.push((cell.subject.clone(), Unresolved::GitUnavailable)),
            }
        }

        if !reasons.is_empty() {
            violations.push(Violation::new(FailureClass::StaleEvidence, cell.subject.clone(), reasons.join("; ")));
        }
    }

    let mut unresolvable = Unresolvable::default();
    for (_, why) in &unread {
        match why {
            Unresolved::BaseMissing => unresolvable.base_missing += 1,
            Unresolved::BaseNotAncestor => unresolvable.base_not_ancestor += 1,
            Unresolved::GitUnavailable => unresolvable.git_unavailable += 1,
        }
    }
    if !unresolvable.is_empty() {
        let mut subjects: Vec<&str> = unread.iter().map(|(subject, _)| subject.as_str()).collect();
        subjects.sort_unstable();
        subjects.dedup();
        unresolvable.subjects = subjects.len();
        unresolvable.first = subjects.iter().take(SHOWN).map(|subject| (*subject).to_string()).collect();
        unresolvable.shallow = memo.shallow(repo);
    }

    Staleness { violations, unresolvable }
}

#[cfg(test)]
mod tests {
    use std::path::PathBuf;

    use canon_model::{Actor, Envelope, RawRecord, RecordKind, RoleId, TaskId};
    use canon_store::git_tier::GitTier;
    use canon_store::tier::{RawWrite, Tier, TierQuery};
    use tempfile::TempDir;

    use super::*;
    use canon_policy::SchemaRegistry;
    use crate::context::GateCtx;
    use crate::policy::{PolicyField, PolicyResolution, StalenessPolicy};

    fn git(repo: &Path, args: &[&str]) {
        GitCall::new(repo).args(args).run().unwrap_or_else(|error| panic!("{error}"));
    }

    fn init_repo(repo: &Path) {
        git(repo, &["init", "-q"]);
        git(repo, &["config", "user.email", "test@example.com"]);
        git(repo, &["config", "user.name", "test"]);
    }

    fn write_and_commit(repo: &Path, relative: &str, content: &str, message: &str) -> String {
        let path = repo.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(&path, content).unwrap();
        git(repo, &["add", "-A"]);
        git(repo, &["commit", "-q", "-m", message]);
        GitCall::new(repo).args(["rev-parse", "HEAD"]).run().unwrap().stdout_trimmed()
    }

    /// Writes one green `EvidenceRecord` with its NATIVE
    /// `evidence_sha`/`surface_ref` fields set (s15 P3b — no longer a
    /// raw-JSON companion, module doc) into `ledger_root`, through the
    /// ordinary typed write path.
    fn write_green_record(ledger_root: &Path, task: &str, role: &str, evidence_sha: &str, surface_ref: &[&str]) {
        let envelope = Envelope::new(1, RecordKind::Evidence, Utc::now(), Actor::new("agent", RoleId::parse(role).unwrap()));
        let record = EvidenceRecord::new(envelope, Some(TaskId::parse(task).unwrap()), None, None, EvidenceVerdict::Faithful)
            .with_evidence_sha(Sha::parse(evidence_sha).unwrap())
            .with_surface_ref(surface_ref.iter().map(std::string::ToString::to_string).collect());
        GitTier::new(ledger_root).write(&record).expect("write evidence record");
    }

    /// Same as [`write_green_record`] but with an explicit `verdict`
    /// and `at` timestamp — needed to construct two records for the
    /// SAME (subject, role) cell where the caller controls which one
    /// is "latest" by `at`, rather than relying on `Utc::now()`'s
    /// natural ordering across two calls a few nanoseconds apart.
    fn write_record_at(ledger_root: &Path, task: &str, role: &str, verdict: EvidenceVerdict, at: DateTime<Utc>, evidence_sha: &str, surface_ref: &[&str]) {
        let envelope = Envelope::new(1, RecordKind::Evidence, at, Actor::new("agent", RoleId::parse(role).unwrap()));
        let record = EvidenceRecord::new(envelope, Some(TaskId::parse(task).unwrap()), None, None, verdict)
            .with_evidence_sha(Sha::parse(evidence_sha).unwrap())
            .with_surface_ref(surface_ref.iter().map(std::string::ToString::to_string).collect());
        GitTier::new(ledger_root).write(&record).expect("write evidence record");
    }

    fn policy_with(max_commits_behind: u32, surface_scoped: bool) -> PolicyResolution {
        PolicyResolution {
            trust_required: Default::default(),
            trust_sample: Default::default(),
            staleness: StalenessPolicy { max_commits_behind: PolicyField::Flat(max_commits_behind), surface_scoped: PolicyField::Flat(surface_scoped) },
            risk_routing: Default::default(),
            risk_tiers: Default::default(),
            spec_coverage: None,
            evidence_binding: None,
            diagnostics: Vec::new(),
        }
    }

    /// A named, fixed UTC constant (s21 design.md R5: never
    /// `Utc::now()` in a test call site of `GateContext::load`) — none
    /// of this module's existing tests exercise a time-bearing CEL
    /// policy (`policy_with` builds `Flat`-only fields), so any fixed
    /// instant is equally valid here.
    fn fixed_now() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z").unwrap().with_timezone(&Utc)
    }

    /// Builds a `GateContext` around a caller-supplied `policy`
    /// (bypassing a real `policy.yaml`) — `evidence`/`violations` are
    /// loaded for real off `ledger_root`, exactly as
    /// `GateContext::load` itself would (s15 P3b: `StalenessCheck` now
    /// reads `ctx.evidence` directly, so this fixture must actually
    /// populate it, never leave it empty).
    fn ctx_for(repo: &Path, ledger_root: &Path, policy: PolicyResolution, now: DateTime<Utc>) -> GateContext {
        let read = GitTier::new(ledger_root).read(&TierQuery::kind(RecordKind::Evidence)).unwrap_or_default();
        let (evidence, _validation_violations) = canon_model::validate_evidence_batch(&read.records);
        GateContext {
            ctx: GateCtx { repo: repo.to_path_buf(), ledger_root: ledger_root.to_path_buf() },
            policy,
            evidence,
            audits: Vec::new(),
            scenarios: Vec::new(),
            divergences: Vec::new(),
            subjects: Vec::new(),
            changes: Vec::new(),
            reviews: Vec::new(),
            findings: Vec::new(),
            violations: read.violations,
            corpus_violations: Vec::new(),
            unreadable_kinds: Vec::new(),
            code_tree: None,
            derived: Default::default(),
            now,
        }
    }

    #[test]
    fn stale_evidence_fires_when_a_declared_surface_file_changes_after_the_evidence_sha() {
        let dir = TempDir::new().unwrap();
        let repo = dir.path();
        init_repo(repo);
        let sha = write_and_commit(repo, "crates/foo/src/bar.rs", "fn bar() {}", "initial");

        let ledger_root = repo.join(".canon/ledger");
        write_green_record(&ledger_root, "s5#1.7", "implementer", &sha, &["crates/foo/src/bar.rs"]);

        // Move HEAD by touching the DECLARED surface file.
        write_and_commit(repo, "crates/foo/src/bar.rs", "fn bar() { /* changed */ }", "surface changed");

        let ctx = ctx_for(repo, &ledger_root, policy_with(50, true), fixed_now());
        let violations = StalenessCheck.run(&ctx);
        assert_eq!(violations.len(), 1);
        assert_eq!(violations[0].class, FailureClass::StaleEvidence);
        assert_eq!(violations[0].subject, "s5#1.7");
        assert!(violations[0].detail.contains("declared surface changed"));
    }

    /// git quotes a name with a non-ASCII character or a quote unless the list is NUL-terminated (0.39): the declared
    /// surface and the name git gave were two strings, and a changed file was reported clean.
    #[test]
    fn stale_evidence_fires_for_a_surface_file_whose_name_git_would_quote() {
        for name in ["é.rs", "a\"b.rs", "with space.rs", "dir/ü ñ.rs"] {
            let dir = TempDir::new().unwrap();
            let repo = dir.path();
            init_repo(repo);
            let sha = write_and_commit(repo, name, "fn a() {}", "initial");
            let ledger_root = repo.join(".canon/ledger");
            write_green_record(&ledger_root, "s5#1.7", "implementer", &sha, &[name]);
            write_and_commit(repo, name, "fn a() { /* changed */ }", "surface changed");

            let violations = StalenessCheck.run(&ctx_for(repo, &ledger_root, policy_with(50, true), fixed_now()));
            assert_eq!(violations.len(), 1, "{name}: {violations:?}");
            assert!(violations[0].detail.contains("declared surface changed"), "{name}: {}", violations[0].detail);
        }
    }

    /// A commit that adds a file whose name is no UTF-8 string: the diff answer is unknown, never "stale" and never
    /// "clean" for the surface it also changed.
    #[test]
    fn gate_git_boundary_07_a_name_that_is_not_utf8_leaves_the_surface_query_unanswered_and_the_evidence_not_stale() {
        let dir = TempDir::new().unwrap();
        let repo = dir.path();
        init_repo(repo);
        let sha = write_and_commit(repo, "crates/foo/src/bar.rs", "fn bar() {}", "initial");
        let ledger_root = repo.join(".canon/ledger");
        write_green_record(&ledger_root, "s5#1.7", "implementer", &sha, &["crates/foo/src/bar.rs"]);

        std::fs::write(repo.join("crates/foo/src/bar.rs"), "fn bar() { /* changed */ }").unwrap();
        git(repo, &["add", "-A"]);
        let blob = GitCall::new(repo).args(["hash-object", "-w", "--stdin"]).stdin(b"x").run().unwrap().stdout_trimmed();
        let mut entry = format!("100644 {blob}\tcrates/").into_bytes();
        entry.extend_from_slice(b"\xff.rs\n");
        GitCall::new(repo).args(["update-index", "--add", "--index-info"]).stdin(&entry).run().unwrap();
        git(repo, &["commit", "-q", "-m", "changes the surface and adds a name that is not UTF-8"]);

        let violations = StalenessCheck.run(&ctx_for(repo, &ledger_root, policy_with(50, true), fixed_now()));
        assert!(violations.is_empty(), "the query has no answer, so nothing is asserted: {violations:?}");
    }

    #[test]
    fn not_stale_when_declared_surfaces_are_untouched_and_well_under_the_ceiling() {
        let dir = TempDir::new().unwrap();
        let repo = dir.path();
        init_repo(repo);
        let sha = write_and_commit(repo, "crates/foo/src/bar.rs", "fn bar() {}", "initial");

        let ledger_root = repo.join(".canon/ledger");
        write_green_record(&ledger_root, "s5#1.7", "implementer", &sha, &["crates/foo/src/bar.rs"]);

        // Move HEAD via an UNRELATED file — the declared surface never changes.
        write_and_commit(repo, "crates/other/src/unrelated.rs", "fn unrelated() {}", "unrelated change");

        let ctx = ctx_for(repo, &ledger_root, policy_with(50, true), fixed_now());
        assert!(StalenessCheck.run(&ctx).is_empty());
    }

    #[test]
    fn ceiling_applies_even_when_the_surface_scoped_diff_is_precisely_clean() {
        // staleness.md §4: the ceiling is an UNCONDITIONAL backstop —
        // never skipped just because the precise tier proved the
        // declared files untouched.
        let dir = TempDir::new().unwrap();
        let repo = dir.path();
        init_repo(repo);
        let sha = write_and_commit(repo, "crates/foo/src/bar.rs", "fn bar() {}", "initial");

        let ledger_root = repo.join(".canon/ledger");
        write_green_record(&ledger_root, "s5#1.7", "implementer", &sha, &["crates/foo/src/bar.rs"]);

        write_and_commit(repo, "crates/other/src/unrelated.rs", "fn unrelated() {}", "unrelated change 1");
        write_and_commit(repo, "crates/other/src/unrelated2.rs", "fn unrelated() {}", "unrelated change 2");

        // Ceiling of 1: two unrelated commits already exceed it, even
        // though the declared surface file is untouched.
        let ctx = ctx_for(repo, &ledger_root, policy_with(1, true), fixed_now());
        let violations = StalenessCheck.run(&ctx);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].detail.contains("commits behind HEAD"));
    }

    #[test]
    fn max_commits_behind_ceiling_fires_with_no_declared_surface_ref() {
        let dir = TempDir::new().unwrap();
        let repo = dir.path();
        init_repo(repo);
        let sha = write_and_commit(repo, "README.md", "hello", "initial");

        let ledger_root = repo.join(".canon/ledger");
        write_green_record(&ledger_root, "s5#1.7", "implementer", &sha, &[]);

        write_and_commit(repo, "README.md", "hello again", "second");
        write_and_commit(repo, "README.md", "hello thrice", "third");

        let ctx = ctx_for(repo, &ledger_root, policy_with(1, true), fixed_now());
        let violations = StalenessCheck.run(&ctx);
        assert_eq!(violations.len(), 1);
        assert!(violations[0].detail.contains("commits behind HEAD"));
    }

    #[test]
    fn unresolvable_evidence_sha_is_never_assumed_stale() {
        let dir = TempDir::new().unwrap();
        let repo = dir.path();
        init_repo(repo);
        write_and_commit(repo, "README.md", "hello", "initial");

        let ledger_root = repo.join(".canon/ledger");
        // Well-formed 40-hex sha grammar, but it never existed in this repo's history.
        let fake_sha = "0".repeat(40);
        write_green_record(&ledger_root, "s5#1.7", "implementer", &fake_sha, &["README.md"]);

        let ctx = ctx_for(repo, &ledger_root, policy_with(50, true), fixed_now());
        assert!(StalenessCheck.run(&ctx).is_empty(), "an unresolvable sha must never be treated as stale");
    }

    #[test]
    fn a_record_with_no_evidence_sha_is_never_assumed_stale() {
        let dir = TempDir::new().unwrap();
        let repo = dir.path();
        init_repo(repo);
        write_and_commit(repo, "README.md", "hello", "initial");

        let ledger_root = repo.join(".canon/ledger");
        let envelope = Envelope::new(1, RecordKind::Evidence, Utc::now(), Actor::new("agent", RoleId::parse("implementer").unwrap()));
        let record = EvidenceRecord::new(envelope, Some(TaskId::parse("s5#1.7").unwrap()), None, None, EvidenceVerdict::Faithful);
        let value = serde_json::to_value(&record).unwrap();
        GitTier::new(&ledger_root).write(&RawWrite(RawRecord(value))).unwrap();

        let ctx = ctx_for(repo, &ledger_root, policy_with(0, true), fixed_now());
        assert!(StalenessCheck.run(&ctx).is_empty(), "no evidence_sha at all means neither tier can be evaluated");
    }

    #[test]
    fn a_non_green_verdict_is_never_checked_for_staleness() {
        let dir = TempDir::new().unwrap();
        let repo = dir.path();
        init_repo(repo);
        let sha = write_and_commit(repo, "crates/foo/src/bar.rs", "fn bar() {}", "initial");

        let ledger_root = repo.join(".canon/ledger");
        let envelope = Envelope::new(1, RecordKind::Evidence, Utc::now(), Actor::new("agent", RoleId::parse("implementer").unwrap()));
        let record = EvidenceRecord::new(envelope, Some(TaskId::parse("s5#1.7").unwrap()), None, None, EvidenceVerdict::Divergent);
        let mut value = serde_json::to_value(&record).unwrap();
        value.as_object_mut().unwrap().insert("evidence_sha".to_string(), serde_json::json!(sha));
        value.as_object_mut().unwrap().insert("surface_ref".to_string(), serde_json::json!(["crates/foo/src/bar.rs"]));
        GitTier::new(&ledger_root).write(&RawWrite(RawRecord(value))).unwrap();

        write_and_commit(repo, "crates/foo/src/bar.rs", "fn bar() { /* changed */ }", "surface changed");

        let ctx = ctx_for(repo, &ledger_root, policy_with(0, true), fixed_now());
        assert!(StalenessCheck.run(&ctx).is_empty(), "staleness only ever degrades an ALREADY-green record");
    }

    #[test]
    fn a_newer_non_green_latest_verdict_suppresses_staleness_for_an_older_superseded_green_record() {
        // Reordering regression: `fold_latest_green_cells` must fold
        // to the LATEST record per cell FIRST, THEN test verdict —
        // never filter to `Faithful` records before the last-wins
        // fold, or an OLD green record would survive here even after
        // a NEWER `Divergent` record for the SAME cell already
        // demoted it in the ledger's own last-wins read model.
        let dir = TempDir::new().unwrap();
        let repo = dir.path();
        init_repo(repo);
        let sha = write_and_commit(repo, "crates/foo/src/bar.rs", "fn bar() {}", "initial");

        let ledger_root = repo.join(".canon/ledger");
        let old_at = Utc::now() - chrono::Duration::hours(1);
        let new_at = Utc::now();

        // OLD Faithful record for this cell...
        write_record_at(&ledger_root, "s5#1.7", "implementer", EvidenceVerdict::Faithful, old_at, &sha, &["crates/foo/src/bar.rs"]);
        // ...superseded by a NEWER Divergent record for the SAME
        // (subject, role) cell — last-wins makes this the cell's
        // current answer, and it is not green.
        write_record_at(&ledger_root, "s5#1.7", "implementer", EvidenceVerdict::Divergent, new_at, &sha, &["crates/foo/src/bar.rs"]);

        // Move HEAD by touching the declared surface file — would
        // trip staleness for a genuinely green cell.
        write_and_commit(repo, "crates/foo/src/bar.rs", "fn bar() { /* changed */ }", "surface changed");

        let ctx = ctx_for(repo, &ledger_root, policy_with(50, true), fixed_now());
        assert!(
            StalenessCheck.run(&ctx).is_empty(),
            "the newer non-green latest verdict must suppress staleness for the older superseded green record"
        );
    }

    // ── s21 `deterministic-gate-clock`: a real `policy.yaml` carrying
    // an `age_days(...)` CEL predicate, loaded through the REAL
    // `GateContext::load` (never `ctx_for`'s hand-built
    // `PolicyResolution`, since the whole point here is that the CEL
    // predicate is compiled from an on-disk `policy.yaml`) at an
    // explicitly injected `now` — task 6. `evidence_at`/the two
    // injected-now helpers are EXPLICIT UTC constants (design.md R5:
    // never `Utc::now() - Duration::days(N)` computed at test-run
    // time), so this test's own expected verdict is reproducible.

    fn evidence_at() -> DateTime<Utc> {
        DateTime::parse_from_rfc3339("2026-01-01T00:00:00Z").unwrap().with_timezone(&Utc)
    }

    /// Age = 1 day: below the fixture's `age_days(record.at) > 5` threshold.
    fn now_before_threshold() -> DateTime<Utc> {
        evidence_at() + chrono::Duration::days(1)
    }

    /// Age = 10 days: above the threshold.
    fn now_after_threshold() -> DateTime<Utc> {
        evidence_at() + chrono::Duration::days(10)
    }

    /// A git repo with ONE green evidence record (fixed `at`,
    /// `evidence_sha` = the repo's initial commit, declaring the
    /// surface file as its own) followed by a SECOND commit that
    /// changes that exact surface file — plus a `.canon/policy.yaml`
    /// whose `staleness.surface_scoped` is a CEL predicate over
    /// `age_days(record.at)`. `surface_scoped` resolving `false`
    /// (age below threshold) means the surface-changed reason never
    /// even gets checked; resolving `true` (age above threshold)
    /// means it does, and the declared surface DID change since the
    /// evidence sha — the one violation this fixture is built to fire.
    fn time_bearing_fixture() -> (TempDir, PathBuf) {
        let dir = TempDir::new().unwrap();
        let repo = dir.path().to_path_buf();
        init_repo(&repo);
        let sha = write_and_commit(&repo, "crates/foo/src/bar.rs", "fn bar() {}", "initial");

        let ledger_root = repo.join(crate::context::DEFAULT_LEDGER_RELATIVE_PATH);
        write_record_at(&ledger_root, "s5#1.7", "implementer", EvidenceVerdict::Faithful, evidence_at(), &sha, &["crates/foo/src/bar.rs"]);

        write_and_commit(&repo, "crates/foo/src/bar.rs", "fn bar() { /* changed */ }", "surface changed");

        std::fs::create_dir_all(repo.join(".canon")).unwrap();
        std::fs::write(repo.join(".canon").join("policy.yaml"), "staleness:\n  surface_scoped:\n    cel: \"age_days(record.at) > 5\"\n").unwrap();

        (dir, repo)
    }

    #[test]
    fn time_bearing_policy_two_loads_at_the_same_injected_now_produce_byte_identical_reports() {
        let (_dir, repo) = time_bearing_fixture();
        let registry = SchemaRegistry::load();
        let now = now_after_threshold();

        let ctx1 = GateContext::load(GateCtx::from_repo(&repo).unwrap(), &registry, now).unwrap();
        assert!(ctx1.policy.is_clean(), "diagnostics: {:?}", ctx1.policy.diagnostics);
        let report1 = StalenessCheck.run(&ctx1);
        assert!(!report1.is_empty(), "sanity: this fixture at now_after_threshold must actually fire a violation, or this test proves nothing");

        let ctx2 = GateContext::load(GateCtx::from_repo(&repo).unwrap(), &registry, now).unwrap();
        let report2 = StalenessCheck.run(&ctx2);

        assert_eq!(report1, report2, "two independent GateContext::load calls at the SAME injected `now` must produce byte-identical reports");
    }

    #[test]
    fn time_bearing_policy_repeated_evaluation_at_a_fixed_now_never_drifts() {
        let (_dir, repo) = time_bearing_fixture();
        let registry = SchemaRegistry::load();
        let now = now_after_threshold();

        let reports: Vec<_> = (0..3).map(|_| StalenessCheck.run(&GateContext::load(GateCtx::from_repo(&repo).unwrap(), &registry, now).unwrap())).collect();
        assert_eq!(reports[0], reports[1], "run 1 vs run 2 must agree");
        assert_eq!(reports[1], reports[2], "run 2 vs run 3 must agree — no run-to-run drift at a fixed now");
    }

    #[test]
    fn time_bearing_policy_verdict_flips_purely_by_advancing_the_injected_now() {
        let (_dir, repo) = time_bearing_fixture();
        let registry = SchemaRegistry::load();

        let ctx_before = GateContext::load(GateCtx::from_repo(&repo).unwrap(), &registry, now_before_threshold()).unwrap();
        assert!(ctx_before.policy.is_clean(), "diagnostics: {:?}", ctx_before.policy.diagnostics);
        assert!(
            StalenessCheck.run(&ctx_before).is_empty(),
            "age below the age_days threshold: surface_scoped resolves false, the surface-changed reason is never checked"
        );

        let ctx_after = GateContext::load(GateCtx::from_repo(&repo).unwrap(), &registry, now_after_threshold()).unwrap();
        let violations = StalenessCheck.run(&ctx_after);
        assert_eq!(violations.len(), 1, "age above the age_days threshold: surface_scoped resolves true and the declared surface changed since the evidence sha");
        assert_eq!(violations[0].class, FailureClass::StaleEvidence);
        assert_eq!(violations[0].subject, "s5#1.7");
    }

    /// The subjects a set of fixed-task records is about, as the check names them.
    fn stale_subjects(violations: &[Violation]) -> Vec<&str> {
        let mut subjects: Vec<&str> = violations.iter().map(|violation| violation.subject.as_str()).collect();
        subjects.sort_unstable();
        subjects
    }

    /// A repository with one commit holding `files`, and a green record per `(task, declared)` pair of `declarations`
    /// whose base is that commit. Returns the repository, its ledger root and the base.
    fn repo_with_records(files: &[&str], declarations: &[(&str, &[&str])]) -> (TempDir, PathBuf, String) {
        let dir = TempDir::new().unwrap();
        let repo = dir.path().to_path_buf();
        init_repo(&repo);
        for file in files {
            let path = repo.join(file);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(&path, "1").unwrap();
        }
        git(&repo, &["add", "-A"]);
        git(&repo, &["commit", "-q", "-m", "base"]);
        let base = GitCall::new(&repo).args(["rev-parse", "HEAD"]).run().unwrap().stdout_trimmed();
        let ledger_root = repo.join(".canon/ledger");
        for (task, declared) in declarations {
            write_green_record(&ledger_root, task, "implementer", &base, declared);
        }
        (dir, ledger_root, base)
    }

    fn violations_of(dir: &TempDir, ledger_root: &Path) -> Vec<Violation> {
        StalenessCheck.run(&ctx_for(dir.path(), ledger_root, policy_with(50, true), fixed_now()))
    }

    #[test]
    fn a_directory_and_a_glob_declared_as_the_surface_go_stale_when_a_file_under_them_changes() {
        let declarations: [(&str, &[&str]); 4] = [("s1#1.1", &["src/"]), ("s1#1.2", &["src"]), ("s1#1.3", &["src/**"]), ("s1#1.4", &["src/*.rs"])];
        let (dir, ledger_root, _) = repo_with_records(&["src/a.rs", "lib/z.rs"], &declarations);
        write_and_commit(dir.path(), "src/a.rs", "2", "edit");

        let violations = violations_of(&dir, &ledger_root);
        assert_eq!(stale_subjects(&violations), ["s1#1.1", "s1#1.2", "s1#1.3", "s1#1.4"], "{violations:?}");
        for (violation, (_, declared)) in stale_subjects(&violations).iter().zip(&declarations) {
            let detail = &violations.iter().find(|v| v.subject == *violation).unwrap().detail;
            assert!(detail.contains(&format!("`{}`", declared[0])) && detail.contains("`src/a.rs`"), "the reason names the declaration and the path: {detail}");
        }
    }

    #[test]
    fn a_glob_that_does_not_cross_a_slash_is_not_stale_for_a_file_in_a_subdirectory() {
        let declarations: [(&str, &[&str]); 3] = [("s1#1.1", &["src/*.rs"]), ("s1#1.2", &["src/"]), ("s1#1.3", &["Cargo.toml"])];
        let (dir, ledger_root, _) = repo_with_records(&["src/a.rs", "src/x/b.rs", "Cargo.toml", "crates/x/Cargo.toml"], &declarations);
        write_and_commit(dir.path(), "src/x/b.rs", "2", "edit one below src");
        write_and_commit(dir.path(), "crates/x/Cargo.toml", "2", "edit another crate's manifest");

        let violations = violations_of(&dir, &ledger_root);
        assert_eq!(stale_subjects(&violations), ["s1#1.2"], "only the directory covers src/x/b.rs; `src/*.rs` stops at the slash and `Cargo.toml` is the root manifest: {violations:?}");
    }

    #[test]
    fn an_exact_file_and_an_effect_give_the_answer_they_gave_before() {
        let declarations: [(&str, &[&str]); 3] = [("s1#1.1", &["src/a.rs"]), ("s1#1.2", &["effect:payments"]), ("s1#1.3", &["src/a.rs", "effect:payments"])];
        let (dir, ledger_root, _) = repo_with_records(&["src/a.rs", "src/b.rs"], &declarations);
        write_and_commit(dir.path(), "src/b.rs", "2", "edit another file");
        assert!(violations_of(&dir, &ledger_root).is_empty());

        write_and_commit(dir.path(), "src/a.rs", "2", "edit the declared file");
        let violations = violations_of(&dir, &ledger_root);
        assert_eq!(stale_subjects(&violations), ["s1#1.1", "s1#1.3"], "{violations:?}");
    }

    #[test]
    fn the_reason_names_the_first_three_declarations_and_paths_and_how_many_there_are() {
        let (dir, ledger_root, _) = repo_with_records(&["src/a.rs"], &[("s1#1.1", &["src/**", "src/", "src/*.rs", "lib/**"])]);
        for name in ["a.rs", "b.rs", "c.rs", "d.rs", "e.rs"] {
            write_and_commit(dir.path(), &format!("src/{name}"), "2", "edit");
        }
        let violations = violations_of(&dir, &ledger_root);
        assert_eq!(violations.len(), 1);
        let detail = &violations[0].detail;
        assert!(detail.contains("3 declared surface ref(s) matched 5 changed path(s)"), "{detail}");
        assert!(detail.contains("declared `src/**`, `src/`, `src/*.rs`;"), "the declarations in the order the record has them; `lib/**` matched nothing: {detail}");
        assert!(detail.contains("changed `src/a.rs`, `src/b.rs`, `src/c.rs` and 2 more"), "{detail}");
    }

    #[test]
    fn a_base_commit_the_repository_does_not_have_is_counted_and_adds_nothing() {
        let (dir, ledger_root, _) = repo_with_records(&["src/a.rs"], &[]);
        write_green_record(&ledger_root, "s1#1.1", "implementer", &"1".repeat(40), &["src/"]);
        write_green_record(&ledger_root, "s1#1.2", "implementer", &"2".repeat(40), &["src/a.rs"]);
        write_and_commit(dir.path(), "src/a.rs", "2", "edit");

        let ctx = ctx_for(dir.path(), &ledger_root, policy_with(0, true), fixed_now());
        assert!(StalenessCheck.run(&ctx).is_empty());
        let unresolvable = ctx.staleness_unresolvable();
        assert_eq!((unresolvable.base_missing, unresolvable.base_not_ancestor, unresolvable.git_unavailable), (2, 0, 0));
        assert_eq!(unresolvable.first, ["s1#1.1", "s1#1.2"]);
        assert!(!unresolvable.shallow);
        assert!(!unresolvable.message().contains("--unshallow"));
    }

    #[test]
    fn a_base_commit_that_is_not_an_ancestor_is_counted_and_still_judged_from_its_range() {
        let (dir, ledger_root, _) = repo_with_records(&["src/a.rs"], &[]);
        let repo = dir.path();
        git(repo, &["checkout", "-q", "-b", "other"]);
        let other = write_and_commit(repo, "src/a.rs", "2", "the other branch");
        write_green_record(&ledger_root, "s1#1.1", "implementer", &other, &["src/"]);
        git(repo, &["checkout", "-q", "-"]);
        write_and_commit(repo, "lib/b.rs", "1", "the branch HEAD is on");

        let ctx = ctx_for(repo, &ledger_root, policy_with(50, true), fixed_now());
        let violations = StalenessCheck.run(&ctx);
        assert_eq!(stale_subjects(&violations), ["s1#1.1"], "`<base>..HEAD` differs in src/a.rs, as it did before: {violations:?}");
        let unresolvable = ctx.staleness_unresolvable();
        assert_eq!((unresolvable.base_missing, unresolvable.base_not_ancestor, unresolvable.git_unavailable), (0, 1, 0));
    }

    #[test]
    fn a_git_that_cannot_answer_counts_its_cells_as_unavailable() {
        let (dir, ledger_root, _) = repo_with_records(&["src/a.rs"], &[("s1#1.1", &["src/"])]);
        let gone = dir.path().join("no-such-directory");
        let ctx = ctx_for(&gone, &ledger_root, policy_with(0, true), fixed_now());
        assert!(StalenessCheck.run(&ctx).is_empty());
        let unresolvable = ctx.staleness_unresolvable();
        assert_eq!((unresolvable.base_missing, unresolvable.base_not_ancestor, unresolvable.git_unavailable), (0, 0, 1));
    }

    #[test]
    fn nothing_is_counted_when_every_base_is_an_ancestor() {
        let (dir, ledger_root, _) = repo_with_records(&["src/a.rs"], &[("s1#1.1", &["src/"])]);
        write_and_commit(dir.path(), "src/a.rs", "2", "edit");
        let ctx = ctx_for(dir.path(), &ledger_root, policy_with(50, true), fixed_now());
        assert_eq!(StalenessCheck.run(&ctx).len(), 1);
        assert!(ctx.staleness_unresolvable().is_empty());
        assert!(unresolvable_cells(&ctx, || panic!("the code tree is not asked for when nothing is unread")).is_empty());
    }

    #[test]
    fn the_report_without_queries_counts_the_cells_the_check_counts() {
        let (dir, ledger_root, _) = repo_with_records(&["src/a.rs"], &[("s1#1.1", &["src/"])]);
        write_green_record(&ledger_root, "s1#1.2", "implementer", &"3".repeat(40), &["src/"]);
        write_green_record(&ledger_root, "s1#1.3", "reviewer", &"4".repeat(40), &["src/"]);
        let ctx = ctx_for(dir.path(), &ledger_root, policy_with(50, true), fixed_now());
        let asked = std::cell::Cell::new(0);
        let alone = unresolvable_cells(&ctx, || {
            asked.set(asked.get() + 1);
            None
        });
        assert_eq!(&alone, ctx.staleness_unresolvable());
        assert_eq!(asked.get(), 1);
        assert_eq!(alone.base_missing, 2);
    }

    #[test]
    fn the_message_counts_the_cells_by_reason_names_three_subjects_and_points_a_shallow_clone_at_unshallow() {
        let unresolvable = Unresolvable { base_missing: 3, base_not_ancestor: 1, git_unavailable: 0, subjects: 4, first: vec!["a".into(), "b".into(), "c".into()], shallow: true };
        let message = unresolvable.message();
        assert!(message.starts_with("4 (subject, role) cell(s)"), "{message}");
        assert!(message.contains("(base_missing 3, base_not_ancestor 1)"), "{message}");
        assert!(message.contains("first: `a`, `b`, `c`, ..."), "{message}");
        assert!(message.contains("git fetch --unshallow"), "{message}");
        assert_eq!(unresolvable.items(), ["base_missing: 3 cell(s)", "base_not_ancestor: 1 cell(s)", "first subjects: a, b, c, ..."]);
    }
}
