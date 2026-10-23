//! The evidence artifact store (0.14 D4): bound evidence stays provable.
//!
//! # Why
//! `canon evidence add --artifact/--report` records a file's sha256. Until
//! 0.14 nothing kept the bytes behind that digest, and nothing re-checked
//! it: a smoke script that rewrites `reports/smoke.json` on every run
//! silently orphaned every record bound to it, and `canon gate check`
//! stayed clean (dogfood eval 1, F9). The digest named bytes nobody could
//! produce any more.
//!
//! # The store
//! `canon evidence add` copies each bound file into
//! `<repo>/.canon/artifacts/sha256/<hex>` ([`canon_model::paths::
//! ARTIFACTS_SHA256_DIR`]), named by its own digest. The directory is
//! git-tracked: the bytes are authored evidence, and no source
//! regenerates a report's earlier contents. Writes go through a temp file
//! in the same directory and a rename, so a reader never sees a partial
//! blob, and a blob whose digest is already stored intact is not written
//! again. A record binding several files stores them all or none: each
//! is staged ([`StoreLock::stage`]: copied to a temp file and re-hashed)
//! first, and only when every one has staged are they published
//! ([`StoreLock::publish`]); a rename that fails rolls back the ones
//! before it. A file larger than the size limit
//! ([`DEFAULT_MAX_ARTIFACT_MIB`], raised with [`MAX_ARTIFACT_FLAG`]) is
//! refused rather than stored.
//!
//! Writes are serialized by a repo-level lock ([`StoreLock`],
//! [`canon_model::paths::ARTIFACT_STORE_LOCK_FILE`]) held from the
//! "already stored?" check through the last rename or rollback. Without
//! it, a rollback could remove a blob another writer had just published,
//! or had found intact and counted as stored — and that writer's record
//! would then name bytes the store lacks. For the same reason a blob
//! whose target already holds an intact copy when it is published (the
//! same digest twice in one record, or a canon without the lock writing
//! it meanwhile) counts as stored: it is not renamed over, and a
//! rollback only ever removes what its own transaction renamed into
//! place.
//!
//! # What the gate checks ([`ArtifactStoreCheck`])
//! Every attachment on every LATEST evidence record: the latest record
//! per task, and the latest per `(project_id, scenario_id)`, the same
//! last-wins rule every other evidence fold uses — and, beside it, the
//! latest AUDIT record per `(project_id, scenario_id)` (0.15 D3), whose
//! counterexample reports are bound like any other. A superseded record
//! is not checked; re-attesting is how stale evidence is fixed.
//!
//! - The stored blob exists and hashes to the recorded digest: clean.
//! - The blob is missing, and the working-tree path still hashes to the
//!   recorded digest: clean, with an advisory ([`unstored_attachments`])
//!   naming `canon artifacts store`. This is the back-compat path for
//!   records written before the store existed (0.11–0.13).
//! - Anything else (a corrupt blob, or a missing blob and a working-tree
//!   file that is gone or changed): a `stale-evidence` violation naming
//!   the path, the recorded digest, and what the blob and working tree
//!   hold now. It says which: a blob that exists but no longer hashes
//!   to its name "does not match its name, so its bytes were altered";
//!   a missing one means "the bound bytes are gone".
//!
//! # Review artifacts (0.16 D5, 0.18 D5)
//! `canon review scenario --artifact` stores what the reviewer looked at
//! through the same [`StoreLock`] and records `{path, sha256}` on the
//! review. The check covers every CURRENT in-effect review
//! ([`crate::review_gate::current_reviews`]): the latest per
//! `(project_id, scenario_id, reviewer)`, less any that went stale
//! against its scenario. A stale review is superseded (the gate reads
//! the newer review, or reports the missing one), so what it bound is not
//! re-checked, the way a superseded evidence record's is not. The rule is
//! the evidence rule above: an intact blob is clean; a missing blob whose
//! working-tree file still matches is an advisory
//! ([`unstored_attachments`], naming `canon artifacts store`, which stores
//! review artifacts too); anything else is `stale-evidence` on the
//! review's scenario, and the detail names the reviewer, the review's
//! actor and its time.
//!
//! The check is NOT narrowed to `spec_coverage.scope`. A binding is a
//! claim the record makes about bytes, whatever its subject's lifecycle
//! status; scope belongs to the coverage policy, and a repo with no
//! `spec_coverage` section would otherwise never have its bindings
//! checked at all — exactly the repo F9 came from. Records with no
//! attachment cost nothing here.
//!
//! # Object-store backend (0.23 D3)
//! `canon.yaml`'s `artifact_store: { backend: local | <tier>, prefix: artifacts/ }`
//! moves the bytes off git: [`ArtifactStore`] is the local directory above
//! (the default, no credentials) or the object-store tier named by
//! `backend`, keyed `<prefix>sha256/<digest>`. Records keep only digests.
//! The rules above hold unchanged, asked of the store instead of the
//! directory, with two differences: the store is asked once per distinct
//! digest (a HEAD and its size/digest metadata; `canon artifacts verify
//! --full` downloads and re-hashes), and the working-tree fallback does
//! not apply — a blob the store lacks is `stale-evidence` whatever the
//! working tree holds, because the fallback exists for records older than
//! the store, and `canon artifacts store` uploads those. A store that
//! cannot be reached or attached ([`Observed::Unavailable`],
//! [`StoreError::Unattached`]) is a violation with a hint, never a pass.

use std::collections::{BTreeMap, BTreeSet};
use std::fs::File;
use std::io::{Read, Write};
use std::marker::PhantomData;
use std::path::{Component, Path, PathBuf};
use std::time::Duration;

use canon_model::{paths, EvidenceAttachment, EvidenceRecord, Review, ReviewArtifact};
use canon_store::artifact_blobs::{ArtifactsBackend, ArtifactsConfig, BlobError, ObjectBlobStore, Probe};
use canon_store::file_lock::{FileLock, FileLockError};
use sha2::{Digest, Sha256};

use crate::context::{GateCheck, GateContext};
use crate::coverage::CellSubject;
use crate::failure_class::{FailureClass, Violation};
use crate::review_gate::current_reviews;
use crate::spec_coverage::latest_by_key;

/// The documented default size limit for one stored file, in MiB.
pub const DEFAULT_MAX_ARTIFACT_MIB: u64 = 25;

/// The flag that raises [`DEFAULT_MAX_ARTIFACT_MIB`], named in every
/// refusal so the operator never has to look it up.
pub const MAX_ARTIFACT_FLAG: &str = "--max-artifact-mib";

/// Bytes in one MiB, for converting the limit flag.
pub const MIB: u64 = 1024 * 1024;

