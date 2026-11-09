//! Preview and commit for the six destructive leaves (0.28 D6; `atip::plan` since 0.35): `purge`, `artifacts move`,
//! `artifacts prune`, `migrate` (and `migrate --revert`), `ledger compact`, `tier age`.
//!
//! A bare run computes the plan and prints it: `effects[]`, the `planDigest` over them and what they were computed from,
//! `committed: false`, exit 0, nothing written (no audit line either). `--commit --plan-digest <PLAN_DIGEST>` computes the plan again
//! and acts only when its digest is the one given; a plan that changed since is `PLAN_STALE` (exit 2) and nothing is written.
//! The flags are [`SafetyArgs`](crate::cli::SafetyArgs), read as a [`Mode`].
//!
//! The plan, its digest, the gate, the preview and the commit call are the `atip` crate's ([`atip::plan`]); this module is canon's
//! side of them: the effect kinds canon uses (`delete`, `skip`, `move`, `rewrite`, `append`, ...), the `inputs` of each plan
//! ([`Reads`]) and how a handler reaches the crate's functions ([`Plan`]).
//!
//! **The digest** (spec 12.6) is `sha256:` and the lower-case hex SHA-256 of the RFC 8785 JSON of
//! `{"effects": [...], "inputs": {"arguments": {...}, "reads": {...}}}`:
//!
//! - `effects`: what the commit makes, in the order the handler lists them (a handler lists them in a deterministic order);
//! - `inputs.arguments`: the normalised arguments of the call (spec 11.7 step 6: an integer as a number, a repeated option as an
//!   array, the operands as `operands`), without the options that only say how to act on a plan: `--commit`, `--plan-digest`,
//!   `--json`, `--input`, canon's own `--repo` and `--as` (which pick where and who, and what is read there is in `reads`), and
//!   the relay options of `migrate`;
//! - `inputs.reads`: a digest of each state and file content the plan read ([`Reads`]), so that a change under the plan that
//!   leaves its effects alone (a file edited in place) is still a changed plan. What each command reads is listed in the
//!   CHANGELOG of 0.35.
//!
//! A handler builds its `Vec<Effect>` and its [`Reads`], wraps them in [`Plan::new`], asks [`Plan::gate`] whether to act, does the
//! work, and finishes with [`Plan::committed`] in the report.

use std::collections::BTreeSet;
use std::fs;
use std::io::Read as _;
use std::path::Path;

use atip::plan::{self as contract, Next, PlanDigest};
use atip::Given;
use serde_json::{Map, Value};
use sha2::{Digest, Sha256};

use crate::cli::SafetyArgs;
use crate::cli_error::{CliError, CliErrorExt as _};
use crate::json_out::Report;
use crate::next::After;
use crate::role::Role;
use crate::status::{NextStep, StepActor};

pub use atip::plan::{Effect, Mode, Preview};

/// Effects a human preview lists before it says how many more there are.
const PREVIEW_LINES: usize = 50;

/// The options of `migrate` that record who relays the human's answer to a plan that re-binds `Approved:` lines: they act on a
/// commit and are not part of the plan, so a preview without them and a commit with them are one plan.
const RELAY_OPTIONS: [&str; 4] = ["relayed-via", "actor", "approver", "note"];

impl SafetyArgs {
    /// The mode the flags select (clap guarantees `--commit` and `--plan-digest` come together).
    pub fn mode(&self) -> Mode {
        contract::mode(self.commit, self.plan_digest.as_deref()).unwrap_or(Mode::Preview)
    }
}

