//! `canon init [--repo <dir>] [--no-agents-md] [--policy <level> |
//! --no-policy]` + `canon init --check [--repo <dir>]` (s19
//! `canon-init-scaffold` spec, 0.14 D2): scaffolds a fresh, WORKING
//! `canon.yaml` skeleton (design D8/D9) at `<repo>/canon.yaml`, plus what
//! a repo needs to start the working loop: a starter `.canon/policy.yaml`
//! a human approves, at the level `--policy` names
//! ([`crate::policy_level`], 0.16 D3; `strict` by default), the
//! commented `plans:` examples (canon's own changes live in
//! `.canon/changes/`, read with no entry; `plans.sources` lists only
//! external plans canon reads, 0.17 D2), and a
//! canon block in `AGENTS.md` between `<!-- canon:begin -->`/`<!-- canon:end -->`.
//! It never overwrites an existing `canon.yaml` or `policy.yaml`; a
//! second `init` on a repo that already has `canon.yaml` only refreshes
//! the `AGENTS.md` block. With `--check` it READ-ONLY validates an
//! EXISTING `canon.yaml` by chaining the SAME three independently
//! strict loaders `canon inventory sync`/`canon ingest plans`/`canon
//! tier age` already use: [`TierPolicy::from_yaml`],
//! [`crate::inventory::load_spec_roots`],
//! [`crate::plans::load_plan_sources_from_config`] (design D7). This
//! module reimplements NONE of their validation logic -- it only
//! chains them and formats one PASS/FAIL/"not configured" line per
//! section, never stopping at the first failure (mirroring `canon fmt
//! --check`'s own "report everything" convention).
//!
//! # Harness choice (0.17 D5)
//! A fresh `init` also picks the harnesses the canon skill is installed
//! for ([`SkillsChoice`]): `--providers` answers; otherwise, on a
//! terminal, it asks with the detected harnesses (`.claude`, `.codex`,
//! `.omp`, `.pi` directories) checked; otherwise it takes the detected
//! ones. The answer is written as `canon.yaml` `skills.providers` and the
//! skill is installed (the stub written from the text built into the
//! binary, with no `canon/skills` tree in the repo), as `canon skills
//! install` does. With no terminal, no flag and nothing detected, it
//! records nothing, installs nothing and says how to choose.
//! `--check` validates `skills:` through
//! [`crate::skills::configured_providers`].
//!
//! # `<repo>` is used literally, never an ancestor walk
//! Every other subcommand's `--repo` resolves through
//! `crate::context::resolve_repo_root`'s nearest-ancestor-`canon.yaml`
//! walk -- appropriate for a command operating INSIDE an already-
//! configured repo. `canon init`'s whole job is bootstrapping the
//! FIRST `canon.yaml`, so walking up to find some OTHER ancestor's
//! existing config would resolve to the wrong place entirely (and
//! could spuriously refuse-as-already-exists against a config this
//! invocation never intended to touch); `<repo>/canon.yaml` (spec.md's
//! own literal join) is used exactly as given, default `.` meaning cwd.

use std::fs;
use std::io::{BufRead, IsTerminal as _, Write as _};
use std::path::Path;

use canon_model::envelope::RecordKind;
use canon_model::paths;
use canon_store::policy::{BackendConfig, TierPolicy};

use crate::cli_error::CliError;
use crate::policy_level::{starter_policy, PolicyLevel};
use crate::skills::{self, Provider};

/// Kinds routed to `hot` by [`skeleton_yaml`] (s32 `sqlite-hot-backend`):
/// the same hot-class set the tiered-storage docs/this repo's own
/// `canon.yaml` already use (task/handoff/session/run/event) --
/// `canon init` can now afford to route them there by default because
/// `hot`'s sqlite backend needs no operator-supplied credential
/// (unlike postgres's `dsn_env` or s3's `bucket_env`, which `init`
/// still can't guess -- `cold`-class kinds stay on `local`). Every
/// OTHER kind (`RecordKind::ALL` minus this set) routes to `local`.
const HOT_KINDS: [RecordKind; 5] = [RecordKind::Task, RecordKind::Handoff, RecordKind::Session, RecordKind::Run, RecordKind::Event];

/// The lines [`scaffold_gitignore`] ensures are present in
/// `<repo>/.gitignore`, each under its own comment: one glob covering
/// the sqlite hot tier's db file AND its WAL/SHM sidecars
/// (`.canon/hot.db-wal`/`.canon/hot.db-shm` -- sqlite's own
/// WAL-journal-mode naming convention), since all three share the
/// `.canon/hot.db` prefix; the read-access audit directory `canon
/// query`/`export`/`purge` append to (#8), which is local operator
/// state and would otherwise show up as untracked after the first
/// query; the derived scenario index, which every reader rebuilds
/// from the `.feature` files when stale (0.16 D2), so committing it
/// would only commit churn; and the lock files that serialize those
/// rebuilds and artifact-store writes, which are local process state;
/// and the three derived directories `canon dashboard`/`canon report`
/// write (the regenerated snapshot, the learn scratch area, the local
/// object-store stand-in), which would otherwise be untracked after the
/// first run (#105, #107); and the archive of superseded scenario index
/// versions, which every rewrite of the derived index fills.
const GITIGNORE_LINES: [(&str, &str); 9] = [
    ("# canon's sqlite hot tier (s32 sqlite-hot-backend) -- db file + WAL/SHM sidecars.", paths::HOT_DB_GITIGNORE),
    ("# canon's read-access audit log (canon query/export/purge) -- local, never committed.", paths::AUDIT_GITIGNORE),
    ("# canon's scenario index -- derived from the .feature files, rebuilt by every reader.", paths::SCENARIO_INDEX_GITIGNORE),
    ("# the archive of superseded scenario index versions -- derived like the index, never committed.", paths::SCENARIO_ARCHIVE_GITIGNORE),
    ("# the lock that serializes scenario index rebuilds -- local process state.", paths::SCENARIO_INDEX_LOCK_FILE),
    ("# the lock that serializes artifact store writes -- local process state.", paths::ARTIFACT_STORE_LOCK_FILE),
    ("# the snapshot `canon dashboard` and `canon report snapshot` regenerate -- derived, never committed.", paths::DASHBOARD_SNAPSHOT_GITIGNORE),
    ("# canon's learn scratch area -- rebuildable, never committed.", paths::LEARN_GITIGNORE),
    ("# canon's local object-store stand-in -- local state, never committed.", paths::R2_LOCAL_GITIGNORE),
];

