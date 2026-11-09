//! Evidence artifact blobs in an object store (0.23 D3).
//!
//! `canon.yaml`'s `artifact_store:` block picks where the bytes behind an
//! evidence digest live: `local` (`.canon/artifacts/sha256`, git-tracked,
//! the default — owned by `canon-gate`) or the object-store tier named
//! by `artifact_store.backend`. This module owns the second: the config
//! ([`ArtifactsConfig`]) and the blob store ([`ObjectBlobStore`]) over the
//! tier's bucket, keyed by digest at `<prefix>sha256/<digest>.zst`: one zstd frame
//! (level 19) of the bytes, whose sha256 is the digest. An object an older canon
//! wrote at `<prefix>sha256/<digest>` (the plain bytes) is still read; canon
//! writes only `.zst`.
//!
//! Records keep only digests. A blob is verified three ways, cheapest
//! first, and never silently skipped:
//! - the KEY is the digest (a writer that hashed wrong would have named
//!   a different key);
//! - [`ObjectBlobStore::probe`] — one HEAD (`.zst` first, the plain name
//!   only when that is absent) — compares the object's size with
//!   `canon-stored-size` (`canon-size` for a plain object) and its digest
//!   with `canon-sha256`,
//!   both written at upload (a store that cannot keep metadata, like the
//!   local-filesystem test substitute, is judged on presence alone);
//! - [`ObjectBlobStore::verify_full`] downloads, decompresses (in chunks) and
//!   re-hashes.
//!
//! An unreachable store is an `Err`, never [`Probe::Missing`]: only the
//! store's own "not found" means absent.

use std::sync::Arc;

use object_store::path::Path as ObjectPath;
use object_store::{Attribute, Attributes, GetOptions, ObjectStore, ObjectStoreExt, PutOptions, PutPayload};
use serde::Deserialize;
use futures::StreamExt;
use sha2::{Digest, Sha256};
use zstd::stream::raw::{InBuffer, Operation, OutBuffer};

use crate::policy::{BackendConfig, R2TierConfig, Rung, TierPolicy, TierPolicyError};
use crate::r2_tier::R2Tier;
use crate::tier::StoreError;

/// `artifact_store.prefix` when the block names none.
pub const DEFAULT_PREFIX: &str = "artifacts/";

/// Object metadata key holding the blob's sha256.
const DIGEST_META: &str = "canon-sha256";
/// Object metadata key holding the blob's size in bytes.
const SIZE_META: &str = "canon-size";
/// Object metadata key holding the number of bytes the `.zst` object takes.
const STORED_SIZE_META: &str = "canon-stored-size";
/// The key suffix of a compressed blob.
const ZST_SUFFIX: &str = ".zst";
/// zstd level of a new blob.
const ZSTD_LEVEL: i32 = 19;
/// Bytes of decompressed output handled at a time by `verify_full`.
const DECODE_BUFFER: usize = 64 * 1024;

/// Where a repo keeps artifact bytes.
#[derive(Debug, Clone)]
pub enum ArtifactsBackend {
    /// `.canon/artifacts/sha256`, git-tracked.
    Local,
    /// The object-store tier `artifact_store.backend` names.
    Tier { name: String, config: R2TierConfig },
}

/// `canon.yaml`'s `artifact_store:` block, resolved.
#[derive(Debug, Clone)]
pub struct ArtifactsConfig {
    pub backend: ArtifactsBackend,
    /// Key prefix inside the bucket, `""` or ending in `/`. Meaningful
    /// only for [`ArtifactsBackend::Tier`].
    pub prefix: String,
}

#[derive(Deserialize, Default)]
struct YamlRoot {
    #[serde(default)]
    artifact_store: Option<ArtifactsYaml>,
}

#[derive(Deserialize)]
#[serde(deny_unknown_fields)]
struct ArtifactsYaml {
    #[serde(default)]
    backend: Option<String>,
    #[serde(default)]
    prefix: Option<String>,
}

impl ArtifactsConfig {
    /// The default: local blobs, today's behavior, no credentials.
    pub fn local() -> Self {
        Self { backend: ArtifactsBackend::Local, prefix: DEFAULT_PREFIX.to_string() }
    }

    /// Resolve the `artifact_store:` block of `canon_yaml`. Absent, or
    /// `backend: local`, is [`ArtifactsConfig::local`] and never touches
    /// the tier policy. Any other `backend` must name a tier of
    /// `tiers:` whose backend is `s3`; `base` resolves the policy the
    /// way every other reader of `canon.yaml` does.
    pub fn from_yaml_at(canon_yaml: &str, base: &std::path::Path) -> Result<Self, TierPolicyError> {
        let root: YamlRoot = serde_yaml::from_str(canon_yaml)?;
        let Some(block) = root.artifact_store else { return Ok(Self::local()) };
        let backend = block.backend.as_deref().unwrap_or("local");
        if backend == "local" {
            if block.prefix.is_some() {
                return Err(TierPolicyError::ArtifactPrefixWithoutObjectStore);
            }
            return Ok(Self::local());
        }
        let rung = Rung::parse(backend).map_err(|source| TierPolicyError::ArtifactBackendUnknown { backend: backend.to_string(), source: Box::new(source) })?;
        let policy = TierPolicy::from_yaml_at(canon_yaml, base)?;
        let config = match policy.tiers.get(&rung) {
            Some(BackendConfig::S3(cfg)) => cfg.clone(),
            Some(_) => {
                return Err(TierPolicyError::ArtifactBackendNotObjectStore { backend: backend.to_string() })
            }
            None => {
                return Err(TierPolicyError::ArtifactBackendUndeclared { backend: backend.to_string() })
            }
        };
        let prefix = normalize_prefix(block.prefix.as_deref().unwrap_or(DEFAULT_PREFIX))?;
        Ok(Self { backend: ArtifactsBackend::Tier { name: backend.to_string(), config }, prefix })
    }
}

/// `""` or a `/`-terminated relative key prefix with no empty, `.` or
/// `..` segment.
fn normalize_prefix(raw: &str) -> Result<String, TierPolicyError> {
    let trimmed = raw.trim_end_matches('/');
    if trimmed.is_empty() {
        return Ok(String::new());
    }
    let bad = trimmed.starts_with('/') || trimmed.contains('\\') || trimmed.split('/').any(|s| s.is_empty() || s == "." || s == "..");
    if bad {
        return Err(TierPolicyError::ArtifactPrefixInvalid { raw: raw.to_string() });
    }
    Ok(format!("{trimmed}/"))
}