/// Whether `sha256` is exactly 64 lowercase hex characters — the only
/// shape a blob may be named by. Checked before any path is built from
/// it, so a digest can never name `..`, a separator, or anything else.
pub fn is_sha256_hex(sha256: &str) -> bool {
    sha256.len() == 64 && sha256.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// `<repo>/.canon/artifacts/sha256`.
pub fn store_dir(repo: &Path) -> PathBuf {
    repo.join(paths::ARTIFACTS_SHA256_DIR)
}

/// The blob path for `sha256` (64 lowercase hex characters — the model
/// refuses any other digest at deserialize time, so it is a safe file
/// name).
pub fn blob_path(repo: &Path, sha256: &str) -> PathBuf {
    store_dir(repo).join(sha256)
}

/// The repository-relative blob path, as printed to operators.
pub fn blob_display(sha256: &str) -> String {
    format!("{}/{sha256}", paths::ARTIFACTS_SHA256_DIR)
}

/// Stream `reader` through sha256.
pub fn sha256_reader(mut reader: impl Read) -> std::io::Result<String> {
    let mut hasher = Sha256::new();
    std::io::copy(&mut reader, &mut hasher)?;
    Ok(format!("{:x}", hasher.finalize()))
}

/// What the store holds for `sha256`, WITHOUT following links: only a
/// regular file at `<store>/<sha256>`, inside a store directory that is
/// itself a real directory, counts. A symlink there (pointing anywhere)
/// or a store directory that is a symlink reads as [`Observed::Missing`]
/// — a link can make an outside file look stored, and the gate must
/// never vouch for bytes that live outside the store.
pub fn stored_blob(repo: &Path, sha256: &str) -> Observed {
    if !is_sha256_hex(sha256) || !store_dir_is_real(repo) {
        return Observed::Missing;
    }
    let path = blob_path(repo, sha256);
    match std::fs::symlink_metadata(&path) {
        Ok(meta) if meta.file_type().is_file() => Observed::of(&path),
        _ => Observed::Missing,
    }
}

/// The store directory exists as a real directory (not a symlink), and
/// so does each `.canon`/`artifacts` component above it.
fn store_dir_is_real(repo: &Path) -> bool {
    let mut path = repo.to_path_buf();
    for component in Path::new(paths::ARTIFACTS_SHA256_DIR).components() {
        path.push(component);
        match std::fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_dir() => {}
            _ => return false,
        }
    }
    true
}

/// Create `.canon`, `artifacts`, `sha256` one component at a time from
/// the repo root, never following a link: an existing component must be
/// a real directory (checked with `symlink_metadata`), a missing one is
/// made with `create_dir` and checked again. `create_dir_all` would
/// follow a symlinked `.canon/artifacts` and create `sha256` outside the
/// store before any check could refuse it.
fn create_store_dir(repo: &Path) -> Result<PathBuf, StoreError> {
    let mut path = repo.to_path_buf();
    for component in Path::new(paths::ARTIFACTS_SHA256_DIR).components() {
        path.push(component);
        match std::fs::symlink_metadata(&path) {
            Ok(meta) if meta.file_type().is_dir() => continue,
            Ok(_) => return Err(StoreError::UnsafeStoreDir),
            Err(e) if e.kind() == std::io::ErrorKind::NotFound => {}
            Err(e) => return Err(e.into()),
        }
        match std::fs::create_dir(&path) {
            Ok(()) => {}
            Err(e) if e.kind() == std::io::ErrorKind::AlreadyExists => {}
            Err(e) => return Err(e.into()),
        }
        if !std::fs::symlink_metadata(&path).is_ok_and(|meta| meta.file_type().is_dir()) {
            return Err(StoreError::UnsafeStoreDir);
        }
    }
    Ok(path)
}

/// What a file at a path, or a blob in a store, holds now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Observed {
    /// No readable file there.
    Missing,
    /// A file whose bytes hash to this digest.
    Sha256(String),
    /// An object that contradicts its own name; the text says how.
    Altered(String),
    /// The store could not be asked (offline, denied); the text says why.
    /// Never read as present, never read as absent.
    Unavailable(String),
}

/// What each file hashed so far in this process held, while its size and
/// modification time are unchanged ([`canon_store::git_tier::FileStamp`]).
/// `canon gate check` asks about the same blob once per record and review
/// that binds it; the bytes are read and hashed once.
static OBSERVED: std::sync::LazyLock<std::sync::Mutex<std::collections::HashMap<PathBuf, (canon_store::git_tier::FileStamp, String)>>> = std::sync::LazyLock::new(Default::default);

impl Observed {
    fn of(path: &Path) -> Self {
        let stamp = std::fs::metadata(path).ok().and_then(|meta| canon_store::git_tier::FileStamp::of(&meta));
        if let Some(stamp) = stamp {
            let seen = OBSERVED.lock().unwrap_or_else(|poisoned| poisoned.into_inner());
            if let Some((_, sha)) = seen.get(path).filter(|(was, _)| *was == stamp) {
                return Observed::Sha256(sha.clone());
            }
        }
        match File::open(path).and_then(sha256_reader) {
            Ok(sha) => {
                if let Some(stamp) = stamp {
                    OBSERVED.lock().unwrap_or_else(|poisoned| poisoned.into_inner()).insert(path.to_path_buf(), (stamp, sha.clone()));
                }
                Observed::Sha256(sha)
            }
            Err(_) => Observed::Missing,
        }
    }

    fn matches(&self, sha256: &str) -> bool {
        matches!(self, Observed::Sha256(actual) if actual == sha256)
    }

    /// `missing`, the digest, or what is wrong with the object.
    pub fn as_str(&self) -> &str {
        match self {
            Observed::Missing => "missing",
            Observed::Sha256(sha) => sha,
            Observed::Altered(why) | Observed::Unavailable(why) => why,
        }
    }
}

/// Whether [`StoreLock::store`] wrote a blob or found it already stored
/// intact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreOutcome {
    Stored,
    AlreadyStored,
}

/// Why a store write wrote nothing.
#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    /// The bytes read now hash differently from the digest being stored:
    /// the file changed after it was bound. Nothing is stored.
    #[error("its bytes now hash to {actual}, not the bound {expected}")]
    Changed { expected: String, actual: String },
    /// The digest is not 64 lowercase hex characters; no path is built.
    #[error("`{0}` is not a sha256 digest (64 lowercase hex characters)")]
    InvalidDigest(String),
    /// A component of `.canon/artifacts/sha256` exists but is not a real
    /// directory (e.g. a symlink); writing through it could land outside
    /// the store.
    #[error("{} is not a real directory; refusing to write the artifact store through it", paths::ARTIFACTS_SHA256_DIR)]
    UnsafeStoreDir,
    /// Another canon process held [`StoreLock`] for the whole wait.
    #[error(
        "another canon process has been writing the artifact store for {}s (it holds `{}`); retry once it finishes",
        .0.as_secs(),
        paths::ARTIFACT_STORE_LOCK_FILE
    )]
    Locked(Duration),
    /// `canon.yaml` or its `artifact_store:` block cannot be used.
    #[error("{0}")]
    Config(String),
    /// The object-store tier `artifact_store.backend` names cannot be attached
    /// (credentials or bucket unset).
    #[error("the artifact store tier `{tier}` cannot be attached: {reason}")]
    Unattached { tier: String, reason: String },
    /// The object store was reached for, and failed or refused.
    #[error("the artifact store tier `{tier}` failed: {reason}")]
    Remote { tier: String, reason: String },
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