/// The digest of `bytes` as the plans write it: `sha256:` and 64 lower-case hex digits.
pub fn digest_of(bytes: &[u8]) -> String {
    format!("{}{}", contract::DIGEST_PREFIX, hex(&Sha256::digest(bytes)))
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

/// The digest of what is at `path`, whatever it is: a file's bytes (`sha256:…`), a directory's entries by name and content
/// (`dir:sha256:…`), a link's target (`link:sha256:…`), `absent` and `unreadable`. A plan that reads a path records this, so a
/// path that appears, changes or goes away is another plan. Never an error: a path that cannot be read is a state like another.
/// `repo` is the root the plan works in: the lock files and the backups `canon migrate` writes are not read as inputs of a plan
/// that runs while it holds the lock or writes them.
pub fn path_digest(repo: &Path, path: &Path) -> String {
    let meta = match fs::symlink_metadata(path) {
        Ok(meta) => meta,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => return "absent".to_string(),
        Err(_) => return "unreadable".to_string(),
    };
    if meta.file_type().is_symlink() {
        return fs::read_link(path).map_or_else(|_| "unreadable".to_string(), |target| format!("link:{}", digest_of(target.to_string_lossy().as_bytes())));
    }
    if meta.is_dir() {
        let Ok(entries) = fs::read_dir(path) else { return "unreadable".to_string() };
        let mut names: Vec<_> = entries.filter_map(Result::ok).map(|entry| entry.file_name()).collect();
        names.sort();
        let mut hasher = Sha256::new();
        for name in names {
            let child = path.join(&name);
            if untracked(repo, &child) {
                continue;
            }
            hasher.update(name.to_string_lossy().as_bytes());
            hasher.update([0]);
            hasher.update(path_digest(repo, &child).as_bytes());
            hasher.update(*b"\n");
        }
        return format!("dir:{}{}", contract::DIGEST_PREFIX, hex(&hasher.finalize()));
    }
    let Ok(mut file) = fs::File::open(path) else { return "unreadable".to_string() };
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        match file.read(&mut buffer) {
            Ok(0) => break,
            Ok(read) => hasher.update(&buffer[..read]),
            Err(_) => return "unreadable".to_string(),
        }
    }
    format!("{}{}", contract::DIGEST_PREFIX, hex(&hasher.finalize()))
}

/// Paths a plan does not read: the three lock files, which a commit creates by holding them, and what `canon migrate` backs up
/// into, which is its output.
fn untracked(repo: &Path, path: &Path) -> bool {
    use canon_model::paths::{ARTIFACT_STORE_LOCK_FILE, MIGRATE_BACKUP_DIR, REPO_WRITE_LOCK_FILE, SCENARIO_INDEX_LOCK_FILE};
    [ARTIFACT_STORE_LOCK_FILE, MIGRATE_BACKUP_DIR, REPO_WRITE_LOCK_FILE, SCENARIO_INDEX_LOCK_FILE].iter().any(|relative| path == repo.join(relative.trim_end_matches('/')))
}

/// What a plan read: the `inputs.reads` of its digest. Each entry is a name and a digest of the state or file content read.
#[derive(Debug, Clone, Default)]
pub struct Reads(Map<String, Value>);

impl Reads {
    pub fn new() -> Self {
        Self::default()
    }

    /// `name` read as `digest` (a [`digest_of`] or [`path_digest`] text).
    #[must_use]
    pub fn digest(mut self, name: &str, digest: impl Into<String>) -> Self {
        self.0.insert(name.to_string(), Value::String(digest.into()));
        self
    }

    /// `name` read as the bytes `bytes`.
    #[must_use]
    pub fn bytes(self, name: &str, bytes: &[u8]) -> Self {
        self.digest(name, digest_of(bytes))
    }

    /// `name` read as the set of `lines` (each a record key, a name): their digest does not depend on the order.
    #[must_use]
    pub fn set<I, S>(self, name: &str, lines: I) -> Self
    where
        I: IntoIterator<Item = S>,
        S: AsRef<str>,
    {
        let sorted: BTreeSet<String> = lines.into_iter().map(|line| line.as_ref().to_string()).collect();
        let mut text = String::new();
        for line in &sorted {
            text.push_str(line);
            text.push('\n');
        }
        self.digest(name, digest_of(text.as_bytes()))
    }

    /// `relative`, a path under `repo`, read as [`path_digest`] says.
    #[must_use]
    pub fn path(self, repo: &Path, relative: &str) -> Self {
        let digest = path_digest(repo, &repo.join(relative));
        self.digest(&format!("path:{relative}"), digest)
    }

    /// Every path the `effects` name, read as [`path_digest`] says: the files a plan moves, rewrites and removes are what it read.
    #[must_use]
    pub fn effect_paths(mut self, repo: &Path, effects: &[Effect]) -> Self {
        let paths: BTreeSet<&str> = effects.iter().filter_map(|effect| effect.path.as_deref()).filter(|path| !untracked(repo, &repo.join(path.trim_end_matches('/')))).collect();
        for path in paths {
            self = self.path(repo, path);
        }
        self
    }

    fn into_map(self) -> Map<String, Value> {
        self.0
    }
}

/// The plan of one destructive run: its effects, what it read and the digest over both, with the call it was computed in.
#[derive(Debug, Clone)]
pub struct Plan {
    plan: contract::Plan,
    digest: PlanDigest,
    /// The command words of the leaf (`["ledger", "compact"]`).
    path: Vec<String>,
    /// The call's arguments, normalised, as the preview's commit call is written from them.
    given: Vec<Given>,
}

