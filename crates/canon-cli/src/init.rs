//! `canon init [--repo <dir>] [--no-agents-md] [--no-policy]` + `canon
//! init --check-config [--repo <dir>]` (s19 `canon-init-scaffold` spec,
//! 0.14 D2): scaffolds a fresh, WORKING `canon.yaml` skeleton (design
//! D8/D9) at `<repo>/canon.yaml`, plus what a repo needs to start the
//! working loop: a starter `.canon/policy.yaml` a human approves, the
//! `openspec/changes/` home the default plans source points at, and a
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
use std::io::Write as _;
use std::path::Path;

use canon_model::envelope::RecordKind;
use canon_model::paths;
use canon_store::policy::{BackendConfig, TierPolicy};

/// Kinds routed to `hot` by [`skeleton_yaml`] (s32 `sqlite-hot-backend`):
/// the same hot-class set the tiered-storage docs/this repo's own
/// `canon.yaml` already use (task/handoff/session/run/event) --
/// `canon init` can now afford to route them there by default because
/// `hot`'s sqlite backend needs no operator-supplied credential
/// (unlike postgres's `dsn_env` or s3's `bucket_env`, which `init`
/// still can't guess -- `cold`-class kinds stay on `local`). Every
/// OTHER kind (`RecordKind::ALL` minus this set) routes to `local`.
const HOT_KINDS: [RecordKind; 5] = [RecordKind::Task, RecordKind::Handoff, RecordKind::Session, RecordKind::Run, RecordKind::Event];

/// The line [`scaffold_gitignore`] ensures is present in
/// `<repo>/.gitignore`: one glob covering the sqlite hot tier's db
/// file AND its WAL/SHM sidecars (`.canon/hot.db-wal`/`.canon/hot.db-shm`
/// -- sqlite's own WAL-journal-mode naming convention), since all
/// three share the `.canon/hot.db` prefix.
const GITIGNORE_LINE: &str = paths::HOT_DB_GITIGNORE;

/// D8/D9's skeleton `canon.yaml` body: every one of `RecordKind::ALL`'s
/// fourteen wire strings (s36: `subject` is the reviewed 13th kind;
/// s43: `finding` the 14th)
/// routed to either `local` (git-backed) or `hot`
/// (sqlite-backed, [`HOT_KINDS`]) -- the two zero-env-var rungs (s32
/// `sqlite-hot-backend`: sqlite needs no operator-supplied credential,
/// unlike postgres/s3) -- commented `tiers.hot` (postgres swap) /
/// `tiers.cold` stanzas documenting the scale-up path, one working
/// `specs.roots[]` entry (D9: a present `specs:` section requires at
/// least one root, so this ships real rather than empty), and one
/// `openspec` plans source rooted at the repo (0.14 D2), so the
/// changes `canon change new` scaffolds under [`CHANGES_DIR`] are
/// imported by `canon ingest plans` and flipped by `canon gate task`
/// with no hand edit.
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
    out.push_str("  roots:\n");
    out.push_str("    - id: root\n");
    out.push_str("      root: specs\n");
    out.push_str("plans:\n");
    out.push_str("  # Where changes and their tasks live: `canon change new` scaffolds\n");
    out.push_str("  # openspec/changes/<slug>/, `canon ingest plans` imports it.\n");
    out.push_str("  sources:\n");
    out.push_str("    - dialect: openspec\n");
    out.push_str("      root: .\n");
    out
}

