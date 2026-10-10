//! Canon skill bundle materialization.
//!
//! The canonical user-facing source is a read-only bundle rooted at
//! `SKILL.src.md`. It projects one directory-shaped `canon` skill
//! (`<root>/skills/canon/SKILL.md` plus `reference/**` and `scripts/**`
//! sidecars) to Claude (`.claude`), Codex (`.agents`), OMP (`.omp`), and Pi
//! (`.pi`). Directory-shaped sources (notably `canon/skills-dev`) retain the
//! legacy per-skill materializer for backwards compatibility.
//!
//! Canon 0.13.0 and earlier projected Codex to a flattened
//! `.codex/skills/canon.md` plus `.codex/skills/canon/**`, a directory Codex
//! never reads. Install removes exactly the files of that legacy projection
//! whose bytes still match the hash canon's own install lock recorded for
//! them; `check` and `doctor` report whatever remains as a remnant.

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
    Omp,
    Pi,
}

#[derive(Debug, Clone, Copy)]
struct ProviderDescriptor {
    name: &'static str,
    /// The provider's project skill root: canon projects to
    /// `<root>/skills/canon/`.
    root: &'static str,
    /// Directories whose presence selects the provider when `--providers`
    /// is omitted.
    markers: &'static [&'static str],
}

impl Provider {
    fn descriptor(self) -> ProviderDescriptor {
        match self {
            Self::Claude => ProviderDescriptor {
                name: "claude",
                root: ".claude",
                markers: &[".claude"],
            },
            // Codex discovers skills in `.agents/skills` from the working
            // directory up to the repository root (and `$HOME/.agents/skills`),
            // never in `.codex/skills`. `.agents` is therefore both the
            // projection root and a detection marker; `.codex` (Codex's own
            // config directory) still marks a repository as a Codex user.
            Self::Codex => ProviderDescriptor {
                name: "codex",
                root: ".agents",
                markers: &[".agents", ".codex"],
            },
            Self::Omp => ProviderDescriptor {
                name: "omp",
                root: ".omp",
                markers: &[".omp"],
            },
            Self::Pi => ProviderDescriptor {
                name: "pi",
                root: ".pi",
                markers: &[".pi"],
            },
        }
    }
}

impl Provider {
    pub fn as_str(self) -> &'static str {
        self.descriptor().name
    }

    pub fn parse(value: &str) -> Result<Self, SkillsError> {
        let normalized = value.trim().to_ascii_lowercase();
        ALL_PROVIDERS
            .into_iter()
            .find(|provider| provider.descriptor().name == normalized.as_str())
            .ok_or(SkillsError::InvalidProvider(normalized))
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
    /// Paths of a legacy `.codex/skills` canon projection still on disk.
    pub remnants: Vec<PathBuf>,
}

