//! Canon skill materialization.
//!
//! The guide lives in the binary (0.28 D10, [`crate::guide`]): `canon skills
//! show` prints it. What `install` projects is a short stub `SKILL.md`
//! (frontmatter from `SKILL.src.md`, a body that carries the canon version and
//! points at `canon skills show canon`) plus the `scripts/**` sidecar, as one
//! directory-shaped `canon` skill (`<root>/skills/canon/`) for Claude
//! (`.claude`), Codex (`.agents`), OMP (`.omp`), and Pi (`.pi`). `reference/**`
//! is never projected; a 0.27 projection's reference files are orphans the lock
//! lists, pruned when their bytes still match it ([`find_orphans`]). The source
//! is the text embedded in the binary; `--skills-source` or
//! `CANON_SKILLS_SOURCE` names a developer override directory
//! ([`resolve_source`]). Directory-shaped sources without `SKILL.src.md`
//! (notably `canon/skills-dev`) retain the legacy per-skill materializer.
//!
//! Canon 0.13.0 and earlier projected Codex to a flattened
//! `.codex/skills/canon.md` plus `.codex/skills/canon/**`, a directory Codex
//! never reads. Install removes exactly the files of that legacy projection
//! whose bytes still match the hash canon's own install lock recorded for
//! them; `check` and `doctor` report whatever remains as a remnant.
//!
//! The install lock (`.canon/skills/.install-lock.json`) records the canon
//! version that wrote it and, when the source is in a git checkout, that
//! checkout's `HEAD` (0.16, #9). `check` and `doctor` name a lock written by
//! another canon, or by a canon before 0.16 that recorded no version, and
//! give the install command that rewrites it. `doctor` also lists other
//! skills in the provider skill roots whose description claims build work
//! (#10, [`build_claims`]); it only reports them.
//!
//! Which providers `install`, `check` and `doctor` serve (0.17 D5,
//! [`select_providers`]): an explicit `--providers`, else `canon.yaml`'s
//! `skills: { providers: [...] }`, else the providers the install lock
//! records (a repo from before 0.17), else the harnesses whose marker
//! directory (`.claude`, `.codex`, `.omp`, `.pi`) exists. Nothing is ever
//! added by default: with no marker, nothing is selected and the commands
//! say how to choose. `install --providers` also rewrites
//! `skills.providers` in an existing `canon.yaml`. `doctor` names a canon
//! projection directory left behind for a provider that is not selected.
//!
//! Formatters (0.18, #25): `install` and `doctor` print the globs of every
//! path canon writes ([`owned_path_globs`]) for a formatter's or linter's
//! ignore list. A projected file whose bytes differ but whose content
//! matches after normalizing only what formatters change is
//! [`FORMATTER_REWRITTEN`], not `stale`, and `doctor` names the ignore
//! entry that keeps the formatter off it.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use canon_model::paths;

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
    /// Directories whose presence selects the provider when neither
    /// `--providers`, `canon.yaml` `skills.providers` nor an install lock
    /// names the providers.
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
            // never in `.codex/skills`, so `.agents` is the projection root.
            // Only `.codex` (Codex's own config directory) marks a
            // repository as a Codex user: other tools use `.agents` too
            // (0.17 D5).
            Self::Codex => ProviderDescriptor {
                name: "codex",
                root: ".agents",
                markers: &[".codex"],
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

    /// The directory whose presence detects this provider.
    pub fn marker(self) -> &'static str {
        self.descriptor().markers[0]
    }

    pub fn parse(value: &str) -> Result<Self, SkillsError> {
        let normalized = value.trim().to_ascii_lowercase();
        ALL_PROVIDERS
            .into_iter()
            .find(|provider| provider.descriptor().name == normalized.as_str())
            .ok_or(SkillsError::InvalidProvider(normalized))
    }
}

/// Where [`select_providers`] took the providers from (0.17 D5).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ProviderOrigin {
    /// An explicit `--providers`.
    Flag,
    /// `canon.yaml`'s `skills.providers`.
    Config,
    /// The install lock's providers: a repo whose `canon.yaml` has no
    /// `skills:` key (written before 0.17).
    Lock,
    /// The provider marker directories present in the target.
    Detected,
}

/// The providers a skills command serves, and where they came from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProviderSelection {
    pub providers: Vec<Provider>,
    pub origin: ProviderOrigin,
}

impl ProviderSelection {
    /// "from canon.yaml skills.providers", "detected: .claude, .omp", ….
    pub fn describe(&self) -> String {
        match self.origin {
            ProviderOrigin::Flag => "from --providers".into(),
            ProviderOrigin::Config => "from canon.yaml skills.providers".into(),
            ProviderOrigin::Lock => "from the install lock; canon.yaml has no skills: key".into(),
            ProviderOrigin::Detected if self.providers.is_empty() => {
                format!("no {MARKER_LIST} directory found")
            }
            ProviderOrigin::Detected => format!(
                "detected: {}",
                self.providers
                    .iter()
                    .flat_map(|p| p.descriptor().markers.iter().copied())
                    .collect::<Vec<_>>()
                    .join(", ")
            ),
        }
    }

    /// The provider names, comma-separated, or `none`.
    pub fn names(&self) -> String {
        provider_names(&self.providers)
    }
}

/// The marker directories, in prose.
const MARKER_LIST: &str = ".claude, .codex, .omp or .pi";

/// How to choose providers when none is selected.
pub const CHOOSE_PROVIDERS: &str =
    "choose with `canon skills install --providers=<list>` (any of claude, codex, omp, pi)";

/// `claude,omp`, or `none` for an empty list.
pub fn provider_names(providers: &[Provider]) -> String {
    if providers.is_empty() {
        return "none".into();
    }
    providers.iter().map(|p| p.as_str()).collect::<Vec<_>>().join(",")
}

/// A projected file the install lock lists and the bundle no longer ships
/// (0.25, #99).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Orphan {
    pub provider: Provider,
    /// Where the file sits, under the target.
    pub path: PathBuf,
    /// `false` when its bytes still match the hash the lock recorded: install
    /// removes it. `true` when they differ (or it is not a regular file):
    /// install keeps it.
    pub modified: bool,
}

