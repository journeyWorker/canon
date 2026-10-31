//! How a write command says where its write landed, and whether a
//! staged write promotes itself.
//!
//! Some writes are STAGED: the record waits in a staging directory and
//! lands in the ledger only when a promote runs. The rest are written
//! directly. An operator who cannot tell the two apart either skips a
//! promote that was needed or runs one that has nothing to do. So every
//! write command ends its success line with exactly one of these
//! suffixes, after ` — `.
//!
//! # Promote on write (0.16 D2)
//! A staged write command (`evidence add`, `finding add`, `finding
//! close`, `divergence add`) promotes in the same run when asked:
//! `--promote` on the command line, or `write: { promote: auto }` in
//! `canon.yaml`. `--no-promote` keeps one write staged under `auto`. The
//! run promotes ONLY the record it just staged, through the same
//! validation and assignment `canon gate promote` / `canon divergence
//! promote` apply (`canon_gate::promote_staged`); a record another author staged stays
//! staged until they, or a full promote, commit it. An absent `write:`
//! key is `manual`: a repo that never set it keeps staging until someone
//! promotes.

use std::path::{Path, PathBuf};

use canon_model::envelope::RecordKind;
use canon_store::policy::{BackendConfig, Rung, TierPolicy};

use crate::cli_error::{CliError, CliErrorExt as _};

/// A record staged for `canon gate promote` (`evidence add`, `finding
/// add`, `finding close`).
pub const STAGED_FOR_GATE_PROMOTE: &str = "run `canon gate promote` to commit it";

/// A write that is already final: a ledger record committed directly
/// (`review add`), a subject record, or a
/// file in the working tree (`scenario new`, `feature new`).
pub const DIRECT: &str = "written directly; nothing to promote";

/// A staged write this same run promoted (`--promote`, or `write:
/// { promote: auto }`): final, like [`DIRECT`], and naming the promotion
/// so nobody runs a second one.
pub const PROMOTED: &str = "staged and promoted in this run; nothing to promote";

/// What the command line said about promotion.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum PromoteFlag {
    /// Neither flag: `canon.yaml`'s `write.promote` decides.
    #[default]
    Config,
    /// `--promote`.
    Promote,
    /// `--no-promote`.
    NoPromote,
}

impl PromoteFlag {
    /// The flag from clap's two mutually exclusive booleans.
    pub fn from_flags(promote: bool, no_promote: bool) -> Self {
        match (promote, no_promote) {
            (true, _) => Self::Promote,
            (_, true) => Self::NoPromote,
            _ => Self::Config,
        }
    }
}

/// `canon.yaml`'s `write.promote`.
#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum PromoteMode {
    /// Staged writes promote themselves.
    Auto,
    /// Staged writes wait for `canon gate promote` (the default).
    Manual,
}

impl PromoteMode {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Auto => "auto",
            Self::Manual => "manual",
        }
    }
}

/// `canon.yaml`'s `write:` section, strict once present.
#[derive(Debug, Default, serde::Deserialize)]
#[serde(deny_unknown_fields)]
struct RawWriteSection {
    #[serde(default)]
    promote: Option<PromoteMode>,
}