/// Whether `digest` is exactly 64 lowercase hex characters.
fn is_digest(digest: &str) -> bool {
    digest.len() == 64 && digest.bytes().all(|b| matches!(b, b'0'..=b'9' | b'a'..=b'f'))
}

/// What one HEAD found for a digest.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Probe {
    /// No object under the digest's key.
    Missing,
    /// An object of `size` bytes whose metadata, where it has any,
    /// agrees with the digest.
    Present { size: u64 },
    /// An object that contradicts its own name; the text says how.
    Altered(String),
}

/// Why a blob operation failed.
#[derive(Debug, thiserror::Error)]
pub enum BlobError {
    #[error("`{0}` is not a sha256 digest (64 lowercase hex characters)")]
    InvalidDigest(String),
    /// The bytes handed over hash differently from the digest to store.
    #[error("its bytes now hash to {actual}, not the bound {expected}")]
    Changed { expected: String, actual: String },
    /// The store could not be reached, or refused the request.
    #[error("{0}")]
    Store(String),
    /// An uploaded object did not read back as what was sent.
    #[error("uploaded {key} but it did not verify: {reason}")]
    Unverified { key: String, reason: String },
}

fn store_err(e: impl std::fmt::Display) -> BlobError {
    BlobError::Store(e.to_string())
}

/// The digest-keyed blob store over an object-store bucket.
pub struct ObjectBlobStore {
    store: Arc<dyn ObjectStore>,
    prefix: String,
    rt: tokio::runtime::Runtime,
}

impl std::fmt::Debug for ObjectBlobStore {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("ObjectBlobStore").field("prefix", &self.prefix).finish_non_exhaustive()
    }
}

impl ObjectBlobStore {
    /// A store over `store` with keys under `prefix` (`""` or ending in `/`).
    pub fn new(store: Arc<dyn ObjectStore>, prefix: impl Into<String>) -> Result<Self, StoreError> {
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().map_err(StoreError::Io)?;
        Ok(Self { store, prefix: prefix.into(), rt })
    }

    /// Attach the tier `config` names (through [`R2Tier::attach`], so the
    /// same credentials and the same debug-build test seam as `canon tier
    /// age`) and keep blobs under `prefix`. Builds a client; performs no I/O.
    pub fn attach(config: &R2TierConfig, prefix: &str) -> Result<Self, StoreError> {
        let tier = R2Tier::attach(config)?;
        Self::new(tier.object_store(), prefix)
    }

    /// The key a new blob for `digest` is written at and shown as:
    /// `<prefix>sha256/<digest>.zst`.
    pub fn key(&self, digest: &str) -> String {
        format!("{}{ZST_SUFFIX}", self.raw_key(digest))
    }

    /// The key of an uncompressed object (what canon 0.39 and older wrote,
    /// still valid): `<prefix>sha256/<digest>`.
    pub fn raw_key(&self, digest: &str) -> String {
        format!("{}sha256/{digest}", self.prefix)
    }

    /// The directory-like key blobs live under: `<prefix>sha256/`.
    pub fn dir_key(&self) -> String {
        format!("{}sha256/", self.prefix)
    }

    fn path(&self, digest: &str) -> Result<ObjectPath, BlobError> {
        if !is_digest(digest) {
            return Err(BlobError::InvalidDigest(digest.to_string()));
        }
        Ok(ObjectPath::from(self.key(digest)))
    }

    fn raw_path(&self, digest: &str) -> Result<ObjectPath, BlobError> {
        if !is_digest(digest) {
            return Err(BlobError::InvalidDigest(digest.to_string()));
        }
        Ok(ObjectPath::from(self.raw_key(digest)))
    }

    /// One HEAD of `path`: `None` when the store says "not found".
    fn head(&self, path: &ObjectPath) -> Result<Option<Head>, BlobError> {
        let opts = GetOptions { head: true, ..GetOptions::default() };
        match self.rt.block_on(self.store.get_opts(path, opts)) {
            Ok(got) => {
                let meta = |key: &'static str| got.attributes.get(&Attribute::Metadata(key.into())).map(|v| v.as_ref().to_string());
                Ok(Some(Head { size: got.meta.size, digest: meta(DIGEST_META), plain: meta(SIZE_META), stored: meta(STORED_SIZE_META) }))
            }
            Err(object_store::Error::NotFound { .. }) => Ok(None),
            Err(e) => Err(store_err(e)),
        }
    }

    /// The `.zst` object's HEAD judged against its own metadata.
    fn probe_zst(&self, digest: &str) -> Result<Option<ZstHead>, BlobError> {
        let Some(head) = self.head(&self.path(digest)?)? else { return Ok(None) };
        let object_size = head.size;
        let plain = head.plain.as_deref().and_then(|v| v.parse::<u64>().ok());
        let verdict = if let Some(recorded) = head.digest.filter(|r| r != digest) {
            Probe::Altered(format!("object metadata records sha256 {recorded}"))
        } else if let Some(recorded) = head.stored.filter(|r| r.parse::<u64>().ok() != Some(object_size)) {
            Probe::Altered(format!("object holds {object_size} bytes, recorded {recorded}"))
        } else {
            Probe::Present { size: plain.unwrap_or(object_size) }
        };
        Ok(Some(ZstHead { probe: verdict, object_size, plain }))
    }

    /// The uncompressed object's HEAD judged against its own metadata.
    fn probe_raw(&self, digest: &str) -> Result<Probe, BlobError> {
        let Some(head) = self.head(&self.raw_path(digest)?)? else { return Ok(Probe::Missing) };
        if let Some(recorded) = head.digest.filter(|r| r != digest) {
            return Ok(Probe::Altered(format!("object metadata records sha256 {recorded}")));
        }
        if let Some(recorded) = head.plain.filter(|r| r.parse::<u64>().ok() != Some(head.size)) {
            return Ok(Probe::Altered(format!("object holds {} bytes, recorded {recorded}", head.size)));
        }
        Ok(Probe::Present { size: head.size })
    }