/// D8/D9's skeleton `canon.yaml` body: every one of `RecordKind::ALL`'s
/// fourteen wire strings (s36: `subject` is the reviewed 13th kind;
/// s43: `finding` the 14th)
/// routed to either `local` (git-backed) or `hot`
/// (sqlite-backed, [`HOT_KINDS`]) -- the two zero-env-var rungs (s32
/// `sqlite-hot-backend`: sqlite needs no operator-supplied credential,
/// unlike postgres/s3) -- commented `tiers.hot` (postgres swap) /
/// `tiers.cold` stanzas documenting the scale-up path, one working
/// `specs.roots[]` entry at [`paths::SPECS_DIR`] (D9: a present `specs:` section
/// requires at least one root, so this ships real rather than empty;
/// 0.17 D1 keeps it inside `.canon/`), and one
/// `plans:` section that configures no source (0.17 D2): `canon change
/// new` writes canon's own changes under
/// [`canon_model::paths::CHANGES_DIR`], which every plan reader reads
/// with no entry, so `plans.sources` lists only external plans canon
/// reads, shown as commented openspec and superpowers examples.
fn skeleton_yaml() -> String {
    let mut routing = String::new();
    for kind in RecordKind::ALL {
        let rung = if HOT_KINDS.contains(&kind) { "hot" } else { "local" };
        routing.push_str(&format!("  {}: {rung}\n", kind.as_str()));
    }

    let mut out = String::new();
    out.push_str("# canon.yaml -- scaffolded by `canon init` (s19 canon-init-scaffold).\n");
    out.push_str("# `local` (git-backed) and `hot` (sqlite-backed, s32 sqlite-hot-\n");
    out.push_str("# backend) both need zero operator-supplied credentials, so every\n");
    out.push_str("# kind below is already routed -- task/handoff/session/run/event to\n");
    out.push_str("# `hot`, everything else to `local`. Flip a `routing:` line to `cold`\n");
    out.push_str("# once you have a real `bucket_env` credential `init` cannot guess\n");
    out.push_str("# (see the commented `tiers.cold` stanza below; s27\n");
    out.push_str("# tier-role-backend-split: routing/aging name a capability RUNG, the\n");
    out.push_str("# backend is tagged separately via `tiers.<rung>.backend`).\n");
    out.push_str("tiers:\n");
    out.push_str("  local:\n");
    out.push_str("    backend: git\n");
    out.push_str(&format!("    root: {}\n", paths::LEDGER_DIR));
    out.push_str("  hot:\n");
    out.push_str("    backend: sqlite\n");
    out.push_str(&format!("    path: {}\n", paths::HOT_DB_FILE));
    out.push_str("  # hot (same-class swap for team-scale multi-agent concurrency --\n");
    out.push_str("  # sqlite's WAL journal mode already covers concurrent batch\n");
    out.push_str("  # ingest from a single operator; swap to postgres once you need a\n");
    out.push_str("  # real server -- comment out the live `hot:` block above and\n");
    out.push_str("  # uncomment this one):\n");
    out.push_str("  #   backend: postgres\n");
    out.push_str("  #   dsn_env: CANON_PG_DSN\n");
    out.push_str("  #   schema: canon_v1\n");
    out.push_str("  # cold:\n");
    out.push_str("  #   backend: s3\n");
    out.push_str("  #   bucket_env: CANON_R2_BUCKET\n");
    out.push_str("  #   prefix: \"canon/\"\n");
    out.push_str("routing:\n");
    out.push_str(&routing);
    out.push_str("specs:\n");
    out.push_str("  # The .feature corpus: <root>/kind=feature/area=<area>/<surface>.feature.\n");
    out.push_str("  roots:\n");
    out.push_str("    - id: root\n");
    out.push_str(&format!("      root: {}\n", paths::SPECS_DIR));
    out.push_str("plans:\n");
    out.push_str("  # canon's own changes live in .canon/changes/<slug>/ (`canon change\n");
    out.push_str("  # new` writes brief.md there); every reader reads them\n");
    out.push_str("  # with no entry here. `sources` lists external plans you prepare,\n");
    out.push_str("  # which canon only reads: it never writes into them, except that\n");
    out.push_str("  # `canon task done` flips a task's checkbox in their tasks.md.\n");
    out.push_str("  # Their briefs and approvals live in .canon/changes/<change>/brief.md.\n");
    out.push_str("  #   - dialect: openspec      # openspec/changes/<change>/proposal.md + tasks.md\n");
    out.push_str("  #     root: .\n");
    out.push_str("  #   - dialect: superpowers   # writing-plans documents\n");
    out.push_str("  #     root: docs/superpowers/plans\n");
    out.push_str("  sources: []\n");
    out.push_str("write:\n");
    out.push_str("  # Staged writes (`canon evidence add`, `canon finding add|close`,\n");
    out.push_str("  # `canon divergence add`) promote in the same run. `manual` keeps\n");
    out.push_str("  # them staged until `canon gate promote`; `--no-promote` keeps one.\n");
    out.push_str("  promote: auto\n");
    out
}

/// The [`GITIGNORE_LINES`] entries `<repo>/.gitignore` lacks, as
/// `(comment, line)`, in order. A missing or unreadable file lacks all.
pub(crate) fn missing_gitignore_lines(repo: &Path) -> Vec<(&'static str, &'static str)> {
    let existing = fs::read_to_string(repo.join(".gitignore")).unwrap_or_default();
    GITIGNORE_LINES.iter().copied().filter(|(_, line)| !existing.lines().any(|have| have.trim() == *line)).collect()
}

/// What `canon skills doctor` and `canon init --check` say when
/// `<repo>/.gitignore` lacks managed lines (#105, #107): the lines, and
/// the command that adds them. `None` when none is missing, and when
/// the directory is neither a git checkout nor carries a `.gitignore`
/// (nothing there for git to show as untracked).
pub(crate) fn gitignore_gap(repo: &Path) -> Option<String> {
    if !repo.join(".git").exists() && !repo.join(".gitignore").exists() {
        return None;
    }
    let missing: Vec<&str> = missing_gitignore_lines(repo).into_iter().map(|(_, line)| line).collect();
    if missing.is_empty() {
        return None;
    }
    Some(format!("`.gitignore` lacks {}, which canon writes and git would show as untracked; fix: `canon migrate`", missing.join(", ")))
}

/// Ensures every [`GITIGNORE_LINES`] entry is present in
/// `<repo>/.gitignore` -- appends each missing one under its comment (or
/// under the comment the file kept without it; never a second copy of a
/// comment), creating the file if absent, so a `.gitignore` a prior `canon init`
/// (or the repo's own `git init`) already wrote is never duplicated.
/// `.gitignore` commonly PRE-EXISTS a `canon init` invocation (e.g. a
/// `git init`-then-`canon init` sequence), and a repo initialized by an
/// older canon lacks the lines added since (#35), so this appends rather
/// than mirroring `canon.yaml`'s create-fails-if-exists refusal: a fresh
/// `canon init`, a re-run on an existing repo and `canon migrate` all
/// call it. Returns the lines it added.
pub(crate) fn scaffold_gitignore(repo: &Path) -> std::io::Result<Vec<&'static str>> {
    let path = repo.join(".gitignore");
    let existing = fs::read_to_string(&path).unwrap_or_default();
    let missing = missing_gitignore_lines(repo);
    if missing.is_empty() {
        return Ok(Vec::new());
    }
    // A managed comment the file kept without its line gets the line
    // directly under it; a comment is never written twice. Every other
    // byte of the file is kept as it was.
    let mut segments: Vec<String> = existing.split_inclusive('\n').map(str::to_string).collect();
    let mut appended = String::new();
    for (comment, line) in &missing {
        match segments.iter().position(|have| have.trim() == *comment) {
            Some(at) => {
                if !segments[at].ends_with('\n') {
                    segments[at].push('\n');
                }
                segments.insert(at + 1, format!("{line}\n"));
            }
            None => appended.push_str(&format!("{comment}\n{line}\n")),
        }
    }
    let mut text: String = segments.concat();
    if !appended.is_empty() && !text.is_empty() && !text.ends_with('\n') {
        text.push('\n');
    }
    text.push_str(&appended);
    fs::write(&path, text)?;
    Ok(missing.into_iter().map(|(_, line)| line).collect())
}

/// [`scaffold_gitignore`], or under `--dry-run` only the lines it would add.
fn gitignore_step(repo: &Path, dry_run: bool) -> std::io::Result<Vec<&'static str>> {
    if dry_run {
        return Ok(missing_gitignore_lines(repo).into_iter().map(|(_, line)| line).collect());
    }
    scaffold_gitignore(repo)
}