impl Orphan {
    /// What the install lock says about its bytes.
    pub fn reason(&self) -> &'static str {
        if self.modified {
            "its bytes differ from the install lock (or it is not a regular file)"
        } else {
            "its bytes still match the install lock"
        }
    }

    /// What to do about it by hand.
    pub fn by_hand(&self) -> String {
        format!("remove it by hand with `rm {}` if you do not need it", self.path.display())
    }

    /// What to do about it, as `check` and `doctor` say it.
    pub fn fix(&self, providers: &[Provider]) -> String {
        if self.modified {
            format!("`canon skills install` keeps it; {}", self.by_hand())
        } else {
            format!("run `{}`, which removes it", install_command(providers))
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CanonicalInstallReport {
    pub providers: Vec<Provider>,
    pub origin: ProviderOrigin,
    pub source_hash: String,
    pub changed: bool,
    /// The `canon.yaml` whose `skills.providers` an explicit `--providers`
    /// rewrote; `None` when it already held that list or does not exist.
    pub config_written: Option<PathBuf>,
    /// Files the previous lock listed and the new bundle no longer ships,
    /// whose bytes still matched the lock: removed.
    pub pruned: Vec<Orphan>,
    /// The same, whose bytes differed: kept.
    pub kept: Vec<Orphan>,
    /// The projected files whose bytes differ from the bundle (or are missing): what an install writes.
    pub rewritten: Vec<PathBuf>,
    /// Whether the install lock differs from the one this install writes.
    pub lock_changed: bool,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillStatus {
    pub provider: Provider,
    pub path: PathBuf,
    /// `ok`, `missing`, `stale` (real drift) or [`FORMATTER_REWRITTEN`].
    pub state: &'static str,
}

/// A projected file whose bytes differ from the bundle but whose
/// content does not: a formatter rewrote it (0.18, #25). It still
/// fails `check`; `doctor` names the ignore entry to add.
pub const FORMATTER_REWRITTEN: &str = "formatter-rewritten";

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct SkillsCheckOutcome {
    pub providers: Vec<Provider>,
    pub origin: ProviderOrigin,
    pub source_hash: String,
    pub statuses: Vec<SkillStatus>,
    pub manifest_ok: bool,
    /// Files the install lock lists that the bundle no longer ships.
    pub orphans: Vec<Orphan>,
    /// Lock keys inside a canon projection that are not plain relative
    /// paths; ignored, never acted on.
    pub invalid_entries: Vec<String>,
    /// Paths of a legacy `.codex/skills` canon projection still on disk.
    pub remnants: Vec<PathBuf>,
    /// Set when the install lock exists but was written by a canon other
    /// than this one.
    pub outdated: Option<OutdatedLock>,
}

/// An install lock another canon wrote (0.16, #9).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct OutdatedLock {
    /// The lock's path.
    pub path: PathBuf,
    /// The canon version the lock records; `None` for a lock written
    /// before 0.16, which records none.
    pub installed_by: Option<String>,
}

impl OutdatedLock {
    /// "installed by X, this canon is Y — run `canon skills install …`".
    pub fn describe(&self, providers: &[Provider]) -> String {
        let by = match &self.installed_by {
            Some(version) => format!("canon {version}"),
            None => "an unknown canon version (the lock predates 0.16)".to_string(),
        };
        format!("installed by {by}, this canon is {CANON_VERSION} — run `{}`", install_command(providers))
    }
}

/// The version of this canon, recorded in every install lock it writes.
pub const CANON_VERSION: &str = env!("CARGO_PKG_VERSION");

#[derive(Debug, thiserror::Error)]
pub enum SkillsError {
    #[error("source directory not found: {0}")]
    SourceNotFound(PathBuf),
    #[error("canonical skill source is missing SKILL.src.md under {0}")]
    CanonicalSourceMissing(PathBuf),
    #[error("invalid provider `{0}`; expected one of: claude, codex, omp, pi, or none")]
    InvalidProvider(String),
    #[error("{0}")]
    Config(String),
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
    #[expect(clippy::map_err_ignore, reason = "`StripPrefixError` only says `path` is not under `target_dir`; the refusal names `path` and its message has no slot for a cause")]
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
        if !entry.file_type().is_ok_and(|t| t.is_dir()) {
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

fn install_legacy(source_dir: &Path, target_dir: &Path, dry_run: bool) -> Result<InstallReport, SkillsError> {
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
        for provider in [Provider::Claude, Provider::Codex].into_iter().filter(|_| !dry_run) {
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
    if !dry_run {
        write_lock(source_dir, &lock)?;
    }
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

/// The files `skills install` projects next to nothing else (0.28 D10): the stub
/// `SKILL.md` (written from `SKILL.src.md`, whose frontmatter it keeps, and
/// this canon's version) and the `scripts/**` sidecar. The guide itself is not
/// projected: `canon skills show` prints it from the binary. `source` is the
/// developer override directory; `None` reads the text embedded in the binary.
fn canonical_bundle(source: Option<&Path>) -> Result<Vec<BundleFile>, SkillsError> {
    let mut files = Vec::new();
    match source {
        None => {
            files.push(BundleFile { relative: PathBuf::from("SKILL.src.md"), bytes: crate::guide::stub(crate::guide::core(), CANON_VERSION).into_bytes() });
            files.extend(crate::guide::scripts().map(|(path, text)| BundleFile { relative: PathBuf::from(path), bytes: text.as_bytes().to_vec() }));
        }
        Some(source_dir) => {
            let root = source_dir.join("SKILL.src.md");
            if !root.is_file() {
                return Err(SkillsError::CanonicalSourceMissing(source_dir.to_path_buf()));
            }
            let text = read_to_string(&root)?;
            files.push(BundleFile { relative: PathBuf::from("SKILL.src.md"), bytes: crate::guide::stub(&text, CANON_VERSION).into_bytes() });
            collect_bundle_files(source_dir, Path::new("scripts"), &mut files)?;
        }
    }
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

/// Every provider, in stable order.
pub const ALL_PROVIDERS: [Provider; 4] = [
    Provider::Claude,
    Provider::Codex,
    Provider::Omp,
    Provider::Pi,
];

fn provider_skill_root(target_dir: &Path, provider: Provider) -> PathBuf {
    target_dir.join(provider.descriptor().root).join("skills")
}

/// `<target_dir>/<root>/skills/canon`: where `provider`'s projection lives.
pub fn provider_bundle_root(target_dir: &Path, provider: Provider) -> PathBuf {
    provider_skill_root(target_dir, provider).join("canon")
}

/// A `--providers` value: a comma-separated list of provider names, or
/// `none` (or an empty value) for none. Sorted, without duplicates.
pub fn parse_provider_list(value: &str) -> Result<Vec<Provider>, SkillsError> {
    let trimmed = value.trim();
    if trimmed.is_empty() || trimmed.eq_ignore_ascii_case("none") {
        return Ok(Vec::new());
    }
    let mut providers = Vec::new();
    for part in trimmed.split(',').filter(|part| !part.trim().is_empty()) {
        providers.push(Provider::parse(part)?);
    }
    if providers.is_empty() {
        return Err(SkillsError::InvalidProvider(value.to_string()));
    }
    providers.sort();
    providers.dedup();
    Ok(providers)
}

/// The providers whose marker directory (`.claude`, `.codex`, `.omp`,
/// `.pi`) exists in `target_dir`, in stable order. `.agents` is not a
/// marker: other tools use it too (0.17 D5).
pub fn detected_providers(target_dir: &Path) -> Vec<Provider> {
    ALL_PROVIDERS
        .into_iter()
        .filter(|provider| provider.descriptor().markers.iter().any(|marker| target_dir.join(marker).is_dir()))
        .collect()
}

/// `canon.yaml`'s `skills:` section, strict once present (0.17 D5).
#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct RawSkillsSection {
    providers: Vec<String>,
}

/// The shape a `skills:` section must have, for diagnostics.
const SKILLS_SECTION_SHAPE: &str = "`skills: { providers: [claude, codex, omp, pi] }`, any subset, `[]` for none";

fn configured_providers_in(text: &str) -> Result<Option<Vec<Provider>>, String> {
    let doc: serde_yaml::Value =
        serde_yaml::from_str(text).map_err(|e| format!("canon.yaml is not valid YAML: {e}"))?;
    let Some(section) = doc.get("skills") else {
        return Ok(None);
    };
    let raw: RawSkillsSection = serde_yaml::from_value(section.clone())
        .map_err(|e| format!("canon.yaml `skills:` section is malformed (expected {SKILLS_SECTION_SHAPE}): {e}"))?;
    let mut providers = Vec::new();
    for name in &raw.providers {
        let provider = ALL_PROVIDERS
            .into_iter()
            .find(|provider| provider.as_str() == name)
            .ok_or_else(|| format!("canon.yaml `skills.providers` names `{name}`; expected any of claude, codex, omp, pi"))?;
        providers.push(provider);
    }
    providers.sort();
    providers.dedup();
    Ok(Some(providers))
}

/// `skills.providers` from the `canon.yaml` at `canon_yaml`: `None` when
/// the file or its `skills:` key is absent. A present section that is not
/// [`SKILLS_SECTION_SHAPE`], or names an unknown provider, fails loud.
pub fn configured_providers(canon_yaml: &Path) -> Result<Option<Vec<Provider>>, String> {
    let Ok(text) = fs::read_to_string(canon_yaml) else {
        return Ok(None);
    };
    configured_providers_in(&text)
}

/// The comment `canon init` and [`rewrite_skills_section`] put above a
/// `skills:` section.
pub const SKILLS_CONFIG_COMMENT: &str = "\
# Harnesses the canon skill is installed for: any of claude, codex, omp, pi
# (`[]` for none). `canon skills install|check|doctor` follow this list;
# `canon skills install --providers=<list>` rewrites it.
";

/// `skills:\n  providers: [claude, omp]\n`.
pub fn skills_config_block(providers: &[Provider]) -> String {
    format!(
        "skills:\n  providers: [{}]\n",
        providers.iter().map(|p| p.as_str()).collect::<Vec<_>>().join(", ")
    )
}

/// `text` with its top-level `skills:` section replaced by
/// [`skills_config_block`] (appended under [`SKILLS_CONFIG_COMMENT`] when
/// absent). Every other byte is kept, and the inserted lines end with the
/// file's own newline (`\r\n` when the file uses it, else `\n`). The
/// section is the `skills:` line and the indented lines after it; a blank
/// line belongs to it only when an indented line follows. The result is
/// refused unless it parses to exactly `providers` with every other key
/// unchanged.
pub(crate) fn rewrite_skills_section(text: &str, providers: &[Provider]) -> Result<String, String> {
    let newline = if text.contains("\r\n") { "\r\n" } else { "\n" };
    let block = skills_config_block(providers).replace('\n', newline);
    let lines: Vec<&str> = text.split_inclusive('\n').collect();
    let rewritten = match lines.iter().position(|line| line.starts_with("skills:")) {
        Some(start) => {
            let mut end = start + 1;
            let mut cursor = start + 1;
            while let Some(line) = lines.get(cursor) {
                if line.trim().is_empty() {
                    cursor += 1;
                } else if line.starts_with(' ') || line.starts_with('\t') {
                    cursor += 1;
                    end = cursor;
                } else {
                    break;
                }
            }
            format!("{}{block}{}", lines[..start].concat(), lines[end..].concat())
        }
        None => {
            let mut out = text.to_string();
            if !out.is_empty() {
                if !out.ends_with('\n') {
                    out.push_str(newline);
                }
                out.push_str(newline);
            }
            out.push_str(&SKILLS_CONFIG_COMMENT.replace('\n', newline));
            out.push_str(&block);
            out
        }
    };
    let refuse = || {
        format!(
            "could not rewrite the `skills:` section of canon.yaml safely; set it by hand to `skills: {{ providers: [{}] }}`",
            providers.iter().map(|p| p.as_str()).collect::<Vec<_>>().join(", ")
        )
    };
    let without_skills = |text: &str| -> Option<serde_yaml::Value> {
        let mut doc: serde_yaml::Value = serde_yaml::from_str(text).ok()?;
        if let Some(map) = doc.as_mapping_mut() {
            map.remove("skills");
        }
        Some(doc)
    };
    let rest_kept = matches!((without_skills(text), without_skills(&rewritten)), (Some(before), Some(after)) if before == after);
    if !rest_kept || configured_providers_in(&rewritten).ok().flatten().as_deref() != Some(providers) {
        return Err(refuse());
    }
    Ok(rewritten)
}

/// The `canon.yaml` in `target_dir` and its text with `skills.providers`
/// set to `providers`; `None` when there is no `canon.yaml` or it already
/// holds exactly that list.
fn planned_config(target_dir: &Path, providers: &[Provider]) -> Result<Option<(PathBuf, String)>, SkillsError> {
    let path = target_dir.join("canon.yaml");
    if !path.is_file() {
        return Ok(None);
    }
    let text = read_to_string(&path)?;
    if configured_providers_in(&text).ok().flatten().as_deref() == Some(providers) {
        return Ok(None);
    }
    let rewritten = rewrite_skills_section(&text, providers).map_err(SkillsError::Config)?;
    Ok(Some((path, rewritten)))
}

/// The providers a skills command serves (0.17 D5), first match wins:
/// an explicit `--providers`; `canon.yaml`'s `skills.providers`; the
/// providers the install lock records (a repo set up before 0.17); the
/// marker directories present. Nothing undetected is ever added, so the
/// result may be empty.
pub fn select_providers(target_dir: &Path, flag: Option<&str>) -> Result<ProviderSelection, SkillsError> {
    if let Some(value) = flag {
        return Ok(ProviderSelection { providers: parse_provider_list(value)?, origin: ProviderOrigin::Flag });
    }
    if let Some(providers) = configured_providers(&target_dir.join("canon.yaml")).map_err(SkillsError::Config)? {
        return Ok(ProviderSelection { providers, origin: ProviderOrigin::Config });
    }
    // A lock canon cannot parse leaves the choice to detection; `check`
    // and `doctor` still report the lock itself.
    if let Some(manifest) = load_manifest(target_dir).ok().flatten() {
        let mut providers = Vec::new();
        for name in &manifest.providers {
            providers.push(Provider::parse(name)?);
        }
        providers.sort();
        providers.dedup();
        return Ok(ProviderSelection { providers, origin: ProviderOrigin::Lock });
    }
    Ok(ProviderSelection { providers: detected_providers(target_dir), origin: ProviderOrigin::Detected })
}

#[derive(Debug, Clone, Serialize, Deserialize, PartialEq, Eq)]
struct CanonicalManifest {
    version: u64,
    /// The canon that wrote the lock (0.16, #9). Absent in a lock written
    /// before 0.16.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    canon_version: Option<String>,
    /// `HEAD` of the git checkout the source was in when the projection
    /// was last written; absent when the source is not in one.
    /// Provenance only: never compared, so a new commit that leaves the
    /// bundle unchanged does not rewrite the lock.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    source_commit: Option<String>,
    source_hash: String,
    providers: Vec<String>,
    files: BTreeMap<String, String>,
    /// Files of providers an explicit `--providers` install did not select
    /// (0.25): their projections are untouched, so the lock keeps what it
    /// recorded for them, and a later install for that provider still
    /// prunes what its bundle dropped. Never part of the projection.
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    retained_files: BTreeMap<String, String>,
}

impl CanonicalManifest {
    /// Whether both record the same projection: the same bundle, providers
    /// and file hashes, whichever canon wrote them.
    fn same_projection(&self, other: &Self) -> bool {
        self.version == other.version && self.source_hash == other.source_hash && self.providers == other.providers && self.files == other.files
    }
}

/// `HEAD` of the git checkout the override `source` is in, if it is in one;
/// `None` for the embedded text.
fn source_commit(source: Option<&Path>) -> Option<String> {
    let source_dir = source?;
    let output = std::process::Command::new("git")
        .arg("-C")
        .arg(source_dir)
        .args(["rev-parse", "--verify", "--quiet", "HEAD^{commit}"])
        .output()
        .ok()?;
    let commit = String::from_utf8(output.stdout).ok()?;
    let commit = commit.trim();
    (output.status.success() && !commit.is_empty()).then(|| commit.to_string())
}

/// The install lock, relative to the target.
const INSTALL_LOCK_FILE: &str = ".canon/skills/.install-lock.json";

fn manifest_path(target_dir: &Path) -> PathBuf {
    target_dir.join(INSTALL_LOCK_FILE)
}

/// `<dir>/**`, the glob for everything under a canon directory constant.
fn dir_glob(dir: &str) -> String {
    format!("{}/**", dir.trim_end_matches('/'))
}

/// The ignore entry for `provider`'s projection: `<root>/skills/canon/**`.
pub fn projection_glob(provider: Provider) -> String {
    dir_glob(&format!("{}/skills/canon", provider.descriptor().root))
}

/// The paths canon writes in a repo, as globs for a formatter's or
/// linter's ignore list (0.18, #25): the projection of each of
/// `providers`, then every machine-written path under `.canon/`, sorted.
/// A formatter that rewrites any of them makes canon read drift (a
/// projection against its install lock, a ledger record against its
/// digest-named file). Derived from `canon_model::paths` and this
/// module's own constants, never typed out a second time. Paths people
/// write by hand are left out so linters still see them:
/// `.canon/specs/`, `.canon/policy.yaml` and `.canon/changes/`.
pub fn owned_path_globs(providers: &[Provider]) -> Vec<String> {
    let artifacts_dir = Path::new(paths::ARTIFACTS_SHA256_DIR).parent().and_then(Path::to_str).unwrap_or(paths::ARTIFACTS_SHA256_DIR);
    let lock_dir = Path::new(INSTALL_LOCK_FILE).parent().and_then(Path::to_str).unwrap_or(INSTALL_LOCK_FILE);
    let dirs = [
        paths::LEDGER_DIR,
        artifacts_dir,
        paths::R2_LOCAL_DIR,
        paths::LEARN_DIR,
        paths::STRATEGIES_DIR,
        paths::VOCAB_DIR,
        paths::PLUGINS_DIR,
        paths::INGEST_DIR,
        paths::MIGRATE_BACKUP_DIR,
        paths::DASHBOARD_SNAPSHOT_DIR,
        paths::AUDIT_GITIGNORE,
        lock_dir,
    ];
    let files = [
        paths::HOT_DB_GITIGNORE,
        paths::POLICY_APPROVAL_FILE,
        paths::REPORT_FILE,
        paths::PRE_COMMIT_SCRIPT,
        paths::SCENARIO_INDEX_LOCK_FILE,
        paths::ARTIFACT_STORE_LOCK_FILE,
    ];
    let mut canon: Vec<String> = dirs.into_iter().map(dir_glob).chain(files.into_iter().map(str::to_string)).collect();
    canon.sort();
    canon.dedup();
    providers.iter().map(|provider| projection_glob(*provider)).chain(canon).collect()
}

/// The lines `canon skills install` and `canon skills doctor` print for
/// [`owned_path_globs`]: one header, then one indented glob per line.
pub fn owned_paths_lines(providers: &[Provider]) -> Vec<String> {
    let mut lines = vec!["owned-paths: canon writes these; exclude them from formatters and linters:".to_string()];
    lines.extend(owned_path_globs(providers).into_iter().map(|glob| format!("  {glob}")));
    lines
}

/// Whether `actual` holds the same content as `expected` once what a
/// formatter changes is set aside (0.18, #25): a JSON file parses to
/// the same value; any other file matches after [`format_insensitive`].
/// `false` when the bytes are identical (that is not a rewrite) or
/// either side is not UTF-8.
fn formatter_rewrote(relative: &Path, actual: &[u8], expected: &[u8]) -> bool {
    if actual == expected {
        return false;
    }
    if relative.extension().is_some_and(|ext| ext == "json") {
        return match (serde_json::from_slice::<serde_json::Value>(actual), serde_json::from_slice::<serde_json::Value>(expected)) {
            (Ok(a), Ok(e)) => a == e,
            _ => false,
        };
    }
    let markdown = relative.extension().is_some_and(|ext| ext == "md");
    match (std::str::from_utf8(actual), std::str::from_utf8(expected)) {
        (Ok(a), Ok(e)) => format_insensitive(a, markdown) == format_insensitive(e, markdown),
        _ => false,
    }
}

/// `text` with only what a prettier-family formatter rewrites set aside,
/// every token boundary kept: CRLF as LF, trailing whitespace per line,
/// a run of blank lines as one, and blank lines at either end. In
/// `markdown` also: a `*`/`+` list marker as `-`, `_` emphasis as `*`,
/// and table-cell padding. Whitespace between tokens and backslashes are
/// never dropped, and a fenced code block is kept verbatim, so a changed
/// command never reads as a rewrite.
fn format_insensitive(text: &str, markdown: bool) -> String {
    let mut lines: Vec<String> = Vec::new();
    let mut fence: Option<&str> = None;
    for raw in text.lines() {
        let opener = ["```", "~~~"].into_iter().find(|mark| raw.trim_start().starts_with(mark));
        if markdown {
            if let Some(mark) = fence {
                lines.push(raw.to_string());
                if raw.trim().starts_with(mark) {
                    fence = None;
                }
                continue;
            }
            if opener.is_some() {
                fence = opener;
                lines.push(raw.trim_end().to_string());
                continue;
            }
        }
        let line = raw.trim_end();
        if line.is_empty() {
            if lines.last().is_some_and(|last| !last.is_empty()) {
                lines.push(String::new());
            }
            continue;
        }
        lines.push(if markdown { markdown_line(line) } else { line.to_string() });
    }
    while lines.last().is_some_and(String::is_empty) {
        lines.pop();
    }
    lines.join("\n")
}

/// One markdown line outside a code fence, with its list marker, `_`
/// emphasis and table padding normalized (see [`format_insensitive`]).
fn markdown_line(line: &str) -> String {
    let indent = &line[..line.len() - line.trim_start().len()];
    let body = line.trim_start();
    if body.starts_with('|') {
        let cells: Vec<String> = body
            .split('|')
            .map(|cell| {
                let cell = cell.trim();
                let rule = cell.trim_matches(':');
                if !rule.is_empty() && rule.chars().all(|ch| ch == '-') {
                    format!("{}---{}", if cell.starts_with(':') { ":" } else { "" }, if cell.ends_with(':') { ":" } else { "" })
                } else {
                    emphasis_as_asterisks(cell)
                }
            })
            .collect();
        return format!("{indent}{}", cells.join("|"));
    }
    let body = match body.strip_prefix("* ").or_else(|| body.strip_prefix("+ ")) {
        Some(rest) => format!("- {rest}"),
        None => body.to_string(),
    };
    format!("{indent}{}", emphasis_as_asterisks(&body))
}

/// `_` as `*` where it can delimit emphasis: outside inline code, and
/// not between two word characters (`snake_case` stays).
fn emphasis_as_asterisks(text: &str) -> String {
    let chars: Vec<char> = text.chars().collect();
    let mut out = String::with_capacity(text.len());
    let mut in_code = false;
    for (i, &ch) in chars.iter().enumerate() {
        if ch == '`' {
            in_code = !in_code;
        }
        let word = |c: Option<&char>| c.is_some_and(|c| c.is_alphanumeric());
        let intraword = word(i.checked_sub(1).and_then(|j| chars.get(j))) && word(chars.get(i + 1));
        out.push(if ch == '_' && !in_code && !intraword { '*' } else { ch });
    }
    out
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
        canon_version: Some(CANON_VERSION.to_string()),
        source_commit: None,
        source_hash,
        providers: providers.iter().map(|p| p.as_str().into()).collect(),
        files: hashes,
        retained_files: BTreeMap::new(),
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

/// Removes the lock-proven files `removals` (`(key, recorded hash)`), then
/// each directory they left empty below `stop_below`, which is never removed
/// (the legacy migration stops at `.codex`, so `.codex/skills` goes when
/// empty; a pruned projection stops at `<root>/skills/canon`). Any non-empty
/// directory and every file canon cannot prove it wrote stay. Each file's
/// hash is checked again at removal time; one that changed since it was
/// selected is kept. Returns the keys actually removed.
fn remove_proven<'k>(target_dir: &Path, removals: &[(&'k str, &str)], stop_below: &str) -> Result<Vec<&'k str>, SkillsError> {
    let mut dirs: Vec<&str> = Vec::new();
    let mut removed = Vec::new();
    for (key, hash) in removals {
        if legacy_fs::remove_file_if_hash(target_dir, key, hash)? {
            removed.push(*key);
        }
        let mut current = *key;
        while let Some((parent, _)) = current.rsplit_once('/') {
            if parent == stop_below || !parent.starts_with(stop_below) {
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
    Ok(removed)
}

/// `<root>/skills/canon`, the directory `provider`'s projection lives in, as
/// a lock key prefix.
fn projection_key_root(provider: Provider) -> String {
    format!("{}/skills/canon", provider.descriptor().root)
}

/// Whether `key`, from the install lock, names a file inside the canon
/// projection `root`. Keys come from a file in the target repository, so
/// anything but plain path components is refused.
fn is_projection_key(root: &str, key: &str) -> bool {
    key.strip_prefix(root).is_some_and(|rest| rest.starts_with('/'))
        && key.split('/').all(|segment| !segment.is_empty() && segment != "." && segment != "..")
        && Path::new(key).components().all(|component| matches!(component, std::path::Component::Normal(_)))
}

/// Lock keys inside a provider's canon projection directory that are not
/// plain relative paths (empty, `.` or `..` segments, a trailing `/`):
/// ignored everywhere, named by `doctor`, dropped when install rewrites the lock.
fn invalid_projection_keys(previous: Option<&CanonicalManifest>) -> Vec<String> {
    let Some(previous) = previous else {
        return Vec::new();
    };
    let roots: Vec<String> = ALL_PROVIDERS.into_iter().map(projection_key_root).collect();
    previous
        .files
        .keys()
        .chain(previous.retained_files.keys())
        .filter(|key| roots.iter().any(|root| key.strip_prefix(root.as_str()).is_some_and(|rest| rest.starts_with('/'))))
        .filter(|key| !roots.iter().any(|root| is_projection_key(root, key)))
        .cloned()
        .collect()
}

/// An [`Orphan`] with the lock key and recorded hash it came from.
struct FoundOrphan<'m> {
    orphan: Orphan,
    key: &'m str,
    hash: &'m str,
}

/// The files of `selected` providers' canon projections that `previous`
/// lists, `expected` (the lock a fresh install would write) does not, and
/// that exist on disk. An unselected provider's projection and a file the
/// lock never listed are never named. A file whose bytes still match the
/// lock's hash is unmodified; a changed, replaced or symlinked one is
/// `modified`.
fn find_orphans<'m>(
    target_dir: &Path,
    previous: Option<&'m CanonicalManifest>,
    expected: &CanonicalManifest,
    selected: &[Provider],
) -> Vec<FoundOrphan<'m>> {
    let Some(previous) = previous else {
        return Vec::new();
    };
    let mut found = Vec::new();
    for provider in selected {
        let root = projection_key_root(*provider);
        for (key, hash) in previous.files.iter().chain(&previous.retained_files) {
            if !is_projection_key(&root, key) || expected.files.contains_key(key) {
                continue;
            }
            let path = target_dir.join(key);
            if fs::symlink_metadata(&path).is_err() {
                continue;
            }
            let modified = legacy_fs::read(target_dir, key).is_none_or(|bytes| content_hash(&bytes) != *hash);
            found.push(FoundOrphan { orphan: Orphan { provider: *provider, path, modified }, key, hash });
        }
    }
    found
}

/// Removes the unmodified orphans, then the directories that leaves empty
/// inside each canon projection. Returns `(pruned, kept)`; a file that
/// changed between detection and removal moves to `kept`.
fn prune_orphans(target_dir: &Path, found: Vec<FoundOrphan<'_>>, selected: &[Provider]) -> Result<(Vec<Orphan>, Vec<Orphan>), SkillsError> {
    let mut pruned = Vec::new();
    let mut kept = Vec::new();
    for provider in selected {
        let (mine, _): (Vec<_>, Vec<_>) = found.iter().partition(|f| f.orphan.provider == *provider);
        let removals: Vec<(&str, &str)> = mine.iter().filter(|f| !f.orphan.modified).map(|f| (f.key, f.hash)).collect();
        let removed = remove_proven(target_dir, &removals, &projection_key_root(*provider))?;
        for found in mine {
            if removed.contains(&found.key) {
                pruned.push(found.orphan.clone());
            } else {
                kept.push(Orphan { modified: true, ..found.orphan.clone() });
            }
        }
    }
    Ok((pruned, kept))
}

/// What [`prune_orphans`] would return, without removing anything: the
/// unmodified orphans prune, the modified ones are kept.
fn preview_prune(found: Vec<FoundOrphan<'_>>) -> (Vec<Orphan>, Vec<Orphan>) {
    found.into_iter().map(|f| f.orphan).partition(|orphan| !orphan.modified)
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

    pub(crate) fn read(target_dir: &Path, key: &str) -> Option<Vec<u8>> {
        let (dir, name) = open_parent(target_dir, key)?;
        read_in(&dir, name)
    }

    /// Whether it unlinked the file: `false` when the file is gone, is not a
    /// regular file, or no longer holds the bytes `hash` names.
    pub(crate) fn remove_file_if_hash(target_dir: &Path, key: &str, hash: &str) -> Result<bool, SkillsError> {
        let Some((dir, name)) = open_parent(target_dir, key) else {
            return Ok(false);
        };
        if read_in(&dir, name).is_none_or(|bytes| content_hash(&bytes) != hash) {
            return Ok(false);
        }
        match unlinkat(&dir, name, AtFlags::empty()) {
            Ok(()) => Ok(true),
            Err(errno) if errno == Errno::NOENT => Ok(false),
            Err(errno) => Err(io_error(target_dir, key, errno)),
        }
    }

    /// `rmdir` relative to the parent's handle: it never follows a symlink
    /// and refuses a non-empty directory atomically.
    pub(crate) fn remove_dir_if_empty(target_dir: &Path, key: &str) -> Result<(), SkillsError> {
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

    pub(crate) fn read(target_dir: &Path, key: &str) -> Option<Vec<u8>> {
        let path = target_dir.join(key);
        reject_symlink_path_under(target_dir, &path).ok()?;
        fs::symlink_metadata(&path).ok()?.is_file().then_some(())?;
        fs::read(&path).ok()
    }

    pub(crate) fn remove_file_if_hash(target_dir: &Path, key: &str, hash: &str) -> Result<bool, SkillsError> {
        if read(target_dir, key).is_none_or(|bytes| content_hash(&bytes) != hash) {
            return Ok(false);
        }
        let path = target_dir.join(key);
        fs::remove_file(&path).map(|()| true).map_err(|source| SkillsError::Io { path, source })
    }

    pub(crate) fn remove_dir_if_empty(target_dir: &Path, key: &str) -> Result<(), SkillsError> {
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

/// The install command for `providers`: what rewrites an outdated lock or
/// migrates a legacy Codex projection.
pub fn install_command(providers: &[Provider]) -> String {
    format!("canon skills install --providers={}", provider_names(providers))
}

/// Build verbs, in every form [`build_claims`] matches.
pub const BUILD_VERBS: [&str; 28] = [
    "build", "builds", "building", "built",
    "implement", "implements", "implementing", "implemented",
    "create", "creates", "creating", "created",
    "make", "makes", "making", "made",
    "scaffold", "scaffolds", "scaffolding", "scaffolded",
    "develop", "develops", "developing", "developed",
    "code", "codes", "coding", "coded",
];

/// Build objects, singular and plural, [`build_claims`] matches.
pub const BUILD_OBJECTS: [&str; 44] = [
    "site", "sites",
    "website", "websites",
    "app", "apps",
    "application", "applications",
    "game", "games",
    "feature", "features",
    "frontend", "frontends",
    "ui", "uis",
    "page", "pages",
    "component", "components",
    "tool", "tools",
    "library", "libraries",
    "service", "services",
    "api", "apis",
    "cli", "clis",
    "server", "servers",
    "backend", "backends",
    "project", "projects",
    "prototype", "prototypes",
    "product", "products",
    "program", "programs",
    "script", "scripts",
];

/// A skill beside canon's in a provider skill root whose description
/// claims build work.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CompetingSkill {
    /// The skill's directory.
    pub path: PathBuf,
    /// Its frontmatter `name`, else its directory name.
    pub name: String,
    /// The [`BUILD_VERBS`] and [`BUILD_OBJECTS`] its claiming sentences
    /// hold, verbs first, each in table order.
    pub claims: Vec<&'static str>,
}

/// The YAML frontmatter block `content` opens with, parsed.
fn skill_frontmatter(content: &str) -> Option<serde_yaml::Mapping> {
    let mut lines = content.split_inclusive('\n');
    if lines.next()?.trim_end() != "---" {
        return None;
    }
    let mut yaml = String::new();
    for line in lines {
        if line.trim_end() == "---" {
            return serde_yaml::from_str(&yaml).ok();
        }
        yaml.push_str(line);
    }
    None
}

/// Whether a skill `description` claims build work (#10), and with which
/// words. The rule: some sentence (text between `.`, `!`, `?`, `;` or a
/// line break) holds both a [`BUILD_VERBS`] word and a [`BUILD_OBJECTS`]
/// word. Words are runs of letters and digits, compared lowercase, so
/// `web-app` holds `app` and `rebuild` holds nothing. A verb alone
/// ("Build release notes") or an object alone ("Manage feature branches")
/// is not a claim. Returns every verb and object of the claiming
/// sentences, verbs first, each in table order; empty when none claims.
fn build_claims(description: &str) -> Vec<&'static str> {
    let mut verbs = Vec::new();
    let mut objects = Vec::new();
    for sentence in description.split(['.', '!', '?', ';', '\n']) {
        let words: Vec<String> = sentence.split(|c: char| !c.is_alphanumeric()).filter(|word| !word.is_empty()).map(str::to_lowercase).collect();
        let held = |table: &[&'static str]| -> Vec<&'static str> { table.iter().copied().filter(|keyword| words.iter().any(|word| word == keyword)).collect() };
        let (sentence_verbs, sentence_objects) = (held(&BUILD_VERBS), held(&BUILD_OBJECTS));
        if !sentence_verbs.is_empty() && !sentence_objects.is_empty() {
            verbs.extend(sentence_verbs);
            objects.extend(sentence_objects);
        }
    }
    let mut claims: Vec<&'static str> = BUILD_VERBS.into_iter().filter(|verb| verbs.contains(verb)).collect();
    claims.extend(BUILD_OBJECTS.into_iter().filter(|object| objects.contains(object)));
    claims
}

/// Every skill (`<root>/<dir>/SKILL.md`) in the four provider skill roots
/// other than canon's own (`canon`, and the `canon-*` legacy names `doctor`
/// reports as remnants) whose frontmatter description claims build work.
/// Read only; a skill without a parsable frontmatter description is
/// skipped.
pub fn competing_skills(target_dir: &Path) -> Vec<CompetingSkill> {
    let mut found = Vec::new();
    for provider in ALL_PROVIDERS {
        let Ok(entries) = fs::read_dir(provider_skill_root(target_dir, provider)) else {
            continue;
        };
        let mut dirs: Vec<(String, PathBuf)> = entries
            .flatten()
            .map(|entry| (entry.file_name().to_string_lossy().into_owned(), entry.path()))
            .filter(|(name, _)| name != "canon" && !name.starts_with("canon-"))
            .collect();
        dirs.sort();
        for (dir_name, path) in dirs {
            let Ok(content) = fs::read_to_string(path.join("SKILL.md")) else {
                continue;
            };
            let Some(frontmatter) = skill_frontmatter(&content) else {
                continue;
            };
            let Some(description) = frontmatter.get("description").and_then(serde_yaml::Value::as_str) else {
                continue;
            };
            let claims = build_claims(description);
            if claims.is_empty() {
                continue;
            }
            let name = frontmatter.get("name").and_then(serde_yaml::Value::as_str).map_or(dir_name, str::to_string);
            found.push(CompetingSkill { path, name, claims });
        }
    }
    found
}

/// Projects the canonical bundle for the [`select_providers`] selection
/// and records it in the install lock. Files the previous lock lists for a
/// selected provider that the bundle no longer ships are pruned when their
/// bytes still match the lock's hash and kept (and reported) otherwise
/// (0.25, #99); see [`find_orphans`]. An explicit `providers` also
/// rewrites `skills.providers` in `<target_dir>/canon.yaml` when that file
/// exists, after the projection is written. With nothing selected and no
/// install lock, nothing at all is written.
pub fn install_canonical(
    source_dir: Option<&Path>,
    target_dir: &Path,
    providers: Option<&str>,
    dry_run: bool,
) -> Result<CanonicalInstallReport, SkillsError> {
    let selection = select_providers(target_dir, providers)?;
    let selected = selection.providers;
    let files = canonical_bundle(source_dir)?;
    let config = match selection.origin {
        ProviderOrigin::Flag => planned_config(target_dir, &selected)?,
        _ => None,
    };
    if let Some((path, _)) = &config {
        reject_symlink_path(path)?;
    }
    let mut manifest = manifest_for(&files, &selected);
    let previous = load_manifest(target_dir)?;
    if let Some(previous) = &previous {
        let unselected_roots: Vec<String> =
            ALL_PROVIDERS.into_iter().filter(|p| !selected.contains(p)).map(projection_key_root).collect();
        manifest.retained_files = previous
            .files
            .iter()
            .chain(&previous.retained_files)
            .filter(|(key, _)| unselected_roots.iter().any(|root| is_projection_key(root, key)))
            .map(|(key, hash)| (key.clone(), hash.clone()))
            .collect();
    }
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
    let orphans = find_orphans(target_dir, previous.as_ref(), &manifest, &selected);
    let lock_current = match &previous {
        Some(previous) => previous.same_projection(&manifest) && previous.retained_files == manifest.retained_files && previous.canon_version == manifest.canon_version,
        // Nothing selected and nothing recorded: there is nothing to lock.
        None => selected.is_empty(),
    };
    let rewritten: Vec<PathBuf> = writes.iter().filter(|(path, bytes)| fs::read(path).ok().as_deref() != Some(bytes.as_slice())).map(|(path, _)| path.clone()).collect();
    let changed = !lock_current || !removals.is_empty() || !rewritten.is_empty();
    let (mut pruned, mut kept) = (Vec::new(), Vec::new());
    if changed && dry_run {
        (pruned, kept) = preview_prune(orphans);
    } else if changed {
        for (path, bytes) in writes {
            write_bytes_under(target_dir, &path, &bytes)?;
        }
        remove_proven(target_dir, &removals, ".codex")?;
        (pruned, kept) = prune_orphans(target_dir, orphans, &selected)?;
        manifest.source_commit = source_commit(source_dir);
        let mut json =
            serde_json::to_string_pretty(&manifest).expect("manifest serialization is infallible");
        json.push('\n');
        write_file_under(target_dir, &manifest_path(target_dir), &json)?;
    }
    let config_written = match config {
        Some((path, text)) => {
            if !dry_run {
                write_file(&path, &text)?;
            }
            Some(path)
        }
        None => None,
    };
    Ok(CanonicalInstallReport {
        providers: selected,
        origin: selection.origin,
        source_hash: manifest.source_hash,
        changed,
        config_written,
        pruned,
        kept,
        rewritten,
        lock_changed: !lock_current,
    })
}

pub fn install(source_dir: &Path, target_dir: &Path, dry_run: bool) -> Result<InstallReport, SkillsError> {
    if source_dir.join("SKILL.src.md").is_file() {
        let report = install_canonical(Some(source_dir), target_dir, None, dry_run)?;
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
    install_legacy(source_dir, target_dir, dry_run)
}

/// The `(major, minor)` of a `X.Y.Z` canon version; `None` when it does not parse.
fn major_minor(version: &str) -> Option<(u64, u64)> {
    let mut parts = version.split('.');
    let major = parts.next()?.parse().ok()?;
    let minor = parts.next()?.parse().ok()?;
    Some((major, minor))
}

/// Whether `target_dir` holds a full skill projection from before the stub (0.27
/// and earlier, 0.28 D9): the install lock was written by an older canon than
/// 0.28 (or by one that recorded no version) and is not this binary's own, or
/// lists a `reference/**` file, while listing projected files at all; or there is no lock and a
/// provider's projection directory holds `reference/`. Cheap: reads the lock and at most four directories.
pub fn has_full_projection(target_dir: &Path) -> bool {
    match load_manifest(target_dir) {
        Ok(Some(lock)) => {
            let lists_reference = lock.files.keys().chain(lock.retained_files.keys()).any(|key| key.contains("/skills/canon/reference/"));
            let older = lock.canon_version.as_deref() != Some(CANON_VERSION) && lock.canon_version.as_deref().is_none_or(|version| major_minor(version).is_some_and(|v| v < STUB_SINCE));
            // A lock that lists no file has no projection to replace.
            !lock.files.is_empty() && (lists_reference || older)
        }
        Ok(None) => ALL_PROVIDERS.into_iter().any(|provider| provider_bundle_root(target_dir, provider).join("reference").is_dir()),
        Err(_) => false,
    }
}

/// The first release whose installed `SKILL.md` is the stub.
const STUB_SINCE: (u64, u64) = (0, 28);

pub fn check(
    source_dir: Option<&Path>,
    target_dir: &Path,
    providers: Option<&str>,
) -> Result<SkillsCheckOutcome, SkillsError> {
    let selection = select_providers(target_dir, providers)?;
    let selected = selection.providers;
    let files = canonical_bundle(source_dir)?;
    let expected = manifest_for(&files, &selected);
    let actual = load_manifest(target_dir)?;
    let mut statuses = Vec::new();
    for provider in &selected {
        for file in &files {
            let path = project_path(target_dir, *provider, &file.relative);
            let expected_bytes = file.bytes.as_slice();
            let state = match fs::read(&path) {
                _ if !path.is_file() => "missing",
                Ok(actual) if actual == expected_bytes => "ok",
                Ok(actual) if formatter_rewrote(&file.relative, &actual, expected_bytes) => FORMATTER_REWRITTEN,
                _ => "stale",
            };
            statuses.push(SkillStatus {
                provider: *provider,
                path,
                state,
            });
        }
    }
    let manifest_ok = match &actual {
        Some(actual) => actual.same_projection(&expected),
        // Nothing selected needs no lock.
        None => selected.is_empty(),
    };
    let orphans = find_orphans(target_dir, actual.as_ref(), &expected, &selected).into_iter().map(|found| found.orphan).collect();
    let invalid_entries = invalid_projection_keys(actual.as_ref());
    let outdated = actual
        .filter(|actual| actual.canon_version != expected.canon_version)
        .map(|actual| OutdatedLock { path: manifest_path(target_dir), installed_by: actual.canon_version });
    Ok(SkillsCheckOutcome {
        providers: selected,
        origin: selection.origin,
        source_hash: expected.source_hash,
        statuses,
        manifest_ok,
        orphans,
        invalid_entries,
        remnants: legacy_codex_remnants(target_dir),
        outdated,
    })
}

/// The canon projection directories (`<root>/skills/canon`) present for a
/// provider outside `selected`, with that provider. Canon no longer
/// updates them; `doctor` names them, `check` ignores them.
pub fn stray_projections(target_dir: &Path, selected: &[Provider]) -> Vec<(Provider, PathBuf)> {
    ALL_PROVIDERS
        .into_iter()
        .filter(|provider| !selected.contains(provider))
        .map(|provider| (provider, provider_bundle_root(target_dir, provider)))
        .filter(|(_, dir)| fs::symlink_metadata(dir).is_ok())
        .collect()
}

pub fn doctor(
    source_dir: Option<&Path>,
    target_dir: &Path,
    providers: Option<&str>,
) -> Result<Vec<String>, SkillsError> {
    let selection = select_providers(target_dir, providers)?;
    let mut lines = vec![match selection.origin {
        ProviderOrigin::Flag => format!("providers: {}", selection.names()),
        _ => format!("providers: {} ({})", selection.names(), selection.describe()),
    }];
    if selection.providers.is_empty() && selection.origin == ProviderOrigin::Detected {
        lines.push(format!("no-providers: no canon skill is installed; {CHOOSE_PROVIDERS}"));
    }
    let selected = selection.providers.clone();
    let mut advisories: Vec<String> = stray_projections(target_dir, &selected)
        .into_iter()
        .map(|(provider, dir)| {
            let mut with = selected.clone();
            with.push(provider);
            with.sort();
            format!(
                "stray: {} — {} is not a selected provider ({}), so canon no longer updates it; remove it with `rm -r {}`, or select it with `{}`",
                dir.display(),
                provider.as_str(),
                selection.describe(),
                dir.display(),
                install_command(&with)
            )
        })
        .collect();
    advisories.extend(competing_skills(target_dir).into_iter().map(|skill| {
        format!(
            "competing-skill: {} (description claims: {}) — if an agent picks `{}` for build work, canon's loop will not run",
            skill.path.display(),
            skill.claims.join(", "),
            skill.name
        )
    }));
    let lines_before_report = lines.len();
    let report = match check(source_dir, target_dir, providers) {
        Ok(report) => report,
        Err(SkillsError::ManifestParse { path, source }) => {
            lines.push(format!("manifest-error: {} ({source})", path.display()));
            lines.extend(advisories);
            return Ok(lines);
        }
        Err(err) => return Err(err),
    };
    for status in report.statuses {
        if status.state == FORMATTER_REWRITTEN {
            lines.push(format!(
                "{FORMATTER_REWRITTEN}: {} ({}) — the same content as canon's projection after whitespace and format normalization, so a formatter rewrote it; add `{}` to that formatter's ignore list, then restore it with `{}`",
                status.path.display(),
                status.provider.as_str(),
                projection_glob(status.provider),
                install_command(&report.providers)
            ));
        } else if status.state != "ok" {
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
    for orphan in &report.orphans {
        lines.push(format!(
            "orphan: {} ({}) — the install lock lists it, but this canon's bundle no longer ships it; {}; {}",
            orphan.path.display(),
            orphan.provider.as_str(),
            orphan.reason(),
            orphan.fix(&report.providers)
        ));
    }
    for key in &report.invalid_entries {
        lines.push(format!(
            "invalid-lock-entry: {key:?} in {} is not a plain relative path inside a canon projection, so canon ignores it; `{}` rewrites the lock without it",
            manifest_path(target_dir).display(),
            install_command(&report.providers)
        ));
    }
    if let Some(outdated) = &report.outdated {
        lines.push(format!("outdated: {} {}", outdated.path.display(), outdated.describe(&report.providers)));
    }
    for remnant in &report.remnants {
        lines.push(format!(
            "legacy-remnant: {} (codex reads .agents/skills, never .codex/skills; fix: `{}`, which removes what canon's install lock proves it wrote; remove anything it keeps by hand)",
            remnant.display(),
            install_command(&report.providers)
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
    if target_dir.join("canon.yaml").is_file() {
        if let Some(gap) = crate::init::gitignore_gap(target_dir) {
            lines.push(format!("gitignore: {gap}"));
        }
    }
    if lines.len() == lines_before_report && !lines.iter().any(|line| line.starts_with("no-providers:")) {
        lines.push("status: healthy".into());
    }
    lines.extend(advisories);
    lines.extend(owned_paths_lines(&report.providers));
    Ok(lines)
}

/// The developer override directory: `--skills-source`, else `CANON_SKILLS_SOURCE`
/// when set and not empty; `None` means the text embedded in the binary (0.28 O15).
pub fn resolve_source(explicit: Option<&Path>) -> Option<PathBuf> {
    explicit.map(Path::to_path_buf).or_else(|| std::env::var_os(CANONICAL_SOURCE_ENV).filter(|value| !value.is_empty()).map(PathBuf::from))
}

/// One skill the legacy (pre-bundle) install wrote.
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct InstalledSkillEntry<'a> {
    pub name: &'a str,
    pub version: u64,
    pub changed: bool,
}

/// What the legacy install (a source without `SKILL.src.md`) reports.
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct LegacyInstall<'a> {
    /// True when `--dry-run` previewed the run without writing.
    pub dry_run: bool,
    pub skills: Vec<InstalledSkillEntry<'a>>,
}

/// An orphan the install pruned.
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PrunedEntry {
    pub path: String,
    pub provider: &'static str,
    pub reason: &'static str,
}

/// An orphan the install kept, with what to do by hand.
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct KeptEntry {
    pub path: String,
    pub provider: &'static str,
    pub reason: &'static str,
    pub hint: String,
}

/// What the bundle install reports.
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct BundleInstall {
    /// True when `--dry-run` previewed the run without writing.
    pub dry_run: bool,
    pub providers: Vec<&'static str>,
    pub selection: String,
    pub changed: bool,
    pub config_written: Option<String>,
    pub pruned: Vec<PrunedEntry>,
    pub kept: Vec<KeptEntry>,
    pub owned_paths: Vec<String>,
}

/// `canon skills install`'s report (`SkillsInstallReport`): the bundle install,
/// or the legacy one for a source without `SKILL.src.md`.
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(untagged)]
pub enum SkillsInstallReport<'a> {
    Bundle(BundleInstall),
    Legacy(LegacyInstall<'a>),
}

impl crate::json_out::Report for SkillsInstallReport<'_> {}

/// The state of one installed skill file.
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SkillFileStatus {
    pub path: String,
    pub state: &'static str,
    pub provider: &'static str,
}

/// An orphan `skills check` found.
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OrphanEntry {
    pub path: String,
    pub provider: &'static str,
    pub modified: bool,
    pub reason: &'static str,
    pub hint: String,
}

/// An install lock written by another canon version.
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct OutdatedEntry {
    pub path: String,
    pub installed_by: Option<String>,
    pub message: String,
}

/// `canon skills check`'s report (`SkillsCheckReport`).
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SkillsCheckReport {
    pub clean: bool,
    pub providers: Vec<&'static str>,
    pub selection: String,
    pub manifest_ok: bool,
    pub statuses: Vec<SkillFileStatus>,
    pub orphans: Vec<OrphanEntry>,
    pub remnants: Vec<String>,
    pub outdated: Option<OutdatedEntry>,
}

impl crate::json_out::Report for SkillsCheckReport {}

/// `canon skills doctor`'s report (`SkillsDoctorReport`).
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SkillsDoctorReport<'a> {
    pub lines: &'a [String],
}

impl crate::json_out::Report for SkillsDoctorReport<'_> {}

/// `--json` of `canon skills show`: the guide `name` names, as this canon embeds it.
#[derive(Debug, Serialize, schemars::JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct SkillsShowReport {
    pub name: String,
    /// The canon version the guide is embedded in.
    pub version: &'static str,
    /// The guide's markdown, as `canon skills show` prints it.
    pub markdown: std::borrow::Cow<'static, str>,
}

impl crate::json_out::Report for SkillsShowReport {}

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
                canon_version: None,
                source_commit: None,
                source_hash: "sha256:legacy".into(),
                providers: vec!["codex".into()],
                files: BTreeMap::from([(key.to_string(), content_hash(b"# topic\n"))]),
                retained_files: BTreeMap::new(),
            };
            let removals = legacy_codex_removals(&target, Some(&manifest));
            assert_eq!(removals, vec![(key, manifest.files[key].as_str())], "{swapped}");

            fs::create_dir_all(&outside).unwrap();
            let moved = outside.join("moved");
            fs::rename(target.join(swapped), &moved).unwrap();
            std::os::unix::fs::symlink(&moved, target.join(swapped)).unwrap();
            remove_proven(&target, &removals, ".codex").unwrap();

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

    #[test]
    fn build_claims_need_a_build_verb_and_a_build_object_in_one_sentence() {
        assert_eq!(build_claims("Build websites and apps"), vec!["build", "websites", "apps"]);
        assert_eq!(build_claims("Builds Sites, web-apps and GAMES."), vec!["builds", "sites", "apps", "games"]);
        assert_eq!(build_claims("Use to implement a feature"), vec!["implement", "feature"]);
        assert_eq!(build_claims("Notes.\nScaffold UI components"), vec!["scaffold", "ui", "components"]);
        assert!(build_claims("Manage feature branches").is_empty(), "an object without a verb is no claim");
        assert!(build_claims("Review pages, apps and game assets").is_empty(), "objects alone are no claim");
        assert!(build_claims("Build release notes").is_empty(), "a verb alone is no claim");
        assert!(build_claims("Build release notes. Track feature flags").is_empty(), "verb and object must share a sentence");
        assert!(build_claims("Rebuilds the index; reviews websites").is_empty(), "substrings never match");
        assert_eq!(build_claims("Build tools for the team"), vec!["build", "tools"]);
        assert_eq!(build_claims("Scaffold a CLI and a backend service"), vec!["scaffold", "service", "cli", "backend"]);
        assert!(build_claims("Write release notes for the project").is_empty(), "an object without a build verb is no claim");
    }

    #[test]
    fn skill_frontmatter_reads_plain_quoted_and_folded_descriptions() {
        for (content, expected) in [
            ("---\nname: sites\ndescription: Build sites\n---\n# Sites\n", "Build sites"),
            ("---\r\nname: x\r\ndescription: \"Build: games\"\r\n---\r\n", "Build: games"),
            ("---\ndescription: >\n  Build apps\n  and games\n---\n", "Build apps and games\n"),
        ] {
            let frontmatter = skill_frontmatter(content).unwrap_or_else(|| panic!("{content:?}"));
            assert_eq!(frontmatter.get("description").and_then(serde_yaml::Value::as_str), Some(expected), "{content:?}");
        }
        assert!(skill_frontmatter("# no frontmatter\n").is_none());
        assert!(skill_frontmatter("---\ndescription: never closed\n").is_none());
    }

    /// skills.install.57/58: what a prettier-family formatter changes
    /// reads as a rewrite; a changed word, line or JSON value is drift.
    #[test]
    fn formatter_rewrites_are_told_apart_from_content_changes() {
        let md = Path::new("reference/topic.md");
        let source = "# Topic\n\nA *role* is a rung of `snake_case`.\n\n* one\n* two\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\n```sh\nrm -rf a b\n```\n";
        let formatted = "# Topic  \r\n\r\n\r\nA _role_ is a rung of `snake_case`.\r\n\r\n- one\r\n- two\r\n\r\n| a   | b   |\r\n| --- | --- |\r\n| 1   | 2   |\r\n\r\n```sh\r\nrm -rf a b\r\n```\r\n\r\n";
        assert!(formatter_rewrote(md, formatted.as_bytes(), source.as_bytes()));
        assert!(!formatter_rewrote(md, source.as_bytes(), source.as_bytes()), "identical bytes are no rewrite");
        assert!(!formatter_rewrote(md, source.replace("rung", "tier").as_bytes(), source.as_bytes()));
        assert!(!formatter_rewrote(md, source.replace("* two\n", "").as_bytes(), source.as_bytes()));
        assert!(!formatter_rewrote(md, source.replace("a rung", "arung").as_bytes(), source.as_bytes()), "inner whitespace is a token boundary");
        assert!(!formatter_rewrote(md, source.replace("snake_case", "snake*case").as_bytes(), source.as_bytes()), "an intraword _ is not emphasis");
        assert!(!formatter_rewrote(md, source.replace("rm -rf a b", "rm -rf  a b").as_bytes(), source.as_bytes()), "a code fence is verbatim");
        assert!(!formatter_rewrote(md, &[0xff, 0xfe], source.as_bytes()), "non-UTF-8 is never a rewrite");

        let sh = Path::new("scripts/hook.sh");
        let script = "#!/bin/sh\nrm -rf a b\nexec canon \\\n  status\n";
        assert!(formatter_rewrote(sh, b"#!/bin/sh  \r\nrm -rf a b\r\nexec canon \\\r\n  status\r\n\r\n", script.as_bytes()));
        assert!(!formatter_rewrote(sh, script.replace("a b", "ab").as_bytes(), script.as_bytes()));
        assert!(!formatter_rewrote(sh, script.replace("\\\n  status", "status").as_bytes(), script.as_bytes()), "a joined continuation is drift");

        let json = Path::new("reference/data.json");
        assert!(formatter_rewrote(json, b"{\n  \"b\": 1,\n  \"a\": [1, 2]\n}\n", br#"{"a":[1,2],"b":1}"#));
        assert!(!formatter_rewrote(json, br#"{"a":[2,1],"b":1}"#, br#"{"a":[1,2],"b":1}"#));
    }

    #[test]
    fn owned_path_globs_lead_with_the_selected_projections() {
        let globs = owned_path_globs(&[Provider::Claude, Provider::Pi]);
        assert_eq!(&globs[..2], [".claude/skills/canon/**".to_string(), ".pi/skills/canon/**".to_string()]);
        for expected in [".canon/ledger/**", ".canon/artifacts/**", ".canon/skills/**", ".canon/hot.db*"] {
            assert!(globs.iter().any(|glob| glob == expected), "{expected} missing from {globs:?}");
        }
        let canon = &globs[2..];
        assert!(canon.windows(2).all(|pair| pair[0] < pair[1]), "sorted, without duplicates: {canon:?}");
    }
}