    /// One HEAD for a blob stored the new way (`.zst` first), a second only
    /// when that is absent (the uncompressed object of an older canon):
    /// absent, present, or contradicting its own metadata. `Err` when the
    /// store cannot be asked (offline, denied, bad credentials), without
    /// trying the other form.
    pub fn probe(&self, digest: &str) -> Result<Probe, BlobError> {
        match self.probe_zst(digest)? {
            Some(zst) => Ok(zst.probe),
            None => self.probe_raw(digest),
        }
    }

    /// Download the blob and re-hash it: [`Probe::Present`] when its bytes
    /// (decompressed, for a `.zst` object) hash to `digest`,
    /// [`Probe::Altered`] naming what they hash to otherwise. When both
    /// forms exist, both must pass.
    pub fn verify_full(&self, digest: &str) -> Result<Probe, BlobError> {
        let zst = self.verify_full_zst(digest)?;
        let raw = self.verify_full_raw(digest)?;
        Ok(match (zst, raw) {
            (Probe::Missing, raw) => raw,
            (zst, Probe::Missing) => zst,
            (Probe::Present { size }, Probe::Present { .. }) => Probe::Present { size },
            (Probe::Altered(why), _) => Probe::Altered(why),
            (_, Probe::Altered(why)) => Probe::Altered(format!("the uncompressed object: {why}")),
        })
    }

    fn verify_full_raw(&self, digest: &str) -> Result<Probe, BlobError> {
        let path = self.raw_path(digest)?;
        let fetched = self.rt.block_on(async {
            let got = self.store.get(&path).await?;
            got.bytes().await
        });
        match fetched {
            Ok(bytes) => {
                let actual = format!("{:x}", Sha256::digest(&bytes));
                Ok(if actual == digest { Probe::Present { size: bytes.len() as u64 } } else { Probe::Altered(format!("the object's bytes hash to {actual}")) })
            }
            Err(object_store::Error::NotFound { .. }) => Ok(Probe::Missing),
            Err(e) => Err(store_err(e)),
        }
    }

    /// Stream the `.zst` object through a zstd decoder into the hash: memory
    /// is one chunk plus one output buffer, never the object. Stops once the
    /// output passes the `canon-size` the object recorded.
    fn verify_full_zst(&self, digest: &str) -> Result<Probe, BlobError> {
        let path = self.path(digest)?;
        let outcome = self.rt.block_on(async {
            let got = match self.store.get(&path).await {
                Ok(got) => got,
                Err(object_store::Error::NotFound { .. }) => return Ok(Probe::Missing),
                Err(e) => return Err(e),
            };
            let limit = got.attributes.get(&Attribute::Metadata(SIZE_META.into())).and_then(|v| v.as_ref().parse::<u64>().ok());
            let mut stream = got.into_stream();
            let mut decoder = match zstd::stream::raw::Decoder::new() {
                Ok(decoder) => decoder,
                Err(e) => return Ok(Probe::Altered(format!("cannot start a zstd decoder: {e}"))),
            };
            let mut hasher = Sha256::new();
            let mut out = vec![0u8; DECODE_BUFFER];
            let (mut total, mut seen, mut frame_done) = (0u64, false, false);
            while let Some(chunk) = stream.next().await {
                let chunk = chunk?;
                seen |= !chunk.is_empty();
                let mut input = InBuffer::around(&chunk);
                loop {
                    let before = input.pos();
                    let mut output = OutBuffer::around(&mut out[..]);
                    let hint = match decoder.run(&mut input, &mut output) {
                        Ok(hint) => hint,
                        Err(e) => return Ok(Probe::Altered(format!("the object does not decompress: {e}"))),
                    };
                    let produced = output.pos();
                    hasher.update(&out[..produced]);
                    total += produced as u64;
                    // A call that moved nothing asks only for the start of a next frame; it does not reopen a frame that ended.
                    if produced > 0 || input.pos() > before {
                        frame_done = hint == 0;
                    }
                    if let Some(limit) = limit.filter(|limit| total > *limit) {
                        return Ok(Probe::Altered(format!("it decompresses to more than the {limit} bytes it recorded")));
                    }
                    if input.pos() >= chunk.len() && produced < out.len() {
                        break;
                    }
                }
            }
            if !seen || !frame_done {
                return Ok(Probe::Altered("the object ends before its zstd frame does (truncated)".to_string()));
            }
            let actual = format!("{:x}", hasher.finalize());
            Ok(if actual == digest { Probe::Present { size: total } } else { Probe::Altered(format!("the object's bytes hash to {actual}")) })
        });
        outcome.map_err(store_err)
    }

    /// Store `bytes` as the blob for `digest`: refuse
    /// ([`BlobError::Changed`]) when they hash differently, skip the upload
    /// when the blob is already there in either form (`Ok(false)`: a `.zst`
    /// object recording these bytes' size, or an uncompressed object of
    /// that size), else compress (zstd level 19, one frame with its content
    /// size and checksum), PUT `<digest>.zst` with size, stored-size and
    /// digest metadata and read it back with [`ObjectBlobStore::probe`]
    /// (`Ok(true)`). Nothing is kept locally.
    pub fn put(&self, digest: &str, bytes: Vec<u8>) -> Result<bool, BlobError> {
        let path = self.path(digest)?;
        let actual = format!("{:x}", Sha256::digest(&bytes));
        if actual != digest {
            return Err(BlobError::Changed { expected: digest.to_string(), actual });
        }
        let size = bytes.len() as u64;
        if let Some(zst) = self.probe_zst(digest)? {
            if matches!(zst.probe, Probe::Present { .. }) && zst.plain.is_none_or(|plain| plain == size) {
                return Ok(false);
            }
        }
        if self.probe_raw(digest)? == (Probe::Present { size }) {
            return Ok(false);
        }
        let compressed = compress(&bytes).map_err(|e| BlobError::Store(format!("compressing {} failed: {e}", self.key(digest))))?;
        drop(bytes);
        let stored = compressed.len() as u64;
        let mut attributes = Attributes::new();
        attributes.insert(Attribute::Metadata(DIGEST_META.into()), digest.to_string().into());
        attributes.insert(Attribute::Metadata(SIZE_META.into()), size.to_string().into());
        attributes.insert(Attribute::Metadata(STORED_SIZE_META.into()), stored.to_string().into());
        let payload = PutPayload::from(compressed);
        let with_meta = PutOptions { attributes, ..PutOptions::default() };
        let put = self.rt.block_on(async {
            match self.store.put_opts(&path, payload.clone(), with_meta).await {
                // A store that cannot keep metadata (the local-filesystem
                // test substitute) takes the bytes alone.
                Err(object_store::Error::NotImplemented { .. }) => self.store.put_opts(&path, payload, PutOptions::default()).await,
                other => other,
            }
        });
        put.map_err(store_err)?;
        let unverified = |reason: String| BlobError::Unverified { key: self.key(digest), reason };
        match self.probe_zst(digest)? {
            Some(ZstHead { probe: Probe::Present { .. }, object_size, plain }) if object_size == stored && plain.is_none_or(|plain| plain == size) => Ok(true),
            Some(ZstHead { probe: Probe::Present { .. }, object_size, .. }) => Err(unverified(format!("it holds {object_size} bytes, sent {stored}"))),
            Some(ZstHead { probe: Probe::Altered(reason), .. }) => Err(unverified(reason)),
            Some(ZstHead { probe: Probe::Missing, .. }) | None => Err(unverified("it reads back as missing".to_string())),
        }
    }