/// Opening marker of the canon block `init` owns in `AGENTS.md`.
pub(crate) const AGENTS_BEGIN: &str = "<!-- canon:begin -->";
/// Closing marker of the canon block `init` owns in `AGENTS.md`.
pub(crate) const AGENTS_END: &str = "<!-- canon:end -->";

/// The canon block, markers included, no trailing newline (0.14 D2):
/// harness-neutral, short, and pointing at `canon status` and the skill
/// for everything else.
const AGENTS_BLOCK: &str = "\
<!-- canon:begin -->
## Canon

Features in this repo are managed with canon. Start with `canon status`,
and run `canon skills show canon` for the full guide before you change a
feature.

Unsure of a command or its flags: `canon schema <command> --json`. Pass
`--json` and read `error.code` and `error.hint` when a command fails.

Roles: if you were spawned as one (planner, implementer, reviewer, orchestrator), set `CANON_ROLE` (or pass `--as`) and run `canon skills show role-<role>` for what the role does; with no role, follow `canon status`.

The loop:

1. Brief: for a new subject or a scope change, write the PRD brief in the change's `.canon/changes/<slug>/brief.md` (intent, outcome, user flow `F1`, `F2`, …, step detail, acceptance, questions and assumptions, out of scope), in the `language:` of `canon.yaml` (unset: the request's language, with \"which language?\" in the questions). Run `canon brief check` until it has no blockers, then stop and report the brief to the human in that language: build and attest nothing for that scope until the human answers. The approval is never assumed, even when the human said they cannot answer until you report; the brief is that report. One human answer approves the brief and the policy; record it in one call: `canon brief approve <slug> --approver <the human> --note \"<their words>\"`. Never approve without a human answer. A brief edited after its approval needs a new answer.
2. Subject: `canon subject new`, then `canon change new` for the change (`--change-kind chore` for a refactor, dependency, build, CI or infra change that alters no behavior: short brief, no scenarios, the suite run again with `canon evidence add --all-scenarios`, then one change-level review with `canon review change <slug>`; behavior-changing work is never a chore); it writes the brief skeleton `.canon/changes/<slug>/brief.md`. A change from an external plan (an openspec `proposal.md`) gets its brief there too: a `brief.md` whose `Source: <path>` line points at the plan; canon never edits the plan.
3. Scenarios: `canon scenario new --flow <F-id>`, one per behavior of a flow step, failure paths included (`--case failure`).
4. Units: split the work; one actor id and session per unit.
5. Implement: tests named by scenario id.
6. Evidence: `canon evidence add` (it promotes itself under `write: { promote: auto }`; only under `write.promote: manual` run `canon gate promote` after it).
7. Independent review: a different session (a separate process with its own session id, never this session under another label; without a subagent tool, start one), one that can run the checks it signs off, records `canon review scenario` (or `canon review change` for a chore) and `canon finding add`, on every change; a reviewer that cannot run the tests says so and does not sign off.
8. Transition: `canon subject move`, with `canon gate check` clean; move to `shipped` yourself once its guard passes, unless the brief says the human ships. The ship guard and `canon evidence add` refuse while the brief is unapproved or was edited after approval, and shipping also needs every flow step covered by a scenario with a failure case; they are backstops, the stop is at step 1.

Inside an approved scope, never stop for a human: fixes, follow-ups and review rounds stay in the approved change, never `canon change new` (a new change is a new scope), and go straight to scenario, implement, evidence and review; a follow-up that adds or changes a step of the brief edits it, which needs a new answer. `tasks.md` is an optional work note nothing gates on. When no human can answer one of the brief's questions, write your assumption into the brief and proceed on it; that covers the brief's questions and work inside an approved scope, never the approval. `.canon/policy.yaml` is what the gate grades against.
<!-- canon:end -->";

/// The byte span `[start, end)` of every standalone `marker` line (the
/// line's trimmed text is exactly the marker) outside fenced code
/// blocks, with `end` before the line's newline. `Err` when the text
/// ends inside an unclosed fence: a block appended there would land in
/// the fence and never be found again.
fn marker_lines(text: &str) -> Result<(Vec<(usize, usize)>, Vec<(usize, usize)>), String> {
    let (mut begins, mut ends) = (Vec::new(), Vec::new());
    let mut open_fence: Option<(char, usize)> = None;
    let mut offset = 0;
    for raw in text.split_inclusive('\n') {
        let line = raw.trim_end_matches(['\n', '\r']);
        let span = (offset, offset + line.len());
        offset += raw.len();
        match (open_fence, canon_ingest::markdown::fence_of(line)) {
            (None, Some((ch, run, _))) => open_fence = Some((ch, run)),
            (Some((open_ch, open_run)), Some((ch, run, true))) if ch == open_ch && run >= open_run => open_fence = None,
            (Some(_), _) => {}
            (None, None) if line.trim() == AGENTS_BEGIN => begins.push(span),
            (None, None) if line.trim() == AGENTS_END => ends.push(span),
            (None, None) => {}
        }
    }
    if open_fence.is_some() {
        return Err("AGENTS.md ends inside an unclosed code fence; close it and rerun".to_string());
    }
    Ok((begins, ends))
}

/// `AGENTS.md`'s content with the canon block merged in. `existing` is
/// the current file (`None` when absent). Only standalone marker lines
/// outside fenced code blocks count as markers, so a file that quotes
/// them in a code sample is not mistaken for one canon owns. With no
/// markers the block is appended after a blank line; with exactly one
/// begin line followed by exactly one end line, that span is replaced
/// in place. Any other arrangement (a duplicate or unmatched marker, an
/// end before the begin, a trailing unclosed fence) is refused rather
/// than guessed at, before anything is written. Bytes outside the
/// markers are never changed, and merging the result again is a
/// byte-identical no-op.
fn merge_agents_md(existing: Option<&str>) -> Result<String, String> {
    let Some(text) = existing else {
        return Ok(format!("{AGENTS_BLOCK}\n"));
    };
    match marker_lines(text)? {
        (begins, ends) if begins.is_empty() && ends.is_empty() => {
            let mut out = text.to_string();
            if !out.is_empty() {
                if !out.ends_with('\n') {
                    out.push('\n');
                }
                out.push('\n');
            }
            out.push_str(AGENTS_BLOCK);
            out.push('\n');
            Ok(out)
        }
        (begins, ends) if begins.len() == 1 && ends.len() == 1 && begins[0].1 <= ends[0].0 => {
            Ok(format!("{}{AGENTS_BLOCK}{}", &text[..begins[0].0], &text[ends[0].1..]))
        }
        (begins, ends) => Err(format!(
            "AGENTS.md must hold exactly one `{AGENTS_BEGIN}` line followed by one `{AGENTS_END}` line outside code fences (found {} begin, {} end); fix the markers by hand and rerun",
            begins.len(),
            ends.len()
        )),
    }
}

/// What a `canon init` run did, step by step: each step is said in human
/// form (stdout, or stderr under `--json`) and kept as `{ "step", "detail" }`
/// for the JSON `data.steps`.
struct InitLog {
    json: bool,
    /// `--dry-run`: every step is a preview and the document says so.
    dry_run: bool,
    /// Whether [`InitLog::finish`] prints the JSON document; `canon demo init`
    /// runs `init` inside its own command and prints one of its own.
    emit_data: bool,
    steps: Vec<InitStep>,
    /// Whether any step wrote (or, under `--dry-run`, would write) something; a run
    /// that did not found the repo already set up and reports `meta.unchanged`.
    wrote: bool,
}

