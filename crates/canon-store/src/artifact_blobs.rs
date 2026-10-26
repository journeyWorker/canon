//! Evidence artifact blobs in an object store (0.23 D3).
//!
//! `canon.yaml`'s `artifact_store:` block picks where the bytes behind an
//! evidence digest live: `local` (`.canon/artifacts/sha256`, git-tracked,
//! the default — owned by `canon-gate`) or the object-store tier named
//! by `artifact_store.backend`. This module owns the second: the config
//! ([`ArtifactsConfig`]) and the blob store ([`ObjectBlobStore`]) over the
//! tier's bucket, keyed by digest at `<prefix>sha256/<digest>`.
//!
//! Records keep only digests. A blob is verified three ways, cheapest
//! first, and never silently skipped:
//! - the KEY is the digest (a writer that hashed wrong would have named
//!   a different key);
//! - [`ObjectBlobStore::probe`] — one HEAD — compares the object's size
//!   with the `canon-size` metadata and its digest with `canon-sha256`,
//!   both written at upload (a store that cannot keep metadata, like the
//!   local-filesystem test substitute, is judged on presence alone);
//! - [`ObjectBlobStore::verify_full`] downloads and re-hashes.
//!
//! An unreachable store is an `Err`, never [`Probe::Missing`]: only the
//! store's own "not found" means absent.

use std::sync::Arc;

use object_store::path::Path as ObjectPath;
use object_store::{Attribute, Attributes, GetOptions, ObjectStore, ObjectStoreExt, PutOptions, PutPayload};
use serde::Deserialize;
use sha2::{Digest, Sha256};

use crate::policy::{BackendConfig, R2TierConfig, Rung, TierPolicy, TierPolicyError};
use crate::r2_tier::R2Tier;
use crate::tier::StoreError;

/// `artifact_store.prefix` when the block names none.
pub const DEFAULT_PREFIX: &str = "artifacts/";