/// Why `canon.yaml`'s `write:` section cannot be read.
#[derive(Debug, thiserror::Error)]
pub enum WriteModeError {
    /// `canon.yaml` is not YAML.
    #[error("canon.yaml is not valid YAML: {0}")]
    Yaml(#[source] serde_yaml::Error),
    /// `write:` is not `{ promote: auto | manual }`.
    #[error("canon.yaml `write:` section is malformed (expected `write: {{ promote: auto | manual }}`): {0}")]
    Section(#[source] serde_yaml::Error),
}

/// `write.promote` from `canon.yaml` at `canon_yaml`. A missing file,
/// an absent `write:` key, or an absent `promote:` is `manual`, so a
/// repo that never opted in sees no change. A present but malformed
/// section fails loud: a typo meant to turn promotion on must not
/// silently leave every record staged.
pub fn configured_promote_mode(canon_yaml: &Path) -> Result<PromoteMode, WriteModeError> {
    let Ok(text) = std::fs::read_to_string(canon_yaml) else {
        return Ok(PromoteMode::Manual);
    };
    let doc: serde_yaml::Value = serde_yaml::from_str(&text).map_err(WriteModeError::Yaml)?;
    let Some(section) = doc.get("write") else {
        return Ok(PromoteMode::Manual);
    };
    let section: RawWriteSection = serde_yaml::from_value(section.clone()).map_err(WriteModeError::Section)?;
    Ok(section.promote.unwrap_or(PromoteMode::Manual))
}

/// Whether a staged write in `repo` (already resolved to its root)
/// promotes in the same run. An explicit flag wins without reading the
/// config.
pub fn should_promote(repo: &Path, flag: PromoteFlag) -> Result<bool, WriteModeError> {
    match flag {
        PromoteFlag::Promote => Ok(true),
        PromoteFlag::NoPromote => Ok(false),
        PromoteFlag::Config => {
            Ok(configured_promote_mode(&repo.join("canon.yaml"))? == PromoteMode::Auto)
        }
    }
}

/// Whether a real run could create the file `path`: `path` is not a
/// directory and its parent is one (`create_parents`: its nearest existing
/// ancestor is). What a `--dry-run` asks instead of writing, so it refuses
/// where the real write would.
pub fn check_creatable(path: &Path, create_parents: bool) -> std::io::Result<()> {
    if path.is_dir() {
        return Err(std::io::Error::new(std::io::ErrorKind::IsADirectory, format!("`{}` is a directory", path.display())));
    }
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    let nearest = if create_parents { parent.ancestors().find(|ancestor| ancestor.exists()) } else { Some(parent) };
    match nearest {
        Some(dir) if dir.is_dir() => Ok(()),
        Some(dir) if dir.exists() => Err(std::io::Error::new(std::io::ErrorKind::NotADirectory, format!("`{}` is not a directory", dir.display()))),
        Some(dir) => Err(std::io::Error::new(std::io::ErrorKind::NotFound, format!("`{}` does not exist", dir.display()))),
        None => Ok(()),
    }
}

/// Whether a real run could write `path` (a file it creates or replaces, or a
/// directory it fills): the nearest ancestor that exists is a directory whose
/// permissions allow writing, and `path` itself, when it exists, is not a
/// read-only file or a directory that forbids writing. What a write command
/// asks before it changes anything, so a target that cannot be written is
/// refused up front ([`unavailable`]) instead of failing after the first write.
pub fn check_writable(path: &Path) -> std::io::Result<()> {
    let permission_denied = |what: &Path| std::io::Error::new(std::io::ErrorKind::PermissionDenied, format!("`{}` is read-only", what.display()));
    if path.exists() {
        let meta = std::fs::metadata(path)?;
        return if meta.permissions().readonly() { Err(permission_denied(path)) } else { Ok(()) };
    }
    let parent = path.parent().filter(|p| !p.as_os_str().is_empty()).unwrap_or(Path::new("."));
    match parent.ancestors().find(|ancestor| ancestor.exists()) {
        Some(dir) if dir.is_dir() => {
            if std::fs::metadata(dir)?.permissions().readonly() {
                Err(permission_denied(dir))
            } else {
                Ok(())
            }
        }
        Some(dir) => Err(std::io::Error::new(std::io::ErrorKind::NotADirectory, format!("`{}` is not a directory", dir.display()))),
        None => Ok(()),
    }
}

/// The refusal for a target [`check_writable`] found unwritable: known before
/// the first write, so exit 2 (`WRITE_UNAVAILABLE`) and nothing changed. A
/// write that began and failed is `WRITE_FAILED` (exit 1).
pub fn unavailable(path: &Path, cause: &std::io::Error) -> CliError {
    CliError::refused("WRITE_UNAVAILABLE", format!("cannot write `{}`: {cause}", path.display())).hint("make the target writable (its permissions, a mounted volume, a missing parent), then run the command again; nothing was changed")
}

/// [`check_writable`] over every target a command writes, refused as
/// [`unavailable`] under `command`. `None` when all can be written.
pub fn refuse_unwritable(targets: &[&Path], command: &str, json: bool) -> Option<u8> {
    targets.iter().find_map(|target| check_writable(target).err().map(|cause| unavailable(target, &cause).emit(command, json)))
}

/// The local paths a write of records of `kinds` lands in, by `canon.yaml`'s
/// routing: a git-backed rung's `kind=<kind>/` directory under its root, a
/// sqlite-backed rung's database file and the directory holding it (SQLite
/// creates its journal beside the file). A rung on a network store (postgres,
/// s3) has no local path and adds none. Empty when `canon.yaml` cannot be read
/// or parsed: the command's own tier build names that failure.
pub fn routed_targets(canon_yaml: &Path, kinds: &[RecordKind]) -> Vec<PathBuf> {
    let Some((policy, base)) = read_policy(canon_yaml) else { return Vec::new() };
    let mut targets = Vec::new();
    for kind in kinds {
        if let Ok(rung) = policy.tier_for(*kind) {
            add_rung_targets(&mut targets, &policy, &base, rung, *kind);
        }
    }
    targets
}

/// The local paths records of `kind` can occupy: those of its routed rung and,
/// when `canon.yaml` ages the kind out, of the rung it ages into (`canon purge`
/// removes from both).
pub fn resident_targets(canon_yaml: &Path, kind: RecordKind) -> Vec<PathBuf> {
    let Some((policy, base)) = read_policy(canon_yaml) else { return Vec::new() };
    let mut targets = Vec::new();
    let routed = policy.tier_for(kind).ok();
    let aged = policy.aging.get(&kind).map(|rule| rule.to);
    for rung in routed.into_iter().chain(aged) {
        add_rung_targets(&mut targets, &policy, &base, rung, kind);
    }
    targets
}

/// The local paths moving `kind` records from rung `from` to rung `to` writes
/// (`canon tier age`): the records leave one and land in the other.
pub fn move_targets(canon_yaml: &Path, kind: RecordKind, from: Rung, to: Rung) -> Vec<PathBuf> {
    let Some((policy, base)) = read_policy(canon_yaml) else { return Vec::new() };
    let mut targets = Vec::new();
    for rung in [from, to] {
        add_rung_targets(&mut targets, &policy, &base, rung, kind);
    }
    targets
}

/// `canon_yaml`'s tier policy and the directory its paths resolve against.
fn read_policy(canon_yaml: &Path) -> Option<(TierPolicy, PathBuf)> {
    let text = std::fs::read_to_string(canon_yaml).ok()?;
    let base = crate::tiers::project_dir(canon_yaml).to_path_buf();
    let policy = TierPolicy::from_yaml_at(&text, &base).ok()?;
    Some((policy, base))
}

/// Adds the local path `kind` records occupy in `rung`, when the rung has one.
fn add_rung_targets(targets: &mut Vec<PathBuf>, policy: &TierPolicy, base: &Path, rung: Rung, kind: RecordKind) {
    let mut add = |path: PathBuf| {
        if !targets.contains(&path) {
            targets.push(path);
        }
    };
    match policy.tiers.get(&rung) {
        Some(BackendConfig::Git(config)) => add(base.join(&config.root).join(format!("kind={}", kind.as_str()))),
        Some(BackendConfig::Sqlite(config)) => {
            add(config.path.clone());
            if let Some(directory) = config.path.parent().filter(|p| !p.as_os_str().is_empty()) {
                add(directory.to_path_buf());
            }
        }
        Some(BackendConfig::Postgres(_) | BackendConfig::S3(_)) | None => {}
    }
}

/// [`check_writable`] over [`routed_targets`]: the first target a write of
/// `kinds` could not land in, as the [`unavailable`] refusal.
pub fn check_kinds_writable(canon_yaml: &Path, kinds: &[RecordKind]) -> Result<(), CliError> {
    for target in routed_targets(canon_yaml, kinds) {
        check_writable(&target).map_err(|cause| unavailable(&target, &cause))?;
    }
    Ok(())
}

/// [`check_kinds_writable`], refused under `command`. `None` when every
/// target can be written.
pub fn refuse_unwritable_kinds(canon_yaml: &Path, kinds: &[RecordKind], command: &str, json: bool) -> Option<u8> {
    check_kinds_writable(canon_yaml, kinds).err().map(|error| error.emit(command, json))
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mode(yaml: Option<&str>) -> Result<PromoteMode, String> {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("canon.yaml");
        if let Some(yaml) = yaml {
            std::fs::write(&path, yaml).unwrap();
        }
        configured_promote_mode(&path).map_err(|error| error.to_string())
    }

    #[test]
    fn an_absent_file_key_or_field_is_manual() {
        assert_eq!(mode(None), Ok(PromoteMode::Manual));
        assert_eq!(
            mode(Some("specs:\n  roots:\n    - id: root\n")),
            Ok(PromoteMode::Manual)
        );
        assert_eq!(mode(Some("write: {}\n")), Ok(PromoteMode::Manual));
    }

    #[test]
    fn auto_and_manual_parse() {
        assert_eq!(
            mode(Some("write:\n  promote: auto\n")),
            Ok(PromoteMode::Auto)
        );
        assert_eq!(
            mode(Some("write: { promote: manual }\n")),
            Ok(PromoteMode::Manual)
        );
    }

    #[test]
    fn a_present_but_malformed_section_fails_loud() {
        mode(Some("write: { promote: always }\n")).unwrap_err();
        mode(Some("write: { promot: auto }\n")).unwrap_err();
        mode(Some("write: auto\n")).unwrap_err();
    }

    #[test]
    fn an_explicit_flag_wins_without_reading_the_config() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("canon.yaml"), "write: { promote: bogus }\n").unwrap();
        assert!(matches!(should_promote(dir.path(), PromoteFlag::Promote), Ok(true)));
        assert!(matches!(should_promote(dir.path(), PromoteFlag::NoPromote), Ok(false)));
        should_promote(dir.path(), PromoteFlag::Config).unwrap_err();
    }

    #[test]
    fn the_refusals_read_as_before_and_keep_their_cause() {
        let message = mode(Some("write: auto\n")).unwrap_err();
        assert!(message.starts_with("canon.yaml `write:` section is malformed (expected `write: { promote: auto | manual }`): "), "{message}");
        let message = mode(Some("write: [\n")).unwrap_err();
        assert!(message.starts_with("canon.yaml is not valid YAML: "), "{message}");
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("canon.yaml"), "write: auto\n").unwrap();
        let error = should_promote(dir.path(), PromoteFlag::Config).unwrap_err();
        assert!(std::error::Error::source(&error).is_some());
    }
}