/// The leaf `leaf` (`ledger compact`) of this run: its words and arguments as `main` recorded them. A handler run without `main`
/// (a test that calls it directly) has the words and no arguments.
fn call_of(leaf: &str) -> (Vec<String>, Vec<Given>) {
    match crate::paging::call() {
        Some((path, given)) => (path.to_vec(), given.to_vec()),
        None => (leaf.split_whitespace().map(str::to_string).collect(), Vec::new()),
    }
}

/// The preview call of this run as a display string, for the hint of a refusal made before a plan exists.
pub fn preview_command(leaf: &str) -> String {
    let (path, given) = call_of(leaf);
    let words: Vec<&str> = path.iter().map(String::as_str).collect();
    contract::preview_next("canon", &words, &given, "").command
}

/// The arguments of the call as the object `inputs.arguments` is: the arguments that make the plan, by option name.
fn arguments_of(leaf: &str, given: &[Given]) -> Map<String, Value> {
    // The flag-only options act on a call, never on a plan: `--commit`, `--plan-digest`, `--json`, `--input`, the paging options
    // and canon's own `--repo` and `--as` (`crate::input::flags_only`).
    given
        .iter()
        .filter(|argument| !argument.flag_only && !argument.is_absent())
        .filter(|argument| !crate::input::flags_only().contains(&argument.name.as_str()))
        .filter(|argument| !(leaf == "migrate" && RELAY_OPTIONS.contains(&argument.name.as_str())))
        .map(|argument| (argument.name.clone(), argument.value.clone()))
        .collect()
}

impl Plan {
    /// The plan of `leaf` (`purge`, `ledger compact`, `migrate`, …) with `effects` and what it `reads`, in the call of this run.
    /// A number a digest cannot hold is `INTERNAL` (no argument of these leaves is one).
    pub fn new(leaf: &str, effects: Vec<Effect>, reads: Reads) -> Result<Self, CliError> {
        let (path, given) = call_of(leaf);
        let mut inputs = Map::new();
        inputs.insert("arguments".to_string(), Value::Object(arguments_of(leaf, &given)));
        inputs.insert("reads".to_string(), Value::Object(reads.into_map()));
        let plan = contract::Plan::new(effects, inputs);
        let digest = plan.digest().map_err(|error| CliError::failed("INTERNAL", format!("canon could not compute the digest of the plan: {error}")).hint("report it with the command line you ran"))?;
        Ok(Self { plan, digest, path, given })
    }

    pub fn digest(&self) -> &str {
        self.digest.as_str()
    }

    /// The same plan whose preview and commit calls leave out the paging options (`--limit`, `--cursor`, `--all`): a listing of a
    /// preview is paged, and what it commits is the whole plan, whichever page was read. The digest never covered them.
    #[must_use]
    pub fn unpaged(mut self) -> Self {
        self.given.retain(|argument| !matches!(argument.name.as_str(), "limit" | "cursor" | "all"));
        self
    }

    pub fn effects(&self) -> &[Effect] {
        &self.plan.effects
    }

    /// The report of the plan as a preview (`committed: false`).
    pub fn preview(&self) -> Preview {
        Preview { plan_digest: self.digest().to_string(), effects: self.plan.effects.clone(), committed: false }
    }

    /// The same report, marked as acted on.
    pub fn committed(&self) -> Preview {
        self.preview().committed()
    }

    fn words(&self) -> Vec<&str> {
        self.path.iter().map(String::as_str).collect()
    }

    /// The preview call that goes with this one: the same arguments without `--commit` and `--plan-digest`.
    pub fn preview_call(&self, why: &str) -> Next {
        contract::preview_next("canon", &self.words(), &self.given, why)
    }

    /// The call that commits this plan (spec 12.5): the arguments of the preview, `--commit`, `--plan-digest=<digest>`, `--json`.
    pub fn commit_call(&self, why: &str) -> Next {
        contract::commit_next("canon", &self.words(), &self.given, &self.digest, why)
    }