/// The next step when the object store cannot be attached or reached.
pub const OBJECT_STORE_HINT: &str = "check that the object store is reachable and that CANON_S3_ENDPOINT, CANON_S3_ACCESS_KEY, CANON_S3_SECRET_KEY and the tier's bucket variable are set, then run the command again (`canon artifacts verify` re-checks the store)";

impl StoreError {
    /// The next step for the operator.
    pub fn hint(&self) -> &'static str {
        match self {
            StoreError::Changed { .. } => "re-run what produced the file and bind it again, so the digest matches its bytes",
            StoreError::InvalidDigest(_) => "a recorded digest must be 64 lowercase hex characters; fix or re-create the record",
            StoreError::UnsafeStoreDir | StoreError::Io(_) => "check that the artifact store is writable (permissions, disk space), then run the command again",
            StoreError::Locked(_) => "wait for the other canon process to finish, then run the command again",
            StoreError::Config(_) => "fix the `artifact_store:` block of canon.yaml (`canon init --check` names the problem), or set `artifact_store: { backend: local }`",
            StoreError::Unattached { .. } | StoreError::Remote { .. } => OBJECT_STORE_HINT,
        }
    }
}

/// How long a writer of the artifact store waits for another one to
/// finish before giving up.
const STORE_LOCK_WAIT: Duration = Duration::from_secs(30);

/// The repo's artifact-store lock ([`paths::ARTIFACT_STORE_LOCK_FILE`]),
/// held until this value is dropped. Every write goes through it:
/// [`StoreLock::stage`], [`StoreLock::publish`] and [`StoreLock::store`]
/// are its methods, and a [`StagedBlob`] cannot outlive it, so the
/// "already stored?" check, the renames and any rollback all happen
/// under one hold (module doc).
///
/// An OS advisory lock on the file ([`canon_store::file_lock`]: `flock`
/// on Unix, `LockFileEx` on Windows), not the file's existence: closing
/// the file releases it, and so does the holder's death, however it
/// died, so a killed writer never leaves a stale lock behind and the
/// file itself is never deleted. Waiting is bounded ([`STORE_LOCK_WAIT`],
/// polled), and running out is [`StoreError::Locked`], naming the file.
#[derive(Debug)]
pub struct StoreLock {
    repo: PathBuf,
    _lock: FileLock,
}

impl StoreLock {
    /// Take the artifact-store lock of the repo rooted at `repo`.
    pub fn acquire(repo: &Path) -> Result<StoreLock, StoreError> {
        Self::acquire_within(repo, STORE_LOCK_WAIT)
    }

    fn acquire_within(repo: &Path, wait: Duration) -> Result<StoreLock, StoreError> {
        match FileLock::acquire(&repo.join(paths::ARTIFACT_STORE_LOCK_FILE), wait) {
            Ok(lock) => Ok(StoreLock { repo: repo.to_path_buf(), _lock: lock }),
            Err(FileLockError::Busy(waited)) => Err(StoreError::Locked(waited)),
            Err(FileLockError::Io(e)) => Err(e.into()),
        }
    }

    /// Store the bytes `reader` yields as the blob for `expected`:
    /// [`StoreLock::stage`] then [`StoreLock::publish`]. Skips the write
    /// when an intact blob is already there. Refuses
    /// ([`StoreError::Changed`]), writing nothing, when the bytes do not
    /// hash to `expected` — the caller recorded that digest, and the
    /// store must hold exactly those bytes or nothing.
    pub fn store(&self, expected: &str, reader: impl Read) -> Result<StoreOutcome, StoreError> {
        let Some(blob) = self.stage(expected, reader)? else {
            return Ok(StoreOutcome::AlreadyStored);
        };
        match self.publish(vec![blob]).map_err(|(_, e)| e)? {
            0 => Ok(StoreOutcome::AlreadyStored),
            _ => Ok(StoreOutcome::Stored),
        }
    }

    /// Copy the bytes `reader` yields into a temp file in the store
    /// directory, hashing them on the way. `None` when an intact blob
    /// for `expected` is already stored; [`StoreError::Changed`], with
    /// the temp file removed, when the bytes do not hash to `expected` —
    /// identical bytes are also the size that was checked when they were
    /// bound.
    ///
    /// "Already stored" is decided by [`stored_blob`], which never
    /// follows a link: a pre-existing symlink (or any non-file) at the
    /// blob path is replaced by [`StoreLock::publish`]'s rename, which
    /// swaps the link itself, never its target.
    pub fn stage(&self, expected: &str, mut reader: impl Read) -> Result<Option<StagedBlob<'_>>, StoreError> {
        if !is_sha256_hex(expected) {
            return Err(StoreError::InvalidDigest(expected.to_string()));
        }
        if stored_blob(&self.repo, expected).matches(expected) {
            return Ok(None);
        }
        let dir = create_store_dir(&self.repo)?;
        let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
        let temp = dir.join(format!(".tmp-{expected}-{}-{nanos}", std::process::id()));
        let mut file = File::options().write(true).create_new(true).open(&temp)?;
        // From here on, dropping `staged` removes the temp file.
        let staged = StagedBlob { temp: Some(temp), sha256: expected.to_string(), _lock: PhantomData };
        let written = copy_hashed(&mut reader, &mut file).and_then(|actual| file.sync_all().map(|()| actual));
        drop(file);
        let actual = written?;
        if actual != expected {
            return Err(StoreError::Changed { expected: expected.to_string(), actual });
        }
        Ok(Some(staged))
    }

    /// Rename every staged blob into place, in order, returning how many
    /// this call wrote. Callers stage ALL their files first, so a file
    /// that fails to read or hash never leaves a sibling published.
    ///
    /// A blob whose target already holds an intact copy (the same digest
    /// staged twice in one call, or a canon without this lock writing
    /// it meanwhile) counts as stored: its temp file is dropped and the
    /// target is left alone. If a rename fails, only the targets this
    /// call itself renamed into place are removed (best effort), the
    /// rest are dropped unpublished, and the error carries the failing
    /// blob's index.
    pub fn publish(&self, staged: Vec<StagedBlob<'_>>) -> Result<usize, (usize, StoreError)> {
        self.publish_paths(staged).map(|renamed| renamed.len())
    }

    /// [`StoreLock::publish`], returning the targets this call renamed
    /// into place. A caller that writes more after publishing (a batch of
    /// records naming these blobs, 0.18) keeps the lock and, if a later
    /// write fails, removes exactly these with [`StoreLock::unpublish`]:
    /// a blob already stored, or published by another writer, is never
    /// among them.
    pub fn publish_paths(&self, staged: Vec<StagedBlob<'_>>) -> Result<Vec<PathBuf>, (usize, StoreError)> {
        let mut renamed: Vec<PathBuf> = Vec::new();
        for (index, mut blob) in staged.into_iter().enumerate() {
            if stored_blob(&self.repo, &blob.sha256).matches(&blob.sha256) {
                continue;
            }
            let target = blob_path(&self.repo, &blob.sha256);
            let temp = blob.temp.take().expect("an unpublished StagedBlob always holds its temp file");
            if let Err(e) = std::fs::rename(&temp, &target) {
                blob.temp = Some(temp);
                self.unpublish(&renamed);
                return Err((index, e.into()));
            }
            renamed.push(target);
        }
        Ok(renamed)
    }

    /// Remove `published` (what [`StoreLock::publish_paths`] returned
    /// under this same lock), best effort.
    pub fn unpublish(&self, published: &[PathBuf]) {
        for target in published {
            let _ = std::fs::remove_file(target);
        }
    }
}