/// Ensures [`GITIGNORE_LINE`] is present in `<repo>/.gitignore` --
/// appends it (creating the file if absent) UNLESS it is already
/// there, so a `.gitignore` a prior `canon init` (or the repo's own
/// `git init`) already wrote is never duplicated. `canon init` itself
/// stays a fresh-repo bootstrap (`run_init`'s `canon.yaml` refuses to
/// overwrite), but `.gitignore` commonly PRE-EXISTS a `canon init`
/// invocation (e.g. a `git init`-then-`canon init` sequence), so this
/// appends rather than mirroring `canon.yaml`'s create-fails-if-exists
/// refusal.
fn scaffold_gitignore(repo: &Path) -> std::io::Result<()> {
    let path = repo.join(".gitignore");
    let existing = fs::read_to_string(&path).unwrap_or_default();
    if existing.lines().any(|line| line.trim() == GITIGNORE_LINE) {
        return Ok(());
    }
    let mut file = fs::OpenOptions::new().create(true).append(true).open(&path)?;
    if !existing.is_empty() && !existing.ends_with('\n') {
        file.write_all(b"\n")?;
    }
    file.write_all(format!("# canon's sqlite hot tier (s32 sqlite-hot-backend) -- db file + WAL/SHM sidecars.\n{GITIGNORE_LINE}\n").as_bytes())?;
    Ok(())
}

/// The repo-relative directory the default `openspec` plans source
/// ([`skeleton_yaml`]) reads changes from. `init` creates it (with a
/// `.gitkeep`, so a clone keeps it): without it the openspec adapter
/// would fall back to treating the repo root itself as the changes dir
/// and report every top-level directory as a malformed change.
pub(crate) const CHANGES_DIR: &str = "openspec/changes";

/// The starter `.canon/policy.yaml` (0.14 D2). It turns `spec_coverage`
/// on so the gate reports unevidenced scenarios, golden-path-only
/// surfaces and unreviewed work from the first scenario on, instead of
/// passing on zero evidence. It parses with zero diagnostics through
/// `canon_gate::PolicyResolution::resolve` (pinned by a unit test
/// below).
const STARTER_POLICY_YAML: &str = "\
# .canon/policy.yaml -- scaffolded by `canon init`.
#
# This file is what `canon gate check` grades the work against. A human
# should review and approve it, and every later change to it: the agent
# doing the work should not write the rules it is graded by.
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

1. Brief: record the request and your assumptions; a human approves it.
2. Subject: `canon subject new`, then `canon change new` for the change.
3. Scenarios: `canon scenario new`, failure paths included (`--case failure`).
4. Units: split the work; one actor id and session per unit.
5. Implement: tests named by scenario id.
6. Evidence: `canon evidence add`, then `canon gate promote`.
7. Independent review: a different session records `canon review add` and `canon finding add`.
8. Transition: `canon subject status`, with `canon gate check` clean.

`.canon/policy.yaml` is what the gate grades against; a human approves it.
<!-- canon:end -->";

