//! What a 0.27-era install leaves in a repo, and how `canon migrate` brings it
//! to 0.28 (D9): hook entries whose command is a spelling 0.28 removed, canon-owned
//! scripts that call removed spellings, and the full skill projection the stub
//! replaces.
//!
//! One detection feeds two consumers. [`announce`] is the cheap check
//! `status`, `gate check` and `context` run: when it finds anything it adds a
//! warning and puts `canon migrate --json` first in `meta.next`. [`plan`] is the
//! migration's own plan: the `effects[]` it lists, and [`Plan::apply`] writes them
//! after [`backup`] copied every file about to change.
//!
//! # Detection rule
//! A repo holds 0.27-era state when any of these holds:
//! - a hook command in `.claude/settings.json` or `.codex/hooks.json` starts with
//!   `canon` and a command spelling [`crate::removed_028::ROWS`] lists as removed
//!   whose replacement is a bare command (`canon gate task` becomes `canon task
//!   done`), see [`rewrite_command`];
//! - the canon-owned pre-commit script (`.canon/scripts/canon-gate-pre-commit.sh`),
//!   or a retrieve hook script a hook command names, differs from the one this
//!   canon ships and calls a removed spelling;
//! - `AGENTS.md` holds the canon block (one begin and one end marker) and the block differs from
//!   the one this canon's `init` writes; user text outside the markers is never touched;
//! - the skill install lock records a canon older than 0.28 (or none), or lists a
//!   `reference/**` file ([`crate::skills::has_full_projection`]).

use std::fs;
use std::path::{Path, PathBuf};

use serde_json::Value;

use crate::removed_028::{Kind, ROWS};
use crate::safety::Effect;
use crate::skills;
use crate::status::{NextStep, StepActor};

/// The settings files `canon gate install-hooks` merges its entry into.
pub const HOOK_FILES: [&str; 2] = [".claude/settings.json", ".codex/hooks.json"];

/// The script a retrieve hook command runs.
const RETRIEVE_SCRIPT: &str = "canon-retrieve-pre-dispatch.sh";

/// The embedded retrieve script (`scripts/canon-retrieve-pre-dispatch.sh`).
fn retrieve_script() -> &'static str {
    crate::guide::scripts().find(|(path, _)| path.ends_with(RETRIEVE_SCRIPT)).map(|(_, text)| text).expect("the build embeds the retrieve hook script")
}

/// Removed commands whose replacement depends on an argument (`review add` is `review scenario` or
/// `review change`, `subject status` is `subject show` or `subject move --to`): a hook command line is
/// never rewritten from them.
const NOT_A_RENAME: [&str; 2] = ["review add", "subject status"];

/// `command` with a removed command spelling replaced by its 0.28 spelling, or `None`
/// when it is not a `canon ...` command line that starts with one. Only the prefix
/// changes; every word after it is kept as written. The longest removed spelling wins.
pub fn rewrite_command(command: &str) -> Option<String> {
    let rest = command.strip_prefix("canon ")?;
    let (row, tail) = ROWS
        .iter()
        .filter(|row| row.kind == Kind::Command && !row.new.contains("--") && !NOT_A_RENAME.contains(&row.old))
        .filter_map(|row| rest.strip_prefix(row.old).filter(|tail| tail.is_empty() || tail.starts_with(char::is_whitespace)).map(|tail| (row, tail)))
        .max_by_key(|(row, _)| row.old.len())?;
    Some(format!("canon {}{tail}", row.new))
}