    /// Every digest the store holds under the prefix, in either form, sorted.
    pub fn list(&self) -> Result<Vec<String>, BlobError> {
        Ok(self.list_objects()?.into_iter().map(|entry| entry.digest).collect())
    }

    /// Every digest the store holds under the prefix with the bytes its
    /// objects take (both forms summed), sorted by digest.
    pub fn list_sized(&self) -> Result<Vec<(String, u64)>, BlobError> {
        Ok(self.list_objects()?.into_iter().map(|entry| (entry.digest.clone(), entry.bytes())).collect())
    }

    /// Every digest the store holds, one entry per digest, with the size of
    /// each form that exists, sorted by digest. Names that are neither
    /// `<digest>` nor `<digest>.zst` are not blobs.
    pub fn list_objects(&self) -> Result<Vec<ObjectEntry>, BlobError> {
        let dir = ObjectPath::from(format!("{}sha256", self.prefix));
        let mut entries: std::collections::BTreeMap<String, ObjectEntry> = std::collections::BTreeMap::new();
        self.rt.block_on(async {
            let mut stream = self.store.list(Some(&dir));
            while let Some(meta) = stream.next().await {
                let meta = meta.map_err(store_err)?;
                let Some(name) = meta.location.filename() else { continue };
                let (digest, is_zst) = match name.strip_suffix(ZST_SUFFIX) {
                    Some(digest) => (digest, true),
                    None => (name, false),
                };
                if !is_digest(digest) {
                    continue;
                }
                let entry = entries.entry(digest.to_string()).or_insert_with(|| ObjectEntry { digest: digest.to_string(), zst: None, raw: None });
                if is_zst {
                    entry.zst = Some(meta.size);
                } else {
                    entry.raw = Some(meta.size);
                }
            }
            Ok::<_, BlobError>(())
        })?;
        Ok(entries.into_values().collect())
    }

    /// Delete both objects of `digest` (`<digest>.zst`, then `<digest>`). An
    /// object the store already lacks is no error (a repeat of an interrupted
    /// run; an object store answers a delete of nothing with success as often
    /// as with "not found"). `Err` when the store could not be reached or
    /// refused a delete; the error names what was already deleted.
    pub fn delete(&self, digest: &str) -> Result<(), BlobError> {
        let (zst, raw) = (self.path(digest)?, self.raw_path(digest)?);
        let gone = |result: object_store::Result<()>| match result {
            Ok(()) | Err(object_store::Error::NotFound { .. }) => Ok(()),
            Err(e) => Err(e),
        };
        gone(self.rt.block_on(self.store.delete(&zst))).map_err(|e| BlobError::Store(format!("could not delete {}: {e}", self.key(digest))))?;
        gone(self.rt.block_on(self.store.delete(&raw))).map_err(|e| BlobError::Store(format!("deleted {}, but could not delete {}: {e}", self.key(digest), self.raw_key(digest))))
    }
}

/// What one HEAD of an object returned.
struct Head {
    size: u64,
    digest: Option<String>,
    plain: Option<String>,
    stored: Option<String>,
}

/// A `.zst` object's HEAD, judged.
struct ZstHead {
    probe: Probe,
    object_size: u64,
    /// The plain size the object's `canon-size` metadata records.
    plain: Option<u64>,
}

/// One digest of the store with the objects that hold it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ObjectEntry {
    pub digest: String,
    /// Bytes of `<digest>.zst`, when that object exists.
    pub zst: Option<u64>,
    /// Bytes of the uncompressed `<digest>` object, when it exists.
    pub raw: Option<u64>,
}

impl ObjectEntry {
    /// The bytes the tier holds for this digest.
    pub fn bytes(&self) -> u64 {
        self.zst.unwrap_or(0) + self.raw.unwrap_or(0)
    }
}

/// One zstd frame: level 19, no dictionary, content size in the header,
/// content checksum on: `zstd -d` and `zstd -t` read it without canon.
fn compress(bytes: &[u8]) -> std::io::Result<Vec<u8>> {
    use std::io::Write;
    let mut encoder = zstd::stream::write::Encoder::new(Vec::new(), ZSTD_LEVEL)?;
    encoder.set_pledged_src_size(Some(bytes.len() as u64))?;
    encoder.include_contentsize(true)?;
    encoder.include_checksum(true)?;
    encoder.write_all(bytes)?;
    encoder.finish()
}

#[cfg(test)]
mod tests {
    use object_store::memory::InMemory;

    use super::*;

    fn digest(bytes: &[u8]) -> String {
        format!("{:x}", Sha256::digest(bytes))
    }

    fn store() -> (Arc<InMemory>, ObjectBlobStore) {
        let memory = Arc::new(InMemory::new());
        let blobs = ObjectBlobStore::new(Arc::<InMemory>::clone(&memory), "artifacts/").unwrap();
        (memory, blobs)
    }

    const CANON_YAML: &str = "tiers:\n  local: { backend: git, root: .canon/ledger }\n  cold: { backend: s3, bucket_env: CANON_R2_BUCKET, prefix: \"canon/\" }\n";

    fn config(artifacts: &str) -> Result<ArtifactsConfig, TierPolicyError> {
        ArtifactsConfig::from_yaml_at(&format!("{CANON_YAML}{artifacts}"), std::path::Path::new("."))
    }

