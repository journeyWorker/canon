//! A 0.29 repo, seen by canon 0.30 (0.30 `ledger-naming`, F2).
//!
//! Record files named by the 0.29 rule (an `area=` level, a 12-hex digest
//! suffix, the `kind=evidence_record/` directory), the `evidence_record`
//! routing/aging key of `canon.yaml` and the `.gitignore` negation that names
//! that directory are what a 0.29 repo leaves. Canon 0.30 uses none of them for
//! a verdict; `canon migrate` moves them. Until it runs:
//!
//! - a command that reads ([`announce`], called with the 0.27 announcement by
//!   `status`, `gate check` and `context`) warns, with no config error, and
//!   puts `canon migrate --json` first in `meta.next`;
//! - a command that writes ([`refuse_write`], called before every command
//!   runs) is refused with the same hint and writes nothing, so a new-rule file
//!   never appears next to an old one.
//!
//! Detection reads directory entries and `canon.yaml` only, never a record.

use std::path::{Path, PathBuf};
use std::process::ExitCode;

use canon_store::partition::{is_old_stem, LEGACY_KIND_DIR};

use crate::cli_error::{CliError, CliErrorExt as _};
use crate::status::{NextStep, StepActor};

/// The leaves that write but touch no ledger record, so a 0.29 repo may run
/// them before it migrates: the migration itself, the setup and install
/// commands, the human's approvals and the commands that write only their own
/// files.
const WRITES_NO_RECORDS: &[&str] = &[
    "migrate",
    "init",
    "skills install",
    "gate install-hooks",
    "policy approve",
    "brief approve",
    "demo init",
    "demo review",
    "dashboard",
    "report build",
    "report snapshot",
    "context-pack add",
    "prompt add",
    "export",
    "task done",
    "artifacts move",
];

/// What a 0.29 repo holds.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Found {
    /// Record files named by the 0.29 rule, in the ledger, the archive and staging.
    pub files: usize,
    /// The first few of them, relative to the tier root they sit in.
    pub examples: Vec<String>,
    /// `canon.yaml` keys spelled the 0.29 way (`routing.evidence_record`).
    pub config_keys: Vec<String>,
    /// `.gitignore` lines naming `kind=evidence_record`.
    pub gitignore_lines: usize,
}

impl Found {
    fn what(&self) -> String {
        let mut parts = Vec::new();
        if self.files > 0 {
            let examples = self.examples.iter().map(|e| format!("`{e}`")).collect::<Vec<_>>().join(", ");
            parts.push(format!("{} record file(s) named by the 0.29 rule, e.g. {examples}", self.files));
        }
        if !self.config_keys.is_empty() {
            parts.push(format!("canon.yaml key(s) {} spelled the 0.29 way", self.config_keys.iter().map(|k| format!("`{k}`")).collect::<Vec<_>>().join(", ")));
        }
        if self.gitignore_lines > 0 {
            parts.push("a .gitignore line naming `kind=evidence_record`".to_string());
        }
        parts.join("; ")
    }

    /// The warning of a read command and the reason of a refused write.
    pub fn message(&self) -> String {
        format!(
            "this repo's ledger is laid out by canon 0.29 ({}); canon 0.30 does not use those files for any judgement. Hint: run `canon migrate --json` to preview the move (a bare run only previews; `--commit --plan-digest <digest>` applies it)",
            self.what()
        )
    }
}

/// The tier roots a 0.29 ledger can sit in: the ledger, its archive and the staging directories.
fn roots(repo: &Path) -> Vec<PathBuf> {
    let ledger = canon_gate::GateCtx::from_repo(repo).map_or_else(|_| repo.join(canon_model::paths::LEDGER_DIR), |ctx| ctx.ledger_root);
    let mut roots = vec![ledger.join("_staging"), ledger.join("_staging_divergence")];
    if let Some(archive) = canon_store::archive::archive_root_of(&ledger) {
        roots.push(archive);
    }
    roots.insert(0, ledger);
    roots
}

/// Every regular file below `dir`, symlinks not followed.
fn files_under(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        match entry.file_type() {
            Ok(t) if t.is_dir() => files_under(&entry.path(), out),
            Ok(t) if t.is_file() => out.push(entry.path()),
            _ => {}
        }
    }
}

/// Every record file under `root` named by the 0.29 rule: below an `area=`
/// level, with a `__<12 hex>` stem, or under `kind=evidence_record/`.
fn old_files(root: &Path) -> Vec<String> {
    let Ok(entries) = std::fs::read_dir(root) else { return Vec::new() };
    let mut found = Vec::new();
    for entry in entries.flatten() {
        let name = entry.file_name().to_string_lossy().into_owned();
        if !name.starts_with("kind=") || !entry.file_type().is_ok_and(|t| t.is_dir()) {
            continue;
        }
        let legacy_dir = name == LEGACY_KIND_DIR;
        let mut files = Vec::new();
        files_under(&entry.path(), &mut files);
        for file in files {
            let relative = file.strip_prefix(root).unwrap_or(&file);
            let stem = file.file_stem().and_then(|s| s.to_str()).unwrap_or_default();
            let under_area = relative.components().any(|c| c.as_os_str().to_string_lossy().starts_with("area="));
            if legacy_dir || under_area || (file.extension().is_some_and(|e| e == "json") && is_old_stem(stem)) {
                found.push(relative.to_string_lossy().replace('\\', "/"));
            }
        }
    }
    found.sort();
    found
}