/// A fence opener/closer line: up to three spaces, then three or more
/// backticks or tildes (CommonMark). Returns the fence character, its
/// run length, and whether only whitespace follows the run (a closing
/// fence may carry nothing else).
fn fence_of(line: &str) -> Option<(char, usize, bool)> {
    let indent = line.len() - line.trim_start_matches(' ').len();
    if indent > 3 {
        return None;
    }
    let rest = &line[indent..];
    let ch = rest.chars().next().filter(|c| *c == '`' || *c == '~')?;
    let run = rest.len() - rest.trim_start_matches(ch).len();
    (run >= 3).then(|| (ch, run, rest[run..].trim().is_empty()))
}

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
        match (open_fence, fence_of(line)) {
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
#[derive(Debug, Clone, Copy)]
pub struct InitOptions {
    /// Write or refresh the canon block in `AGENTS.md`.
    pub agents_md: bool,
    /// Write the starter `.canon/policy.yaml`.
    pub policy: bool,
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

/// Write the starter policy unless `.canon/policy.yaml` exists, which
/// is left byte-for-byte alone.
fn scaffold_policy(repo: &Path) -> std::io::Result<()> {
    let path = repo.join(paths::POLICY_FILE);
    fs::create_dir_all(repo.join(paths::CANON_DIR))?;
    match fs::OpenOptions::new().write(true).create_new(true).open(&path) {
        Ok(mut file) => {
            file.write_all(STARTER_POLICY_YAML.as_bytes())?;
            println!("canon init: wrote {} — a human should review and approve it: the gate grades the work against it", path.display());
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            println!("canon init: {} already exists — left unchanged", path.display());
        }
        Err(e) => return Err(e),
    }
    Ok(())
}

/// Create [`CHANGES_DIR`] with a `.gitkeep` when it is absent or empty.
fn scaffold_changes_dir(repo: &Path) -> std::io::Result<()> {
    let dir = repo.join(CHANGES_DIR);
    fs::create_dir_all(&dir)?;
    if fs::read_dir(&dir)?.next().is_none() {
        fs::write(dir.join(".gitkeep"), "")?;
        println!("canon init: created {}", dir.display());
    }
    Ok(())
}

/// `canon init [--repo <dir>] [--no-agents-md] [--no-policy]` (task 4.1,
/// 0.14 D2). Returns the process exit code.
///
/// - A fresh repo gets `canon.yaml`, the `.gitignore` line, the
///   `openspec/changes/` plans home, the starter policy (unless
///   `--no-policy`) and the `AGENTS.md` block (unless `--no-agents-md`):
///   exit `0`.
/// - A repo whose `canon.yaml` exists keeps it byte-identical
///   (`create_new` is the atomic refusal) and only refreshes the
///   `AGENTS.md` block: exit `0`. With `--no-agents-md` there is nothing
///   left to do, so that is the pre-0.14 refusal, exit `2`.
/// - Unbalanced `AGENTS.md` markers refuse (exit `2`) before anything is
///   written.
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
    match fs::OpenOptions::new().write(true).create_new(true).open(&canon_yaml_path) {
        Ok(mut file) => {
            if let Err(e) = file.write_all(skeleton_yaml().as_bytes()) {
                eprintln!("canon init: failed to write `{}`: {e}", canon_yaml_path.display());
                return 2;
            }
            println!("canon init: wrote {}", canon_yaml_path.display());
        }
        Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {
            let Some((existing, merged)) = agents else {
                eprintln!("canon init: refused — `{}` already exists; never overwriting an existing config", canon_yaml_path.display());
                return 2;
            };
            println!("canon init: {} already exists — left unchanged; only the AGENTS.md block is refreshed", canon_yaml_path.display());
            if let Err(e) = write_agents_md(&agents_path, existing.as_deref(), &merged) {
                eprintln!("canon init: failed to write `{}`: {e}", agents_path.display());
                return 2;
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
    if let Err(e) = scaffold_changes_dir(repo) {
        eprintln!("canon init: failed to create `{}`: {e}", repo.join(CHANGES_DIR).display());
        return 2;
    }
    if options.policy {
        if let Err(e) = scaffold_policy(repo) {
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
    println!("canon init: next: `canon skills install` to install authoring guidance");
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

    print!("{report}");
    if all_ok { 0 } else { 1 }
}

#[cfg(test)]
mod tests {
    use super::*;

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
    }

    #[test]
    fn scaffold_gitignore_creates_a_fresh_file_with_the_hot_db_glob() {
        let dir = tempfile::tempdir().unwrap();
        scaffold_gitignore(dir.path()).unwrap();
        let text = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(text.lines().any(|line| line.trim() == GITIGNORE_LINE), "{text}");
    }

    #[test]
    fn scaffold_gitignore_appends_to_an_existing_file_without_disturbing_it() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join(".gitignore"), "target/\n").unwrap();
        scaffold_gitignore(dir.path()).unwrap();
        let text = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert!(text.contains("target/\n"), "{text}");
        assert!(text.lines().any(|line| line.trim() == GITIGNORE_LINE), "{text}");
    }

    #[test]
    fn scaffold_gitignore_is_idempotent() {
        let dir = tempfile::tempdir().unwrap();
        scaffold_gitignore(dir.path()).unwrap();
        scaffold_gitignore(dir.path()).unwrap();
        let text = std::fs::read_to_string(dir.path().join(".gitignore")).unwrap();
        assert_eq!(text.matches(GITIGNORE_LINE).count(), 1, "the line must never be duplicated across repeated scaffolds: {text}");
    }

    #[test]
    fn starter_policy_resolves_with_zero_diagnostics_and_turns_spec_coverage_on() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".canon")).unwrap();
        std::fs::write(dir.path().join(paths::POLICY_FILE), STARTER_POLICY_YAML).unwrap();
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