#[derive(Debug, thiserror::Error)]
pub enum SkillsError {
    #[error("source directory not found: {0}")]
    SourceNotFound(PathBuf),
    #[error("canonical skill source is missing SKILL.src.md under {0}")]
    CanonicalSourceMissing(PathBuf),
    #[error("invalid provider `{0}`; expected one of: claude, codex, omp, pi")]
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
        for provider in [Provider::Claude, Provider::Codex] {
            write_file_under(
                target_dir,
                &provider_skill_root(target_dir, provider)
                    .join(&skill.name)
                    .join("SKILL.md"),
                &skill.content,
            )?;
        }
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

const ALL_PROVIDERS: [Provider; 4] = [
    Provider::Claude,
    Provider::Codex,
    Provider::Omp,
    Provider::Pi,
];

fn provider_skill_root(target_dir: &Path, provider: Provider) -> PathBuf {
    target_dir.join(provider.descriptor().root).join("skills")
}

fn provider_bundle_root(target_dir: &Path, provider: Provider) -> PathBuf {
    provider_skill_root(target_dir, provider).join("canon")
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
        let detected = ALL_PROVIDERS
            .into_iter()
            .map(|provider| {
                let markers = provider.descriptor().markers;
                (
                    markers.iter().any(|marker| target_dir.join(marker).exists()),
                    provider,
                )
            })
            .collect::<Vec<_>>();
        if detected.iter().all(|(exists, _)| !exists) {
            vec![Provider::Claude, Provider::Codex]
        } else {
            detected
                .into_iter()
                .filter_map(|(exists, provider)| exists.then_some(provider))
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

/// Where a bundle file lands inside `<root>/skills/canon/`.
fn projected_relative(relative: &Path) -> PathBuf {
    if relative == Path::new("SKILL.src.md") {
        PathBuf::from("SKILL.md")
    } else {
        relative.to_path_buf()
    }
}

fn manifest_for(files: &[BundleFile], providers: &[Provider]) -> CanonicalManifest {
    let source_hash = bundle_hash(files);
    let mut hashes = BTreeMap::new();
    for provider in providers {
        for file in files {
            let path = Path::new(provider.descriptor().root)
                .join("skills/canon")
                .join(projected_relative(&file.relative));
            hashes.insert(
                path.to_string_lossy().into_owned(),
                content_hash(&file.bytes),
            );
        }
    }
    CanonicalManifest {
        version: 1,
        source_hash,
        providers: providers.iter().map(|p| p.as_str().into()).collect(),
        files: hashes,
    }
}

fn project_path(target_dir: &Path, provider: Provider, relative: &Path) -> PathBuf {
    provider_bundle_root(target_dir, provider).join(projected_relative(relative))
}

/// The skill root canon 0.13.0 and earlier projected Codex into. Codex never
/// reads it.
const LEGACY_CODEX_SKILLS: &str = ".codex/skills";
/// The legacy flattened Codex entrypoint.
const LEGACY_CODEX_ENTRYPOINT: &str = ".codex/skills/canon.md";
/// The legacy Codex sidecar directory (`reference/**`, `scripts/**`).
const LEGACY_CODEX_BUNDLE: &str = ".codex/skills/canon";

/// Whether a manifest key names a file of the legacy Codex projection.
/// Keys come from a file in the target repository, so anything but plain
/// path components is refused.
fn is_legacy_codex_key(key: &str) -> bool {
    let in_projection = key == LEGACY_CODEX_ENTRYPOINT
        || key
            .strip_prefix(LEGACY_CODEX_BUNDLE)
            .is_some_and(|rest| rest.starts_with('/'));
    in_projection
        && Path::new(key)
            .components()
            .all(|component| matches!(component, std::path::Component::Normal(_)))
}

/// The legacy Codex files canon's previous install lock proves canon wrote,
/// as `(key, recorded hash)`: recorded under the legacy projection, and still
/// holding exactly the bytes whose hash the lock recorded. An edited,
/// replaced, symlinked, or unrecorded file is never selected.
fn legacy_codex_removals<'m>(
    target_dir: &Path,
    previous: Option<&'m CanonicalManifest>,
) -> Vec<(&'m str, &'m str)> {
    let Some(previous) = previous else {
        return Vec::new();
    };
    previous
        .files
        .iter()
        .filter(|(key, _)| is_legacy_codex_key(key))
        .filter(|(key, hash)| {
            legacy_fs::read(target_dir, key).is_some_and(|bytes| content_hash(&bytes) == **hash)
        })
        .map(|(key, hash)| (key.as_str(), hash.as_str()))
        .collect()
}

/// Removes the proven legacy Codex files, then each directory they left
/// empty, up to and including `.codex/skills`. `.codex` itself, any
/// non-empty directory, and every file canon cannot prove it wrote stay.
/// Each file's hash is checked again at removal time; one that changed since
/// [`legacy_codex_removals`] is kept.
fn remove_legacy_codex(target_dir: &Path, removals: &[(&str, &str)]) -> Result<(), SkillsError> {
    let mut dirs: Vec<&str> = Vec::new();
    for (key, hash) in removals {
        legacy_fs::remove_file_if_hash(target_dir, key, hash)?;
        let mut current = *key;
        while let Some((parent, _)) = current.rsplit_once('/') {
            if parent.len() < LEGACY_CODEX_SKILLS.len() {
                break;
            }
            dirs.push(parent);
            current = parent;
        }
    }
    // Deepest first, so a parent is only removed after its children; the
    // key tie-break makes duplicates adjacent for `dedup`.
    dirs.sort_by(|a, b| {
        b.matches('/')
            .count()
            .cmp(&a.matches('/').count())
            .then_with(|| a.cmp(b))
    });
    dirs.dedup();
    for dir in dirs {
        legacy_fs::remove_dir_if_empty(target_dir, dir)?;
    }
    Ok(())
}

/// Filesystem access for the legacy Codex migration. Keys are
/// [`is_legacy_codex_key`]-validated `/`-separated relative paths.
///
/// On unix every component below `target_dir` is opened with `O_NOFOLLOW`,
/// and the file is hashed and unlinked relative to its parent's directory
/// handle, so replacing `.codex`, `skills`, or `canon` with a symlink, before
/// or during the migration, can never make it read or delete a file outside
/// the target.
#[cfg(unix)]
mod legacy_fs {
    use std::io::Read;
    use std::os::fd::OwnedFd;
    use std::path::Path;

    use rustix::fs::{openat, unlinkat, AtFlags, Mode, OFlags, CWD};
    use rustix::io::Errno;

    use super::{content_hash, SkillsError};

    fn io_error(target_dir: &Path, key: &str, errno: Errno) -> SkillsError {
        SkillsError::Io {
            path: target_dir.join(key),
            source: errno.into(),
        }
    }

    /// The directory holding `key`'s last component, reached without
    /// following a symlink below `target_dir`, and that component's name.
    /// `None` when any component is missing, a symlink, or not a directory.
    fn open_parent<'k>(target_dir: &Path, key: &'k str) -> Option<(OwnedFd, &'k str)> {
        let (dirs, name) = key.rsplit_once('/')?;
        let flags = OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC;
        let mut dir = openat(CWD, target_dir, flags, Mode::empty()).ok()?;
        for component in dirs.split('/') {
            dir = openat(&dir, component, flags | OFlags::NOFOLLOW, Mode::empty()).ok()?;
        }
        Some((dir, name))
    }

    /// The bytes of the regular file `name` in `dir`; `None` for a symlink,
    /// a non-regular file, or any read failure.
    fn read_in(dir: &OwnedFd, name: &str) -> Option<Vec<u8>> {
        let flags = OFlags::RDONLY | OFlags::NOFOLLOW | OFlags::NONBLOCK | OFlags::CLOEXEC;
        let mut file = std::fs::File::from(openat(dir, name, flags, Mode::empty()).ok()?);
        if !file.metadata().ok()?.is_file() {
            return None;
        }
        let mut bytes = Vec::new();
        file.read_to_end(&mut bytes).ok()?;
        Some(bytes)
    }

    pub(super) fn read(target_dir: &Path, key: &str) -> Option<Vec<u8>> {
        let (dir, name) = open_parent(target_dir, key)?;
        read_in(&dir, name)
    }

    pub(super) fn remove_file_if_hash(target_dir: &Path, key: &str, hash: &str) -> Result<(), SkillsError> {
        let Some((dir, name)) = open_parent(target_dir, key) else {
            return Ok(());
        };
        if read_in(&dir, name).is_none_or(|bytes| content_hash(&bytes) != hash) {
            return Ok(());
        }
        match unlinkat(&dir, name, AtFlags::empty()) {
            Ok(()) => Ok(()),
            Err(errno) if errno == Errno::NOENT => Ok(()),
            Err(errno) => Err(io_error(target_dir, key, errno)),
        }
    }

    /// `rmdir` relative to the parent's handle: it never follows a symlink
    /// and refuses a non-empty directory atomically.
    pub(super) fn remove_dir_if_empty(target_dir: &Path, key: &str) -> Result<(), SkillsError> {
        let Some((dir, name)) = open_parent(target_dir, key) else {
            return Ok(());
        };
        match unlinkat(&dir, name, AtFlags::REMOVEDIR) {
            Ok(()) => Ok(()),
            Err(errno)
                if [Errno::NOENT, Errno::NOTEMPTY, Errno::EXIST, Errno::NOTDIR].contains(&errno) =>
            {
                Ok(())
            }
            Err(errno) => Err(io_error(target_dir, key, errno)),
        }
    }
}

/// Non-unix fallback. LIMITATION: without no-follow directory handles, each
/// operation checks the path for symlinks and then acts on it by path, so a
/// component replaced by a symlink or junction between the check and the
/// read, unlink, or rmdir is followed. The check narrows that window; it does
/// not close it.
#[cfg(not(unix))]
mod legacy_fs {
    use std::fs;
    use std::path::Path;

