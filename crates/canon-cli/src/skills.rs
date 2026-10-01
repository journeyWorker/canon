//! Canon skill bundle materialization.
//!
//! The canonical user-facing source is a read-only bundle rooted at
//! `SKILL.src.md`. It projects one `canon` skill to Claude and/or Codex and
//! keeps references and scripts as lazy sidecars. Directory-shaped sources
//! (notably `canon/skills-dev`) retain the legacy materializer for backwards
//! compatibility.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

pub const CANONICAL_SOURCE_ENV: &str = "CANON_SKILLS_SOURCE";

#[derive(Debug, Clone)]
pub struct DiscoveredSkill {
    pub name: String,
    pub skill_md_path: PathBuf,
    pub content: String,
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
pub struct LockEntry {
    #[serde(rename = "contentHash")]
    pub content_hash: String,
    pub version: u64,
}

#[derive(Debug, Clone, Default, Serialize, Deserialize, PartialEq, Eq)]
pub struct Lock {
    pub skills: BTreeMap<String, LockEntry>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InstalledSkill {
    pub name: String,
    pub version: u64,
    pub changed: bool,
}

#[derive(Debug, Clone)]
pub struct InstallReport {
    pub installed: Vec<InstalledSkill>,
    pub lock: Lock,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Ord, PartialOrd)]
pub enum Provider {
    Claude,
    Codex,
}

impl Provider {
    pub fn as_str(self) -> &'static str {
        match self {
            Self::Claude => "claude",
            Self::Codex => "codex",
        }
    }

