//! `canon skills install`: materializes `canon/skills/<name>/SKILL.md` (the
//! single source of truth, design decision 9) into a consumer repo's
//! `.claude/skills/<name>/SKILL.md` (verbatim copy) and `.codex/skills/
//! <name>.md` (canon's own flattened convention, design D4) — gemini is
//! never touched (decision 11).
//!
//! The install is deterministic and timestamp-free: `canon/skills/
//! .install-lock.json` records only a content hash and a monotonic version
//! integer per skill, never a `generatedAt` field. Re-running with no
//! source changes is a byte-identical no-op (skill-materialization spec,
//! scenario "Re-running with no source changes is a byte-identical
//! no-op") — this is the fix for the `generatedAt`-poisoned-hash failure
//! mode a donor's agent-manifest materialization documents against its
//! own `agent-manifest` package: the lock's
//! `contentHash` is computed over the skill's own semantic bytes (the
//! `SKILL.md` file content), never over an artifact that embeds its own
//! wall-clock generation time.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

/// One companion skill discovered under `canon/skills/<name>/SKILL.md`.
#[derive(Debug, Clone)]
pub struct DiscoveredSkill {
    pub name: String,
    pub skill_md_path: PathBuf,
    pub content: String,
}

/// `canon/skills/.install-lock.json`'s per-skill entry. Field order is the
/// struct declaration order (`contentHash` before `version`); no
/// `generatedAt` field ever exists on this type (decision 11).
#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LockEntry {
    #[serde(rename = "contentHash")]
    pub content_hash: String,
    pub version: u64,
}

/// The full lock file: `{ "skills": { "<name>": LockEntry, ... } }`. A
/// `BTreeMap` keeps the serialized key order alphabetical regardless of
/// filesystem scan order, so two runs over an unchanged source directory
/// produce byte-identical JSON.
#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Lock {
    pub skills: BTreeMap<String, LockEntry>,
}

/// One skill's outcome from an `install` run, reported to the CLI caller.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledSkill {
    pub name: String,
    pub version: u64,
    pub changed: bool,
}

/// The full result of an `install` run: per-skill outcomes plus the final
/// lock snapshot that was written to disk.
#[derive(Debug, Clone)]
pub struct InstallReport {
    pub installed: Vec<InstalledSkill>,
    pub lock: Lock,
}

#[derive(Debug, thiserror::Error)]
pub enum SkillsError {
    #[error("source directory not found: {0}")]
    SourceNotFound(PathBuf),
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
    #[error("malformed lock file at {path}: {source}")]
    LockParse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
}