    use super::{content_hash, reject_symlink_path_under, SkillsError};

    pub(super) fn read(target_dir: &Path, key: &str) -> Option<Vec<u8>> {
        let path = target_dir.join(key);
        reject_symlink_path_under(target_dir, &path).ok()?;
        fs::symlink_metadata(&path).ok()?.is_file().then_some(())?;
        fs::read(&path).ok()
    }

    pub(super) fn remove_file_if_hash(target_dir: &Path, key: &str, hash: &str) -> Result<(), SkillsError> {
        if read(target_dir, key).is_none_or(|bytes| content_hash(&bytes) != hash) {
            return Ok(());
        }
        let path = target_dir.join(key);
        fs::remove_file(&path).map_err(|source| SkillsError::Io { path, source })
    }

    pub(super) fn remove_dir_if_empty(target_dir: &Path, key: &str) -> Result<(), SkillsError> {
        let path = target_dir.join(key);
        if reject_symlink_path_under(target_dir, &path).is_err()
            || !fs::symlink_metadata(&path).is_ok_and(|metadata| metadata.is_dir())
        {
            return Ok(());
        }
        let empty = fs::read_dir(&path)
            .map_err(|source| SkillsError::Io {
                path: path.clone(),
                source,
            })?
            .next()
            .is_none();
        if empty {
            fs::remove_dir(&path).map_err(|source| SkillsError::Io { path, source })?;
        }
        Ok(())
    }
}

/// Legacy Codex projection paths still present in `target_dir`.
fn legacy_codex_remnants(target_dir: &Path) -> Vec<PathBuf> {
    [LEGACY_CODEX_ENTRYPOINT, LEGACY_CODEX_BUNDLE]
        .into_iter()
        .map(|relative| target_dir.join(relative))
        .filter(|path| fs::symlink_metadata(path).is_ok())
        .collect()
}

/// The command that migrates a legacy Codex projection for `providers`.
pub fn legacy_codex_fix(providers: &[Provider]) -> String {
    format!(
        "canon skills install --providers={}",
        providers
            .iter()
            .map(|p| p.as_str())
            .collect::<Vec<_>>()
            .join(",")
    )
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
    for provider in &selected {
        for file in &files {
            writes.push((
                project_path(target_dir, *provider, &file.relative),
                file.bytes.clone(),
            ));
        }
    }
    for (path, _) in &writes {
        reject_symlink_path_under(target_dir, path)?;
    }
    reject_symlink_path_under(target_dir, &manifest_path(target_dir))?;
    let removals = legacy_codex_removals(target_dir, previous.as_ref());
    let changed = previous.as_ref() != Some(&manifest)
        || !removals.is_empty()
        || writes
            .iter()
            .any(|(path, bytes)| fs::read(path).ok().as_deref() != Some(bytes.as_slice()));
    if changed {
        for (path, bytes) in writes {
            write_bytes_under(target_dir, &path, &bytes)?;
        }
        remove_legacy_codex(target_dir, &removals)?;
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
            let path = project_path(target_dir, *provider, &file.relative);
            let expected_bytes = file.bytes.as_slice();
            let state = if !path.is_file() {
                "missing"
            } else if fs::read(&path).ok().as_deref() != Some(expected_bytes) {
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
        remnants: legacy_codex_remnants(target_dir),
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
    for remnant in &report.remnants {
        lines.push(format!(
            "legacy-remnant: {} (codex reads .agents/skills, never .codex/skills; fix: `{}`, which removes what canon's install lock proves it wrote; remove anything it keeps by hand)",
            remnant.display(),
            legacy_codex_fix(&report.providers)
        ));
    }
    let roots = ALL_PROVIDERS
        .into_iter()
        .map(|provider| provider_skill_root(target_dir, provider))
        .chain([target_dir.join(LEGACY_CODEX_SKILLS)]);
    for root in roots {
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
    fn legacy_codex_keys_are_confined_to_the_legacy_projection() {
        assert!(is_legacy_codex_key(".codex/skills/canon.md"));
        assert!(is_legacy_codex_key(".codex/skills/canon/reference/topic.md"));
        assert!(!is_legacy_codex_key(".codex/skills/canonical.md"));
        assert!(!is_legacy_codex_key(".codex/skills/canon-old.md"));
        assert!(!is_legacy_codex_key(".codex/skills/user.md"));
        assert!(!is_legacy_codex_key(".codex/skills/canon/../../../etc/passwd"));
        assert!(!is_legacy_codex_key(".claude/skills/canon/SKILL.md"));
    }

    /// The race the no-follow walk closes: the removal set is proven, then a
    /// legacy directory is swapped for a symlink to an outside copy before
    /// the unlink. Removing by path would follow it and delete the copy.
    #[cfg(unix)]
    #[test]
    fn legacy_codex_removal_never_follows_a_dir_swapped_after_the_proof() {
        let key = ".codex/skills/canon/reference/topic.md";
        for swapped in [".codex/skills/canon", ".codex/skills", ".codex"] {
            let tmp = tempfile::tempdir().unwrap();
            let target = tmp.path().join("target");
            let outside = tmp.path().join("outside");
            fs::create_dir_all(target.join(".codex/skills/canon/reference")).unwrap();
            fs::write(target.join(key), "# topic\n").unwrap();
            let manifest = CanonicalManifest {
                version: 1,
                source_hash: "sha256:legacy".into(),
                providers: vec!["codex".into()],
                files: BTreeMap::from([(key.to_string(), content_hash(b"# topic\n"))]),
            };
            let removals = legacy_codex_removals(&target, Some(&manifest));
            assert_eq!(removals, vec![(key, manifest.files[key].as_str())], "{swapped}");

            fs::create_dir_all(&outside).unwrap();
            let moved = outside.join("moved");
            fs::rename(target.join(swapped), &moved).unwrap();
            std::os::unix::fs::symlink(&moved, target.join(swapped)).unwrap();
            remove_legacy_codex(&target, &removals).unwrap();

            let outside_file = moved.join(key.strip_prefix(swapped).unwrap().trim_start_matches('/'));
            assert!(outside_file.is_file(), "swapping {swapped} let the unlink follow it");
            assert!(fs::symlink_metadata(target.join(swapped)).unwrap().file_type().is_symlink());
        }
    }

    #[test]
    fn invalid_provider_is_rejected() {
        assert!(matches!(
            Provider::parse("gemini"),
            Err(SkillsError::InvalidProvider(_))
        ));
    }
}