    pub fn parse(value: &str) -> Result<Self, SkillsError> {
        match value.trim().to_ascii_lowercase().as_str() {
            "claude" => Ok(Self::Claude),
            "codex" => Ok(Self::Codex),
            other => Err(SkillsError::InvalidProvider(other.to_string())),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalInstallReport {
    pub providers: Vec<Provider>,
    pub source_hash: String,
    pub changed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillStatus {
    pub provider: Provider,
    pub path: PathBuf,
    pub state: &'static str,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillsCheckReport {
    pub providers: Vec<Provider>,
    pub source_hash: String,
    pub statuses: Vec<SkillStatus>,
    pub manifest_ok: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum SkillsError {
    #[error("source directory not found: {0}")]
    SourceNotFound(PathBuf),
    #[error("canonical skill source is missing SKILL.src.md under {0}")]
    CanonicalSourceMissing(PathBuf),
    #[error("invalid provider `{0}`; expected one of: claude, codex")]
    InvalidProvider(String),
    #[error("canonical bundle has an unsupported or unsafe symlink at {0}")]
    UnsafeSymlink(PathBuf),
    #[error("refusing to overwrite a symlink at {0}")]
    RefuseSymlink(PathBuf),
    #[error("malformed lock file at {path}: {source}")]
    LockParse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("malformed canonical manifest at {path}: {source}")]
    ManifestParse {
        path: PathBuf,
        #[source]
        source: serde_json::Error,
    },
    #[error("io error at {path}: {source}")]
    Io {
        path: PathBuf,
        #[source]
        source: std::io::Error,
    },
}

fn read_bytes(path: &Path) -> Result<Vec<u8>, SkillsError> {
    fs::read(path).map_err(|source| SkillsError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn read_to_string(path: &Path) -> Result<String, SkillsError> {
    String::from_utf8(read_bytes(path)?).map_err(|source| SkillsError::Io {
        path: path.to_path_buf(),
        source: std::io::Error::new(std::io::ErrorKind::InvalidData, source),
    })
}

fn reject_symlink_path(path: &Path) -> Result<(), SkillsError> {
    if let Ok(metadata) = fs::symlink_metadata(path) {
        if metadata.file_type().is_symlink() {
            return Err(SkillsError::RefuseSymlink(path.to_path_buf()));
        }
    }
    Ok(())
}

fn reject_symlink_path_under(target_dir: &Path, path: &Path) -> Result<(), SkillsError> {
    let relative = path
        .strip_prefix(target_dir)
        .map_err(|_| SkillsError::RefuseSymlink(path.to_path_buf()))?;
    let mut current = target_dir.to_path_buf();
    if let Ok(metadata) = fs::symlink_metadata(&current) {
        if metadata.file_type().is_symlink() {
            return Err(SkillsError::RefuseSymlink(path.to_path_buf()));
        }
    }
    for component in relative.components() {
        current.push(component);
        if let Ok(metadata) = fs::symlink_metadata(&current) {
            if metadata.file_type().is_symlink() {
                return Err(SkillsError::RefuseSymlink(path.to_path_buf()));
            }
        }
    }
    Ok(())
}

fn write_bytes_under(target_dir: &Path, path: &Path, bytes: &[u8]) -> Result<(), SkillsError> {
    reject_symlink_path_under(target_dir, path)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| SkillsError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(path, bytes).map_err(|source| SkillsError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn write_file_under(target_dir: &Path, path: &Path, content: &str) -> Result<(), SkillsError> {
    write_bytes_under(target_dir, path, content.as_bytes())
}

fn write_bytes(path: &Path, bytes: &[u8]) -> Result<(), SkillsError> {
    reject_symlink_path(path)?;
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent).map_err(|source| SkillsError::Io {
            path: parent.to_path_buf(),
            source,
        })?;
    }
    fs::write(path, bytes).map_err(|source| SkillsError::Io {
        path: path.to_path_buf(),
        source,
    })
}

fn write_file(path: &Path, content: &str) -> Result<(), SkillsError> {
    write_bytes(path, content.as_bytes())
}

pub fn content_hash(bytes: &[u8]) -> String {
    let mut hasher = Sha256::new();
    hasher.update(bytes);
    format!("sha256:{:x}", hasher.finalize())
}

pub fn discover_skills(source_dir: &Path) -> Result<Vec<DiscoveredSkill>, SkillsError> {
    if !source_dir.is_dir() {
        return Err(SkillsError::SourceNotFound(source_dir.to_path_buf()));
    }
    let mut names = Vec::new();
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
        if entry.path().join("SKILL.md").is_file() {
            names.push(entry.file_name().to_string_lossy().into_owned());
        }
    }
    names.sort();
    names
        .into_iter()
        .map(|name| {
            let skill_md_path = source_dir.join(&name).join("SKILL.md");
            Ok(DiscoveredSkill {
                name,
                content: read_to_string(&skill_md_path)?,
                skill_md_path,
            })
        })
        .collect()
}

fn parse_frontmatter(fallback_name: &str, content: &str) -> (String, String, String) {
    let mut name = fallback_name.to_string();
    let mut description = String::new();
    if let Some(rest) = content.strip_prefix("---\n") {
        if let Some(end) = rest.find("\n---\n") {
            let frontmatter = &rest[..end];
            let body = &rest[end + "\n---\n".len()..];
            for line in frontmatter.lines() {
                if let Some((key, value)) = line.split_once(':') {
                    match key.trim() {
                        "name" => name = value.trim().to_string(),
                        "description" => description = value.trim().to_string(),
                        _ => {}
                    }
                }
            }
            return (name, description, body.trim_start_matches('\n').to_string());
        }
    }
    (name, description, content.to_string())
}

pub fn flatten_for_codex(name: &str, description: &str, body: &str) -> String {
    let mut out = format!("# {name}\n");
    if !description.is_empty() {
        out.push_str("\n> ");
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

fn write_lock(source_dir: &Path, lock: &Lock) -> Result<(), SkillsError> {
    let mut json = serde_json::to_string_pretty(lock).expect("Lock serialization is infallible");
    json.push('\n');
    write_file(&source_dir.join(".install-lock.json"), &json)
}

fn install_legacy(source_dir: &Path, target_dir: &Path) -> Result<InstallReport, SkillsError> {
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
        write_file_under(
            target_dir,
            &target_dir
                .join(".claude/skills")
                .join(&skill.name)
                .join("SKILL.md"),
            &skill.content,
        )?;
        write_file_under(
            target_dir,
            &target_dir
                .join(".codex/skills")
                .join(format!("{}.md", skill.name)),
            &flatten_for_codex(&name, &description, &body),
        )?;
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

#[derive(Debug, Clone)]
struct BundleFile {
    relative: PathBuf,
    bytes: Vec<u8>,
}

fn collect_bundle_files(
    root: &Path,
    relative_root: &Path,
    out: &mut Vec<BundleFile>,
) -> Result<(), SkillsError> {
    let dir = root.join(relative_root);
    if !dir.exists() {
        return Ok(());
    }
    for entry in fs::read_dir(&dir).map_err(|source| SkillsError::Io {
        path: dir.clone(),
        source,
    })? {
        let entry = entry.map_err(|source| SkillsError::Io {
            path: dir.clone(),
            source,
        })?;
        let file_type = entry.file_type().map_err(|source| SkillsError::Io {
            path: entry.path(),
            source,
        })?;
        let relative = relative_root.join(entry.file_name());
        if file_type.is_symlink() {
            return Err(SkillsError::UnsafeSymlink(entry.path()));
        }
        if file_type.is_dir() {
            collect_bundle_files(root, &relative, out)?;
        } else if file_type.is_file() {
            out.push(BundleFile {
                relative,
                bytes: read_bytes(&entry.path())?,
            });
        }
    }
    Ok(())
}

fn canonical_bundle(source_dir: &Path) -> Result<Vec<BundleFile>, SkillsError> {
    let root = source_dir.join("SKILL.src.md");
    if !root.is_file() {
        return Err(SkillsError::CanonicalSourceMissing(
            source_dir.to_path_buf(),
        ));
    }
    let mut files = vec![BundleFile {
        relative: PathBuf::from("SKILL.src.md"),
        bytes: read_bytes(&root)?,
    }];
    collect_bundle_files(source_dir, Path::new("reference"), &mut files)?;
    collect_bundle_files(source_dir, Path::new("scripts"), &mut files)?;
    files.sort_by(|a, b| a.relative.cmp(&b.relative));
    Ok(files)
}

fn bundle_hash(files: &[BundleFile]) -> String {
    let mut hasher = Sha256::new();
    for file in files {
        let path = file.relative.to_string_lossy();
        hasher.update(path.as_bytes());
        hasher.update([0]);
        hasher.update(&file.bytes);
        hasher.update([0]);
    }
    format!("sha256:{:x}", hasher.finalize())
}

fn parse_providers(value: Option<&str>, target_dir: &Path) -> Result<Vec<Provider>, SkillsError> {
    let mut providers = if let Some(value) = value {
        let mut parsed = Vec::new();
        for part in value.split(',').filter(|part| !part.trim().is_empty()) {
            parsed.push(Provider::parse(part)?);
        }
        if parsed.is_empty() {
            return Err(SkillsError::InvalidProvider(value.to_string()));
        }
        parsed
    } else {
        let claude = target_dir.join(".claude").exists();
        let codex = target_dir.join(".codex").exists();
        if !claude && !codex {
            vec![Provider::Claude, Provider::Codex]
        } else {
            [(claude, Provider::Claude), (codex, Provider::Codex)]
                .into_iter()
                .filter_map(|(yes, provider)| yes.then_some(provider))
                .collect()
        }
    };
    providers.sort();
    providers.dedup();
    Ok(providers)
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct CanonicalManifest {
    version: u64,
    source_hash: String,
    providers: Vec<String>,
    files: BTreeMap<String, String>,
}

fn manifest_path(target_dir: &Path) -> PathBuf {
    target_dir.join(".canon/skills/.install-lock.json")
}

fn load_manifest(target_dir: &Path) -> Result<Option<CanonicalManifest>, SkillsError> {
    let path = manifest_path(target_dir);
    if !path.is_file() {
        return Ok(None);
    }
    let raw = read_to_string(&path)?;
    serde_json::from_str(&raw)
        .map(Some)
        .map_err(|source| SkillsError::ManifestParse { path, source })
}

fn projected_relative(provider: Provider, relative: &Path) -> Option<PathBuf> {
    if relative == Path::new("SKILL.src.md") {
        return (provider == Provider::Claude).then(|| PathBuf::from("SKILL.md"));
    }
    Some(relative.to_path_buf())
}

fn manifest_for(files: &[BundleFile], providers: &[Provider]) -> CanonicalManifest {
    let source_hash = bundle_hash(files);
    let mut hashes = BTreeMap::new();
    for provider in providers {
        for file in files {
            let Some(relative) = projected_relative(*provider, &file.relative) else {
                continue;
            };
            let prefix = match provider {
                Provider::Claude => ".claude/skills/canon",
                Provider::Codex => ".codex/skills/canon",
            };
            let path = PathBuf::from(prefix).join(relative);
            hashes.insert(
                path.to_string_lossy().into_owned(),
                content_hash(&file.bytes),
            );
        }
        if let Some(skill) = files
            .iter()
            .find(|file| file.relative == Path::new("SKILL.src.md"))
        {
            let (_, description, body) =
                parse_frontmatter("canon", &String::from_utf8_lossy(&skill.bytes));
            if *provider == Provider::Codex {
                hashes.insert(
                    ".codex/skills/canon.md".into(),
                    content_hash(flatten_for_codex("canon", &description, &body).as_bytes()),
                );
            }
        }
    }
    CanonicalManifest {
        version: 1,
        source_hash,
        providers: providers.iter().map(|p| p.as_str().into()).collect(),
        files: hashes,
    }
}

fn project_path(target_dir: &Path, provider: Provider, relative: &Path) -> Option<PathBuf> {
    let relative = projected_relative(provider, relative)?;
    Some(match provider {
        Provider::Claude => target_dir.join(".claude/skills/canon").join(relative),
        Provider::Codex => target_dir.join(".codex/skills/canon").join(relative),
    })
}

pub fn install_canonical(
    source_dir: &Path,
    target_dir: &Path,
    providers: Option<&str>,
) -> Result<CanonicalInstallReport, SkillsError> {
    let selected = parse_providers(providers, target_dir)?;
    let files = canonical_bundle(source_dir)?;
    let manifest = manifest_for(&files, &selected);
    let previous = load_manifest(target_dir)?;
    let mut writes: Vec<(PathBuf, Vec<u8>)> = Vec::new();
    let source_skill = files
        .iter()
        .find(|file| file.relative == Path::new("SKILL.src.md"))
        .expect("canonical source exists");
    let (_, description, body) =
        parse_frontmatter("canon", &String::from_utf8_lossy(&source_skill.bytes));
    for provider in &selected {
        for file in &files {
            if let Some(path) = project_path(target_dir, *provider, &file.relative) {
                writes.push((path, file.bytes.clone()));
            }
        }
        if *provider == Provider::Codex {
            writes.push((
                target_dir.join(".codex/skills/canon.md"),
                flatten_for_codex("canon", &description, &body).into_bytes(),
            ));
        }
    }
    for (path, _) in &writes {
        reject_symlink_path_under(target_dir, path)?;
    }
    reject_symlink_path_under(target_dir, &manifest_path(target_dir))?;
    let changed = previous.as_ref() != Some(&manifest)
        || writes
            .iter()
            .any(|(path, bytes)| fs::read(path).ok().as_deref() != Some(bytes.as_slice()));
    if changed {
        for (path, bytes) in writes {
            write_bytes_under(target_dir, &path, &bytes)?;
        }
        let mut json =
            serde_json::to_string_pretty(&manifest).expect("manifest serialization is infallible");
        json.push('\n');
        write_file_under(target_dir, &manifest_path(target_dir), &json)?;
    }
    Ok(CanonicalInstallReport {
        providers: selected,
        source_hash: manifest.source_hash,
        changed,
    })
}

pub fn install(source_dir: &Path, target_dir: &Path) -> Result<InstallReport, SkillsError> {
    if source_dir.join("SKILL.src.md").is_file() {
        let report = install_canonical(source_dir, target_dir, None)?;
        let mut lock = Lock::default();
        lock.skills.insert(
            "canon".into(),
            LockEntry {
                content_hash: report.source_hash,
                version: 1,
            },
        );
        return Ok(InstallReport {
            installed: vec![InstalledSkill {
                name: "canon".into(),
                version: 1,
                changed: report.changed,
            }],
            lock,
        });
    }
    install_legacy(source_dir, target_dir)
}

pub fn check(
    source_dir: &Path,
    target_dir: &Path,
    providers: Option<&str>,
) -> Result<SkillsCheckReport, SkillsError> {
    let selected = parse_providers(providers, target_dir)?;
    let files = canonical_bundle(source_dir)?;
    let expected = manifest_for(&files, &selected);
    let actual = load_manifest(target_dir)?;
    let mut statuses = Vec::new();
    for provider in &selected {
        for file in &files {
            let Some(path) = project_path(target_dir, *provider, &file.relative) else {
                continue;
            };
            let state = if !path.is_file() {
                "missing"
            } else if fs::read(&path).ok().as_deref() != Some(file.bytes.as_slice()) {
                "stale"
            } else {
                "ok"
            };
            statuses.push(SkillStatus {
                provider: *provider,
                path,
                state,
            });
        }
        if *provider == Provider::Codex {
            let path = target_dir.join(".codex/skills/canon.md");
            let skill = files
                .iter()
                .find(|file| file.relative == Path::new("SKILL.src.md"))
                .unwrap();
            let (_, description, body) =
                parse_frontmatter("canon", &String::from_utf8_lossy(&skill.bytes));
            let expected_bytes = flatten_for_codex("canon", &description, &body).into_bytes();
            let state = if !path.is_file() {
                "missing"
            } else if fs::read(&path).ok().as_deref() != Some(expected_bytes.as_slice()) {
                "stale"
            } else {
                "ok"
            };
            statuses.push(SkillStatus {
                provider: *provider,
                path,
                state,
            });
        }
    }
    let manifest_ok = actual.as_ref() == Some(&expected);
    Ok(SkillsCheckReport {
        providers: selected,
        source_hash: expected.source_hash,
        statuses,
        manifest_ok,
    })
}

pub fn doctor(
    source_dir: &Path,
    target_dir: &Path,
    providers: Option<&str>,
) -> Result<Vec<String>, SkillsError> {
    let selected = parse_providers(providers, target_dir)?;
    let mut lines = vec![format!(
        "providers: {}",
        selected
            .iter()
            .map(|p| p.as_str())
            .collect::<Vec<_>>()
            .join(",")
    )];
    let report = match check(source_dir, target_dir, providers) {
        Ok(report) => report,
        Err(SkillsError::ManifestParse { path, source }) => {
            lines.push(format!("manifest-error: {} ({source})", path.display()));
            return Ok(lines);
        }
        Err(err) => return Err(err),
    };
    for status in report.statuses {
        if status.state != "ok" {
            lines.push(format!(
                "{}: {} ({})",
                status.state,
                status.path.display(),
                status.provider.as_str()
            ));
        }
    }
    if !report.manifest_ok {
        lines.push(format!("stale: {}", manifest_path(target_dir).display()));
    }
    for root in [
        target_dir.join(".claude/skills"),
        target_dir.join(".codex/skills"),
    ] {
        if let Ok(entries) = fs::read_dir(&root) {
            for entry in entries.flatten() {
                let name = entry.file_name().to_string_lossy().into_owned();
                if name.starts_with("canon-") {
                    lines.push(format!("legacy-remnant: {}", entry.path().display()));
                }
            }
        }
    }
    if lines.len() == 1 {
        lines.push("status: healthy".into());
    }
    Ok(lines)
}

pub fn resolve_source(explicit: Option<&Path>) -> PathBuf {
    explicit
        .map(Path::to_path_buf)
        .or_else(|| std::env::var_os(CANONICAL_SOURCE_ENV).map(PathBuf::from))
        .unwrap_or_else(|| PathBuf::from("canon/skills"))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn codex_flattening_removes_frontmatter() {
        let output = flatten_for_codex("canon", "desc", "body\n");
        assert_eq!(output, "# canon\n\n> desc\n\nbody\n");
        assert!(!output.contains("---"));
    }

    #[test]
    fn invalid_provider_is_rejected() {
        assert!(matches!(
            Provider::parse("gemini"),
            Err(SkillsError::InvalidProvider(_))
        ));
    }
}