/// A blob written, hashed and synced to a temp file in the store
/// directory, not yet visible under its name; it borrows the
/// [`StoreLock`] it was staged under. [`StoreLock::publish`] renames it
/// into place; dropped unpublished, its temp file is removed, so the
/// store is left as it was.
#[derive(Debug)]
pub struct StagedBlob<'l> {
    temp: Option<PathBuf>,
    sha256: String,
    _lock: PhantomData<&'l StoreLock>,
}

impl Drop for StagedBlob<'_> {
    fn drop(&mut self) {
        if let Some(temp) = self.temp.take() {
            let _ = std::fs::remove_file(temp);
        }
    }
}

/// Copy `reader` into `file`, returning the sha256 of the bytes copied.
fn copy_hashed(reader: &mut impl Read, file: &mut File) -> std::io::Result<String> {
    let mut hasher = Sha256::new();
    let mut buffer = [0u8; 64 * 1024];
    loop {
        let read = reader.read(&mut buffer)?;
        if read == 0 {
            break;
        }
        hasher.update(&buffer[..read]);
        file.write_all(&buffer[..read])?;
    }
    Ok(format!("{:x}", hasher.finalize()))
}

/// Where a repo keeps the bytes behind its evidence digests (0.23 D3):
/// the local `.canon/artifacts/sha256` store, git-tracked and the
/// default, or an object-store tier chosen by `canon.yaml`'s
/// `artifact_store: { backend: <tier>, prefix: artifacts/ }`.
///
/// With an object store, a write uploads by digest key
/// (`<prefix>sha256/<digest>`), reads it back, and keeps no local copy;
/// the gate asks the store (one HEAD plus size/digest metadata per
/// distinct digest; [`ArtifactStore::observe`] with `full` downloads and
/// re-hashes). A store that cannot be reached is
/// [`Observed::Unavailable`], which fails the gate with a hint — absence
/// is only ever the store's own "not found", and the working-tree
/// fallback of the local store does not apply.
#[derive(Debug)]
pub enum ArtifactStore {
    Local,
    Object(ObjectArtifacts),
}

/// The object-store side of [`ArtifactStore`]: the tier's name and the
/// digest-keyed blobs in its bucket.
#[derive(Debug)]
pub struct ObjectArtifacts {
    pub tier: String,
    blobs: ObjectBlobStore,
}

/// `<repo>/canon.yaml`'s `artifact_store:` block, resolved; the local default
/// when the file or the block is absent.
pub fn artifacts_config(repo: &Path) -> Result<ArtifactsConfig, StoreError> {
    let path = repo.join("canon.yaml");
    let content = match std::fs::read_to_string(&path) {
        Ok(content) => content,
        Err(e) if e.kind() == std::io::ErrorKind::NotFound => return Ok(ArtifactsConfig::local()),
        Err(e) => return Err(StoreError::Config(format!("reading `{}`: {e}", path.display()))),
    };
    ArtifactsConfig::from_yaml_at(&content, repo).map_err(|e| StoreError::Config(e.to_string()))
}

impl ArtifactStore {
    /// The store `<repo>/canon.yaml` configures. Builds the client for
    /// an object store (credentials from the tier's own configuration);
    /// performs no network I/O.
    pub fn open(repo: &Path) -> Result<Self, StoreError> {
        Self::from_config(artifacts_config(repo)?)
    }

    /// The store `config` names, as [`ArtifactStore::open`].
    pub fn from_config(config: ArtifactsConfig) -> Result<Self, StoreError> {
        match config.backend {
            ArtifactsBackend::Local => Ok(ArtifactStore::Local),
            ArtifactsBackend::Tier { name, config: tier } => match ObjectBlobStore::attach(&tier, &config.prefix) {
                Ok(blobs) => Ok(ArtifactStore::Object(ObjectArtifacts { tier: name, blobs })),
                Err(e) => Err(StoreError::Unattached { tier: name, reason: e.to_string() }),
            },
        }
    }

    pub fn is_local(&self) -> bool {
        matches!(self, ArtifactStore::Local)
    }

    /// The blob's name as printed to operators.
    pub fn display(&self, sha256: &str) -> String {
        match self {
            ArtifactStore::Local => blob_display(sha256),
            ArtifactStore::Object(object) => format!("{}:{}", object.tier, object.blobs.key(sha256)),
        }
    }

    /// Where the store keeps blobs, for a sentence ("under …").
    pub fn location(&self) -> String {
        match self {
            ArtifactStore::Local => format!("{}/", paths::ARTIFACTS_SHA256_DIR),
            ArtifactStore::Object(object) => format!("the `{}` object store ({})", object.tier, object.blobs.key("")),
        }
    }

    /// What the store holds for `sha256`. The local store reads the file
    /// (no links followed, [`stored_blob`]); an object store answers
    /// from one HEAD, or downloads and re-hashes with `full`.
    pub fn observe(&self, repo: &Path, sha256: &str, full: bool) -> Observed {
        match self {
            ArtifactStore::Local => stored_blob(repo, sha256),
            ArtifactStore::Object(object) => {
                let probe = if full { object.blobs.verify_full(sha256) } else { object.blobs.probe(sha256) };
                match probe {
                    Ok(Probe::Missing) => Observed::Missing,
                    Ok(Probe::Present { .. }) => Observed::Sha256(sha256.to_string()),
                    Ok(Probe::Altered(why)) => Observed::Altered(why),
                    Err(BlobError::InvalidDigest(_)) => Observed::Missing,
                    Err(e) => Observed::Unavailable(format!("unavailable: {e}")),
                }
            }
        }
    }
}

