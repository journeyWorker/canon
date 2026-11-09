//! Reproducible, privacy-bounded context snapshots for runs.
//!
//! A [`ContextPack`] is a versioned manifest whose selected inputs are copied
//! into an immutable, content-addressed registry under `.canon`.  The pack
//! never walks a repository: callers explicitly select files, while dispatch
//! adds only the bounded capability surface and policy file.  Verification
//! re-reads registry objects and checks every digest, so replay does not
//! resolve today's documents.
use std::ffi::OsStr;
use std::path::{Component, Path, PathBuf};

use canon_model::records::StrategyRef;
use canon_model::secret_scan;
use canon_store::git::{DIFF_SHAPE, GitCall, STATUS_SHAPE};
use canon_store::write_atomic;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::json_out::Report;
use crate::cli_error::{CliError, CliErrorExt as _};

pub const CONTEXT_PACK_FORMAT_VERSION: u32 = 1;
pub const CONTEXT_PACK_DIR: &str = ".canon/context-packs";
pub const PROMPT_REGISTRY_DIR: &str = ".canon/prompts";

#[derive(Debug, thiserror::Error)]
pub enum ContextPackError {
    #[error("reading {path}: {source}")]
    Io { path: String, source: std::io::Error },
    #[error("serializing context pack: {0}")]
    Serialize(String),
    #[error("invalid selected path `{0}`: paths must be relative, stay inside the repository, and contain no symlink")]
    UnsafePath(String),
    #[error("unsupported context input manifest version {0}; expected 1")]
    InvalidManifestVersion(u32),
    #[error("selected context input is missing: {0}")]
    MissingInput(String),
    #[error("selected context input contains plaintext secret material: {label} (line {line}, {rule})")]
    SecretDetected { label: String, line: usize, rule: &'static str },
    #[error("immutable context object {digest} already contains different bytes")]
    ObjectConflict { digest: String },
    #[error("context pack `{0}` is missing")]
    MissingPack(String),
    #[error("context pack `{id}` is stale or tampered: {detail}")]
    Tampered { id: String, detail: String },
    #[error("git identity unavailable: {0}")]
    Git(String),
    #[error("prompt bundle `{name}@{version}` is invalid: {detail}")]
    PromptBundle { name: String, version: String, detail: String },
    /// A directory the store writes into cannot be written, known before the
    /// write (`WRITE_DENIED` or `WRITE_INVALID`).
    #[error("{}", .0.message)]
    Unavailable(CliError),
}

impl ContextPackError {
    /// The shared error contract for this failure (0.20 D3): exit `2` for
    /// a bad input, `1` for an operational or integrity failure.
    pub fn to_cli_error(&self) -> CliError {
        match self {
            Self::UnsafePath(_) => CliError::refused("CONTEXT_PATH_INVALID", self.to_string()).hint("use a repository-relative path that stays inside the repository (no `..`, no absolute path)"),
            Self::InvalidManifestVersion(_) => CliError::refused("CONTEXT_MANIFEST_INVALID", self.to_string()).hint("set `manifestVersion` in the manifest to the version the message names, then run `canon context-pack add` again"),
            Self::MissingInput(_) => CliError::refused("CONTEXT_INPUT_NOT_FOUND", self.to_string()).hint("create the missing file or remove it from the manifest, then run `canon context-pack add` again"),
            Self::SecretDetected { .. } => CliError::refused("CONTEXT_INPUT_INVALID", self.to_string()).reason("SECRET").hint("remove the secret from the input, or drop the file from the manifest; canon never stores a detected secret"),
            Self::MissingPack(_) => CliError::refused("CONTEXT_PACK_NOT_FOUND", self.to_string()).hint("check the pack id (sha256:<64 hex digits>), or create the pack with `canon context-pack add --manifest <file>`"),
            Self::PromptBundle { .. } => CliError::refused("PROMPT_BUNDLE_INVALID", self.to_string()).hint("fix the bundle name, version or manifest (`canon schema prompt add` lists them), then run `canon prompt add` again"),
            Self::Serialize(_) => CliError::refused("SERIALIZE_FAILED", self.to_string()).hint("this is a canon bug: report it together with the command line that produced it"),
            Self::Io { .. } => crate::read_cause::repo_io_error(self.to_string()),
            Self::ObjectConflict { .. } => CliError::failed("CONTEXT_OBJECT_CONFLICT", self.to_string()).hint("an object already stored under this digest differs from the new content: inspect it with `git status` and restore or remove it, then run the command again"),
            Self::Tampered { .. } => CliError::failed("CONTEXT_PACK_CORRUPT", self.to_string()).hint("restore the pack from version control (`git checkout -- <path>`), or create a fresh pack with `canon context-pack add`"),
            Self::Git(_) => CliError::failed("CONTEXT_GIT_REQUIRED", self.to_string()).hint("run from inside a git repository with git installed (`git init` creates one)"),
            Self::Unavailable(error) => error.clone(),
        }
    }
}

fn io(path: &Path, source: std::io::Error) -> ContextPackError {
    ContextPackError::Io { path: path.display().to_string(), source }
}

fn digest(bytes: &[u8]) -> String {
    format!("sha256:{:x}", Sha256::digest(bytes))
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContentRef {
    pub digest: String,
    pub size: u64,
    pub replayable: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct FileSnapshot {
    pub path: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub version: Option<String>,
    pub content: ContentRef,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct RepoIdentity {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub commit: Option<String>,
    pub dirty: bool,
    pub state_digest: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct WorkRefs {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub work_ref: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub task_ref: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PromptSnapshot {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub system: Option<FileSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt: Option<FileSnapshot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub examples: Vec<FileSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub output_schema: Option<FileSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Default, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ToolSnapshot {
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub schemas: Vec<FileSnapshot>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub capability_policy: Option<FileSnapshot>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptBundleSelection {
    pub name: String,
    pub version: String,
}

/// Explicit input selection for a context pack. Empty vectors mean no files
/// are captured; they do not authorize a repository walk.
#[derive(Debug, Clone, Default, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ContextPackSpec {
    /// Version of this explicit input manifest. Zero is the in-process
    /// builder default; serialized manifests MUST state version 1.
    #[serde(default)]
    pub manifest_version: u32,
    pub capability_version: u32,
    pub work_ref: Option<String>,
    pub task_ref: Option<String>,
    pub selected_docs: Vec<PathBuf>,
    pub knowledge_map: Option<PathBuf>,
    pub prompt_system: Option<PathBuf>,
    pub prompt: Option<PathBuf>,
    pub examples: Vec<PathBuf>,
    pub output_schema: Option<PathBuf>,
    pub tool_schemas: Vec<PathBuf>,
    pub capability_policy: Option<PathBuf>,
    pub prompt_bundle: Option<PromptBundleSelection>,
    pub skill_bundle_digest: Option<String>,
    pub provider: Option<String>,
    pub model: Option<String>,
    /// `None` means the canonical `.canon/policy.yaml` path. `Some` is still
    /// constrained by the same safe-path and symlink checks.
    pub policy: Option<PathBuf>,
    pub canonical_knowledge_map_digest: Option<String>,
    pub injected_guidance: Vec<StrategyRef>,
}

impl ContextPackSpec {
    /// The bounded capture used at dispatch. It intentionally selects no
    /// arbitrary repository documents and records policy absence distinctly.
    pub fn for_dispatch(
        provider: Option<String>,
        model: Option<String>,
        skill_bundle_digest: Option<String>,
        canonical_knowledge_map_digest: String,
        capability_version: u32,
        injected_guidance: Vec<StrategyRef>,
    ) -> Self {
        Self {
            capability_version,
            provider,
            model,
            skill_bundle_digest,
            canonical_knowledge_map_digest: Some(canonical_knowledge_map_digest),
            injected_guidance,
            ..Self::default()
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct ContextPack {
    pub format_version: u32,
    pub id: String,
    pub input_manifest_version: u32,
    pub capability_version: u32,
    pub repo: RepoIdentity,
    pub work: WorkRefs,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub knowledge_map: Option<FileSnapshot>,
    pub knowledge_map_digest: String,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub selected_docs: Vec<FileSnapshot>,
    pub prompts: PromptSnapshot,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub prompt_bundle: Option<PromptBundleVersion>,
    pub tools: ToolSnapshot,
    pub skill_bundle_digest: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub provider: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub model: Option<String>,
    /// `None` means policy was absent; `Some` with size zero means an empty
    /// policy file was present. This distinction is replay-significant.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub policy: Option<FileSnapshot>,
    #[serde(default, skip_serializing_if = "Vec::is_empty")]
    pub injected_guidance: Vec<StrategyRef>,
    /// True only for a future digest-only external reference. Current default
    /// capture rejects plaintext secrets, so ordinary packs are replayable.
    pub non_replayable: bool,
}

impl Report for ContextPack {}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct PromptBundleVersion {
    pub name: String,
    pub version: String,
    pub digest: String,
    pub prompts: PromptSnapshot,
}

impl Report for PromptBundleVersion {}

/// Register a prompt bundle version by copying only its explicitly selected
/// files into the same immutable content registry used by ContextPack.
pub fn register_prompt_bundle(
    repo: &Path,
    name: &str,
    version: &str,
    spec: &ContextPackSpec,
) -> Result<PromptBundleVersion, ContextPackError> {
    let repo = repo.canonicalize().map_err(|e| io(repo, e))?;
    PENDING.with(|queue| queue.borrow_mut().clear());
    ensure_registry(&repo)?;
    validate_label(name)?;
    validate_label(version)?;
    let prompts = capture_prompts(&repo, spec)?;
    let unsigned = PromptBundleVersion { name: name.to_string(), version: version.to_string(), digest: String::new(), prompts };
    let digest_bytes = serde_json::to_vec(&unsigned).map_err(|e| ContextPackError::Serialize(e.to_string()))?;
    let bundle = PromptBundleVersion { digest: digest(&digest_bytes), ..unsigned };
    let path = repo.join(PROMPT_REGISTRY_DIR).join(name).join(format!("{version}.json"));
    queue_immutable(&path, serde_json::to_vec_pretty(&bundle).map_err(|e| ContextPackError::Serialize(e.to_string()))?);
    flush_immutable()?;
    Ok(bundle)
}

pub fn select_prompt_bundle(repo: &Path, name: &str, version: &str) -> Result<PromptBundleVersion, ContextPackError> {
    ensure_registry(repo)?;
    validate_label(name)?;
    validate_label(version)?;
    let path = repo.join(PROMPT_REGISTRY_DIR).join(name).join(format!("{version}.json"));
    let bytes = std::fs::read(&path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound {
            ContextPackError::PromptBundle { name: name.to_string(), version: version.to_string(), detail: "version is not registered".to_string() }
        } else {
            io(&path, e)
        }
    })?;
    let bundle: PromptBundleVersion = serde_json::from_slice(&bytes).map_err(|e| ContextPackError::PromptBundle {
        name: name.to_string(),
        version: version.to_string(),
        detail: e.to_string(),
    })?;
    let unsigned = PromptBundleVersion { digest: String::new(), ..bundle.clone() };
    let expected = digest(&serde_json::to_vec(&unsigned).map_err(|e| ContextPackError::Serialize(e.to_string()))?);
    if expected != bundle.digest {
        return Err(ContextPackError::PromptBundle { name: name.to_string(), version: version.to_string(), detail: "bundle digest mismatch".to_string() });
    }
    verify_file_snapshots(repo, &bundle.prompts, &bundle.digest)?;
    Ok(bundle)
}

pub fn create(repo: &Path, spec: &ContextPackSpec) -> Result<ContextPack, ContextPackError> {
    if spec.manifest_version > CONTEXT_PACK_FORMAT_VERSION {
        return Err(ContextPackError::InvalidManifestVersion(spec.manifest_version));
    }
    let repo = repo.canonicalize().map_err(|e| io(repo, e))?;
    PENDING.with(|queue| queue.borrow_mut().clear());
    ensure_registry(&repo)?;
    let repo_identity = repo_identity(&repo)?;
    let knowledge_map = spec.knowledge_map.as_deref().map(|p| capture_file(&repo, p)).transpose()?;
    let knowledge_map_digest = spec.canonical_knowledge_map_digest.clone().unwrap_or_else(|| {
        knowledge_map.as_ref().map_or_else(|| digest(b"canonical-knowledge-map:absent"), |f| f.content.digest.clone())
    });
    let selected_docs = spec.selected_docs.iter().map(|p| capture_file(&repo, p)).collect::<Result<Vec<_>, _>>()?;
    let prompt_bundle = spec.prompt_bundle.as_ref().map(|selection| select_prompt_bundle(&repo, &selection.name, &selection.version)).transpose()?;
    let prompts = match &prompt_bundle {
        Some(bundle) => bundle.prompts.clone(),
        None => capture_prompts(&repo, spec)?,
    };
    let tools = ToolSnapshot {
        schemas: spec.tool_schemas.iter().map(|p| capture_file(&repo, p)).collect::<Result<Vec<_>, _>>()?,
        capability_policy: spec.capability_policy.as_deref().map(|p| capture_file(&repo, p)).transpose()?,
    };
    let policy_path = spec.policy.as_deref().unwrap_or_else(|| Path::new(".canon/policy.yaml"));
    let policy = capture_optional_file(&repo, policy_path)?;
    let mut pack = ContextPack {
        format_version: CONTEXT_PACK_FORMAT_VERSION,
        id: String::new(),
        input_manifest_version: if spec.manifest_version == 0 { CONTEXT_PACK_FORMAT_VERSION } else { spec.manifest_version },
        capability_version: spec.capability_version,
        repo: repo_identity,
        work: WorkRefs { work_ref: spec.work_ref.clone(), task_ref: spec.task_ref.clone() },
        knowledge_map,
        knowledge_map_digest,
        selected_docs,
        prompts,
        prompt_bundle,
        tools,
        skill_bundle_digest: spec.skill_bundle_digest.clone(),
        provider: spec.provider.clone(),
        model: spec.model.clone(),
        policy,
        injected_guidance: spec.injected_guidance.clone(),
        non_replayable: false,
    };
    for strategy in &pack.injected_guidance {
        let label = format!("injected guidance {}", strategy.strategy_id);
        scan_sensitive_text(&strategy.title, &label)?;
        scan_sensitive_text(&strategy.content, &label)?;
    }
    let unsigned = serde_json::to_vec(&pack).map_err(|e| ContextPackError::Serialize(e.to_string()))?;
    pack.id = digest(&unsigned);
    let manifest = serde_json::to_vec_pretty(&pack).map_err(|e| ContextPackError::Serialize(e.to_string()))?;
    let path = pack_path(&repo, &pack.id);
    queue_immutable(&path, manifest);
    flush_immutable()?;
    Ok(pack)
}
/// Load and validate an explicit JSON input manifest without creating a pack.
/// Dispatch uses this to merge runtime lineage into the selected spec while
/// retaining the manifest's exact document/prompt/tool selections.
pub fn load_manifest_spec(repo: &Path, manifest: &Path) -> Result<ContextPackSpec, ContextPackError> {
    let repo = repo.canonicalize().map_err(|e| io(repo, e))?;
    let path = safe_path(&repo, manifest)?;
    let bytes = std::fs::read(&path).map_err(|e| io(&path, e))?;
    let spec: ContextPackSpec = serde_json::from_slice(&bytes).map_err(|e| ContextPackError::Serialize(e.to_string()))?;
    if spec.manifest_version != CONTEXT_PACK_FORMAT_VERSION {
        return Err(ContextPackError::InvalidManifestVersion(spec.manifest_version));
    }
    Ok(spec)
}

/// Load an explicit JSON input manifest and create its immutable pack.
pub fn create_from_manifest(repo: &Path, manifest: &Path) -> Result<ContextPack, ContextPackError> {
    let spec = load_manifest_spec(repo, manifest)?;
    mark_unchanged_if_nothing_written(|| create(repo, &spec))
}

/// Register a prompt bundle described by a repository-local JSON manifest.
pub fn register_prompt_bundle_from_manifest(repo: &Path, name: &str, version: &str, manifest: &Path) -> Result<PromptBundleVersion, ContextPackError> {
    let repo = repo.canonicalize().map_err(|e| io(repo, e))?;
    let path = safe_path(&repo, manifest)?;
    let bytes = std::fs::read(&path).map_err(|e| io(&path, e))?;
    let spec: ContextPackSpec = serde_json::from_slice(&bytes).map_err(|e| ContextPackError::Serialize(e.to_string()))?;
    mark_unchanged_if_nothing_written(|| register_prompt_bundle(&repo, name, version, &spec))
}


pub fn show(repo: &Path, id: &str) -> Result<ContextPack, ContextPackError> {
    if !is_digest(id) {
        return Err(ContextPackError::MissingPack(id.to_string()));
    }
    let repo = repo.canonicalize().map_err(|e| io(repo, e))?;
    let path = pack_path(&repo, id);
    let bytes = std::fs::read(&path).map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound { ContextPackError::MissingPack(id.to_string()) } else { io(&path, e) }
    })?;
    let pack: ContextPack = serde_json::from_slice(&bytes).map_err(|e| ContextPackError::Tampered { id: id.to_string(), detail: e.to_string() })?;
    verify_pack_inner(&repo, &pack)?;
    Ok(pack)
}

pub fn verify(repo: &Path, id: &str) -> Result<(), ContextPackError> {
    let _ = show(repo, id)?;
    Ok(())
}

/// Dispatch helper: create a pack and return its identity and context digest.
pub fn create_dispatch_pack(repo: &Path, spec: &ContextPackSpec, context_bytes: &[u8], capability_version: u32) -> Result<(ContextPack, String), ContextPackError> {
    let mut spec = spec.clone();
    spec.capability_version = capability_version;
    if spec.canonical_knowledge_map_digest.is_none() {
        spec.canonical_knowledge_map_digest = Some(digest(context_bytes));
    }
    let pack = create(repo, &spec)?;
    Ok((pack, digest(context_bytes)))
}

fn verify_pack_inner(repo: &Path, pack: &ContextPack) -> Result<(), ContextPackError> {
    if !is_digest(&pack.id) {
        return Err(ContextPackError::Tampered { id: pack.id.clone(), detail: "invalid pack identity".to_string() });
    }
    if pack.format_version != CONTEXT_PACK_FORMAT_VERSION || pack.input_manifest_version != CONTEXT_PACK_FORMAT_VERSION {
        return Err(ContextPackError::Tampered { id: pack.id.clone(), detail: format!("unsupported pack/input manifest version {}/{}", pack.format_version, pack.input_manifest_version) });
    }
    let mut unsigned = pack.clone();
    unsigned.id.clear();
    let expected = digest(&serde_json::to_vec(&unsigned).map_err(|e| ContextPackError::Serialize(e.to_string()))?);
    if expected != pack.id {
        return Err(ContextPackError::Tampered { id: pack.id.clone(), detail: "manifest identity digest mismatch".to_string() });
    }
    if let Some(file) = &pack.knowledge_map { verify_file(repo, file, &pack.id)?; }
    for file in &pack.selected_docs { verify_file(repo, file, &pack.id)?; }
    verify_file_snapshots(repo, &pack.prompts, &pack.id)?;
    for file in &pack.tools.schemas { verify_file(repo, file, &pack.id)?; }
    if let Some(file) = &pack.tools.capability_policy { verify_file(repo, file, &pack.id)?; }
    if let Some(file) = &pack.policy { verify_file(repo, file, &pack.id)?; }
    Ok(())
}

fn verify_file_snapshots(repo: &Path, prompts: &PromptSnapshot, id: &str) -> Result<(), ContextPackError> {
    if let Some(file) = &prompts.system { verify_file(repo, file, id)?; }
    if let Some(file) = &prompts.prompt { verify_file(repo, file, id)?; }
    for file in &prompts.examples { verify_file(repo, file, id)?; }
    if let Some(file) = &prompts.output_schema { verify_file(repo, file, id)?; }
    Ok(())
}

fn verify_file(repo: &Path, file: &FileSnapshot, id: &str) -> Result<(), ContextPackError> {
    if !is_digest(&file.content.digest) {
        return Err(ContextPackError::Tampered { id: id.to_string(), detail: format!("invalid content digest for {}", file.path) });
    }
    let object = object_path(repo, &file.content.digest);
    let bytes = std::fs::read(&object).map_err(|e| ContextPackError::Tampered { id: id.to_string(), detail: format!("{}: {e}", object.display()) })?;
    let stored: StoredContent = serde_json::from_slice(&bytes).map_err(|e| ContextPackError::Tampered { id: id.to_string(), detail: e.to_string() })?;
    if stored.digest != file.content.digest || digest(&stored.bytes) != file.content.digest || stored.bytes.len() as u64 != file.content.size {
        return Err(ContextPackError::Tampered { id: id.to_string(), detail: format!("content object {} does not match its manifest", file.path) });
    }
    Ok(())
}

#[derive(Debug, Clone, Serialize, Deserialize)]
struct StoredContent {
    digest: String,
    bytes: Vec<u8>,
}

fn capture_prompts(repo: &Path, spec: &ContextPackSpec) -> Result<PromptSnapshot, ContextPackError> {
    Ok(PromptSnapshot {
        system: spec.prompt_system.as_deref().map(|p| capture_file(repo, p)).transpose()?,
        prompt: spec.prompt.as_deref().map(|p| capture_file(repo, p)).transpose()?,
        examples: spec.examples.iter().map(|p| capture_file(repo, p)).collect::<Result<Vec<_>, _>>()?,
        output_schema: spec.output_schema.as_deref().map(|p| capture_file(repo, p)).transpose()?,
    })
}

fn capture_file(repo: &Path, relative: &Path) -> Result<FileSnapshot, ContextPackError> {
    let path = safe_path(repo, relative)?;
    let bytes = std::fs::read(&path).map_err(|e| io(&path, e))?;
    scan_sensitive_text(&String::from_utf8_lossy(&bytes), relative.to_string_lossy().as_ref())?;
    let file_digest = digest(&bytes);
    let object = StoredContent { digest: file_digest.clone(), bytes };
    let object_bytes = serde_json::to_vec(&object).map_err(|e| ContextPackError::Serialize(e.to_string()))?;
    queue_immutable(&object_path(repo, &file_digest), object_bytes);
    Ok(FileSnapshot {
        path: relative.to_string_lossy().replace('\\', "/"),
        version: Some(file_digest.clone()),
        content: ContentRef { digest: file_digest, size: object.bytes.len() as u64, replayable: true },
    })
}

fn capture_optional_file(repo: &Path, relative: &Path) -> Result<Option<FileSnapshot>, ContextPackError> {
    match safe_path(repo, relative) {
        Ok(path) => {
            if !path.exists() { return Ok(None); }
            capture_file(repo, relative).map(Some)
        }
        Err(ContextPackError::MissingInput(_)) if !relative.is_absolute() && !repo.join(relative).exists() => Ok(None),
        Err(error) => Err(error),
    }
}

fn safe_path(repo: &Path, relative: &Path) -> Result<PathBuf, ContextPackError> {
    if relative.is_absolute() || relative.components().any(|c| matches!(c, Component::ParentDir | Component::RootDir | Component::Prefix(_))) {
        return Err(ContextPackError::UnsafePath(relative.display().to_string()));
    }
    let mut current = repo.to_path_buf();
    for component in relative.components() {
        let Component::Normal(part) = component else { return Err(ContextPackError::UnsafePath(relative.display().to_string())); };
        current.push(part);
        let metadata = std::fs::symlink_metadata(&current).map_err(|e| {
            if e.kind() == std::io::ErrorKind::NotFound { ContextPackError::MissingInput(relative.display().to_string()) } else { io(&current, e) }
        })?;
        if metadata.file_type().is_symlink() { return Err(ContextPackError::UnsafePath(relative.display().to_string())); }
    }
    let canonical = current.canonicalize().map_err(|e| {
        if e.kind() == std::io::ErrorKind::NotFound { ContextPackError::MissingInput(relative.display().to_string()) } else { io(&current, e) }
    })?;
    if !canonical.starts_with(repo) { return Err(ContextPackError::UnsafePath(relative.display().to_string())); }
    Ok(canonical)
}
fn ensure_registry(repo: &Path) -> Result<(), ContextPackError> {
    for relative in [".canon", CONTEXT_PACK_DIR, PROMPT_REGISTRY_DIR] {
        let path = repo.join(relative);
        if let Ok(metadata) = std::fs::symlink_metadata(&path) {
            if metadata.file_type().is_symlink() {
                return Err(ContextPackError::UnsafePath(relative.to_string()));
            }
        }
    }
    Ok(())
}


/// The `git status` entries that are not the pack stores' own files, each followed by one NUL. An empty entry (what splitting empty output, or the
/// output's last NUL, leaves) is no entry: a clean repository has no entries at all.
fn filter_registry_status(status: &[u8]) -> Vec<u8> {
    status
        .split(|byte| *byte == 0)
        .filter(|entry| {
            let text = String::from_utf8_lossy(entry);
            !entry.is_empty()
                && !text.contains(".canon/context-packs/")
                && !text.contains(".canon/prompts/")
                && !text.contains(".canon/dispatch/")
        })
        .flat_map(|entry| entry.iter().copied().chain(std::iter::once(0)))
        .collect()
}

/// The diff whose bytes are hashed into the state digest: the same line whatever gitconfig the host holds ([`DIFF_SHAPE`]).
fn state_diff_args() -> Vec<&'static str> {
    let mut args = vec!["diff", "--no-ext-diff", "--binary"];
    args.extend(DIFF_SHAPE);
    args.push("HEAD");
    args
}

/// The status whose entries are hashed into the state digest ([`STATUS_SHAPE`]).
fn state_status_args() -> Vec<&'static str> {
    let mut args = vec!["status", "--porcelain=v1", "-z"];
    args.extend(STATUS_SHAPE);
    args
}

fn repo_identity(repo: &Path) -> Result<RepoIdentity, ContextPackError> {
    let commit = git_output(repo, &["rev-parse", "HEAD"], false)?;
    let diff = git_bytes(repo, &state_diff_args())?;
    let status = filter_registry_status(&git_bytes(repo, &state_status_args())?);
    let dirty = !diff.is_empty() || !status.is_empty();
    let state_digest = if dirty {
        let mut state = Vec::with_capacity(diff.len() + status.len() + 1);
        state.extend_from_slice(&diff);
        state.push(0);
        state.extend_from_slice(&status);
        digest(&state)
    } else {
        digest(b"clean")
    };
    Ok(RepoIdentity { commit, dirty, state_digest })
}

/// A git answer that is a line of text. A call git answered with an exit status is `None` (or an error when
/// `required`); a call that got no answer (it did not start, ran past its deadline or wrote past its limit) is
/// always the error `CONTEXT_GIT_REQUIRED` carries.
fn git_output(repo: &Path, args: &[&str], required: bool) -> Result<Option<String>, ContextPackError> {
    match GitCall::new(repo).args(args).run() {
        Ok(output) => Ok(Some(output.stdout_trimmed())),
        Err(e) if e.is_exit() && required => Err(ContextPackError::Git(e.stderr_text())),
        Err(e) if e.is_exit() => Ok(None),
        Err(e) => Err(ContextPackError::Git(e.to_string())),
    }
}

/// The bytes git wrote; nothing for a call git answered with an exit status (see [`git_output`] for the rest).
fn git_bytes(repo: &Path, args: &[&str]) -> Result<Vec<u8>, ContextPackError> {
    match GitCall::new(repo).args(args).run() {
        Ok(output) => Ok(output.stdout),
        Err(e) if e.is_exit() => Ok(Vec::new()),
        Err(e) => Err(ContextPackError::Git(e.to_string())),
    }
}

fn is_digest(value: &str) -> bool {
    let Some(hex) = value.strip_prefix("sha256:") else { return false; };
    hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_hexdigit())
}

/// The usage error for an operand that is not a pack id: `sha256:` and 64 lower-case hex digits; `None` for a well-formed one (which may still name no pack).
pub fn pack_id_usage_error(id: &str) -> Option<CliError> {
    let well_formed = id.strip_prefix("sha256:").is_some_and(|hex| hex.len() == 64 && hex.bytes().all(|byte| byte.is_ascii_digit() || (b'a'..=b'f').contains(&byte)));
    if well_formed {
        return None;
    }
    Some(CliError::refused("USAGE", format!("invalid value `{id}` for <PACK_ID>: expected `sha256:` followed by 64 lower-case hex digits")).hint("pass the pack id `canon context-pack add` printed (sha256:<64 lower-case hex digits>)").field("PACK_ID"))
}

fn pack_path(repo: &Path, id: &str) -> PathBuf { repo.join(CONTEXT_PACK_DIR).join("manifests").join(format!("{id}.json")) }
fn object_path(repo: &Path, id: &str) -> PathBuf { repo.join(CONTEXT_PACK_DIR).join("objects").join(id.replace(':', "-") + ".json") }

std::thread_local! {
    /// How many immutable files this thread has actually written; a call that
    /// finds every file already stored (byte-identical) leaves it unchanged.
    static FRESH_WRITES: std::cell::Cell<usize> = const { std::cell::Cell::new(0) };
}

/// Runs `op` and says canon found everything it would store already stored
/// (see [`crate::json_out::mark_unchanged`]).
fn mark_unchanged_if_nothing_written<T>(op: impl FnOnce() -> Result<T, ContextPackError>) -> Result<T, ContextPackError> {
    let before = FRESH_WRITES.with(std::cell::Cell::get);
    let out = op()?;
    if FRESH_WRITES.with(std::cell::Cell::get) == before {
        crate::json_out::mark_unchanged();
    }
    Ok(out)
}

thread_local! {
    /// The immutable files a command will store, collected while it captures its inputs and written by [`flush_immutable`].
    static PENDING: std::cell::RefCell<Vec<(std::path::PathBuf, Vec<u8>)>> = const { std::cell::RefCell::new(Vec::new()) };
}

/// Queue an immutable file: nothing is written until every file of the command is known and judged, so a refusal (`CONTEXT_OBJECT_CONFLICT`,
/// `WRITE_DENIED` or `WRITE_INVALID`) leaves the tree as it was.
fn queue_immutable(path: &Path, bytes: Vec<u8>) {
    PENDING.with(|queue| queue.borrow_mut().push((path.to_path_buf(), bytes)));
}

/// Judge every queued file (an existing one that differs is a conflict, a target that cannot take a file is unavailable), then write them.
fn flush_immutable() -> Result<(), ContextPackError> {
    let queued = PENDING.with(|queue| std::mem::take(&mut *queue.borrow_mut()));
    for (path, bytes) in &queued {
        judge_immutable(path, bytes)?;
    }
    for (path, bytes) in &queued {
        write_immutable(path, bytes)?;
    }
    Ok(())
}

fn judge_immutable(path: &Path, bytes: &[u8]) -> Result<(), ContextPackError> {
    if path.exists() {
        let existing = std::fs::read(path).map_err(|e| io(path, e))?;
        if existing != bytes {
            let digest = path.file_stem().and_then(OsStr::to_str).unwrap_or("unknown").to_string();
            return Err(ContextPackError::ObjectConflict { digest });
        }
        return Ok(());
    }
    crate::write_mode::check_writable(path).map_err(|cause| ContextPackError::Unavailable(crate::write_mode::unwritable(path, &cause)))
}

fn write_immutable(path: &Path, bytes: &[u8]) -> Result<(), ContextPackError> {
    if path.exists() {
        let existing = std::fs::read(path).map_err(|e| io(path, e))?;
        if existing != bytes {
            let digest = path.file_stem().and_then(OsStr::to_str).unwrap_or("unknown").to_string();
            return Err(ContextPackError::ObjectConflict { digest });
        }
        return Ok(());
    }
    // A target that cannot take the file is refused before the write (`WRITE_DENIED` or `WRITE_INVALID`), not after it fails.
    crate::write_mode::check_writable(path).map_err(|cause| ContextPackError::Unavailable(crate::write_mode::unwritable(path, &cause)))?;
    write_atomic(path, bytes).map_err(|e| io(path, e))?;
    FRESH_WRITES.with(|n| n.set(n.get() + 1));
    Ok(())
}

fn validate_label(value: &str) -> Result<(), ContextPackError> {
    if value.is_empty() || value == "." || value == ".." || value.contains('/') || value.contains('\\') || value.contains('\0') {
        return Err(ContextPackError::UnsafePath(value.to_string()));
    }
    Ok(())
}

/// Refuses `text` when the shared scanner finds secret material in it, naming `label`, the line and the rule (never the value).
fn scan_sensitive_text(text: &str, label: &str) -> Result<(), ContextPackError> {
    match secret_scan::scan(text).first() {
        Some(span) => Err(ContextPackError::SecretDetected { label: label.to_string(), line: secret_scan::line_of(text, span.start), rule: span.kind.rule_name() }),
        None => Ok(()),
    }
}

/// `canon context-pack verify --json`'s `data`.
#[derive(Debug, Clone, Serialize, JsonSchema)]
#[serde(rename_all = "camelCase")]
pub struct VerifyReport {
    /// The pack id that was checked.
    pub id: String,
    /// `true` when the pack verifies; `false` in the `data` of the failure `CONTEXT_PACK_VERIFY_FAILED` (a pack that cannot be read at all is the refusal `CONTEXT_PACK_CORRUPT` or `CONTEXT_PACK_NOT_FOUND`, with no `data`).
    pub verified: bool,
}

impl Report for VerifyReport {}

#[cfg(test)]
mod tests {
    use super::*;
    use std::fs;
    use tempfile::tempdir;

    fn fixture() -> tempfile::TempDir {
        let dir = tempdir().unwrap();
        fs::create_dir_all(dir.path().join(".canon")).unwrap();
        fs::write(dir.path().join(".canon/policy.yaml"), b"rules: []\n").unwrap();
        dir
    }

    #[test]
    fn same_inputs_have_stable_identity() {
        let dir = fixture();
        fs::write(dir.path().join("doc.md"), b"hello\n").unwrap();
        let spec = ContextPackSpec { selected_docs: vec![PathBuf::from("doc.md")], ..Default::default() };
        let first = create(dir.path(), &spec).unwrap();
        let second = create(dir.path(), &spec).unwrap();
        assert_eq!(first.id, second.id);
    }

    #[test]
    fn repository_mutation_does_not_change_replay_bytes() {
        let dir = fixture();
        fs::write(dir.path().join("doc.md"), b"before\n").unwrap();
        let pack = create(dir.path(), &ContextPackSpec { selected_docs: vec![PathBuf::from("doc.md")], ..Default::default() }).unwrap();
        fs::write(dir.path().join("doc.md"), b"after\n").unwrap();
        verify(dir.path(), &pack.id).unwrap();
    }

    #[test]
    fn missing_selected_input_writes_no_pack_manifest() {
        let dir = fixture();
        let spec = ContextPackSpec { selected_docs: vec![PathBuf::from("missing.md")], ..Default::default() };
        assert!(matches!(create(dir.path(), &spec), Err(ContextPackError::MissingInput(_))));
        assert!(!dir.path().join(CONTEXT_PACK_DIR).exists());
    }

    #[test]
    fn policy_absence_differs_from_empty_bytes() {
        let dir = fixture();
        fs::remove_file(dir.path().join(".canon/policy.yaml")).unwrap();
        let absent = create(dir.path(), &ContextPackSpec::default()).unwrap();
        fs::write(dir.path().join(".canon/policy.yaml"), b"").unwrap();
        let empty = create(dir.path(), &ContextPackSpec::default()).unwrap();
        assert_ne!(absent.id, empty.id);
        assert!(absent.policy.is_none());
        assert_eq!(empty.policy.as_ref().unwrap().content.size, 0);
    }

    #[test]
    fn tampering_content_object_fails_verification() {
        let dir = fixture();
        fs::write(dir.path().join("doc.md"), b"hello\n").unwrap();
        let pack = create(dir.path(), &ContextPackSpec { selected_docs: vec![PathBuf::from("doc.md")], ..Default::default() }).unwrap();
        let object = object_path(dir.path(), &pack.selected_docs[0].content.digest);
        fs::write(object, b"{}").unwrap();
        assert!(matches!(verify(dir.path(), &pack.id), Err(ContextPackError::Tampered { .. })));
    }

    #[test]
    fn plaintext_secret_is_rejected() {
        let dir = fixture();
        fs::write(dir.path().join("secret.txt"), b"api_key=do-not-store\n").unwrap();
        let spec = ContextPackSpec { selected_docs: vec![PathBuf::from("secret.txt")], ..Default::default() };
        assert!(matches!(create(dir.path(), &spec), Err(ContextPackError::SecretDetected { .. })));
    }

    /// dispatch.secret-scan.13 (F2 failure path): the refusal names the file, the line and the rule, never the value, and stores nothing.
    #[test]
    fn dispatch_secret_scan_13_a_secret_refusal_names_the_file_the_line_and_the_rule_and_stores_nothing() {
        let dir = fixture();
        fs::write(dir.path().join("secret.txt"), b"one\ntwo\nkey AKIAIOSFODNN7EXAMPLE\n").unwrap();
        let spec = ContextPackSpec { selected_docs: vec![PathBuf::from("secret.txt")], ..Default::default() };
        let message = create(dir.path(), &spec).unwrap_err().to_string();
        assert_eq!(message, "selected context input contains plaintext secret material: secret.txt (line 3, aws-access-key-id)");
        assert!(!message.contains("AKIA"), "the message does not carry the value");
        assert!(!dir.path().join(CONTEXT_PACK_DIR).exists(), "a refusal stores nothing");
    }

    /// dispatch.secret-scan.17 (F3 failure path): a text is judged the same whether a file holds it or an injected guidance does (each as written, not as its JSON).
    #[test]
    fn dispatch_secret_scan_17_a_text_gets_the_same_verdict_as_a_file_and_as_guidance() {
        let texts = [
            "risk-high",
            "task-list disk-usage mask-all Akiashi",
            "AKIAIOSFODNN7EXAMPLE",
            "first\nsecond\napi_key=abcd1234",
            "{\"api_key\": \"abcd1234efgh\"}",
            "{\"api_key\": \"short\"}",
            "api_key = 'abcd1234'",
            "-----BEGIN OPENSSH PRIVATE KEY-----\nAAAA\n-----END OPENSSH PRIVATE KEY-----",
        ];
        for text in texts {
            let dir = fixture();
            fs::write(dir.path().join("doc.md"), text).unwrap();
            let as_file = create(dir.path(), &ContextPackSpec { selected_docs: vec![PathBuf::from("doc.md")], ..Default::default() });
            let guidance = ContextPackSpec { injected_guidance: vec![StrategyRef::new("s-1", "plain title", text)], ..Default::default() };
            let as_guidance = create(dir.path(), &guidance);
            match (as_file, as_guidance) {
                (Ok(_), Ok(_)) => {}
                (Err(file), Err(guidance)) => {
                    assert!(matches!(file, ContextPackError::SecretDetected { .. }), "{text:?}");
                    assert_eq!(file.to_string().replace("doc.md", "injected guidance s-1"), guidance.to_string(), "{text:?}");
                }
                (file, guidance) => panic!("{text:?}: a file says {:?}, guidance says {:?}", file.is_ok(), guidance.is_ok()),
            }
        }
    }

    /// dispatch.secret-scan.16 (F3): a guidance title is scanned and the refusal names the strategy.
    #[test]
    fn dispatch_secret_scan_16_a_guidance_title_is_scanned_and_the_refusal_names_the_strategy() {
        let dir = fixture();
        let guidance = ContextPackSpec { injected_guidance: vec![StrategyRef::new("s-9", "token\nsk-proj-abcdEFGH_ijkl-mnop1234", "plain")], ..Default::default() };
        let message = create(dir.path(), &guidance).unwrap_err().to_string();
        assert_eq!(message, "selected context input contains plaintext secret material: injected guidance s-9 (line 2, api-secret-key)");
        assert!(!dir.path().join(CONTEXT_PACK_DIR).exists());
    }

    fn git(dir: &Path, args: &[&str]) -> Vec<u8> {
        let out = GitCall::new(dir)
            .args(["-c", "user.name=t", "-c", "user.email=t@example.com", "-c", "commit.gpgsign=false"])
            .args(args)
            .run()
            .unwrap_or_else(|e| panic!("git {args:?}: {e}"));
        out.stdout
    }

    /// A git repository whose one commit holds the fixture (`.canon/policy.yaml`).
    fn committed_fixture() -> tempfile::TempDir {
        let dir = fixture();
        git(dir.path(), &["init", "-q", "-b", "main"]);
        git(dir.path(), &["add", "."]);
        git(dir.path(), &["commit", "-q", "-m", "x"]);
        dir
    }

    /// dispatch.secret-scan.26 (F5): the pack stores' status entries and the empty ones are dropped.
    #[test]
    fn dispatch_secret_scan_26_the_status_entries_of_the_pack_stores_and_the_empty_ones_are_dropped() {
        assert_eq!(filter_registry_status(b""), b"");
        assert_eq!(filter_registry_status(b"\0"), b"");
        assert_eq!(filter_registry_status(b"?? a\0"), b"?? a\0");
        assert_eq!(filter_registry_status(b"?? .canon/context-packs/\0?? a\0 M b\0?? .canon/dispatch/r.json\0?? .canon/prompts/\0"), b"?? a\0 M b\0");
    }

    /// dispatch.secret-scan.24 (F5): a clean repository's pack is not dirty and carries the digest of "clean", also when an earlier pack left files in the pack store.
    #[test]
    fn dispatch_secret_scan_24_a_clean_repository_is_not_dirty_and_a_pack_it_left_does_not_dirty_it() {
        let dir = committed_fixture();
        let first = create(dir.path(), &ContextPackSpec::default()).unwrap();
        assert!(first.repo.commit.is_some());
        assert!(!first.repo.dirty);
        assert_eq!(first.repo.state_digest, digest(b"clean"));
        assert!(dir.path().join(CONTEXT_PACK_DIR).exists(), "the first pack left its files under the pack store");
        let second = create(dir.path(), &ContextPackSpec::default()).unwrap();
        assert_eq!(second.repo, first.repo, "the files the first pack left do not make the repository dirty");
        assert_eq!(second.id, first.id);
        assert_eq!(show(dir.path(), &first.id).unwrap(), first, "a stored pack answers as it did");
        verify(dir.path(), &first.id).unwrap();
    }

    /// dispatch.secret-scan.25 (F5 failure path): a changed tracked file or an untracked file outside the pack stores is dirty, and the digest is the diff, a NUL and each status entry with one NUL.
    #[test]
    fn dispatch_secret_scan_25_a_change_outside_the_pack_stores_is_dirty_and_each_status_entry_ends_in_one_nul() {
        let dir = committed_fixture();
        create(dir.path(), &ContextPackSpec::default()).unwrap();
        fs::write(dir.path().join("stray.txt"), b"x\n").unwrap();
        let untracked = create(dir.path(), &ContextPackSpec::default()).unwrap();
        assert!(untracked.repo.dirty);
        assert_eq!(untracked.repo.state_digest, digest(b"\0?? stray.txt\0"));
        fs::remove_file(dir.path().join("stray.txt")).unwrap();

        fs::write(dir.path().join(".canon/policy.yaml"), b"rules: [a]\n").unwrap();
        let tracked = create(dir.path(), &ContextPackSpec::default()).unwrap();
        assert!(tracked.repo.dirty);
        let mut state = git(dir.path(), &state_diff_args());
        assert!(!state.is_empty());
        state.push(0);
        state.extend_from_slice(b" M .canon/policy.yaml\0");
        assert_eq!(tracked.repo.state_digest, digest(&state));
    }

    /// dispatch.secret-scan.27 (F5): no git repository and no commit are read as no change, as before.
    #[test]
    fn dispatch_secret_scan_27_no_repository_and_no_commit_are_no_change() {
        let plain = tempdir().unwrap();
        let pack = create(plain.path(), &ContextPackSpec::default()).unwrap();
        assert_eq!((pack.repo.commit, pack.repo.dirty), (None, false));
        assert_eq!(pack.repo.state_digest, digest(b"clean"));

        let unborn = tempdir().unwrap();
        git(unborn.path(), &["init", "-q", "-b", "main"]);
        let pack = create(unborn.path(), &ContextPackSpec::default()).unwrap();
        assert_eq!((pack.repo.commit, pack.repo.dirty), (None, false));
        assert_eq!(pack.repo.state_digest, digest(b"clean"));
    }
}
