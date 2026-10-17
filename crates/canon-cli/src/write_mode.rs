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

use std::path::Path;

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

/// `write.promote` from `canon.yaml` at `canon_yaml`. A missing file,
/// an absent `write:` key, or an absent `promote:` is `manual`, so a
/// repo that never opted in sees no change. A present but malformed
/// section fails loud: a typo meant to turn promotion on must not
/// silently leave every record staged.
pub fn configured_promote_mode(canon_yaml: &Path) -> Result<PromoteMode, String> {
    let Ok(text) = std::fs::read_to_string(canon_yaml) else {
        return Ok(PromoteMode::Manual);
    };
    let doc: serde_yaml::Value =
        serde_yaml::from_str(&text).map_err(|e| format!("canon.yaml is not valid YAML: {e}"))?;
    let Some(section) = doc.get("write") else {
        return Ok(PromoteMode::Manual);
    };
    let section: RawWriteSection = serde_yaml::from_value(section.clone())
        .map_err(|e| format!("canon.yaml `write:` section is malformed (expected `write: {{ promote: auto | manual }}`): {e}"))?;
    Ok(section.promote.unwrap_or(PromoteMode::Manual))
}

/// Whether a staged write in `repo` (already resolved to its root)
/// promotes in the same run. An explicit flag wins without reading the
/// config.
pub fn should_promote(repo: &Path, flag: PromoteFlag) -> Result<bool, String> {
    match flag {
        PromoteFlag::Promote => Ok(true),
        PromoteFlag::NoPromote => Ok(false),
        PromoteFlag::Config => {
            Ok(configured_promote_mode(&repo.join("canon.yaml"))? == PromoteMode::Auto)
        }
    }
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
        configured_promote_mode(&path)
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
        assert!(mode(Some("write: { promote: always }\n")).is_err());
        assert!(mode(Some("write: { promot: auto }\n")).is_err());
        assert!(mode(Some("write: auto\n")).is_err());
    }

    #[test]
    fn an_explicit_flag_wins_without_reading_the_config() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::write(dir.path().join("canon.yaml"), "write: { promote: bogus }\n").unwrap();
        assert_eq!(should_promote(dir.path(), PromoteFlag::Promote), Ok(true));
        assert_eq!(
            should_promote(dir.path(), PromoteFlag::NoPromote),
            Ok(false)
        );
        assert!(should_promote(dir.path(), PromoteFlag::Config).is_err());
    }
}