impl ObjectArtifacts {
    /// Upload the bytes `reader` yields as the blob for `expected`, read
    /// it back, keep nothing locally. [`StoreError::Changed`], with
    /// nothing uploaded, when the bytes do not hash to `expected`;
    /// [`StoreOutcome::AlreadyStored`] when an object of that size is
    /// already there.
    pub fn upload(&self, expected: &str, mut reader: impl Read) -> Result<StoreOutcome, StoreError> {
        let mut bytes = Vec::new();
        reader.read_to_end(&mut bytes)?;
        match self.blobs.put(expected, bytes) {
            Ok(true) => Ok(StoreOutcome::Stored),
            Ok(false) => Ok(StoreOutcome::AlreadyStored),
            Err(e) => Err(self.error(e)),
        }
    }

    /// Every digest the store holds.
    pub fn list(&self) -> Result<Vec<String>, StoreError> {
        self.blobs.list().map_err(|e| self.error(e))
    }

    fn error(&self, e: BlobError) -> StoreError {
        match e {
            BlobError::Changed { expected, actual } => StoreError::Changed { expected, actual },
            BlobError::InvalidDigest(digest) => StoreError::InvalidDigest(digest),
            other => StoreError::Remote { tier: self.tier.clone(), reason: other.to_string() },
        }
    }
}

/// The working-tree file a repository-relative attachment path names,
/// or `None` when the recorded path would leave the repository (a
/// hand-written record; the CLI only ever records in-repo paths).
pub fn working_tree_path(repo: &Path, relative: &str) -> Option<PathBuf> {
    let path = Path::new(relative);
    if path.components().all(|c| matches!(c, Component::Normal(_) | Component::CurDir)) {
        Some(repo.join(path))
    } else {
        None
    }
}

fn working_tree(repo: &Path, relative: &str) -> Observed {
    working_tree_path(repo, relative).map_or(Observed::Missing, |path| Observed::of(&path))
}

/// The latest declared record per task and per `(project_id,
/// scenario_id)`, then the latest audit per `(project_id, scenario_id)`
/// (module doc), each once, every one carrying an attachment.
fn latest_records(ctx: &GateContext) -> Vec<&EvidenceRecord> {
    let by_task: Vec<&EvidenceRecord> = ctx.evidence.iter().filter(|r| r.task_id.is_some()).collect();
    let by_scenario: Vec<&EvidenceRecord> = ctx.evidence.iter().filter(|r| r.scenario_id.is_some()).collect();
    let mut latest: BTreeSet<*const EvidenceRecord> = BTreeSet::new();
    for record in latest_by_key(&by_task, |r| r.task_id.clone()) {
        latest.insert(*record as *const _);
    }
    for record in latest_by_key(&by_scenario, |r| (r.project_id.clone(), r.scenario_id.clone())) {
        latest.insert(*record as *const _);
    }
    let declared = ctx.evidence.iter().filter(|r| latest.contains(&(*r as *const _)));
    let audits = crate::audit::latest_audits(ctx).into_values();
    declared.chain(audits).filter(|r| !r.attachments.is_empty()).collect()
}

/// Each distinct `(path, sha256)` a record binds (one report can carry
/// several case attachments with the same bytes).
fn distinct_files(record: &EvidenceRecord) -> Vec<&EvidenceAttachment> {
    let mut seen = BTreeSet::new();
    record.attachments.iter().filter(|a| seen.insert((a.path.as_str(), a.sha256.as_str()))).collect()
}

fn subject_of(record: &EvidenceRecord) -> String {
    CellSubject::of(record).map(|s| s.as_str().to_string()).unwrap_or_default()
}

/// Who binds a file: a latest evidence record, or a current review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Binder {
    Evidence,
    Review { reviewer: String, actor: String, at: String },
}

/// One `(path, sha256)` binding the gate re-checks: a distinct attachment
/// of a latest evidence record, or of a current in-effect review.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BoundFileRef {
    pub subject: String,
    pub path: String,
    pub sha256: String,
    pub binder: Binder,
}

/// Every binding the gate re-checks (module doc): the latest evidence's,
/// then the current reviews'.
pub fn bound_files(ctx: &GateContext) -> Vec<BoundFileRef> {
    let mut out = Vec::new();
    for record in latest_records(ctx) {
        for attachment in distinct_files(record) {
            out.push(BoundFileRef { subject: subject_of(record), path: attachment.path.clone(), sha256: attachment.sha256.clone(), binder: Binder::Evidence });
        }
    }
    for review in current_reviews(ctx) {
        for artifact in distinct_review_files(review) {
            out.push(BoundFileRef {
                subject: review.target().to_string(),
                path: artifact.path.clone(),
                sha256: artifact.sha256.clone(),
                binder: Binder::Review {
                    reviewer: review.reviewer.clone(),
                    actor: review.envelope.actor.agent_id.to_string(),
                    at: review.envelope.at.to_rfc3339_opts(chrono::SecondsFormat::Secs, true),
                },
            });
        }
    }
    out
}

/// How a bound file fared against the store.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FileStatus {
    /// The store holds the blob, intact.
    Stored,
    /// Local store only: no blob, but the working-tree file still hashes
    /// to the digest (module doc's back-compat path).
    WorkingTreeOnly,
    /// Missing, altered, or the store could not be asked.
    Failed,
}

/// One bound file with what the store and the working tree hold for it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FileCheck {
    pub file: BoundFileRef,
    pub blob: Observed,
    pub tree: Observed,
    pub status: FileStatus,
}

/// Judge every [`bound_files`] entry against `store` ([`check_files`]).
pub fn check_bound_files(ctx: &GateContext, store: &ArtifactStore, full: bool) -> Vec<FileCheck> {
    check_files(&ctx.ctx.repo, bound_files(ctx), store, full)
}

/// Judge `files` against `store`. Each distinct digest is asked of the
/// store once; `full` re-hashes the stored bytes instead of trusting
/// size/digest metadata. Once the store fails to answer, the rest are
/// [`Observed::Unavailable`] without asking again (an unreachable store
/// is not retried per file).
fn check_files(repo: &Path, files: Vec<BoundFileRef>, store: &ArtifactStore, full: bool) -> Vec<FileCheck> {
    let mut seen: BTreeMap<String, Observed> = BTreeMap::new();
    let mut outage: Option<String> = None;
    let mut checks = Vec::new();
    for file in files {
        let blob = match (&outage, seen.get(&file.sha256)) {
            (Some(reason), _) => Observed::Unavailable(reason.clone()),
            (None, Some(blob)) => blob.clone(),
            (None, None) => {
                let blob = store.observe(repo, &file.sha256, full);
                if let Observed::Unavailable(reason) = &blob {
                    outage = Some(reason.clone());
                }
                seen.insert(file.sha256.clone(), blob.clone());
                blob
            }
        };
        let (tree, status) = if blob.matches(&file.sha256) {
            (Observed::Missing, FileStatus::Stored)
        } else {
            let tree = working_tree(repo, &file.path);
            let status = if blob == Observed::Missing && store.is_local() && tree.matches(&file.sha256) { FileStatus::WorkingTreeOnly } else { FileStatus::Failed };
            (tree, status)
        };
        checks.push(FileCheck { file, blob, tree, status });
    }
    checks
}