fn read_to_string(path: &Path) -> Result<String, SkillsError> {
    fs::read_to_string(path).map_err(|source| SkillsError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn write_file(path: &Path, content: &str) -> Result<(), SkillsError> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| SkillsError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(path, content).map_err(|source| SkillsError::Io {
        path: path.to_path_buf(),
        source,
    })
}

/// `sha256:<hex>` over the exact bytes given — never over a re-derived or
/// re-serialized projection that could pick up incidental formatting
/// differences.
pub fn content_hash(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{:x}", hasher.finalize())
}

/// Discover every `<source_dir>/<name>/SKILL.md`, sorted by name for
/// deterministic iteration.
pub fn discover_skills(source_dir: &Path) -> Result<Vec<DiscoveredSkill>, SkillsError> {
    if !source_dir.is_dir() {
        return Err(SkillsError::SourceNotFound(source_dir.to_path_buf()));
    }
    let mut names: Vec<String> = Vec::new();
    for entry in fs::read_dir(source_dir).map_err(|source| SkillsError::Io {
        path: source_dir.to_path_buf(),
        source,
    })? {
        let entry = entry.map_err(|source| SkillsError::Io {
            path: source_dir.to_path_buf(),
            source,
        })?;
        if !entry.file_type().map(|t| t.is_dir()).unwrap_or(false) {
            continue;
        }
        let skill_md = entry.path().join("SKILL.md");
        if skill_md.is_file() {
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    names.sort();

    let mut skills = Vec::with_capacity(names.len());
    for name in names {
        let skill_md_path = source_dir.join(&name).join("SKILL.md");
        let content = read_to_string(&skill_md_path)?;
        skills.push(DiscoveredSkill {
            name,
            skill_md_path,
            content,
        });
    }
    Ok(skills)
}

/// Extract `name`/`description` from a `SKILL.md`'s YAML frontmatter
/// (`---\nname: …\ndescription: …\n---\n<body>`). Falls back to the
/// directory name / empty description when frontmatter is absent or a
/// field is missing — never fails the whole install over a formatting
/// wrinkle in one skill file.
fn parse_frontmatter(fallback_name: &str, content: &str) -> (String, String, String) {
    let mut name = fallback_name.to_string();
    let mut description = String::new();

    if let Some(rest) = content.strip_prefix("---\n") {
        if let Some(end) = rest.find("\n---\n") {
            let frontmatter = &rest[..end];
            let body = &rest[end + "\n---\n".len()..];
            for line in frontmatter.lines() {
                if let Some((key, value)) = line.split_once(':') {
                    let key = key.trim();
                    let value = value.trim();
                    match key {
                        "name" => name = value.to_string(),
                        "description" => description = value.to_string(),
                        _ => {}
                    }
                }
            }
            return (name, description, body.trim_start_matches('\n').to_string());
        }
    }
    (name, description, content.to_string())
}

/// canon's `.codex/skills/<name>.md` flattening (design D4): Codex has no
/// native skill-directory concept, so this is canon's own convention — a
/// single flat file with the name/description promoted to a header block
/// followed by the skill body, replacing the YAML frontmatter Claude Code
/// reads natively.
pub fn flatten_for_codex(name: &str, description: &str, body: &str) -> String {
    let mut out = String::new();
    out.push_str("# ");
    out.push_str(name);
    out.push('\n');
    if !description.is_empty() {
        out.push('\n');
        out.push_str("> ");
        out.push_str(description);
        out.push('\n');
    }
    out.push('\n');
    out.push_str(body.trim_end());
    out.push('\n');
    out
}

fn load_lock(source_dir: &Path) -> Result<Lock, SkillsError> {
    let lock_path = source_dir.join(".install-lock.json");
    if !lock_path.is_file() {
        return Ok(Lock::default());
    }
    let raw = read_to_string(&lock_path)?;
    serde_json::from_str(&raw).map_err(|source| SkillsError::LockParse {
        path: lock_path,
        source,
    })
}

/// Canonical, timestamp-free JSON serialization: `BTreeMap` gives
/// alphabetical key order and `to_string_pretty` gives stable 2-space
/// indentation; a trailing newline makes the file POSIX-text-file clean.
fn write_lock(source_dir: &Path, lock: &Lock) -> Result<(), SkillsError> {
    let lock_path = source_dir.join(".install-lock.json");
    let mut json = serde_json::to_string_pretty(lock).expect("Lock serialization is infallible");
    json.push('\n');
    write_file(&lock_path, &json)
}

/// Materialize every skill under `source_dir` (`canon/skills/`) into
/// `target_dir`'s `.claude/skills/<name>/SKILL.md` and `.codex/skills/
/// <name>.md`, then write the updated lock back into `source_dir`.
///
/// Idempotent: running twice with no source change writes byte-identical
/// output both times (skill-materialization spec). A skill's version
/// increments by exactly one when its content hash changes; unrelated
/// skills' lock entries are left untouched.
pub fn install(source_dir: &Path, target_dir: &Path) -> Result<InstallReport, SkillsError> {
    let discovered = discover_skills(source_dir)?;
    let previous_lock = load_lock(source_dir)?;

    let mut new_skills = BTreeMap::new();
    let mut installed = Vec::with_capacity(discovered.len());

    for skill in &discovered {
        let hash = content_hash(skill.content.as_bytes());
        let (name, description, body) = parse_frontmatter(&skill.name, &skill.content);

        let (version, changed) = match previous_lock.skills.get(&skill.name) {
            Some(prev) if prev.content_hash == hash => (prev.version, false),
            Some(prev) => (prev.version + 1, true),
            None => (1, true),
        };

        new_skills.insert(
            skill.name.clone(),
            LockEntry {
                content_hash: hash,
                version,
            },
        );

        let claude_path = target_dir
            .join(".claude")
            .join("skills")
            .join(&skill.name)
            .join("SKILL.md");
        write_file(&claude_path, &skill.content)?;

        let codex_path = target_dir
            .join(".codex")
            .join("skills")
            .join(format!("{}.md", skill.name));
        let flattened = flatten_for_codex(&name, &description, &body);
        write_file(&codex_path, &flattened)?;

        installed.push(InstalledSkill {
            name: skill.name.clone(),
            version,
            changed,
        });
    }

    let lock = Lock { skills: new_skills };
    write_lock(source_dir, &lock)?;

    Ok(InstallReport { installed, lock })
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn content_hash_is_stable_and_content_addressed() {
        let a = content_hash(b"hello");
        let b = content_hash(b"hello");
        let c = content_hash(b"hello!");
        assert_eq!(a, b);
        assert_ne!(a, c);
        assert!(a.starts_with("sha256:"));
    }

    #[test]
    fn parse_frontmatter_extracts_name_and_description() {
        let content = "---\nname: repo-scaffold\ndescription: how to add a crate\n---\n\nbody text\n";
        let (name, description, body) = parse_frontmatter("fallback", content);
        assert_eq!(name, "repo-scaffold");
        assert_eq!(description, "how to add a crate");
        assert_eq!(body, "body text\n");
    }

    #[test]
    fn parse_frontmatter_falls_back_without_delimiters() {
        let (name, description, body) = parse_frontmatter("fallback", "just a body\n");
        assert_eq!(name, "fallback");
        assert_eq!(description, "");
        assert_eq!(body, "just a body\n");
    }

    #[test]
    fn flatten_for_codex_renders_header_and_body() {
        let flattened = flatten_for_codex("repo-scaffold", "how to add a crate", "body text\n");
        assert!(flattened.starts_with("# repo-scaffold\n"));
        assert!(flattened.contains("> how to add a crate\n"));
        assert!(flattened.trim_end().ends_with("body text"));
    }
}