/// The `canon.yaml` keys spelled the 0.29 way.
fn config_keys(repo: &Path) -> Vec<String> {
    let Ok(text) = std::fs::read_to_string(repo.join("canon.yaml")) else { return Vec::new() };
    match canon_store::policy::TierPolicy::from_yaml_at(&text, repo) {
        Ok(policy) => policy.legacy_kind_keys,
        // A config that does not parse is reported by the command that needs it; the textual scan finds the key anyway.
        Err(_) => text
            .lines()
            .filter(|line| line.trim_start().starts_with(&format!("{}:", canon_store::policy::LEGACY_EVIDENCE_KEY)))
            .map(|_| canon_store::policy::LEGACY_EVIDENCE_KEY.to_string())
            .collect(),
    }
}

/// What `repo` holds from canon 0.29, or `None` for a repo already on the 0.30 layout.
pub fn detect(repo: &Path) -> Option<Found> {
    let mut found = Found::default();
    for root in roots(repo) {
        let files = old_files(&root);
        found.files += files.len();
        let room = 3 - found.examples.len().min(3);
        found.examples.extend(files.into_iter().take(room));
    }
    found.config_keys = config_keys(repo);
    found.gitignore_lines = std::fs::read_to_string(repo.join(".gitignore")).map_or(0, |text| text.lines().filter(|line| line.contains(LEGACY_KIND_DIR)).count());
    (found.files > 0 || !found.config_keys.is_empty() || found.gitignore_lines > 0).then_some(found)
}

/// The `canon migrate --json` step that comes first in `meta.next`.
pub fn migrate_step(found: &Found) -> NextStep {
    NextStep {
        command: "canon migrate --json".into(),
        why: format!("this repo holds canon 0.29 state ({}); the preview lists what it moves and rewrites and the exact command that applies it", found.what()),
        actor: StepActor::Agent,
        role: Some(crate::role::Role::Orchestrator),
    }
}

/// For `status`, `gate check` and `context`: warns when `repo` is a 0.29 repo. With `lead`, also puts
/// `canon migrate --json` first in `meta.next` (the 0.27 announcement already does when it speaks).
pub fn announce(repo: &Path, lead: bool) {
    if let Some(found) = detect(repo) {
        crate::json_out::warn("OLD_LAYOUT_FOUND", found.message());
        if lead {
            crate::json_out::prepend_next(migrate_step(&found));
        }
    }
}

/// Whether the leaf `command` (`evidence add`) is refused in a 0.29 repo: it writes, and not only its own files.
pub fn refuses_in_old_layout(command: &str) -> bool {
    crate::schema::META.iter().find(|row| row.path == command).is_some_and(|row| row.effect != crate::schema::Effect::Read) && !WRITES_NO_RECORDS.contains(&command)
}

/// A command that writes records is refused in a 0.29 repo, with the hint of [`Found::message`]: the new-rule
/// file a write would make must never sit beside an old one.
pub fn refuse_write(command_tree: &clap::Command, parsed: &clap::ArgMatches) -> Option<ExitCode> {
    let mut names = Vec::new();
    let mut leaf = parsed;
    while let Some((name, sub)) = leaf.subcommand() {
        names.push(name);
        leaf = sub;
    }
    let command = names.join(" ");
    if !refuses_in_old_layout(&command) || !crate::run::entry::leaf_takes_repo(command_tree, &names) {
        return None;
    }
    let repo = leaf.get_one::<PathBuf>("repo");
    let root = crate::context::resolve_repo_root(repo.map_or(Path::new("."), PathBuf::as_path));
    if !root.join("canon.yaml").is_file() {
        return None;
    }
    let found = detect(&root)?;
    let json = leaf.try_get_one::<bool>("json").ok().flatten().copied().unwrap_or(false);
    let error = CliError::refused("OLD_LAYOUT", found.message())
        .hint("run `canon migrate --json` to preview the move, then `canon migrate --commit --plan-digest <digest>`; nothing was written");
    Some(ExitCode::from(error.emit(&format!("canon {command}"), json)))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn write(root: &Path, relative: &str, text: &str) {
        let path = root.join(relative);
        std::fs::create_dir_all(path.parent().unwrap()).unwrap();
        std::fs::write(path, text).unwrap();
    }

    /// ledger-naming.old-layout.01: only the files named by the 0.29 rule are found, in every tier root.
    #[test]
    fn only_the_files_named_by_the_old_rule_are_found() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "canon.yaml", "routing:\n  evidence: local\n");
        write(dir.path(), ".canon/ledger/kind=change/add-login_0123abcd.json", "{}");
        assert_eq!(detect(dir.path()), None, "a 0.30 file is not old");
        write(dir.path(), ".canon/ledger/kind=review/area=gate/canon__gate.check.01__aaaa__0123456789ab.json", "{}");
        write(dir.path(), ".canon/archive/kind=evidence_record/x__0123456789ab.json", "{}");
        write(dir.path(), ".canon/ledger/_staging/kind=finding/c__0001__0001__0123456789ab.json", "{}");
        let found = detect(dir.path()).unwrap();
        assert_eq!(found.files, 3);
        assert!(found.message().contains("canon migrate"));
    }

    #[test]
    fn the_config_key_and_the_gitignore_line_alone_make_an_old_repo() {
        let dir = tempfile::tempdir().unwrap();
        write(dir.path(), "canon.yaml", "routing:\n  evidence_record: local\n");
        write(dir.path(), ".gitignore", "!.canon/ledger/kind=evidence_record/\n");
        let found = detect(dir.path()).unwrap();
        assert_eq!((found.files, found.config_keys, found.gitignore_lines), (0, vec!["routing.evidence_record".to_string()], 1));
    }
}