/// One attachment on a latest record, or a current review, whose bytes
/// exist only in the working tree (module doc's back-compat path): not a
/// violation, an advisory pointing at `canon artifacts store`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnstoredAttachment {
    pub subject: String,
    pub path: String,
    pub sha256: String,
    /// The reviewer, when a review (not an evidence record) binds it.
    pub reviewer: Option<String>,
}

impl UnstoredAttachment {
    pub fn line(&self) -> String {
        match &self.reviewer {
            None => format!("{} {} (sha256 {}) is verified against the working tree only", self.subject, self.path, self.sha256),
            Some(reviewer) => format!("{} {} (sha256 {}, review by `{reviewer}`) is verified against the working tree only", self.subject, self.path, self.sha256),
        }
    }
}

/// The advisories `canon gate check` prints below its violations: every
/// attachment on a latest record, and every artifact on a current
/// review, with no stored blob whose working-tree file still matches.
/// Sorted by subject, then path, evidence before reviews. Only the local
/// store has this path: an object-store repo has none (the check
/// refuses instead).
pub fn unstored_attachments(ctx: &GateContext) -> Vec<UnstoredAttachment> {
    if !matches!(ArtifactStore::open(&ctx.ctx.repo), Ok(ArtifactStore::Local)) {
        return Vec::new();
    }
    let mut out: Vec<UnstoredAttachment> = check_bound_files(ctx, &ArtifactStore::Local, false)
        .into_iter()
        .filter(|check| check.status == FileStatus::WorkingTreeOnly)
        .map(|check| UnstoredAttachment {
            subject: check.file.subject,
            path: check.file.path,
            sha256: check.file.sha256,
            reviewer: match check.file.binder {
                Binder::Evidence => None,
                Binder::Review { reviewer, .. } => Some(reviewer),
            },
        })
        .collect();
    out.sort_by(|a, b| (&a.subject, &a.path, &a.reviewer).cmp(&(&b.subject, &b.path, &b.reviewer)));
    out
}

/// Each distinct `(path, sha256)` a review binds.
fn distinct_review_files(review: &Review) -> Vec<&ReviewArtifact> {
    let mut seen = BTreeSet::new();
    review.artifacts.iter().filter(|a| seen.insert((a.path.as_str(), a.sha256.as_str()))).collect()
}

/// The `canon gate check` arm (module doc).
pub struct ArtifactStoreCheck;

impl GateCheck for ArtifactStoreCheck {
    fn name(&self) -> &'static str {
        "evidence-artifacts"
    }

    fn run(&self, ctx: &GateContext) -> Vec<Violation> {
        let files = bound_files(ctx);
        if files.is_empty() {
            return Vec::new();
        }
        // A store that cannot be opened (unusable `artifact_store:` block, no
        // credentials) proves nothing: refuse, never fall back to local.
        let store = match ArtifactStore::open(&ctx.ctx.repo) {
            Ok(store) => store,
            Err(e) => return vec![Violation::new(FailureClass::StaleEvidence, "artifacts".to_string(), format!("the artifact store cannot be used, so no bound file can be verified: {e} — {}", e.hint()))],
        };
        let mut violations: Vec<Violation> = check_files(&ctx.ctx.repo, files, &store, false)
            .into_iter()
            .filter(|check| check.status == FileStatus::Failed)
            .map(|check| {
                let FileCheck { file, blob, tree, .. } = check;
                let stored = store.display(&file.sha256);
                let detail = match &file.binder {
                    Binder::Evidence => format!(
                        "bound file `{}` recorded sha256 {}; stored blob {stored}: {}; working tree: {} — {}",
                        file.path,
                        file.sha256,
                        blob.as_str(),
                        tree.as_str(),
                        remedy(&blob, &store, "re-run and re-attest with `canon evidence add`")
                    ),
                    Binder::Review { reviewer, actor, at } => format!(
                        "review by `{reviewer}` (actor `{actor}`, {at}) bound file `{}` recorded sha256 {}; stored blob {stored}: {}; working tree: {} — {}",
                        file.path,
                        file.sha256,
                        blob.as_str(),
                        tree.as_str(),
                        remedy(&blob, &store, "look again and re-record the review with `canon review scenario <SCENARIO_ID> --artifact <path>` (or `canon review change <CHANGE_ID> --artifact <path>`)")
                    ),
                };
                Violation::new(FailureClass::StaleEvidence, file.subject, detail)
            })
            .collect();
        violations.sort_by(|a, b| (&a.subject, &a.detail).cmp(&(&b.subject, &b.detail)));
        violations
    }
}