impl InitLog {
    fn new(json: bool, dry_run: bool, emit_data: bool) -> Self {
        Self { json, dry_run, emit_data, steps: Vec::new(), wrote: false }
    }

    fn step(&mut self, step: &str, detail: impl Into<String>, line: String) {
        crate::say!(self.json, "{line}");
        self.steps.push(InitStep { step: step.to_string(), detail: detail.into() });
    }

    fn finish(self, repo: &Path) -> i32 {
        if self.emit_data {
            if !self.wrote {
                crate::json_out::mark_unchanged();
            }
            crate::next::finish(repo, "init", &crate::next::Scope::Repo, false, self.json, &InitReport { dry_run: self.dry_run, steps: self.steps });
        }
        0
    }
}

/// One step `canon init` took.
#[derive(serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InitStep {
    pub step: String,
    pub detail: String,
}

/// `canon init`'s report (`InitReport`): what it did, step by step.
#[derive(serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InitReport {
    /// True when `--dry-run` previewed the run without writing.
    pub dry_run: bool,
    pub steps: Vec<InitStep>,
}

impl crate::json_out::Report for InitReport {}

/// One `canon.yaml` section `init --check-config` examined.
#[derive(serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InitCheckSection {
    pub section: String,
    /// `pass`, `fail`, `warn` or `not-configured`.
    pub status: String,
    pub detail: String,
}

/// `canon init --check-config`'s report (`InitCheckReport`).
#[derive(serde::Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InitCheckReport {
    pub ok: bool,
    pub sections: Vec<InitCheckSection>,
}

impl crate::json_out::Report for InitCheckReport {}

/// What `canon init` writes besides `canon.yaml` (the opt-out flags).
#[derive(Debug, Clone)]
pub struct InitOptions {
    /// Write or refresh the canon block in `AGENTS.md`.
    pub agents_md: bool,
    /// The level of the starter `.canon/policy.yaml` to write; `None`
    /// writes none.
    pub policy: Option<PolicyLevel>,
    /// How a fresh `init` picks the harnesses to install the canon skill
    /// for (0.17 D5).
    pub skills: SkillsChoice,
    /// `--language <tag>` (0.19 D6): recorded as `canon.yaml`'s
    /// `language:`, the language briefs are written in. `None` records
    /// nothing (an unset language reads as `en`).
    pub language: Option<String>,
    /// `--dry-run`: run every check, write nothing, report what would be
    /// written or refreshed.
    pub dry_run: bool,
}

/// How a fresh `canon init` picks the harnesses (0.17 D5).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum SkillsChoice {
    /// Ask on a terminal, the detected harnesses checked; without a
    /// terminal, the detected harnesses.
    Ask,
    /// `--providers <list>`: this list (`none` or empty for none), no prompt.
    Providers(String),
    /// Record and install nothing (`canon demo init`).
    Skip,
}

/// The harnesses a fresh `init` installs for, and whether the choice is
/// recorded as `skills.providers` (an answer, or a detection that found
/// something; nothing detected without an answer records nothing).
struct InitSkills {
    providers: Vec<Provider>,
    recorded: bool,
}

/// Asks which harnesses get the canon skill: the four providers with the
/// `detected` ones checked, then a comma- or space-separated list, `none`,
/// or an empty line (or end of input) for the checked ones. An unknown
/// name is reported and asked again.
fn prompt_providers(detected: &[Provider], input: &mut impl BufRead, out: &mut impl std::io::Write) -> std::io::Result<Vec<Provider>> {
    writeln!(out, "canon init: which harnesses should get the canon skill?")?;
    for provider in skills::ALL_PROVIDERS {
        if detected.contains(&provider) {
            writeln!(out, "  [x] {:<7} ({} found)", provider.as_str(), provider.marker())?;
        } else {
            writeln!(out, "  [ ] {}", provider.as_str())?;
        }
    }
    loop {
        write!(out, "Comma-separated list, `none`, or Enter for the checked ones [{}]: ", skills::provider_names(detected))?;
        out.flush()?;
        let mut line = String::new();
        if input.read_line(&mut line)? == 0 {
            writeln!(out)?;
            return Ok(detected.to_vec());
        }
        let answer = line.trim();
        if answer.is_empty() {
            return Ok(detected.to_vec());
        }
        match skills::parse_provider_list(&answer.replace(char::is_whitespace, ",")) {
            Ok(providers) => return Ok(providers),
            Err(e) => writeln!(out, "canon init: {e}")?,
        }
    }
}

/// Resolves [`InitOptions::skills`] for a fresh `init` in `repo`. `Err`
/// for an unknown `--providers` name, before anything is written.
fn choose_skills(repo: &Path, choice: &SkillsChoice) -> Result<Option<InitSkills>, String> {
    match choice {
        SkillsChoice::Skip => Ok(None),
        SkillsChoice::Providers(list) => skills::parse_provider_list(list)
            .map(|providers| Some(InitSkills { providers, recorded: true }))
            .map_err(|e| e.to_string()),
        SkillsChoice::Ask => {
            let detected = skills::detected_providers(repo);
            if std::io::stdin().is_terminal() && std::io::stderr().is_terminal() {
                let providers = prompt_providers(&detected, &mut std::io::stdin().lock(), &mut std::io::stderr())
                    .map_err(|e| format!("failed to read the harness choice: {e}"))?;
                Ok(Some(InitSkills { providers, recorded: true }))
            } else {
                let recorded = !detected.is_empty();
                Ok(Some(InitSkills { providers: detected, recorded }))
            }
        }
    }
}

/// The `skills:` section appended to the skeleton: the recorded choice,
/// or a commented example when nothing is recorded.
fn skills_section(skills: Option<&InitSkills>) -> String {
    let mut out = String::from(skills::SKILLS_CONFIG_COMMENT);
    match skills {
        Some(chosen) if chosen.recorded => out.push_str(&skills::skills_config_block(&chosen.providers)),
        _ => out.push_str("# skills:\n#   providers: [claude, codex]\n"),
    }
    out
}

/// The `language:` line appended to the skeleton (0.19 D6): the recorded
/// tag, or a commented example when none is given.
fn language_section(language: Option<&crate::language::Language>) -> String {
    let mut out = String::from("# The language briefs are written in: templates and prose rules ship for\n# `en` and `ko`; any other tag gets `en` headings and the structure rules.\n");
    match language {
        Some(language) => out.push_str(&format!("language: {}\n", language.tag)),
        None => out.push_str("# language: en\n"),
    }
    out
}