    #[test]
    fn no_artifacts_block_or_backend_local_is_the_local_default() {
        assert!(matches!(config("").unwrap().backend, ArtifactsBackend::Local));
        assert!(matches!(config("artifact_store: { backend: local }\n").unwrap().backend, ArtifactsBackend::Local));
    }

    #[test]
    fn artifact_config_errors_are_the_old_strings() {
        let msg = |artifacts: &str| config(artifacts).unwrap_err().to_string();
        assert_eq!(
            msg("artifact_store: { backend: local, prefix: x/ }\n"),
            "canon.yaml TierPolicy: canon.yaml `artifact_store.prefix` only applies to an object-store backend; set `artifact_store.backend` to the name of an s3 tier (e.g. `cold`) or drop the prefix"
        );
        let unknown = config("artifact_store: { backend: warm }\n").unwrap_err();
        assert_eq!(
            unknown.to_string(),
            "canon.yaml TierPolicy: canon.yaml `artifact_store.backend: warm` is neither `local` nor the name of a tier in `tiers:` (local/hot/cold)"
        );
        assert!(std::error::Error::source(&unknown).is_some(), "the rung parse failure stays reachable as the cause");
        assert_eq!(
            msg("artifact_store: { backend: local-tier }\n"),
            "canon.yaml TierPolicy: canon.yaml `artifact_store.backend: local-tier` is neither `local` nor the name of a tier in `tiers:` (local/hot/cold)"
        );
        assert_eq!(
            msg("artifact_store: { backend: hot }\n"),
            "canon.yaml TierPolicy: canon.yaml `artifact_store.backend: hot` names tier `hot`, but `tiers:` declares no such tier; add `tiers.hot` with `backend: s3` or use `local`"
        );
        let not_object_store = ArtifactsConfig::from_yaml_at(
            "tiers:\n  hot: { backend: sqlite, path: h.db }\nartifact_store: { backend: hot }\n",
            std::path::Path::new("."),
        )
        .unwrap_err();
        assert_eq!(
            not_object_store.to_string(),
            "canon.yaml TierPolicy: canon.yaml `artifact_store.backend: hot` names tier `hot`, which is not an object store; artifacts live in an `s3` tier (`tiers.hot.backend: s3`) or `local`"
        );
        assert_eq!(
            msg("artifact_store: { backend: cold, prefix: ../x }\n"),
            "canon.yaml TierPolicy: canon.yaml `artifact_store.prefix: ../x` must be a relative key prefix such as `artifacts/` (no leading `/`, empty, `.` or `..` segment)"
        );
    }

    #[test]
    fn a_tier_name_selects_that_tier_with_the_default_or_a_normalized_prefix() {
        let c = config("artifact_store: { backend: cold }\n").unwrap();
        assert!(matches!(&c.backend, ArtifactsBackend::Tier { name, config } if name == "cold" && config.bucket_env == "CANON_R2_BUCKET"));
        assert_eq!(c.prefix, "artifacts/");
        assert_eq!(config("artifact_store: { backend: cold, prefix: evidence/blobs }\n").unwrap().prefix, "evidence/blobs/");
        assert_eq!(config("artifact_store: { backend: cold, prefix: \"\" }\n").unwrap().prefix, "");
    }

    #[test]
    fn a_backend_that_is_not_an_s3_tier_is_refused_naming_the_fix() {
        let err = config("artifact_store: { backend: hot }\n").unwrap_err().to_string();
        assert!(err.contains("declares no such tier"), "{err}");
        let err = config("artifact_store: { backend: local, prefix: x/ }\n").unwrap_err().to_string();
        assert!(err.contains("only applies to an object-store backend"), "{err}");
        let err = config("artifact_store: { backend: s3 }\n").unwrap_err().to_string();
        assert!(err.contains("neither `local` nor the name of a tier"), "{err}");
        for bad in ["/abs/", "a//b/", "../x/", "a/./b/"] {
            let err = config(&format!("artifact_store: {{ backend: cold, prefix: \"{bad}\" }}\n")).unwrap_err().to_string();
            assert!(err.contains("relative key prefix"), "{bad}: {err}");
        }
        assert!(config("artifact_store: { backend: cold, bucket: x }\n").is_err(), "an unknown key is refused, not ignored");
    }

    fn rt() -> tokio::runtime::Runtime {
        tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap()
    }

    /// The object an older canon wrote: the plain bytes at `<digest>`, with its metadata.
    fn put_legacy(memory: &InMemory, blobs: &ObjectBlobStore, bytes: &[u8]) -> String {
        let d = digest(bytes);
        let mut attributes = Attributes::new();
        attributes.insert(Attribute::Metadata(DIGEST_META.into()), d.clone().into());
        attributes.insert(Attribute::Metadata(SIZE_META.into()), bytes.len().to_string().into());
        let opts = PutOptions { attributes, ..PutOptions::default() };
        rt().block_on(memory.put_opts(&ObjectPath::from(blobs.raw_key(&d)), PutPayload::from(bytes.to_vec()), opts)).unwrap();
        d
    }