/// Object metadata key holding the blob's sha256.
const DIGEST_META: &str = "canon-sha256";
/// Object metadata key holding the blob's size in bytes.
const SIZE_META: &str = "canon-size";

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

    /// The key `digest` lives at: `<prefix>sha256/<digest>`.
    pub fn key(&self, digest: &str) -> String {
        format!("{}sha256/{digest}", self.prefix)
    }

    fn path(&self, digest: &str) -> Result<ObjectPath, BlobError> {
        if !is_digest(digest) {
            return Err(BlobError::InvalidDigest(digest.to_string()));
        }
        Ok(ObjectPath::from(self.key(digest)))
    }

    /// One HEAD: absent, present, or contradicting its own name. `Err`
    /// when the store cannot be asked (offline, denied, bad credentials).
    pub fn probe(&self, digest: &str) -> Result<Probe, BlobError> {
        let path = self.path(digest)?;
        let head = GetOptions { head: true, ..GetOptions::default() };
        let got = self.rt.block_on(self.store.get_opts(&path, head));
        let got = match got {
            Ok(got) => got,
            Err(object_store::Error::NotFound { .. }) => return Ok(Probe::Missing),
            Err(e) => return Err(store_err(e)),
        };
        let size = got.meta.size;
        let meta = |key: &'static str| got.attributes.get(&Attribute::Metadata(key.into())).map(|v| v.as_ref().to_string());
        if let Some(recorded) = meta(DIGEST_META) {
            if recorded != digest {
                return Ok(Probe::Altered(format!("object metadata records sha256 {recorded}")));
            }
        }
        if let Some(recorded) = meta(SIZE_META) {
            if recorded.parse::<u64>().ok() != Some(size) {
                return Ok(Probe::Altered(format!("object holds {size} bytes, recorded {recorded}")));
            }
        }
        Ok(Probe::Present { size })
    }

    /// Download the blob and re-hash it: [`Probe::Present`] when its bytes
    /// hash to `digest`, [`Probe::Altered`] naming what they hash to
    /// otherwise.
    pub fn verify_full(&self, digest: &str) -> Result<Probe, BlobError> {
        let path = self.path(digest)?;
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

    /// Store `bytes` as the blob for `digest`: refuse
    /// ([`BlobError::Changed`]) when they hash differently, skip the
    /// upload when an object of the same size is already there
    /// (`Ok(false)`), else PUT with size and digest metadata and read it
    /// back with [`ObjectBlobStore::probe`] (`Ok(true)`). Nothing is kept
    /// locally.
    pub fn put(&self, digest: &str, bytes: Vec<u8>) -> Result<bool, BlobError> {
        let path = self.path(digest)?;
        let actual = format!("{:x}", Sha256::digest(&bytes));
        if actual != digest {
            return Err(BlobError::Changed { expected: digest.to_string(), actual });
        }
        let size = bytes.len() as u64;
        if self.probe(digest)? == (Probe::Present { size }) {
            return Ok(false);
        }
        let mut attributes = Attributes::new();
        attributes.insert(Attribute::Metadata(DIGEST_META.into()), digest.to_string().into());
        attributes.insert(Attribute::Metadata(SIZE_META.into()), size.to_string().into());
        let payload = PutPayload::from(bytes);
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
        match self.probe(digest)? {
            Probe::Present { size: now } if now == size => Ok(true),
            Probe::Present { size: now } => Err(BlobError::Unverified { key: self.key(digest), reason: format!("it holds {now} bytes, sent {size}") }),
            Probe::Altered(reason) => Err(BlobError::Unverified { key: self.key(digest), reason }),
            Probe::Missing => Err(BlobError::Unverified { key: self.key(digest), reason: "it reads back as missing".to_string() }),
        }
    }

    /// Every digest the store holds under the prefix, sorted.
    pub fn list(&self) -> Result<Vec<String>, BlobError> {
        use futures::StreamExt;
        let dir = ObjectPath::from(format!("{}sha256", self.prefix));
        let mut digests: Vec<String> = self.rt.block_on(async {
            let mut stream = self.store.list(Some(&dir));
            let mut out = Vec::new();
            while let Some(meta) = stream.next().await {
                let meta = meta.map_err(store_err)?;
                if let Some(name) = meta.location.filename().filter(|n| is_digest(n)) {
                    out.push(name.to_string());
                }
            }
            Ok::<_, BlobError>(out)
        })?;
        digests.sort();
        Ok(digests)
    }
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

    #[test]
    fn put_uploads_once_by_digest_key_and_probe_verifies_size_and_digest_metadata() {
        let (memory, blobs) = store();
        let bytes = b"report bytes".to_vec();
        let d = digest(&bytes);
        assert_eq!(blobs.probe(&d).unwrap(), Probe::Missing);
        assert!(blobs.put(&d, bytes.clone()).unwrap(), "first put uploads");
        assert!(!blobs.put(&d, bytes.clone()).unwrap(), "second put finds it and uploads nothing");
        assert_eq!(blobs.key(&d), format!("artifacts/sha256/{d}"));
        assert_eq!(blobs.probe(&d).unwrap(), Probe::Present { size: bytes.len() as u64 });
        assert_eq!(blobs.verify_full(&d).unwrap(), Probe::Present { size: bytes.len() as u64 });
        assert_eq!(blobs.list().unwrap(), vec![d.clone()]);

        // Someone overwrites the object with other bytes, keeping no
        // metadata of ours: the size no longer matches what we recorded.
        let tampered = ObjectPath::from(blobs.key(&d));
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        rt.block_on(memory.put(&tampered, PutPayload::from(b"rewritten by somebody else".to_vec()))).unwrap();
        // Metadata was dropped with the overwrite, so HEAD still reads
        // "present"; only the full check can tell.
        assert!(matches!(blobs.probe(&d).unwrap(), Probe::Present { .. }));
        assert!(matches!(blobs.verify_full(&d).unwrap(), Probe::Altered(why) if why.contains(&digest(b"rewritten by somebody else"))));
        // And put heals it: the size differs from the bound bytes.
        assert!(blobs.put(&d, bytes.clone()).unwrap());
        assert_eq!(blobs.verify_full(&d).unwrap(), Probe::Present { size: bytes.len() as u64 });
    }

    #[test]
    fn put_refuses_bytes_that_do_not_hash_to_the_digest_and_writes_nothing() {
        let (_, blobs) = store();
        let d = digest(b"what was bound");
        let err = blobs.put(&d, b"what is there now".to_vec()).unwrap_err();
        assert!(matches!(err, BlobError::Changed { .. }), "{err}");
        assert_eq!(blobs.probe(&d).unwrap(), Probe::Missing);
        assert!(matches!(blobs.put("../x", vec![]), Err(BlobError::InvalidDigest(_))));
    }

    #[test]
    fn probe_flags_an_object_whose_size_contradicts_the_recorded_metadata() {
        let (memory, blobs) = store();
        let bytes = b"a complete report".to_vec();
        let d = digest(&bytes);
        blobs.put(&d, bytes.clone()).unwrap();
        // A truncated copy that still carries the metadata written at upload.
        let mut attributes = Attributes::new();
        attributes.insert(Attribute::Metadata(DIGEST_META.into()), d.clone().into());
        attributes.insert(Attribute::Metadata(SIZE_META.into()), bytes.len().to_string().into());
        let rt = tokio::runtime::Builder::new_current_thread().enable_all().build().unwrap();
        let opts = PutOptions { attributes, ..PutOptions::default() };
        rt.block_on(memory.put_opts(&ObjectPath::from(blobs.key(&d)), PutPayload::from(b"a complete".to_vec()), opts)).unwrap();
        let probe = blobs.probe(&d).unwrap();
        assert!(matches!(&probe, Probe::Altered(why) if why.contains("holds 10 bytes, recorded 17")), "{probe:?}");
        assert!(blobs.put(&d, bytes).unwrap(), "put replaces an object that contradicts its name");
        assert_eq!(blobs.probe(&d).unwrap(), Probe::Present { size: 17 });
    }
}