/// Installs the canon skill for the chosen harnesses through
/// [`skills::install_canonical`] (which follows the `skills.providers`
/// just written), and prints what happened. `Err` carries the exit code.
fn install_chosen_skills(repo: &Path, canon_yaml_path: &Path, chosen: &InitSkills, dry_run: bool, log: &mut InitLog) -> Result<(), i32> {
    if chosen.providers.is_empty() {
        if chosen.recorded {
            log.step("skills-none", "skills.providers is []", "canon init: skills.providers is [] — no canon skill installed; change it with `canon skills install --providers=<list>`".to_string());
        } else {
            log.step("skills-none", "no harness detected", format!("canon init: no harness detected (no .claude, .codex, .omp or .pi directory), so no canon skill is installed; {}", skills::CHOOSE_PROVIDERS));
        }
        return Ok(());
    }
    let source = skills::resolve_source(None);
    // A preview has not written `skills.providers` yet, so it names the choice.
    let names = chosen.providers.iter().map(|p| p.as_str()).collect::<Vec<_>>().join(",");
    match skills::install_canonical(source.as_deref(), repo, dry_run.then_some(names.as_str()), dry_run) {
        Ok(report) => {
            let dirs = report
                .providers
                .iter()
                .map(|provider| skills::provider_bundle_root(repo, *provider).display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            let verb = if dry_run { "would install" } else { "installed" };
            log.wrote |= report.changed || report.config_written.is_some();
            log.step("skills-installed", dirs.clone(), format!("canon init: {verb} the canon skill for {} ({dirs})", skills::provider_names(&report.providers)));
            Ok(())
        }
        Err(e) => {
            Err(CliError::refused(
                "INIT_SKILL_INSTALL_FAILED",
                format!(
                    "wrote `skills: {{ providers: [{}] }}` to {} but did not install the canon skill: {e}",
                    chosen.providers.iter().map(|p| p.as_str()).collect::<Vec<_>>().join(", "),
                    canon_yaml_path.display()
                ),
            )
            .hint(format!("once the skill source is available, run `canon skills install` (the override {} names a directory without a SKILL.src.md; unset it, or pass --skills-source <dir>; it follows skills.providers)", skills::CANONICAL_SOURCE_ENV))
            .emit("canon init", log.json))
        }
    }
}

/// Write `merged` to `AGENTS.md` unless it already holds exactly those
/// bytes, and print what happened.
fn write_agents_md(path: &Path, existing: Option<&str>, merged: &str, dry_run: bool, log: &mut InitLog) -> std::io::Result<()> {
    if existing == Some(merged) {
        log.step("agents-md-current", path.display().to_string(), format!("canon init: the canon block in {} is already current", path.display()));
        return Ok(());
    }
    if !dry_run {
        fs::write(path, merged)?;
    }
    let verb = match (existing.is_some(), dry_run) {
        (true, false) => "refreshed",
        (false, false) => "wrote",
        (true, true) => "would refresh",
        (false, true) => "would write",
    };
    log.wrote = true;
    log.step("agents-md", path.display().to_string(), format!("canon init: {verb} the canon block in {}", path.display()));
    Ok(())
}

/// Write the starter policy at `level` (0.14 D2, 0.16 D3) unless
/// `.canon/policy.yaml` exists, which is left byte-for-byte alone. Every
/// level parses with zero diagnostics through
/// `canon_gate::PolicyResolution::resolve` (pinned by
/// [`crate::policy_level`]'s tests).
fn scaffold_policy(repo: &Path, level: PolicyLevel, dry_run: bool, log: &mut InitLog) -> std::io::Result<()> {
    let path = repo.join(paths::POLICY_FILE);
    let created = if dry_run {
        if path.exists() { Err(std::io::ErrorKind::AlreadyExists.into()) } else { Ok(None) }
    } else {
        fs::create_dir_all(repo.join(paths::CANON_DIR))?;
        fs::OpenOptions::new().write(true).create_new(true).open(&path).map(Some)
    };
    match created {
        Ok(file) => {
            if let Some(mut file) = file {
                file.write_all(starter_policy(level).as_bytes())?;
            }
            let verb = if dry_run { "would write" } else { "wrote" };
            log.wrote = true;
            log.step(
                "policy",
                path.display().to_string(),
                format!(
                    "canon init: {verb} {} — a human reviews it and approves it with `canon policy approve --approver <who>`: the gate grades the work against it, and `canon status` and `canon gate check` say so until it is approved",
                    path.display()
                ),
            );
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            log.step("policy-exists", path.display().to_string(), format!("canon init: {} already exists — left unchanged", path.display()));
        }
        Err(e) => return Err(e),
    }
    Ok(())
}

/// `canon init [--repo <dir>] [--no-agents-md] [--policy <level> |
/// --no-policy]` (task 4.1, 0.14 D2, 0.16 D3). Returns the process exit
/// code.
///
/// - A fresh repo gets `canon.yaml`, the `.gitignore` line, the
///   commented plans examples, the starter policy at the requested
///   level (unless `--no-policy`) and the `AGENTS.md` block (unless
///   `--no-agents-md`):
///   exit `0`.
/// - A repo whose `canon.yaml` exists keeps it byte-identical
///   (`create_new` is the atomic refusal) and only refreshes the
///   `AGENTS.md` block and adds the canon `.gitignore` lines it lacks
///   (#35, a repo initialized by an older canon): exit `0`. With
///   `--no-agents-md` and no line missing there is nothing left to do,
///   so that is the pre-0.14 refusal, exit `2`.
/// - Unbalanced `AGENTS.md` markers, or an unknown `--providers` name,
///   refuse (exit `2`) before anything is written.
/// - A fresh repo also records and installs the harness choice
///   ([`SkillsChoice`], 0.17 D5); an install that fails after the writes
///   exits `2` naming the retry. A rerun never asks and never installs.
pub fn run_init(repo: &Path, options: InitOptions, json: bool) -> i32 {
    run_init_in(repo, options, json, true)
}

/// [`run_init`], with `emit_data: false` leaving the JSON document to the
/// calling command (`canon demo init`).
pub(crate) fn run_init_in(repo: &Path, options: InitOptions, json: bool, emit_data: bool) -> i32 {
    let dry_run = options.dry_run;
    let mut log = InitLog::new(json, dry_run, emit_data);
    if let Err(e) = (!dry_run).then(|| fs::create_dir_all(repo)).transpose() {
        return CliError::refused("WRITE_FAILED", format!("failed to create `{}`: {e}", repo.display())).hint("check that the path is writable and the disk has room, then run the command again").emit("canon init", json);
    }
    let agents_path = repo.join("AGENTS.md");
    let language = match options.language.as_deref().map(crate::language::Language::parse).transpose() {
        Ok(language) => language,
        Err(e) => {
            return CliError::refused("INIT_INVALID_ARGUMENT", format!("--language: {e}")).hint("`canon schema init` lists the accepted arguments and their forms").emit("canon init", json);
        }
    };
    let agents = if options.agents_md {
        let existing = match fs::read_to_string(&agents_path) {
            Ok(text) => Some(text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => {
                return CliError::refused("REPO_UNREADABLE", format!("failed to read `{}`: {e}", agents_path.display())).hint("check that the repository is readable from here, or pass --repo <path> for its root").emit("canon init", json);
            }
        };
        match merge_agents_md(existing.as_deref()) {
            Ok(merged) => Some((existing, merged)),
            Err(e) => {
                return CliError::refused("INIT_AGENTS_MD_CONFLICT", e.to_string()).hint("resolve the conflict in AGENTS.md by hand, or run `canon init --no-agents-md`").emit("canon init", json);
            }
        }
    } else {
        None
    };

    let canon_yaml_path = repo.join("canon.yaml");
    let skills = if canon_yaml_path.exists() {
        None
    } else {
        match choose_skills(repo, &options.skills) {
            Ok(skills) => skills,
            Err(e) => {
                return CliError::refused("INIT_INVALID_ARGUMENT", e.to_string()).hint("`canon schema init` lists the accepted arguments and their forms").emit("canon init", json);
            }
        }
    };
    let created = if dry_run {
        if canon_yaml_path.exists() { Err(std::io::ErrorKind::AlreadyExists.into()) } else { Ok(None) }
    } else {
        fs::OpenOptions::new().write(true).create_new(true).open(&canon_yaml_path).map(Some)
    };
    match created {
        Ok(file) => {
            if let Some(mut file) = file {
                let config = skeleton_yaml() + &language_section(language.as_ref()) + &skills_section(skills.as_ref());
                if let Err(e) = file.write_all(config.as_bytes()) {
                    return CliError::refused("WRITE_FAILED", format!("failed to write `{}`: {e}", canon_yaml_path.display())).hint("check that the path is writable and the disk has room, then run the command again").emit("canon init", json);
                }
            }
            let verb = if dry_run { "would write" } else { "wrote" };
            log.wrote = true;
            log.step("config", canon_yaml_path.display().to_string(), format!("canon init: {verb} {}", canon_yaml_path.display()));
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            let missing = missing_gitignore_lines(repo);
            if agents.is_none() && missing.is_empty() {
                if emit_data {
                    log.step("config-exists", canon_yaml_path.display().to_string(), format!("canon init: {} already exists and nothing is left to refresh", canon_yaml_path.display()));
                    return log.finish(repo);
                }
                return CliError::refused("INIT_CONFIG_EXISTS", format!("`{}` already exists; never overwriting an existing config", canon_yaml_path.display())).hint("keep the existing canon.yaml and check it with `canon init --check`, or remove it first to start over").emit("canon init", json);
            }
            let what = if agents.is_some() { "the AGENTS.md block and the canon .gitignore lines" } else { "the canon .gitignore lines" };
            let refreshed = if dry_run { format!("{what} would be refreshed") } else { format!("{what} are refreshed") };
            log.step("config-exists", canon_yaml_path.display().to_string(), format!("canon init: {} already exists — left unchanged; {refreshed}", canon_yaml_path.display()));
            if matches!(options.skills, SkillsChoice::Providers(_)) {
                log.step("providers-ignored", "--providers", "canon init: --providers is not applied to an existing canon.yaml; run `canon skills install --providers=<list>`, which records the list in skills.providers".to_string());
            }
            if options.language.is_some() {
                log.step("language-ignored", "--language", "canon init: --language is not applied to an existing canon.yaml; add `language: <tag>` to it by hand".to_string());
            }
            match gitignore_step(repo, dry_run) {
                Ok(added) if !added.is_empty() => {
                    log.wrote = true;
                    log.step("gitignore", added.join(", "), format!("canon init: {} .gitignore: {}", if dry_run { "would add to" } else { "added to" }, added.join(", ")));
                }
                Ok(_) => {}
                Err(e) => {
                    return CliError::refused("WRITE_FAILED", format!("failed to update `.gitignore`: {e}")).hint("check that the path is writable and the disk has room, then run the command again").emit("canon init", json);
                }
            }
            if let Some((existing, merged)) = agents {
                if let Err(e) = write_agents_md(&agents_path, existing.as_deref(), &merged, dry_run, &mut log) {
                    return CliError::refused("WRITE_FAILED", format!("failed to write `{}`: {e}", agents_path.display())).hint("check that the path is writable and the disk has room, then run the command again").emit("canon init", json);
                }
            }
            return log.finish(repo);
        }
        Err(e) => {
            return CliError::refused("WRITE_FAILED", format!("failed to create `{}`: {e}", canon_yaml_path.display())).hint("check that the path is writable and the disk has room, then run the command again").emit("canon init", json);
        }
    }

    if let Err(e) = gitignore_step(repo, dry_run) {
        return CliError::refused("WRITE_FAILED", format!("wrote `{}` but failed to update `.gitignore`: {e}", canon_yaml_path.display())).hint("check that the path is writable and the disk has room, then run the command again").emit("canon init", json);
    }
    if let Some(level) = options.policy {
        if let Err(e) = scaffold_policy(repo, level, dry_run, &mut log) {
            return CliError::refused("WRITE_FAILED", format!("failed to write `{}`: {e}", repo.join(paths::POLICY_FILE).display())).hint("check that the path is writable and the disk has room, then run the command again").emit("canon init", json);
        }
    }
    if let Some((existing, merged)) = agents {
        if let Err(e) = write_agents_md(&agents_path, existing.as_deref(), &merged, dry_run, &mut log) {
            return CliError::refused("WRITE_FAILED", format!("failed to write `{}`: {e}", agents_path.display())).hint("check that the path is writable and the disk has room, then run the command again").emit("canon init", json);
        }
    }
    if let Some(chosen) = &skills {
        if let Err(code) = install_chosen_skills(repo, &canon_yaml_path, chosen, dry_run, &mut log) {
            return code;
        }
    }
    log.finish(repo)
}

/// `canon init --check [--repo <dir>]` (task 4.2). Returns the
/// process exit code: `2` when `<repo>/canon.yaml` is missing (fails
/// loud, distinct from any content report), `0` when every PRESENT
/// section parses clean under its own existing loader, `1` when at
/// least one present section fails -- printing one PASS/FAIL/"not
/// configured" line per section regardless (never stopping at the
/// first failure, design D7).
pub fn run_check_config(repo: &Path, json: bool) -> i32 {
    let canon_yaml_path = repo.join("canon.yaml");
    let Ok(text) = fs::read_to_string(&canon_yaml_path) else {
        return CliError::refused("CONFIG_MISSING", format!("`{}` does not exist; run `canon init` first", canon_yaml_path.display())).hint("run `canon init` in the repository root first").emit("canon init --check", json);
    };

    let mut all_ok = true;
    let mut report = String::new();

    // s29 design D9: `TierPolicy::from_yaml_at` has no `canon-store`
    // dependency to call `validate_schema_ident` itself, so a
    // `tiers.<rung>.schema` `PgTier::connect` would reject at attach
    // time could otherwise parse clean here -- checked explicitly, so
    // `[PASS] tiers/routing/aging` can never be printed over a
    // malformed schema.
    match TierPolicy::from_yaml_at(&text, repo) {
        Ok(policy) => {
            let bad_schema = policy.tiers.values().find_map(|cfg| match cfg {
                BackendConfig::Postgres(pg) => canon_store::pg_tier::validate_schema_ident(&pg.schema).err(),
                _ => None,
            });
            match bad_schema {
                None => report.push_str("[PASS] tiers/routing/aging\n"),
                Some(e) => {
                    all_ok = false;
                    report.push_str(&format!("[FAIL] tiers/routing/aging: {e}\n"));
                }
            }
        }
        Err(e) => {
            all_ok = false;
            report.push_str(&format!("[FAIL] tiers/routing/aging: {e}\n"));
        }
    }

    // `load_spec_roots` resolves the single default root even for an
    // absent `specs:` key (a legitimate, already-successful state) --
    // this section is PASS/FAIL only, never "not configured".
    match crate::inventory::load_spec_roots(&canon_yaml_path) {
        Ok(_) => report.push_str("[PASS] specs\n"),
        Err(e) => {
            all_ok = false;
            report.push_str(&format!("[FAIL] specs: {e}\n"));
        }
    }

    // `load_plan_sources_from_config` itself can't distinguish a
    // legitimately ABSENT `plans:` key from a present-but-empty
    // `sources: []` (both resolve to `Ok(vec![])`, its own established
    // fail-soft-on-absent contract) -- that distinction is made here,
    // once, off the SAME already-parsed YAML doc, never a second
    // config parser.
    let plans_present = serde_yaml::from_str::<serde_yaml::Value>(&text).ok().and_then(|doc| doc.get("plans").cloned()).is_some();
    if !plans_present {
        report.push_str("[not configured] plans\n");
    } else {
        match crate::plans::load_plan_sources_from_config(&canon_yaml_path, repo) {
            Ok(_) => report.push_str("[PASS] plans\n"),
            Err(e) => {
                all_ok = false;
                report.push_str(&format!("[FAIL] plans: {e}\n"));
            }
        }
    }

    // `write:` (0.16 D2): absent is the `manual` default, so it is
    // reported as such rather than as a pass of nothing.
    let write_present = serde_yaml::from_str::<serde_yaml::Value>(&text).ok().and_then(|doc| doc.get("write").cloned()).is_some();
    match crate::write_mode::configured_promote_mode(&canon_yaml_path) {
        Ok(mode) if write_present => report.push_str(&format!("[PASS] write (promote: {})\n", mode.as_str())),
        Ok(_) => report.push_str("[not configured] write (promote: manual)\n"),
        Err(e) => {
            all_ok = false;
            report.push_str(&format!("[FAIL] write: {e}\n"));
        }
    }

    // `skills:` (0.17 D5): absent falls back to the install lock's
    // providers, then the detected harnesses, so it is not a failure.
    match skills::configured_providers(&canon_yaml_path) {
        Ok(Some(providers)) => report.push_str(&format!("[PASS] skills (providers: {})\n", skills::provider_names(&providers))),
        Ok(None) => report.push_str("[not configured] skills (providers: the install lock's, else the detected harnesses)\n"),
        Err(e) => {
            all_ok = false;
            report.push_str(&format!("[FAIL] skills: {e}\n"));
        }
    }

    // `language:` (0.19 D6): absent reads as `en`, so it is not a failure.
    match crate::language::from_yaml(&text) {
        Ok(language) if text.lines().any(|l| l.starts_with("language:")) => report.push_str(&format!("[PASS] language ({})\n", language.tag)),
        Ok(_) => {}
        Err(e) => {
            all_ok = false;
            report.push_str(&format!("[FAIL] language: {e}\n"));
        }
    }

    // The managed `.gitignore` lines (#105, #107): a gap is advice, not a
    // config failure, so it never changes the exit code.
    if let Some(gap) = gitignore_gap(repo) {
        report.push_str(&format!("[WARN] gitignore: {gap}\n"));
    }

    // `artifact_store:` (0.23 D3): absent is the local default and prints
    // nothing, like `language:`. A backend that names no s3 tier fails
    // here, not at the first evidence write.
    let artifacts_present = serde_yaml::from_str::<serde_yaml::Value>(&text).ok().and_then(|doc| doc.get("artifact_store").cloned()).is_some();
    match canon_store::artifact_blobs::ArtifactsConfig::from_yaml_at(&text, repo) {
        Ok(config) if artifacts_present => match config.backend {
            canon_store::artifact_blobs::ArtifactsBackend::Local => report.push_str("[PASS] artifact_store (backend: local)\n"),
            canon_store::artifact_blobs::ArtifactsBackend::Tier { name, .. } => report.push_str(&format!("[PASS] artifact_store (backend: {name}, prefix: {})\n", config.prefix)),
        },
        Ok(_) => {}
        Err(e) => {
            all_ok = false;
            report.push_str(&format!("[FAIL] artifact_store: {e}\n"));
        }
    }

    crate::say_raw!(json, "{report}");
    let sections: Vec<InitCheckSection> = report
        .lines()
        .filter_map(|line| {
            let (status, rest) = line.strip_prefix('[')?.split_once("] ")?;
            let section = rest.split([' ', ':']).next().unwrap_or(rest);
            let status = if status == "not configured" { "not-configured" } else { status };
            Some(InitCheckSection { section: section.to_string(), status: status.to_ascii_lowercase(), detail: rest.to_string() })
        })
        .collect();
    let data = InitCheckReport { ok: all_ok, sections };
    if all_ok {
        if json {
            crate::json_out::data("init", &data);
        }
        0
    } else {
        CliError::failed("CONFIG_UNREADABLE", "a present canon.yaml section fails its loader").hint("data.sections names each failing section").emit_with_data("canon init --check", json, &data)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// authoring.init.96: on a terminal init lists the four harnesses with
    /// the detected ones checked; Enter (or end of input) keeps the checked
    /// ones, a list or `none` replaces them, and an unknown name is asked
    /// again.
    #[test]
    fn authoring_init_96_the_prompt_prechecks_the_detected_harnesses() {
        let detected = [Provider::Claude, Provider::Omp];
        let ask = |answers: &str| {
            let mut out = Vec::new();
            let chosen = prompt_providers(&detected, &mut answers.as_bytes(), &mut out).unwrap();
            (chosen, String::from_utf8(out).unwrap())
        };

        let (chosen, shown) = ask("\n");
        assert_eq!(chosen, detected);
        assert_eq!(
            shown,
            "canon init: which harnesses should get the canon skill?\n  [x] claude  (.claude found)\n  [ ] codex\n  [x] omp     (.omp found)\n  [ ] pi\nComma-separated list, `none`, or Enter for the checked ones [claude,omp]: "
        );
        assert_eq!(ask("").0, detected, "end of input keeps the checked ones");
        assert_eq!(ask("pi, codex\n").0, vec![Provider::Codex, Provider::Pi]);
        assert_eq!(ask("codex omp\n").0, vec![Provider::Codex, Provider::Omp], "spaces separate too");
        assert_eq!(ask("none\n").0, vec![]);
        let (chosen, shown) = ask("gemini\nclaude\n");
        assert_eq!(chosen, vec![Provider::Claude]);
        assert!(shown.contains("canon init: invalid provider `gemini`"), "{shown}");
        assert_eq!(shown.matches("Enter for the checked ones").count(), 2, "asked again: {shown}");
    }

    /// The `skills:` section init appends parses back to the choice, and
    /// the skeleton with it still loads under every section loader.
    #[test]
    fn skills_section_round_trips_through_the_config_loader() {
        let dir = tempfile::tempdir().unwrap();
        let canon_yaml = dir.path().join("canon.yaml");
        let chosen = InitSkills { providers: vec![Provider::Codex, Provider::Pi], recorded: true };
        std::fs::write(&canon_yaml, skeleton_yaml() + &skills_section(Some(&chosen))).unwrap();
        assert_eq!(skills::configured_providers(&canon_yaml).unwrap(), Some(vec![Provider::Codex, Provider::Pi]));
        assert!(TierPolicy::from_yaml_at(&std::fs::read_to_string(&canon_yaml).unwrap(), dir.path()).is_ok());
        std::fs::write(&canon_yaml, skeleton_yaml() + &skills_section(None)).unwrap();
        assert_eq!(skills::configured_providers(&canon_yaml).unwrap(), None);
    }

    #[test]
    fn skeleton_yaml_routes_hot_class_kinds_to_hot_and_the_rest_to_local() {
        let yaml = skeleton_yaml();
        for kind in RecordKind::ALL {
            let expected = if HOT_KINDS.contains(&kind) { "hot" } else { "local" };
            assert!(
                yaml.contains(&format!("{}: {expected}", kind.as_str())),
                "missing `{expected}`-routing line for `{}`: {yaml}",
                kind.as_str()
            );
        }
    }

    #[test]
    fn skeleton_yaml_configures_hot_as_sqlite_with_a_resolved_path() {
        let dir = tempfile::tempdir().unwrap();
        let canon_yaml_path = dir.path().join("canon.yaml");
        std::fs::write(&canon_yaml_path, skeleton_yaml()).unwrap();
        let text = std::fs::read_to_string(&canon_yaml_path).unwrap();

        let policy = TierPolicy::from_yaml_at(&text, dir.path()).unwrap();
        let hot = policy.tiers.get(&canon_store::policy::Rung::Hot).expect("scaffolded config must configure a `hot` rung");
        match hot {
            BackendConfig::Sqlite(cfg) => assert_eq!(cfg.path, dir.path().join(".canon/hot.db")),
            other => panic!("expected the scaffolded `hot` rung to be sqlite-backed, got {other:?}"),
        }
    }

    #[test]
    fn skeleton_yaml_parses_clean_through_every_existing_loader() {
        let dir = tempfile::tempdir().unwrap();
        let canon_yaml_path = dir.path().join("canon.yaml");
        std::fs::write(&canon_yaml_path, skeleton_yaml()).unwrap();
        let text = std::fs::read_to_string(&canon_yaml_path).unwrap();

        assert!(TierPolicy::from_yaml_at(&text, dir.path()).is_ok());
        assert!(crate::inventory::load_spec_roots(&canon_yaml_path).is_ok());
        assert!(crate::plans::load_plan_sources_from_config(&canon_yaml_path, dir.path()).is_ok());
        assert_eq!(crate::write_mode::configured_promote_mode(&canon_yaml_path), Ok(crate::write_mode::PromoteMode::Auto), "a new repo promotes on write");
    }

    fn has_every_gitignore_line(text: &str) -> bool {
        GITIGNORE_LINES.iter().all(|(_, line)| text.lines().any(|have| have.trim() == *line))
    }

    #[test]
    fn scaffold_gitignore_creates_a_fresh_file_with_the_hot_db_glob_and_the_audit_dir() {
        let dir = tempfile::tempdir().unwrap();
        scaffold_gitignore(dir.path()).unwrap();
        let text = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(has_every_gitignore_line(&text), "{text}");
        assert!(text.lines().any(|line| line.trim() == paths::AUDIT_GITIGNORE), "{text}");
    }

    #[test]
    fn scaffold_gitignore_appends_to_an_existing_file_without_disturbing_it() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".gitignore"), "target/\n").unwrap();
        scaffold_gitignore(dir.path()).unwrap();
        let text = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(text.contains("target/\n"), "{text}");
        assert!(has_every_gitignore_line(&text), "{text}");
    }

    #[test]
    fn scaffold_gitignore_is_idempotent_and_adds_only_the_missing_line() {
        let dir = tempfile::tempdir().unwrap();
        // A repo initialized before #8 already carries the hot-db line.
        std::fs::write(dir.path().join(".gitignore"), format!("{}\n", paths::HOT_DB_GITIGNORE)).unwrap();
        scaffold_gitignore(dir.path()).unwrap();
        scaffold_gitignore(dir.path()).unwrap();
        let text = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        for (_, line) in GITIGNORE_LINES {
            assert_eq!(text.lines().filter(|have| have.trim() == line).count(), 1, "`{line}` must appear exactly once across repeated scaffolds: {text}");
        }
    }

    /// The default (`strict`) starter is 0.15's starter byte for byte,
    /// under one added level line.
    #[test]
    fn strict_starter_policy_is_the_pre_level_starter_under_one_level_line() {
        const PRE_LEVEL_STARTER: &str = "\
# .canon/policy.yaml -- scaffolded by `canon init`.
#
# This file is what `canon gate check` grades the work against. A human
# should review and approve it, and every later change to it, with
# `canon policy approve --approver <who>`: the agent doing the work should not
# write the rules it is graded by.
spec_coverage:
  # Every scenario in the spec corpus needs evidence (`canon evidence
  # add`). Add `scope: [building, verifying]` to check only scenarios
  # whose subject is in one of those statuses.
  require_evidence: true
  # Every feature surface (`<area>.<surface>`) needs at least one
  # `@case:failure` scenario, so no feature is specified by its golden
  # path alone.
  require_cases: [failure]
  # Every scenario of a subject in `verifying` or `shipped` needs a
  # review (`canon review scenario`) by an actor other than its evidence
  # author, and an open blocker finding on an adopted change blocks.
  require_review: {}
";
        assert_eq!(starter_policy(PolicyLevel::Strict), format!("# canon policy level: strict\n{PRE_LEVEL_STARTER}"));
    }

    #[test]
    fn starter_policy_resolves_with_zero_diagnostics_and_turns_spec_coverage_on() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".canon")).unwrap();
        std::fs::write(dir.path().join(paths::POLICY_FILE), starter_policy(PolicyLevel::Strict)).unwrap();
        let policy = canon_gate::PolicyResolution::resolve(dir.path(), &canon_policy::SchemaRegistry::load());
        assert!(policy.diagnostics.is_empty(), "the starter policy must load clean, unknown-key check included: {:?}", policy.diagnostics);
        match policy.spec_coverage {
            Some(canon_gate::SpecCoverage::Active { require_evidence, require_cases, require_review, .. }) => {
                assert!(require_evidence);
                assert_eq!(require_cases, vec!["failure".to_string()]);
                assert!(require_review.is_some(), "require_review must be on");
            }
            other => panic!("expected an active spec_coverage section, got {other:?}"),
        }
    }

    #[test]
    fn merge_agents_md_creates_appends_and_is_idempotent() {
        let fresh = merge_agents_md(None).unwrap();
        assert!(fresh.starts_with(AGENTS_BEGIN) && fresh.ends_with(&format!("{AGENTS_END}\n")), "{fresh}");
        assert_eq!(merge_agents_md(Some(&fresh)).unwrap(), fresh);

        let existing = "# Project\n\nOur rules.";
        let merged = merge_agents_md(Some(existing)).unwrap();
        assert!(merged.starts_with("# Project\n\nOur rules.\n\n"), "{merged}");
        assert_eq!(merge_agents_md(Some(&merged)).unwrap(), merged);
    }

    #[test]
    fn merge_agents_md_replaces_only_the_marked_span() {
        let existing = format!("before\n{AGENTS_BEGIN}\nstale text\n{AGENTS_END}\nafter {AGENTS_END}\n");
        let merged = merge_agents_md(Some(&existing)).unwrap();
        assert_eq!(merged, format!("before\n{AGENTS_BLOCK}\nafter {AGENTS_END}\n"));
    }

    #[test]
    fn merge_agents_md_refuses_any_marker_arrangement_but_one_ordered_pair() {
        for text in [
            format!("{AGENTS_BEGIN}\nhalf a block\n"),
            format!("{AGENTS_END}\n"),
            format!("{AGENTS_BEGIN}\n{AGENTS_BEGIN}\nx\n{AGENTS_END}\n"),
            format!("{AGENTS_BEGIN}\nx\n{AGENTS_END}\n{AGENTS_END}\n"),
            format!("{AGENTS_END}\nx\n{AGENTS_BEGIN}\n"),
            format!("{AGENTS_BEGIN}\na\n{AGENTS_END}\n{AGENTS_BEGIN}\nb\n{AGENTS_END}\n"),
        ] {
            assert!(merge_agents_md(Some(&text)).is_err(), "must refuse: {text}");
        }
    }

    #[test]
    fn merge_agents_md_ignores_markers_inside_code_fences() {
        let existing = format!("# Docs\n\n```md\n{AGENTS_BEGIN}\nquoted\n{AGENTS_END}\n```\n\n~~~~\n{AGENTS_BEGIN}\n~~~\nstill fenced\n~~~~\n");
        let merged = merge_agents_md(Some(&existing)).unwrap();
        assert_eq!(merged, format!("{existing}\n{AGENTS_BLOCK}\n"), "fenced markers are text, so the block is appended");
        assert_eq!(merge_agents_md(Some(&merged)).unwrap(), merged);
    }

    #[test]
    fn merge_agents_md_refuses_a_trailing_unclosed_fence() {
        assert!(merge_agents_md(Some("# Docs\n\n```\nnever closed\n")).is_err());
    }
}