    /// Whether the run acts: `Ok(false)` for a preview, `Ok(true)` when `mode` commits this very plan, `PLAN_STALE` (exit 2,
    /// nothing written) when it commits another, with the preview as the first `meta.next` entry and a hint that says to run it.
    ///
    /// A commit that passes is under the repository write lock `main` took before the plan was computed ([`crate::write_lock`]), so
    /// nothing changes the plan between this check and the effects. A debug build pauses here for the milliseconds of
    /// `CANON_TEST_PAUSE_AFTER_GATE_MS`: the tests' way to keep a commit inside that window while another writer tries to enter it.
    pub fn gate(&self, mode: &Mode) -> Result<bool, CliError> {
        let commit = self.check(mode)?;
        #[cfg(debug_assertions)]
        if commit {
            if let Some(millis) = std::env::var("CANON_TEST_PAUSE_AFTER_GATE_MS").ok().and_then(|text| text.parse::<u64>().ok()) {
                std::thread::sleep(std::time::Duration::from_millis(millis));
            }
        }
        Ok(commit)
    }

    fn check(&self, mode: &Mode) -> Result<bool, CliError> {
        contract::gate(mode, self.digest(), self.preview_call("the plan changed or the digest is not this command's; plan it again, review it, then commit the digest it prints")).map_err(|stale| {
            // The crate's refusal is canon's registered code: the row of `all_codes` has its meaning, exit and fix.
            debug_assert!(stale.error.code == "PLAN_STALE" && crate::cli_error::code_info("PLAN_STALE").is_some_and(|row| row.exit == stale.error.exit));
            for entry in stale.next {
                crate::json_out::prepend_next(NextStep { command: entry.command, why: entry.reason.unwrap_or_default(), actor: StepActor::Agent, role: Some(Role::Orchestrator) });
            }
            stale.error
        })
    }

    /// The step after a preview: the exact command that commits this plan.
    pub fn next_step(&self, why: &str) -> After {
        let call = self.commit_call(why);
        After::fixed(vec![NextStep { command: call.command, why: why.to_string(), actor: StepActor::Agent, role: Some(Role::Orchestrator) }])
    }

    /// The commit command as a human types it: the same as [`Plan::commit_call`] without `--json`.
    pub fn commit_command(&self) -> String {
        let call = self.commit_call("");
        let argv: Vec<String> = call.argv.into_iter().filter(|word| word != "--json").collect();
        Next::new("canon", argv, "").command
    }

    /// The human lines of a preview: each effect, then the commit command.
    pub fn preview_lines(&self) -> Vec<String> {
        let effects = &self.plan.effects;
        let mut lines: Vec<String> = effects.iter().take(PREVIEW_LINES).map(effect_line).collect();
        if effects.len() > PREVIEW_LINES {
            lines.push(format!("  ... and {} more (`--json` lists every effect)", effects.len() - PREVIEW_LINES));
        }
        lines.push(self.commit_hint());
        lines
    }

    /// The closing line of a human preview: the digest and the commit command.
    pub fn commit_hint(&self) -> String {
        format!("planDigest {}; nothing written. To act: {}", self.digest(), self.commit_command())
    }

    /// A preview's output: the report with the commit call as `meta.next[0]` (JSON), or the effects and that command as lines
    /// (human). `command` is the `data` command id (`purge`, `artifacts move`, ...).
    pub fn finish_preview<R: Report>(&self, command: &str, why: &str, json: bool, report: &R) {
        if json {
            crate::json_out::data_with_next(command, report, &self.next_step(why));
        } else {
            for line in self.preview_lines() {
                println!("{line}");
            }
        }
    }
}

impl Plan {
    /// [`Plan::finish_preview`] for a plan a commit would refuse (a tier it needs is not attached): the same report, but the step after it
    /// is the preview again with what to set (`blocked`), never the `--commit` command; the human closing line says the same.
    pub fn finish_blocked_preview<R: Report>(&self, command: &str, blocked: &str, json: bool, report: &R) {
        if json {
            let call = self.preview_call(blocked);
            crate::json_out::data_with_next(command, report, &After::fixed(vec![NextStep { command: call.command, why: blocked.to_string(), actor: StepActor::Agent, role: Some(Role::Orchestrator) }]));
        } else {
            let effects = &self.plan.effects;
            for effect in effects.iter().take(PREVIEW_LINES) {
                println!("{}", effect_line(effect));
            }
            if effects.len() > PREVIEW_LINES {
                println!("  ... and {} more (`--json` lists every effect)", effects.len() - PREVIEW_LINES);
            }
            println!("nothing written. Not committable yet: {blocked}");
        }
    }
}