    /// Overwrite the `.zst` object of `d` with `body`, carrying `meta` as its metadata.
    fn overwrite_zst(memory: &InMemory, blobs: &ObjectBlobStore, d: &str, body: Vec<u8>, meta: &[(&'static str, String)]) {
        let mut attributes = Attributes::new();
        for (key, value) in meta {
            attributes.insert(Attribute::Metadata((*key).into()), value.clone().into());
        }
        let opts = PutOptions { attributes, ..PutOptions::default() };
        rt().block_on(memory.put_opts(&ObjectPath::from(blobs.key(d)), PutPayload::from(body), opts)).unwrap();
    }

    fn stored_zst(memory: &InMemory, blobs: &ObjectBlobStore, d: &str) -> Vec<u8> {
        rt().block_on(async { memory.get(&ObjectPath::from(blobs.key(d))).await.unwrap().bytes().await.unwrap().to_vec() })
    }

    fn keys(memory: &InMemory) -> Vec<String> {
        rt().block_on(async {
            let mut listed: Vec<String> = memory.list(None).map(|meta| meta.unwrap().location.to_string()).collect().await;
            listed.sort();
            listed
        })
    }

    fn meta_of(memory: &InMemory, blobs: &ObjectBlobStore, d: &str, key: &'static str) -> Option<String> {
        rt().block_on(async {
            let got = memory.get(&ObjectPath::from(blobs.key(d))).await.unwrap();
            got.attributes.get(&Attribute::Metadata(key.into())).map(|v| v.as_ref().to_string())
        })
    }

    /// Varied but compressible bytes of `len`.
    fn report(len: usize) -> Vec<u8> {
        const LINE: &[u8] = b"<testcase name=\"case\" time=\"0.01\"/>\n";
        (0..len).map(|i| LINE[i % LINE.len()] ^ ((i / 4096) as u8 & 3)).collect()
    }

    /// artifacts.object-zstd.01 (F1): a new blob is one `<digest>.zst` object (no plain one); the frame decompresses to the bytes, names its content size and
    /// carries a checksum; the metadata holds the plain digest and size and the stored size; a second put uploads nothing.
    #[test]
    fn artifacts_object_zstd_01_put_writes_one_zst_object_that_decompresses_to_the_bytes() {
        let (memory, blobs) = store();
        let bytes = report(300_000);
        let d = digest(&bytes);
        assert!(blobs.put(&d, bytes.clone()).unwrap(), "first put uploads");
        assert_eq!(keys(&memory), vec![format!("artifacts/sha256/{d}.zst")], "only the .zst name exists");
        assert_eq!(blobs.key(&d), format!("artifacts/sha256/{d}.zst"));
        assert_eq!(blobs.raw_key(&d), format!("artifacts/sha256/{d}"));
        let stored = stored_zst(&memory, &blobs, &d);
        assert!(stored.len() < bytes.len() / 4, "a report shrinks: {} of {}", stored.len(), bytes.len());
        assert_eq!(zstd::decode_all(&stored[..]).unwrap(), bytes);
        assert_eq!(zstd::zstd_safe::get_frame_content_size(&stored).unwrap(), Some(bytes.len() as u64), "the frame header carries the content size");
        let mut flipped = stored.clone();
        *flipped.last_mut().unwrap() ^= 1;
        assert!(zstd::decode_all(&flipped[..]).is_err(), "the frame carries a content checksum");
        assert_eq!(meta_of(&memory, &blobs, &d, DIGEST_META).as_deref(), Some(d.as_str()), "the digest is the plain bytes'");
        assert_eq!(meta_of(&memory, &blobs, &d, SIZE_META), Some(bytes.len().to_string()));
        assert_eq!(meta_of(&memory, &blobs, &d, STORED_SIZE_META), Some(stored.len().to_string()));
        assert_eq!(blobs.probe(&d).unwrap(), Probe::Present { size: bytes.len() as u64 });
        assert_eq!(blobs.verify_full(&d).unwrap(), Probe::Present { size: bytes.len() as u64 });
        assert!(!blobs.put(&d, bytes).unwrap(), "second put finds it and uploads nothing");
        assert_eq!(blobs.list().unwrap(), vec![d]);
        // Bytes that do not shrink are written the same way.
        let noise: Vec<u8> = (0..4096u32).map(|i| (i.wrapping_mul(2_654_435_761) >> 13) as u8).collect();
        let n = digest(&noise);
        assert!(blobs.put(&n, noise.clone()).unwrap());
        assert_eq!(zstd::decode_all(&stored_zst(&memory, &blobs, &n)[..]).unwrap(), noise);
    }

    /// artifacts.object-zstd.02 (F1 failure): bytes that do not hash to the digest are refused as `Changed` and nothing is uploaded, in either form.
    #[test]
    fn artifacts_object_zstd_02_put_refuses_bytes_that_do_not_hash_to_the_digest_and_writes_nothing() {
        let (memory, blobs) = store();
        let d = digest(b"what was bound");
        let err = blobs.put(&d, b"what is there now".to_vec()).unwrap_err();
        assert!(matches!(err, BlobError::Changed { .. }), "{err}");
        assert!(keys(&memory).is_empty(), "no object of either form");
        assert_eq!(blobs.probe(&d).unwrap(), Probe::Missing);
        assert!(matches!(blobs.put("../x", vec![]), Err(BlobError::InvalidDigest(_))));
        assert!(matches!(blobs.put(&format!("{d}.zst"), vec![]), Err(BlobError::InvalidDigest(_))), "the suffix is added by the key, never accepted in a digest");
    }

    /// artifacts.object-zstd.03 (F2): the same digest held as `.zst` only and as a plain object only answers the same to the HEAD and to the full check; a
    /// plain object is not uploaded again as `.zst`; both forms present answer with the `.zst`.
    #[test]
    fn artifacts_object_zstd_03_a_zst_object_and_a_plain_object_answer_the_same() {
        let bytes = report(50_000);
        let d = digest(&bytes);
        let (_, new) = store();
        new.put(&d, bytes.clone()).unwrap();
        let (old_memory, old) = store();
        put_legacy(&old_memory, &old, &bytes);
        for blobs in [&new, &old] {
            assert_eq!(blobs.probe(&d).unwrap(), Probe::Present { size: bytes.len() as u64 });
            assert_eq!(blobs.verify_full(&d).unwrap(), Probe::Present { size: bytes.len() as u64 });
            assert_eq!(blobs.list().unwrap(), vec![d.clone()]);
        }
        assert!(!old.put(&d, bytes.clone()).unwrap(), "a plain object already holds it");
        assert_eq!(keys(&old_memory), vec![format!("artifacts/sha256/{d}")], "no .zst appears beside it");
        // The plain object of an older canon (it re-uploaded the digest) beside a `.zst`.
        let (both_memory, both) = store();
        both.put(&d, bytes.clone()).unwrap();
        put_legacy(&both_memory, &both, &bytes);
        assert_eq!(both.probe(&d).unwrap(), Probe::Present { size: bytes.len() as u64 });
        assert_eq!(both.verify_full(&d).unwrap(), Probe::Present { size: bytes.len() as u64 });
        assert_eq!(both.list().unwrap(), vec![d.clone()], "one digest, however many forms");
    }

    /// artifacts.object-zstd.04 (F2 failure): no object of either form is `Missing`; a `.zst` whose size differs from the `canon-stored-size` it recorded (a
    /// truncated upload) is `Altered` at the HEAD, as is a metadata digest that is not its name.
    #[test]
    fn artifacts_object_zstd_04_missing_is_missing_and_a_zst_of_the_wrong_stored_size_is_altered() {
        let (memory, blobs) = store();
        let bytes = report(20_000);
        let d = digest(&bytes);
        assert_eq!(blobs.probe(&d).unwrap(), Probe::Missing);
        assert_eq!(blobs.verify_full(&d).unwrap(), Probe::Missing);
        blobs.put(&d, bytes.clone()).unwrap();
        let stored = stored_zst(&memory, &blobs, &d);
        let meta = [(DIGEST_META, d.clone()), (SIZE_META, bytes.len().to_string()), (STORED_SIZE_META, stored.len().to_string())];
        overwrite_zst(&memory, &blobs, &d, stored[..stored.len() - 5].to_vec(), &meta);
        let probe = blobs.probe(&d).unwrap();
        assert!(matches!(&probe, Probe::Altered(why) if why.contains(&format!("holds {} bytes, recorded {}", stored.len() - 5, stored.len()))), "{probe:?}");
        assert!(matches!(blobs.verify_full(&d).unwrap(), Probe::Altered(_)));
        let other = [(DIGEST_META, digest(b"another")), (SIZE_META, bytes.len().to_string()), (STORED_SIZE_META, stored.len().to_string())];
        overwrite_zst(&memory, &blobs, &d, stored.clone(), &other);
        assert!(matches!(blobs.probe(&d).unwrap(), Probe::Altered(why) if why.contains("metadata records sha256")));
        // An upload that is replaced by a whole one reads as present again, and put replaces a contradicting object.
        assert!(blobs.put(&d, bytes.clone()).unwrap(), "put replaces an object that contradicts its name");
        assert_eq!(blobs.probe(&d).unwrap(), Probe::Present { size: bytes.len() as u64 });
    }

    /// An `ObjectStore` that hands its bytes out `chunk` bytes at a time, so a reader that holds the whole object cannot hide.
    fn chunked(memory: &Arc<InMemory>, chunk: usize) -> ObjectBlobStore {
        ObjectBlobStore::new(Arc::new(object_store::chunked::ChunkedStore::new(Arc::<InMemory>::clone(memory), chunk)), "artifacts/").unwrap()
    }

    /// artifacts.object-zstd.05 (F3): `verify_full` decompresses a `.zst` object while hashing it, whatever size the chunks it arrives in; the same bytes
    /// pass in 7-byte, 64 KiB and whole-object chunks, a frame that ends at a chunk boundary included.
    #[test]
    fn artifacts_object_zstd_05_verify_full_decompresses_and_hashes_a_zst_object_chunk_by_chunk() {
        let (memory, blobs) = store();
        let bytes = report(3 * 1024 * 1024 + 11);
        let d = digest(&bytes);
        blobs.put(&d, bytes.clone()).unwrap();
        let stored = stored_zst(&memory, &blobs, &d).len();
        for chunk in [7, 4096, 64 * 1024, stored, stored * 2] {
            let reader = chunked(&memory, chunk);
            assert_eq!(reader.verify_full(&d).unwrap(), Probe::Present { size: bytes.len() as u64 }, "chunks of {chunk}");
        }
        let empty = digest(b"");
        blobs.put(&empty, Vec::new()).unwrap();
        assert_eq!(chunked(&memory, 3).verify_full(&empty).unwrap(), Probe::Present { size: 0 }, "an empty blob is one frame too");
        assert_eq!(blobs.verify_full(&empty).unwrap(), Probe::Present { size: 0 });
    }

    /// artifacts.object-zstd.06 (F3 failure): a flipped byte, a cut tail, a file that is no zstd and a frame that decompresses to other bytes are each
    /// `Altered`; a frame that decompresses past the `canon-size` it recorded stops at that limit. An object whose reads fail is an `Err`, never a pass.
    #[test]
    fn artifacts_object_zstd_06_a_damaged_zst_object_is_altered_and_a_bomb_stops_at_its_recorded_size() {
        let (memory, blobs) = store();
        let bytes = report(100_000);
        let d = digest(&bytes);
        blobs.put(&d, bytes.clone()).unwrap();
        let stored = stored_zst(&memory, &blobs, &d);
        let meta = [(DIGEST_META, d.clone()), (SIZE_META, bytes.len().to_string())];
        let mut flipped = stored.clone();
        flipped[stored.len() / 2] ^= 0x40;
        let mut other_bytes = bytes.clone();
        other_bytes[10] ^= 1;
        let other_frame = super::compress(&other_bytes).unwrap();
        let mut two_frames = stored.clone();
        two_frames.extend_from_slice(&stored[..20]);
        for (what, body) in [
            ("a flipped byte", flipped),
            ("a cut tail", stored[..stored.len() - 9].to_vec()),
            ("a cut to one byte", stored[..1].to_vec()),
            ("no zstd", b"someone else's bytes".to_vec()),
            ("an empty object", Vec::new()),
            ("other bytes in a valid frame", other_frame),
            ("a valid frame and the start of another", two_frames),
        ] {
            overwrite_zst(&memory, &blobs, &d, body, &meta);
            for reader in [chunked(&memory, 13), chunked(&memory, usize::MAX / 2)] {
                assert!(matches!(reader.verify_full(&d).unwrap(), Probe::Altered(_)), "{what}");
            }
        }
        // The decompression bomb: ~16 MiB of zeros recorded as 1000 bytes stops at the limit, whatever it hashes to.
        let zeros = vec![0u8; 16 * 1024 * 1024];
        overwrite_zst(&memory, &blobs, &d, super::compress(&zeros).unwrap(), &[(DIGEST_META, d.clone()), (SIZE_META, "1000".to_string())]);
        let probe = blobs.verify_full(&d).unwrap();
        assert!(matches!(&probe, Probe::Altered(why) if why.contains("more than the 1000 bytes")), "{probe:?}");
        // No recorded size (a store that keeps no metadata) has no limit: it hashes everything and reports the hash.
        overwrite_zst(&memory, &blobs, &d, super::compress(&zeros).unwrap(), &[]);
        assert!(matches!(blobs.verify_full(&d).unwrap(), Probe::Altered(why) if why.contains(&digest(&zeros))));
    }

    /// artifacts.object-zstd.09 (F5): a digest held in both forms is one entry whose size is both objects, and `delete` removes both names; a repeat is no
    /// error. Names that are not blobs (`<digest>.zst.bak`, a short name) are not listed.
    #[test]
    fn artifacts_object_zstd_09_list_counts_a_digest_once_and_delete_removes_both_forms() {
        let (memory, blobs) = store();
        let (a, b) = (report(30_000), b"second, longer report".to_vec());
        let (da, db) = (digest(&a), digest(&b));
        blobs.put(&da, a.clone()).unwrap();
        put_legacy(&memory, &blobs, &a);
        blobs.put(&db, b.clone()).unwrap();
        for stray in [format!("artifacts/sha256/{da}.zst.bak"), "artifacts/sha256/short.zst".to_string(), format!("artifacts/sha256/{}", &da[1..])] {
            rt().block_on(memory.put(&ObjectPath::from(stray), PutPayload::from(b"x".to_vec()))).unwrap();
        }
        let listed = blobs.list_objects().unwrap();
        let mut expected = vec![
            ObjectEntry { digest: da.clone(), zst: Some(stored_zst(&memory, &blobs, &da).len() as u64), raw: Some(a.len() as u64) },
            ObjectEntry { digest: db.clone(), zst: Some(stored_zst(&memory, &blobs, &db).len() as u64), raw: None },
        ];
        expected.sort_by(|x, y| x.digest.cmp(&y.digest));
        assert_eq!(listed, expected);
        assert_eq!(blobs.list_sized().unwrap().iter().find(|(d, _)| *d == da).unwrap().1, expected.iter().find(|e| e.digest == da).unwrap().bytes());
        blobs.delete(&da).unwrap();
        assert_eq!(blobs.list().unwrap(), vec![db.clone()], "both names of the first digest are gone, the other stays");
        blobs.delete(&da).unwrap();
        assert_eq!(blobs.probe(&da).unwrap(), Probe::Missing);
        assert!(matches!(blobs.delete("../x"), Err(BlobError::InvalidDigest(_))));
    }

    /// Delegates to an `InMemory`, refusing to delete any object whose name is not `.zst` while `refuse` is set.
    #[derive(Debug)]
    struct RefusesPlainDelete {
        inner: Arc<InMemory>,
        refuse: std::sync::atomic::AtomicBool,
    }

    impl std::fmt::Display for RefusesPlainDelete {
        fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
            write!(f, "RefusesPlainDelete")
        }
    }

    #[async_trait::async_trait]
    impl ObjectStore for RefusesPlainDelete {
        async fn put_opts(&self, location: &ObjectPath, payload: PutPayload, opts: PutOptions) -> object_store::Result<object_store::PutResult> {
            self.inner.put_opts(location, payload, opts).await
        }
        async fn put_multipart_opts(&self, location: &ObjectPath, opts: object_store::PutMultipartOptions) -> object_store::Result<Box<dyn object_store::MultipartUpload>> {
            self.inner.put_multipart_opts(location, opts).await
        }
        async fn get_opts(&self, location: &ObjectPath, options: GetOptions) -> object_store::Result<object_store::GetResult> {
            self.inner.get_opts(location, options).await
        }
        fn delete_stream(&self, locations: futures::stream::BoxStream<'static, object_store::Result<ObjectPath>>) -> futures::stream::BoxStream<'static, object_store::Result<ObjectPath>> {
            let inner = Arc::clone(&self.inner);
            let refuse = self.refuse.load(std::sync::atomic::Ordering::SeqCst);
            locations
                .then(move |location| {
                    let inner = Arc::clone(&inner);
                    async move {
                        let location = location?;
                        if refuse && !location.as_ref().ends_with(".zst") {
                            return Err(object_store::Error::Generic { store: "test", source: "delete refused".into() });
                        }
                        inner.delete(&location).await?;
                        Ok(location)
                    }
                })
                .boxed()
        }
        fn list(&self, prefix: Option<&ObjectPath>) -> futures::stream::BoxStream<'static, object_store::Result<object_store::ObjectMeta>> {
            self.inner.list(prefix)
        }
        async fn list_with_delimiter(&self, prefix: Option<&ObjectPath>) -> object_store::Result<object_store::ListResult> {
            self.inner.list_with_delimiter(prefix).await
        }
        async fn copy_opts(&self, from: &ObjectPath, to: &ObjectPath, options: object_store::CopyOptions) -> object_store::Result<()> {
            self.inner.copy_opts(from, to, options).await
        }
    }

    /// artifacts.object-zstd.10 (F5 failure): when the store refuses the second name, the error says the first is already deleted and the second is not;
    /// the digest is still listed (by the name that remains), and a repeat once the store accepts removes it.
    #[test]
    fn artifacts_object_zstd_10_a_refused_second_delete_names_what_went_and_a_repeat_removes_the_rest() {
        let memory = Arc::new(InMemory::new());
        let flaky = Arc::new(RefusesPlainDelete { inner: Arc::clone(&memory), refuse: std::sync::atomic::AtomicBool::new(false) });
        let blobs = ObjectBlobStore::new(Arc::<RefusesPlainDelete>::clone(&flaky), "artifacts/").unwrap();
        let bytes = report(10_000);
        let d = put_legacy(&memory, &blobs, &bytes);
        blobs.put(&digest(b"kept"), b"kept".to_vec()).unwrap();
        // The `.zst` of the same digest, beside the plain object.
        overwrite_zst(&memory, &blobs, &d, super::compress(&bytes).unwrap(), &[]);
        flaky.refuse.store(true, std::sync::atomic::Ordering::SeqCst);
        let err = blobs.delete(&d).unwrap_err().to_string();
        assert!(err.contains(&format!("deleted artifacts/sha256/{d}.zst, but could not delete artifacts/sha256/{d}")), "{err}");
        let left = blobs.list_objects().unwrap();
        assert_eq!(left.iter().find(|e| e.digest == d).map(|e| (e.zst, e.raw.is_some())), Some((None, true)), "the plain name remains and is listed");
        assert_eq!(left.len(), 2, "the other blob is untouched");
        flaky.refuse.store(false, std::sync::atomic::Ordering::SeqCst);
        blobs.delete(&d).unwrap();
        assert_eq!(blobs.list().unwrap(), vec![digest(b"kept")]);
    }
}