/// Whether a line of a script runs a removed spelling: `canon <old command>`, or a
/// `canon <command>` line that also carries a removed option of that command.
/// Comment lines never count.
fn line_uses_removed(line: &str) -> bool {
    let line = line.trim_start();
    if line.starts_with('#') {
        return false;
    }
    let word_end = |tail: &str| tail.is_empty() || !tail.starts_with(|c: char| c.is_ascii_alphanumeric() || c == '-' || c == '_');
    ROWS.iter().any(|row| match row.kind {
        Kind::Command => line.match_indices(&format!("canon {}", row.old)).any(|(at, hit)| word_end(&line[at + hit.len()..])),
        Kind::Option => {
            let words = || line.split(|c: char| c.is_whitespace() || c == '"' || c == '\'' || c == '=');
            row.scope.iter().any(|scope| line.contains(&format!("canon {scope}"))) && words().any(|word| word == row.old)
        }
        Kind::Bare => false,
    })
}

/// Whether `text` runs a removed canon spelling outside its comments.
pub fn uses_removed_spelling(text: &str) -> bool {
    text.lines().any(line_uses_removed)
}

/// Every `command` string of a settings document, in document order.
fn commands(value: &Value, out: &mut Vec<String>) {
    match value {
        Value::Object(map) => {
            for (key, member) in map {
                match (key.as_str(), member) {
                    ("command", Value::String(command)) => out.push(command.clone()),
                    _ => commands(member, out),
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|item| commands(item, out)),
        _ => {}
    }
}

/// `value` with every `command` string run through `f`; whether anything changed.
fn map_commands(value: &mut Value, f: &impl Fn(&str) -> Option<String>) -> bool {
    match value {
        Value::Object(map) => {
            let mut changed = false;
            for (key, member) in map.iter_mut() {
                match (key.as_str(), &mut *member) {
                    ("command", Value::String(command)) => {
                        if let Some(new) = f(command) {
                            *command = new;
                            changed = true;
                        }
                    }
                    _ => changed |= map_commands(member, f),
                }
            }
            changed
        }
        Value::Array(items) => items.iter_mut().fold(false, |changed, item| map_commands(item, f) | changed),
        _ => false,
    }
}

/// One settings file whose hook commands move to their 0.28 spelling.
#[derive(Debug, Clone)]
pub struct HookFile {
    /// Repo-relative path (`/`-separated).
    pub path: &'static str,
    /// The command pairs `(from, to)`, once each, in document order.
    pub edits: Vec<(String, String)>,
    /// The file as it will be written.
    pub text: String,
}

/// The settings files with a hook command to rewrite, and what they become.
pub fn hook_files(repo: &Path) -> Vec<HookFile> {
    HOOK_FILES.iter().filter_map(|path| hook_file(repo, path)).collect()
}

fn hook_file(repo: &Path, path: &'static str) -> Option<HookFile> {
    let original = fs::read_to_string(repo.join(path)).ok()?;
    let parsed: Value = serde_json::from_str(&original).ok()?;
    let mut found = Vec::new();
    commands(&parsed, &mut found);
    let mut edits: Vec<(String, String)> = Vec::new();
    for command in found {
        if let Some(new) = rewrite_command(&command) {
            if !edits.iter().any(|(from, _)| *from == command) {
                edits.push((command, new));
            }
        }
    }
    if edits.is_empty() {
        return None;
    }
    let mut expected = parsed;
    map_commands(&mut expected, &|command| rewrite_command(command));
    // Replace the string literals in place so the file keeps its layout and key order; a file
    // whose literals are spelled another way (escapes) is written back whole.
    let literal = |text: &str| serde_json::to_string(text).expect("a string serializes");
    let mut text = original;
    for (from, to) in &edits {
        text = text.replace(&literal(from), &literal(to));
    }
    if serde_json::from_str::<Value>(&text).ok().as_ref() != Some(&expected) {
        text = format!("{}\n", serde_json::to_string_pretty(&expected).expect("settings serialize"));
    }
    Some(HookFile { path, edits, text })
}

/// A canon-owned script that calls a removed spelling, and the bytes that replace it.
#[derive(Debug, Clone)]
pub struct ScriptFile {
    /// Repo-relative path (`/`-separated).
    pub path: String,
    pub text: &'static str,
}

/// The scripts to regenerate from the sources embedded in this canon: the pre-commit script and
/// each retrieve hook script a hook command names (inside the repo), when it differs and calls a
/// removed spelling.
pub fn script_files(repo: &Path) -> Vec<ScriptFile> {
    let mut out = Vec::new();
    let mut consider = |path: String, shipped: &'static str| {
        let Ok(current) = fs::read_to_string(repo.join(&path)) else { return };
        if current != shipped && uses_removed_spelling(&current) && !out.iter().any(|script: &ScriptFile| script.path == path) {
            out.push(ScriptFile { path, text: shipped });
        }
    };
    consider(canon_model::paths::PRE_COMMIT_SCRIPT.to_string(), canon_gate::PRE_COMMIT_SCRIPT);
    let mut found = Vec::new();
    for file in HOOK_FILES {
        if let Some(parsed) = fs::read_to_string(repo.join(file)).ok().and_then(|text| serde_json::from_str::<Value>(&text).ok()) {
            commands(&parsed, &mut found);
        }
    }
    let mut named: Vec<String> = found.iter().flat_map(|command| command.split_whitespace()).filter_map(retrieve_script_path).collect();
    named.sort();
    named.dedup();
    for path in named {
        consider(path, retrieve_script());
    }
    out
}

/// The repo-relative path of a retrieve hook script a command word names: a relative path, or one
/// under `$CLAUDE_PROJECT_DIR`; `None` for any other word.
fn retrieve_script_path(word: &str) -> Option<String> {
    let word = word.trim_matches(|c| c == '"' || c == '\'');
    let word = ["${CLAUDE_PROJECT_DIR}/", "$CLAUDE_PROJECT_DIR/"].iter().find_map(|prefix| word.strip_prefix(prefix)).unwrap_or(word);
    let word = word.strip_prefix("./").unwrap_or(word);
    let safe = word.ends_with(RETRIEVE_SCRIPT) && !word.starts_with('/') && !word.split('/').any(|part| part == ".." || part.is_empty());
    safe.then(|| word.to_string())
}

/// What the migration does about 0.27-era hooks, scripts and skill.
#[derive(Debug, Default)]
pub struct Plan {
    pub hooks: Vec<HookFile>,
    pub scripts: Vec<ScriptFile>,
    pub skill: Option<SkillStub>,
    /// `AGENTS.md` as it will be written: only its canon block changes.
    pub agents: Option<AgentsFile>,
}

/// The `AGENTS.md` whose canon block is out of date.
#[derive(Debug, Clone)]
pub struct AgentsFile {
    pub text: String,
}

/// The file `AGENTS.md` for `repo` rewritten with this canon's block, when it holds exactly one canon block
/// (one begin and one end marker) that differs; `None` without markers (nothing of canon's to refresh), with
/// an arrangement `init` would refuse, or when the block is current.
pub fn agents_file(repo: &Path) -> Option<AgentsFile> {
    let current = fs::read_to_string(repo.join("AGENTS.md")).ok()?;
    let (begins, ends) = crate::init::marker_lines(&current).ok()?;
    if begins.len() != 1 || ends.len() != 1 {
        return None;
    }
    let merged = crate::init::merge_agents_md(Some(&current)).ok()?;
    (merged != current).then_some(AgentsFile { text: merged })
}

/// The full skill projection and what replaces it: the files an install writes and prunes.
#[derive(Debug)]
pub struct SkillStub {
    /// Repo-relative files the stub install writes whose bytes differ (`rewrite` when present, `write` when not).
    pub rewritten: Vec<(String, bool)>,
    /// Repo-relative files the install removes (their bytes still match the lock).
    pub pruned: Vec<String>,
    /// Repo-relative files the install keeps because their bytes changed.
    pub kept: Vec<String>,
    /// Whether the install lock is rewritten.
    pub lock: bool,
}

impl Plan {
    pub fn is_empty(&self) -> bool {
        self.hooks.is_empty() && self.scripts.is_empty() && self.skill.is_none() && self.agents.is_none()
    }

    /// The effects, in a fixed order: hooks, scripts, the skill's files, the lock.
    pub fn effects(&self) -> Vec<Effect> {
        let mut out = Vec::new();
        for file in &self.hooks {
            let detail = file.edits.iter().map(|(from, to)| format!("hook command `{from}` becomes `{to}`")).collect::<Vec<_>>().join("; ");
            out.push(Effect::path("rewrite", file.path).detail(detail));
        }
        for script in &self.scripts {
            out.push(Effect::path("rewrite", script.path.clone()).detail("regenerated from the script this canon ships; it called a removed spelling"));
        }
        if self.agents.is_some() {
            out.push(Effect::path("rewrite", "AGENTS.md").detail("the canon block between its markers becomes the one this canon's `init` writes; text outside the markers is untouched"));
        }
        if let Some(skill) = &self.skill {
            for (path, existed) in &skill.rewritten {
                out.push(Effect::path(if *existed { "rewrite" } else { "write" }, path.clone()).detail("the stub that points to `canon skills show canon`"));
            }
            for path in &skill.pruned {
                out.push(Effect::path("remove", path.clone()).detail("the guide is built into the binary now; its bytes match the install lock"));
            }
            if skill.lock {
                out.push(Effect::path("rewrite", SKILLS_LOCK).detail("records this canon's version"));
            }
        }
        out
    }

    /// The files [`Plan::apply`] overwrites or removes, repo-relative: what [`backup`] copies.
    pub fn backed_up(&self) -> Vec<String> {
        let mut out: Vec<String> = self.hooks.iter().map(|file| file.path.to_string()).chain(self.scripts.iter().map(|script| script.path.clone())).collect();
        if self.agents.is_some() {
            out.push("AGENTS.md".to_string());
        }
        if let Some(skill) = &self.skill {
            out.extend(skill.rewritten.iter().filter(|(_, existed)| *existed).map(|(path, _)| path.clone()));
            out.extend(skill.pruned.iter().cloned());
            if skill.lock {
                out.push(SKILLS_LOCK.to_string());
            }
        }
        out
    }

    /// What the plan leaves alone, one note each: modified files of the old projection the install keeps.
    pub fn notes(&self) -> Vec<String> {
        self.skill.iter().flat_map(|skill| skill.kept.iter().map(|path| format!("{path} kept: its bytes differ from the install lock; remove it by hand if you do not need it"))).collect()
    }

    /// Writes the plan.
    pub fn apply(&self, repo: &Path) -> Result<(), String> {
        for file in &self.hooks {
            write_file(&repo.join(file.path), file.text.as_bytes())?;
        }
        for script in &self.scripts {
            let path = repo.join(&script.path);
            write_file(&path, script.text.as_bytes())?;
            #[cfg(unix)]
            {
                use std::os::unix::fs::PermissionsExt;
                fs::set_permissions(&path, fs::Permissions::from_mode(0o755)).map_err(|e| format!("could not make {} executable: {e}", script.path))?;
            }
        }
        if let Some(agents) = &self.agents {
            write_file(&repo.join("AGENTS.md"), agents.text.as_bytes())?;
        }
        if self.skill.is_some() {
            skills::install_canonical(skills::resolve_source(None).as_deref(), repo, None, false).map_err(|e| format!("could not install the skill stub: {e}"))?;
        }
        Ok(())
    }
}

/// The skill install lock, repo-relative.
const SKILLS_LOCK: &str = ".canon/skills/.install-lock.json";

fn write_file(path: &Path, bytes: &[u8]) -> Result<(), String> {
    let temp = path.with_extension("canon-migrate.tmp");
    fs::write(&temp, bytes).and_then(|()| fs::rename(&temp, path)).map_err(|e| format!("could not write {}: {e}", path.display()))
}

/// The repo-relative `/`-separated form of `path` under `repo`.
fn relative(repo: &Path, path: &Path) -> String {
    path.strip_prefix(repo).unwrap_or(path).components().map(|c| c.as_os_str().to_string_lossy().into_owned()).collect::<Vec<_>>().join("/")
}

/// The plan for `repo`: empty when nothing is 0.27-era.
pub fn plan(repo: &Path) -> Result<Plan, String> {
    let skill = if skills::has_full_projection(repo) {
        let preview = skills::install_canonical(skills::resolve_source(None).as_deref(), repo, None, true).map_err(|e| format!("could not plan the skill stub: {e}"))?;
        let mut rewritten: Vec<(String, bool)> = preview.rewritten.iter().map(|path| (relative(repo, path), path.exists())).collect();
        rewritten.sort();
        let mut pruned: Vec<String> = preview.pruned.iter().map(|orphan| relative(repo, &orphan.path)).collect();
        pruned.sort();
        let mut kept: Vec<String> = preview.kept.iter().map(|orphan| relative(repo, &orphan.path)).collect();
        kept.sort();
        (preview.changed || preview.lock_changed).then_some(SkillStub { rewritten, pruned, kept, lock: preview.lock_changed })
    } else {
        None
    };
    // A projected script the stub install rewrites is one effect, the skill's.
    let mut scripts = script_files(repo);
    if let Some(skill) = &skill {
        scripts.retain(|script| !skill.rewritten.iter().any(|(path, _)| *path == script.path));
    }
    Ok(Plan { hooks: hook_files(repo), scripts, skill, agents: agents_file(repo) })
}

/// Copies each of `paths` (repo-relative, regular files that exist) to
/// `<repo>/.canon/migrate-backup/<stamp>/<path>`, bytes unchanged. Returns the directory, or `None`
/// when there was nothing to copy.
pub fn backup(repo: &Path, paths: &[String]) -> Result<Option<PathBuf>, String> {
    let existing: Vec<&String> = paths.iter().filter(|path| fs::symlink_metadata(repo.join(path)).is_ok_and(|meta| meta.is_file())).collect();
    if existing.is_empty() {
        return Ok(None);
    }
    let stamp = chrono::Utc::now().format("%Y%m%dT%H%M%SZ").to_string();
    let base = repo.join(canon_model::paths::MIGRATE_BACKUP_DIR);
    // A second run within the same second must not overwrite the first run's copies.
    let mut dir = base.join(&stamp);
    let mut n = 1;
    while dir.exists() {
        dir = base.join(format!("{stamp}-{n}"));
        n += 1;
    }
    for path in existing {
        let to = dir.join(path);
        let bytes = fs::read(repo.join(path)).map_err(|e| format!("could not read {path} for its backup: {e}"))?;
        if let Some(parent) = to.parent() {
            fs::create_dir_all(parent).map_err(|e| format!("could not create {}: {e}", parent.display()))?;
        }
        fs::write(&to, bytes).map_err(|e| format!("could not write the backup {}: {e}", to.display()))?;
    }
    Ok(Some(dir))
}

/// What a cheap scan found, for the warning.
pub struct Detected {
    pub hooks: bool,
    pub scripts: bool,
    pub skill: bool,
    pub agents: bool,
}

impl Detected {
    fn what(&self) -> String {
        let mut parts = Vec::new();
        if self.hooks {
            parts.push("a hook command canon 0.28 removed (`canon gate task`, now `canon task done`)");
        }
        if self.scripts {
            parts.push("a canon script that calls a removed command spelling");
        }
        if self.agents {
            parts.push("an AGENTS.md canon block from an earlier canon (it may name removed commands; `canon migrate` refreshes it)");
        }
        if self.skill {
            parts.push("a full skill projection (0.28 installs a stub that points to `canon skills show canon`)");
        }
        parts.join("; ")
    }
}

/// Whether `repo` holds 0.27-era state (module doc, "Detection rule"). Reads two small JSON files, up to
/// three scripts and the skill lock.
pub fn detect(repo: &Path) -> Option<Detected> {
    let found = Detected { hooks: !hook_files(repo).is_empty(), scripts: !script_files(repo).is_empty(), skill: skills::has_full_projection(repo), agents: agents_file(repo).is_some() };
    (found.hooks || found.scripts || found.skill || found.agents).then_some(found)
}

/// Said whenever the full 0.27 skill projection is on disk: those files are the old guide and still name removed
/// commands, so an agent that reads them first copies a dead spelling.
const STALE_GUIDE: &str = "the canon skill files installed in this repo (.claude, .agents, .pi, .omp, .canon/skills) are the 0.27 guide and name commands 0.28 removed (`canon migrate --dry-run` among them): do not follow them; run `canon migrate --json` (a bare run only previews), then read `canon skills show canon`";

/// The `canon migrate --json` step that comes first in `meta.next`.
fn migrate_step(found: &Detected) -> NextStep {
    let stale = if found.skill { format!("; {STALE_GUIDE}") } else { String::new() };
    NextStep {
        command: "canon migrate --json".into(),
        why: format!("this repo holds 0.27-era canon state ({}); the preview lists what it rewrites and the exact command that applies it{stale}", found.what()),
        actor: StepActor::Agent,
        role: Some(crate::role::Role::Orchestrator),
    }
}

/// For `status`, `gate check` and `context`: when `repo` holds 0.27-era state, says so as a warning and
/// puts `canon migrate --json` first in `meta.next` ([`crate::json_out::prepend_next`]).
pub fn announce(repo: &Path) {
    if let Some(found) = detect(repo) {
        let stale = if found.skill { format!("; {STALE_GUIDE}") } else { "; run `canon migrate --json` to preview the upgrade".to_string() };
        crate::json_out::warn(format!("this repo holds 0.27-era canon state ({}){stale}", found.what()));
        crate::json_out::prepend_next(migrate_step(&found));
    }
}

/// For the `migrate` preview: the stale-guide warning when the 0.27 skill projection is still installed.
pub fn warn_stale_guide(repo: &Path) {
    if skills::has_full_projection(repo) {
        crate::json_out::warn(STALE_GUIDE.to_string());
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_removed_command_prefix_is_rewritten_and_its_arguments_kept() {
        assert_eq!(rewrite_command("canon gate task").as_deref(), Some("canon task done"));
        assert_eq!(rewrite_command("canon gate task my-change#3 --repo .").as_deref(), Some("canon task done my-change#3 --repo ."));
        assert_eq!(rewrite_command("canon gate check"), None);
        assert_eq!(rewrite_command("canon gate taskfoo"), None);
        assert_eq!(rewrite_command("other gate task"), None);
    }

    #[test]
    fn a_script_line_with_a_removed_spelling_is_found_and_comments_are_not() {
        assert!(uses_removed_spelling("REGIME=\"$(canon regime-key --role a --repo b)\"\n"));
        assert!(uses_removed_spelling("canon retrieve --role \"$ROLE\" --json\n"));
        assert!(!uses_removed_spelling("# canon regime-key is how it used to be built\ncanon retrieve --for-role \"$ROLE\"\n"));
        assert!(!uses_removed_spelling("canon gate check || true\n"));
    }

    #[test]
    fn a_retrieve_script_word_is_taken_only_inside_the_repo() {
        assert_eq!(retrieve_script_path("\"$CLAUDE_PROJECT_DIR/.claude/hooks/canon-retrieve-pre-dispatch.sh\"").as_deref(), Some(".claude/hooks/canon-retrieve-pre-dispatch.sh"));
        assert_eq!(retrieve_script_path("./hooks/canon-retrieve-pre-dispatch.sh").as_deref(), Some("hooks/canon-retrieve-pre-dispatch.sh"));
        assert_eq!(retrieve_script_path("/etc/canon-retrieve-pre-dispatch.sh"), None);
        assert_eq!(retrieve_script_path("../x/canon-retrieve-pre-dispatch.sh"), None);
    }
}