/// One effect as a human line.
pub fn effect_line(effect: &Effect) -> String {
    let target = effect.path.as_deref().or(effect.record.as_deref()).unwrap_or("");
    match &effect.detail {
        Some(Value::String(detail)) => format!("  {} {target} ({detail})", effect.action),
        Some(detail) => format!("  {} {target} ({})", effect.action, atip::jcs::to_string(detail).unwrap_or_else(|_| detail.to_string())),
        None => format!("  {} {target}", effect.action),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn plan(effects: Vec<Effect>, reads: Reads) -> Plan {
        Plan::new("purge", effects, reads).unwrap()
    }

    #[test]
    fn digest_is_sha256_prefixed_and_follows_the_effects_their_order_and_the_reads() {
        let a = vec![Effect::path("delete", "a"), Effect::record("delete", "b")];
        let b = vec![Effect::record("delete", "b"), Effect::path("delete", "a")];
        let digest = plan(a.clone(), Reads::new()).digest().to_string();
        assert!(digest.strip_prefix("sha256:").is_some_and(|hex| hex.len() == 64 && hex.chars().all(|c| matches!(c, '0'..='9' | 'a'..='f'))), "{digest}");
        assert_eq!(digest, plan(a.clone(), Reads::new()).digest());
        assert_ne!(digest, plan(b, Reads::new()).digest(), "the order of the effects");
        assert_ne!(digest, plan(a[..1].to_vec(), Reads::new()).digest(), "the effects");
        assert_ne!(digest, plan(a.clone(), Reads::new().bytes("canon.yaml", b"routing: {}")).digest(), "what the plan read");
        assert_ne!(plan(a.clone(), Reads::new().bytes("x", b"1")).digest(), plan(a, Reads::new().bytes("x", b"2")).digest(), "what the plan read, changed");
    }

    #[test]
    fn a_stale_digest_is_plan_stale_and_the_right_one_commits() {
        let plan = plan(vec![Effect::path("delete", "a")], Reads::new());
        assert!(!plan.gate(&Mode::Preview).unwrap());
        assert!(plan.gate(&Mode::Commit(plan.digest().to_string())).unwrap());
        let error = plan.gate(&Mode::Commit("0".into())).unwrap_err();
        assert_eq!((error.code, error.exit), ("PLAN_STALE", 2));
        let unprefixed = plan.digest().trim_start_matches("sha256:").to_string();
        assert_eq!(plan.gate(&Mode::Commit(unprefixed)).unwrap_err().code, "PLAN_STALE", "the 0.34 form is another digest");
    }

    #[test]
    fn a_path_is_read_as_what_is_in_it_and_the_locks_are_not() {
        let dir = tempfile::TempDir::new().unwrap();
        let repo = dir.path();
        assert_eq!(path_digest(repo, &repo.join("missing")), "absent");
        fs::create_dir_all(repo.join("tree/sub")).unwrap();
        fs::write(repo.join("tree/a"), "1").unwrap();
        fs::write(repo.join("tree/sub/b"), "2").unwrap();
        let before = path_digest(repo, &repo.join("tree"));
        assert!(before.starts_with("dir:sha256:"), "{before}");
        assert_eq!(before, path_digest(repo, &repo.join("tree")));
        fs::write(repo.join("tree/sub/b"), "3").unwrap();
        assert_ne!(before, path_digest(repo, &repo.join("tree")), "a file edited in place");
        fs::create_dir_all(repo.join(".canon")).unwrap();
        let canon = path_digest(repo, &repo.join(".canon"));
        fs::write(repo.join(canon_model::paths::REPO_WRITE_LOCK_FILE), "").unwrap();
        fs::write(repo.join(canon_model::paths::SCENARIO_INDEX_LOCK_FILE), "").unwrap();
        assert_eq!(canon, path_digest(repo, &repo.join(".canon")), "the lock files are not state a plan reads");
    }

    #[test]
    fn the_arguments_leave_out_what_acts_on_a_plan_and_what_is_flag_only() {
        let given = vec![
            Given::option("kind", "run"),
            Given::option("before", "2020-01-01"),
            Given::switch("commit", true),
            Given::option("plan-digest", "sha256:x"),
            Given::switch("json", true),
            Given::option("repo", "/tmp/x").tool_flag_only(),
            Given::option("actor", "me"),
        ];
        let purge = arguments_of("purge", &given);
        assert_eq!(serde_json::Value::Object(purge), serde_json::json!({"actor": "me", "before": "2020-01-01", "kind": "run"}));
        let migrate = arguments_of("migrate", &given);
        assert!(!migrate.contains_key("actor"), "a relay option is not part of the plan of `migrate`");
    }
}