/// Why a bound file failed and what to do: a blob that exists but no
/// longer hashes to its own name was altered in place; one that is absent
/// took the only proof with it (0.14 acceptance rerun G11); an object
/// store that could not be asked proved nothing either way, and is never
/// read as "present".
fn remedy(blob: &Observed, store: &ArtifactStore, reattest: &str) -> String {
    match blob {
        Observed::Unavailable(_) => "the artifact store could not be reached, so the bound bytes cannot be verified; check that it is reachable and that its credentials (CANON_S3_ENDPOINT, CANON_S3_ACCESS_KEY, CANON_S3_SECRET_KEY) are set, then run the gate again".to_string(),
        Observed::Missing if !store.is_local() => format!("the bound bytes are not in the artifact store; `canon artifacts store` uploads them again when the working-tree file still matches, otherwise {reattest}"),
        Observed::Missing => format!("the bound bytes are gone, {reattest}"),
        _ => format!("the stored blob does not match its name, so its bytes were altered, {reattest}"),
    }
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeMap;

    use canon_model::{Actor, Envelope, EvidenceVerdict, ProjectId, RecordKind, RoleId, ScenarioId};
    use chrono::{Duration, Utc};
    use tempfile::TempDir;

    use super::*;
    use crate::policy::{PolicyField, PolicyResolution, StalenessPolicy};

    /// One store transaction under the repo's lock.
    fn store(repo: &Path, expected: &str, reader: impl Read) -> Result<StoreOutcome, StoreError> {
        StoreLock::acquire(repo)?.store(expected, reader)
    }

    fn sha(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    fn evidence(id: &str, path: &str, digest: &str, offset: i64) -> EvidenceRecord {
        let envelope = Envelope::new(1, RecordKind::EvidenceRecord, Utc::now() + Duration::seconds(offset), Actor::new("agent", RoleId::parse("implementer").unwrap()));
        EvidenceRecord::new(envelope, None, Some(ScenarioId::parse(id).unwrap()), None, EvidenceVerdict::Faithful)
            .with_project_id(ProjectId::parse("p").unwrap())
            .with_attachments(vec![EvidenceAttachment { path: path.into(), sha256: digest.into(), format: None, case: None, outcome: None }])
    }

    fn ctx(repo: &Path, evidence: Vec<EvidenceRecord>) -> GateContext {
        GateContext {
            ctx: crate::context::GateCtx { repo: repo.to_path_buf(), ledger_root: repo.join(".canon/ledger") },
            policy: PolicyResolution {
                trust_required: BTreeMap::new(),
                trust_sample: BTreeMap::new(),
                staleness: StalenessPolicy { max_commits_behind: PolicyField::Flat(50), surface_scoped: PolicyField::Flat(true) },
                risk_routing: BTreeMap::new(),
                risk_tiers: BTreeMap::new(),
                spec_coverage: None,
                evidence_binding: None,
                diagnostics: Vec::new(),
            },
            evidence,
            audits: Vec::new(),
            scenarios: Vec::new(),
            divergences: Vec::new(),
            subjects: Vec::new(),
            changes: Vec::new(),
            reviews: Vec::new(),
            findings: Vec::new(),
            violations: Vec::new(),
            corpus_violations: Vec::new(),
            unreadable_kinds: Vec::new(),
            code_tree: None,
            now: Utc::now(),
        }
    }

    #[test]
    fn store_writes_once_dedupes_and_refuses_bytes_that_do_not_match_the_digest() {
        let dir = TempDir::new().unwrap();
        let digest = sha(b"report");
        assert_eq!(store(dir.path(), &digest, &b"report"[..]).unwrap(), StoreOutcome::Stored);
        assert_eq!(std::fs::read(blob_path(dir.path(), &digest)).unwrap(), b"report");
        assert_eq!(store(dir.path(), &digest, &b"report"[..]).unwrap(), StoreOutcome::AlreadyStored);

        let other = sha(b"other");
        let refused = store(dir.path(), &other, &b"changed"[..]).unwrap_err();
        assert!(matches!(refused, StoreError::Changed { .. }), "{refused}");
        assert!(!blob_path(dir.path(), &other).exists());
        let leftovers: Vec<_> = std::fs::read_dir(store_dir(dir.path())).unwrap().map(|e| e.unwrap().file_name()).collect();
        assert_eq!(leftovers.len(), 1, "a refused store leaves no temp file: {leftovers:?}");

        // A corrupt blob is replaced, not trusted.
        std::fs::write(blob_path(dir.path(), &digest), b"tampered").unwrap();
        assert_eq!(store(dir.path(), &digest, &b"report"[..]).unwrap(), StoreOutcome::Stored);
        assert_eq!(std::fs::read(blob_path(dir.path(), &digest)).unwrap(), b"report");
    }

    #[test]
    fn a_stored_blob_keeps_the_record_clean_after_the_working_tree_file_changes() {
        let dir = TempDir::new().unwrap();
        let digest = sha(b"v1");
        std::fs::write(dir.path().join("smoke.json"), b"v1").unwrap();
        store(dir.path(), &digest, &b"v1"[..]).unwrap();
        std::fs::write(dir.path().join("smoke.json"), b"v2").unwrap();
        let ctx = ctx(dir.path(), vec![evidence("a.b.01", "smoke.json", &digest, 0)]);
        assert!(ArtifactStoreCheck.run(&ctx).is_empty());
        assert!(unstored_attachments(&ctx).is_empty());
    }

    #[test]
    fn a_missing_blob_falls_back_to_a_matching_working_tree_as_an_advisory() {
        let dir = TempDir::new().unwrap();
        let digest = sha(b"v1");
        std::fs::write(dir.path().join("smoke.json"), b"v1").unwrap();
        let ctx = ctx(dir.path(), vec![evidence("a.b.01", "smoke.json", &digest, 0)]);
        assert!(ArtifactStoreCheck.run(&ctx).is_empty());
        assert_eq!(unstored_attachments(&ctx), vec![UnstoredAttachment { subject: "a.b.01".into(), path: "smoke.json".into(), sha256: digest, reviewer: None }]);
    }

    #[test]
    fn a_missing_blob_and_a_changed_or_absent_file_is_stale_evidence_naming_both_digests() {
        let dir = TempDir::new().unwrap();
        let digest = sha(b"v1");
        std::fs::write(dir.path().join("smoke.json"), b"v2").unwrap();
        let ctx_changed = ctx(dir.path(), vec![evidence("a.b.01", "smoke.json", &digest, 0), evidence("a.b.02", "gone.json", &digest, 0)]);
        let violations = ArtifactStoreCheck.run(&ctx_changed);
        assert_eq!(violations.len(), 2, "{violations:?}");
        assert!(violations.iter().all(|v| v.class == FailureClass::StaleEvidence));
        assert_eq!(violations[0].subject, "a.b.01");
        assert!(violations[0].detail.contains("`smoke.json`") && violations[0].detail.contains(&digest) && violations[0].detail.contains(&sha(b"v2")), "{}", violations[0].detail);
        assert!(violations[1].detail.contains("working tree: missing"), "{}", violations[1].detail);
        assert!(violations.iter().all(|v| v.detail.contains("the bound bytes are gone") && !v.detail.contains("altered")), "a missing blob is gone, not altered: {violations:?}");
        assert!(unstored_attachments(&ctx_changed).is_empty());
    }

    #[test]
    fn a_corrupt_blob_is_stale_even_when_the_working_tree_still_matches() {
        let dir = TempDir::new().unwrap();
        let digest = sha(b"v1");
        std::fs::write(dir.path().join("smoke.json"), b"v1").unwrap();
        std::fs::create_dir_all(store_dir(dir.path())).unwrap();
        std::fs::write(blob_path(dir.path(), &digest), b"tampered").unwrap();
        let violations = ArtifactStoreCheck.run(&ctx(dir.path(), vec![evidence("a.b.01", "smoke.json", &digest, 0)]));
        assert_eq!(violations.len(), 1, "{violations:?}");
        assert!(violations[0].detail.contains(&sha(b"tampered")), "{}", violations[0].detail);
        assert!(
            violations[0].detail.contains("the stored blob does not match its name, so its bytes were altered") && !violations[0].detail.contains("gone"),
            "an altered blob is named as altered, never as gone: {}",
            violations[0].detail
        );
    }

    #[test]
    fn only_the_latest_record_per_subject_is_checked() {
        let dir = TempDir::new().unwrap();
        let stale = sha(b"old");
        let fresh = sha(b"new");
        store(dir.path(), &fresh, &b"new"[..]).unwrap();
        let ctx = ctx(dir.path(), vec![evidence("a.b.01", "r.json", &stale, 0), evidence("a.b.01", "r.json", &fresh, 10)]);
        assert!(ArtifactStoreCheck.run(&ctx).is_empty(), "a superseded record's binding is not re-checked");
    }

    #[test]
    fn a_recorded_path_that_leaves_the_repository_is_never_read() {
        assert!(working_tree_path(Path::new("/repo"), "../outside").is_none());
        assert!(working_tree_path(Path::new("/repo"), "/etc/passwd").is_none());
        assert_eq!(working_tree_path(Path::new("/repo"), "reports/a.json"), Some(PathBuf::from("/repo/reports/a.json")));
    }

    #[test]
    fn a_digest_that_is_not_64_lowercase_hex_builds_no_path() {
        let dir = TempDir::new().unwrap();
        for bad in ["../../etc/passwd", &"A".repeat(64), &"a".repeat(63), ""] {
            assert!(matches!(store(dir.path(), bad, &b"x"[..]), Err(StoreError::InvalidDigest(_))), "{bad}");
            assert_eq!(stored_blob(dir.path(), bad), Observed::Missing);
        }
        assert!(!store_dir(dir.path()).exists(), "no store directory is created for an invalid digest");
    }

    /// A pre-existing `<digest>` symlink pointing outside the store (at
    /// a file whose bytes match) is never "already stored": the gate
    /// reads it as missing, and `store` replaces the link itself with a
    /// regular file, leaving the link's target untouched.
    #[cfg(unix)]
    #[test]
    fn a_symlinked_blob_is_not_stored_and_is_replaced_by_a_regular_file() {
        let dir = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        let digest = sha(b"v1");
        let target = outside.path().join("elsewhere.json");
        std::fs::write(&target, b"v1").unwrap();
        std::fs::create_dir_all(store_dir(dir.path())).unwrap();
        std::os::unix::fs::symlink(&target, blob_path(dir.path(), &digest)).unwrap();

        assert_eq!(stored_blob(dir.path(), &digest), Observed::Missing, "a link never counts as stored");
        let violations = ArtifactStoreCheck.run(&ctx(dir.path(), vec![evidence("a.b.01", "gone.json", &digest, 0)]));
        assert_eq!(violations.len(), 1, "a linked blob does not prove the record: {violations:?}");

        assert_eq!(store(dir.path(), &digest, &b"v1"[..]).unwrap(), StoreOutcome::Stored);
        let meta = std::fs::symlink_metadata(blob_path(dir.path(), &digest)).unwrap();
        assert!(meta.file_type().is_file(), "the link was replaced by a regular file");
        assert_eq!(std::fs::read(&target).unwrap(), b"v1", "the link's target is untouched");
        assert!(stored_blob(dir.path(), &digest).matches(&digest));
    }

    #[cfg(unix)]
    #[test]
    fn a_symlinked_store_directory_is_refused() {
        let dir = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".canon/artifacts")).unwrap();
        std::os::unix::fs::symlink(outside.path(), store_dir(dir.path())).unwrap();
        let digest = sha(b"v1");
        assert!(matches!(store(dir.path(), &digest, &b"v1"[..]), Err(StoreError::UnsafeStoreDir)));
        assert!(std::fs::read_dir(outside.path()).unwrap().next().is_none(), "nothing written through the link");
    }

    /// Review fix: a symlinked `.canon/artifacts` must be refused BEFORE
    /// anything is created through it — `create_dir_all` used to make
    /// `sha256` inside the link's outside target first.
    #[cfg(unix)]
    #[test]
    fn a_symlinked_ancestor_of_the_store_is_refused_before_anything_is_created() {
        let dir = TempDir::new().unwrap();
        let outside = TempDir::new().unwrap();
        std::fs::create_dir_all(dir.path().join(".canon")).unwrap();
        std::os::unix::fs::symlink(outside.path(), dir.path().join(".canon/artifacts")).unwrap();
        let digest = sha(b"v1");
        assert!(matches!(store(dir.path(), &digest, &b"v1"[..]), Err(StoreError::UnsafeStoreDir)));
        assert!(std::fs::read_dir(outside.path()).unwrap().next().is_none(), "the outside directory stays empty");
    }

    /// evidence.add.66: a blob already intact at its target when the
    /// transaction publishes (written by another writer) is not claimed
    /// by this transaction, so its rollback leaves it in place.
    #[test]
    fn evidence_add_66_a_rollback_never_removes_a_blob_another_writer_published() {
        let dir = TempDir::new().unwrap();
        let (a, b) = (sha(b"first"), sha(b"second"));
        let lock = StoreLock::acquire(dir.path()).unwrap();
        let staged = vec![lock.stage(&a, &b"first"[..]).unwrap().unwrap(), lock.stage(&b, &b"second"[..]).unwrap().unwrap()];
        // Another writer stores A meanwhile; B's rename will fail.
        std::fs::write(blob_path(dir.path(), &a), b"first").unwrap();
        std::fs::create_dir_all(blob_path(dir.path(), &b).join("occupied")).unwrap();

        let (index, _) = lock.publish(staged).unwrap_err();
        assert_eq!(index, 1, "the error names B");
        assert!(stored_blob(dir.path(), &a).matches(&a), "the other writer's blob survives the rollback");
        let mut left: Vec<String> = std::fs::read_dir(store_dir(dir.path())).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
        left.sort();
        let mut expected = vec![a.clone(), b.clone()];
        expected.sort();
        assert_eq!(left, expected, "A's blob and B's blocking directory only: no temp file is left");
    }

    /// evidence.add.66: an already-intact target counts as stored, so a
    /// digest staged twice in one transaction is written once.
    #[test]
    fn evidence_add_66_publish_counts_only_the_blobs_it_wrote() {
        let dir = TempDir::new().unwrap();
        let digest = sha(b"same");
        let lock = StoreLock::acquire(dir.path()).unwrap();
        let staged = vec![lock.stage(&digest, &b"same"[..]).unwrap().unwrap(), lock.stage(&digest, &b"same"[..]).unwrap().unwrap()];
        assert_eq!(lock.publish(staged).unwrap(), 1);
        assert_eq!(std::fs::read_dir(store_dir(dir.path())).unwrap().count(), 1, "one blob, no temp file");
        assert_eq!(lock.store(&digest, &b"same"[..]).unwrap(), StoreOutcome::AlreadyStored);
    }

    /// evidence.add.66: a second writer of the same repo waits for the
    /// lock, is refused naming the lock file when the wait runs out, and
    /// gets it once the first writer is done.
    #[test]
    fn evidence_add_66_a_second_store_writer_waits_for_the_lock() {
        let dir = TempDir::new().unwrap();
        let first = StoreLock::acquire(dir.path()).unwrap();
        let busy = StoreLock::acquire_within(dir.path(), std::time::Duration::from_millis(120)).unwrap_err();
        assert!(matches!(busy, StoreError::Locked(_)), "{busy}");
        assert!(busy.to_string().contains(paths::ARTIFACT_STORE_LOCK_FILE) && busy.to_string().contains("retry"), "{busy}");
        drop(first);
        // A parallel test that forks a child briefly holds a copy of the fd
        // until the child execs, so the release is not instant: retry within
        // a bound, as `acquire` does.
        let second = StoreLock::acquire_within(dir.path(), std::time::Duration::from_secs(5)).unwrap();
        assert_eq!(second.store(&sha(b"v1"), &b"v1"[..]).unwrap(), StoreOutcome::Stored);
        assert!(dir.path().join(paths::ARTIFACT_STORE_LOCK_FILE).is_file(), "the lock file is never deleted");
    }
}
