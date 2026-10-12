//! `canon init [--repo <dir>] [--no-agents-md] [--policy <level> |
//! --no-policy]` + `canon init --check-config [--repo <dir>]` (s19
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
//! the `AGENTS.md` block. With `--check-config` it READ-ONLY validates an
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
//! skill is installed from the default skill source, as `canon skills
//! install` does. With no terminal, no flag and nothing detected, it
//! records nothing, installs nothing and says how to choose.
//! `--check-config` validates `skills:` through
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
/// rebuilds and artifact-store writes, which are local process state.
const GITIGNORE_LINES: [(&str, &str); 5] = [
    ("# canon's sqlite hot tier (s32 sqlite-hot-backend) -- db file + WAL/SHM sidecars.", paths::HOT_DB_GITIGNORE),
    ("# canon's read-access audit log (canon query/export/purge) -- local, never committed.", paths::AUDIT_GITIGNORE),
    ("# canon's scenario index -- derived from the .feature files, rebuilt by every reader.", paths::SCENARIO_INDEX_GITIGNORE),
    ("# the lock that serializes scenario index rebuilds -- local process state.", paths::SCENARIO_INDEX_LOCK_FILE),
    ("# the lock that serializes artifact store writes -- local process state.", paths::ARTIFACT_STORE_LOCK_FILE),
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
    out.push_str("  # new` writes brief.md + tasks.md there); every reader reads them\n");
    out.push_str("  # with no entry here. `sources` lists external plans you prepare,\n");
    out.push_str("  # which canon only reads: it never writes into them, except that\n");
    out.push_str("  # `canon gate task` flips a task's checkbox in their tasks.md.\n");
    out.push_str("  # Their briefs and approvals live in .canon/changes/<change>/brief.md.\n");
    out.push_str("  #   - dialect: openspec      # openspec/changes/<change>/proposal.md + tasks.md\n");
    out.push_str("  #     root: .\n");
    out.push_str("  #   - dialect: superpowers   # writing-plans documents\n");
    out.push_str("  #     root: docs/superpowers/plans\n");
    out.push_str("  sources: []\n");
    out.push_str("write:\n");
    out.push_str("  # Staged writes (`canon evidence add`, `canon finding add|close`,\n");
    out.push_str("  # `canon divergence stage`) promote in the same run. `manual` keeps\n");
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

/// Ensures every [`GITIGNORE_LINES`] entry is present in
/// `<repo>/.gitignore` -- appends each missing one under its comment
/// (creating the file if absent), so a `.gitignore` a prior `canon init`
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
    let mut file = fs::OpenOptions::new().create(true).append(true).open(&path)?;
    if !existing.is_empty() && !existing.ends_with('\n') {
        file.write_all(b"\n")?;
    }
    for (comment, line) in &missing {
        file.write_all(format!("{comment}\n{line}\n").as_bytes())?;
    }
    Ok(missing.into_iter().map(|(_, line)| line).collect())
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
and read the canon skill (`canon skills install`) before you change a
feature.

The loop:

1. Brief: for a new subject or a scope change, record the request and your assumptions in the change's `.canon/changes/<slug>/brief.md`, then stop and report the brief: build and attest nothing for that scope until the human answers. The approval is never assumed, even when the human said they cannot answer until you report; the brief is that report. One human answer approves the brief and the policy. Record it at once: an `Approved: <who> <when> — \"<their words>\"` line in that `brief.md`, and `canon policy approve --by <the human> --note \"<their words>\"`. Never approve without a human answer.
2. Subject: `canon subject new`, then `canon change new` for the change; it writes `.canon/changes/<slug>/brief.md`, which holds the brief. A change from an external plan (an openspec `proposal.md`) gets its brief there too: a `brief.md` whose `Source: <path>` line points at the plan; canon never edits the plan.
3. Scenarios: `canon scenario new`, failure paths included (`--case failure`).
4. Units: split the work; one actor id and session per unit.
5. Implement: tests named by scenario id.
6. Evidence: `canon evidence add` (it promotes itself under `write: { promote: auto }`; only under `write.promote: manual` run `canon gate promote` after it).
7. Independent review: a different session records `canon review add` and `canon finding add`, on every change.
8. Transition: `canon subject status`, with `canon gate check` clean; move to `shipped` yourself once its guard passes, unless the brief says the human ships. The ship guard and `canon evidence add` refuse while the brief is unapproved; they are backstops, the stop is at step 1.

Inside an approved scope, never stop for a human: fixes, follow-ups and review rounds stay in the approved change, as tasks in its `tasks.md`, never `canon change new` (a new change is a new scope), and go straight to scenario, implement, evidence and review. When no human can answer one of the brief's questions, write your assumption into the brief and proceed on it; that covers the brief's questions and work inside an approved scope, never the approval. `.canon/policy.yaml` is what the gate grades against.
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

/// Installs the canon skill for the chosen harnesses through
/// [`skills::install_canonical`] (which follows the `skills.providers`
/// just written), and prints what happened. `Err` carries the exit code.
fn install_chosen_skills(repo: &Path, canon_yaml_path: &Path, chosen: &InitSkills) -> Result<(), i32> {
    if chosen.providers.is_empty() {
        if chosen.recorded {
            println!("canon init: skills.providers is [] — no canon skill installed; change it with `canon skills install --providers=<list>`");
        } else {
            println!("canon init: no harness detected (no .claude, .codex, .omp or .pi directory), so no canon skill is installed; {}", skills::CHOOSE_PROVIDERS);
        }
        return Ok(());
    }
    let source = skills::resolve_source(None);
    match skills::install_canonical(&source, repo, None) {
        Ok(report) => {
            let dirs = report
                .providers
                .iter()
                .map(|provider| skills::provider_bundle_root(repo, *provider).display().to_string())
                .collect::<Vec<_>>()
                .join(", ");
            println!("canon init: installed the canon skill for {} ({dirs})", skills::provider_names(&report.providers));
            Ok(())
        }
        Err(e) => {
            eprintln!(
                "canon init: wrote `skills: {{ providers: [{}] }}` to {} but did not install the canon skill: {e}; once the skill source is available, run `canon skills install` (set {} or pass --source <dir>; it follows skills.providers)",
                chosen.providers.iter().map(|p| p.as_str()).collect::<Vec<_>>().join(", "),
                canon_yaml_path.display(),
                skills::CANONICAL_SOURCE_ENV
            );
            Err(2)
        }
    }
}

/// Write `merged` to `AGENTS.md` unless it already holds exactly those
/// bytes, and print what happened.
fn write_agents_md(path: &Path, existing: Option<&str>, merged: &str) -> std::io::Result<()> {
    if existing == Some(merged) {
        println!("canon init: the canon block in {} is already current", path.display());
        return Ok(());
    }
    fs::write(path, merged)?;
    let verb = if existing.is_some() { "refreshed" } else { "wrote" };
    println!("canon init: {verb} the canon block in {}", path.display());
    Ok(())
}

/// Write the starter policy at `level` (0.14 D2, 0.16 D3) unless
/// `.canon/policy.yaml` exists, which is left byte-for-byte alone. Every
/// level parses with zero diagnostics through
/// `canon_gate::PolicyResolution::resolve` (pinned by
/// [`crate::policy_level`]'s tests).
fn scaffold_policy(repo: &Path, level: PolicyLevel) -> std::io::Result<()> {
    let path = repo.join(paths::POLICY_FILE);
    fs::create_dir_all(repo.join(paths::CANON_DIR))?;
    match fs::OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(mut file) => {
            file.write_all(starter_policy(level).as_bytes())?;
            println!(
                "canon init: wrote {} — a human reviews it and approves it with `canon policy approve --by <who>`: the gate grades the work against it, and `canon status` and `canon gate check` say so until it is approved",
                path.display()
            );
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            println!("canon init: {} already exists — left unchanged", path.display());
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
pub fn run_init(repo: &Path, options: InitOptions) -> i32 {
    if let Err(e) = fs::create_dir_all(repo) {
        eprintln!("canon init: failed to create `{}`: {e}", repo.display());
        return 2;
    }
    let agents_path = repo.join("AGENTS.md");
    let agents = if options.agents_md {
        let existing = match fs::read_to_string(&agents_path) {
            Ok(text) => Some(text),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => None,
            Err(e) => {
                eprintln!("canon init: failed to read `{}`: {e}", agents_path.display());
                return 2;
            }
        };
        match merge_agents_md(existing.as_deref()) {
            Ok(merged) => Some((existing, merged)),
            Err(e) => {
                eprintln!("canon init: refused — {e}");
                return 2;
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
                eprintln!("canon init: refused — {e}");
                return 2;
            }
        }
    };
    match fs::OpenOptions::new().write(true).create_new(true).open(&canon_yaml_path) {
        Ok(mut file) => {
            let config = skeleton_yaml() + &skills_section(skills.as_ref());
            if let Err(e) = file.write_all(config.as_bytes()) {
                eprintln!("canon init: failed to write `{}`: {e}", canon_yaml_path.display());
                return 2;
            }
            println!("canon init: wrote {}", canon_yaml_path.display());
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            let missing = missing_gitignore_lines(repo);
            if agents.is_none() && missing.is_empty() {
                eprintln!("canon init: refused — `{}` already exists; never overwriting an existing config", canon_yaml_path.display());
                return 2;
            }
            let refreshed = if agents.is_some() { "the AGENTS.md block and the canon .gitignore lines are refreshed" } else { "the canon .gitignore lines are refreshed" };
            println!("canon init: {} already exists — left unchanged; {refreshed}", canon_yaml_path.display());
            if matches!(options.skills, SkillsChoice::Providers(_)) {
                println!("canon init: --providers is not applied to an existing canon.yaml; run `canon skills install --providers=<list>`, which records the list in skills.providers");
            }
            match scaffold_gitignore(repo) {
                Ok(added) if !added.is_empty() => println!("canon init: added to .gitignore: {}", added.join(", ")),
                Ok(_) => {}
                Err(e) => {
                    eprintln!("canon init: failed to update `.gitignore`: {e}");
                    return 2;
                }
            }
            if let Some((existing, merged)) = agents {
                if let Err(e) = write_agents_md(&agents_path, existing.as_deref(), &merged) {
                    eprintln!("canon init: failed to write `{}`: {e}", agents_path.display());
                    return 2;
                }
            }
            return 0;
        }
        Err(e) => {
            eprintln!("canon init: failed to create `{}`: {e}", canon_yaml_path.display());
            return 2;
        }
    }

    if let Err(e) = scaffold_gitignore(repo) {
        eprintln!("canon init: wrote `{}` but failed to update `.gitignore`: {e}", canon_yaml_path.display());
        return 2;
    }
    if let Some(level) = options.policy {
        if let Err(e) = scaffold_policy(repo, level) {
            eprintln!("canon init: failed to write `{}`: {e}", repo.join(paths::POLICY_FILE).display());
            return 2;
        }
    }
    if let Some((existing, merged)) = agents {
        if let Err(e) = write_agents_md(&agents_path, existing.as_deref(), &merged) {
            eprintln!("canon init: failed to write `{}`: {e}", agents_path.display());
            return 2;
        }
    }
    if let Some(chosen) = &skills {
        if let Err(code) = install_chosen_skills(repo, &canon_yaml_path, chosen) {
            return code;
        }
    }
    0
}

/// `canon init --check-config [--repo <dir>]` (task 4.2). Returns the
/// process exit code: `2` when `<repo>/canon.yaml` is missing (fails
/// loud, distinct from any content report), `0` when every PRESENT
/// section parses clean under its own existing loader, `1` when at
/// least one present section fails -- printing one PASS/FAIL/"not
/// configured" line per section regardless (never stopping at the
/// first failure, design D7).
pub fn run_check_config(repo: &Path) -> i32 {
    let canon_yaml_path = repo.join("canon.yaml");
    let Ok(text) = fs::read_to_string(&canon_yaml_path) else {
        eprintln!("canon init --check-config: refused — `{}` does not exist; run `canon init` first", canon_yaml_path.display());
        return 2;
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

    print!("{report}");
    if all_ok { 0 } else { 1 }
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
# `canon policy approve --by <who>`: the agent doing the work should not
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
  # review (`canon review add`) by an actor other than its evidence
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
