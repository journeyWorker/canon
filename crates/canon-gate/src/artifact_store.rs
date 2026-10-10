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
//! again. A file larger than the size limit
//! ([`DEFAULT_MAX_ARTIFACT_MIB`], raised with [`MAX_ARTIFACT_FLAG`]) is
//! refused rather than stored.
//!
//! # What the gate checks ([`ArtifactStoreCheck`])
//! Every attachment on every LATEST evidence record: the latest record
//! per task, and the latest per `(project_id, scenario_id)`, the same
//! last-wins rule every other evidence fold uses. A superseded record is
//! not checked; re-attesting is how stale evidence is fixed.
//!
//! - The stored blob exists and hashes to the recorded digest: clean.
//! - The blob is missing, and the working-tree path still hashes to the
//!   recorded digest: clean, with an advisory ([`unstored_attachments`])
//!   naming `canon evidence vault`. This is the back-compat path for
//!   records written before the store existed (0.11–0.13).
//! - Anything else (a corrupt blob, or a missing blob and a working-tree
//!   file that is gone or changed): a `stale-evidence` violation naming
//!   the path, the recorded digest, and what the blob and working tree
//!   hold now.
//!
//! The check is NOT narrowed to `spec_coverage.scope`. A binding is a
//! claim the record makes about bytes, whatever its subject's lifecycle
//! status; scope belongs to the coverage policy, and a repo with no
//! `spec_coverage` section would otherwise never have its bindings
//! checked at all — exactly the repo F9 came from. Records with no
//! attachment cost nothing here.

use std::collections::BTreeSet;
use std::fs::File;
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

use canon_model::{paths, EvidenceAttachment, EvidenceRecord};
use sha2::{Digest, Sha256};

use crate::context::{GateCheck, GateContext};
use crate::coverage::CellSubject;
use crate::failure_class::{FailureClass, Violation};
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

/// What a file at a path holds now.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Observed {
    /// No readable file there.
    Missing,
    /// A file whose bytes hash to this digest.
    Sha256(String),
}

impl Observed {
    fn of(path: &Path) -> Self {
        match File::open(path).and_then(sha256_reader) {
            Ok(sha) => Observed::Sha256(sha),
            Err(_) => Observed::Missing,
        }
    }

    fn matches(&self, sha256: &str) -> bool {
        matches!(self, Observed::Sha256(actual) if actual == sha256)
    }

    /// `missing`, or the digest.
    pub fn as_str(&self) -> &str {
        match self {
            Observed::Missing => "missing",
            Observed::Sha256(sha) => sha,
        }
    }
}

/// Whether [`store`] wrote a blob or found it already stored intact.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StoreOutcome {
    Stored,
    AlreadyStored,
}

/// Why [`store`] wrote nothing.
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
    #[error("{0}")]
    Io(#[from] std::io::Error),
}

/// Store the bytes `reader` yields as the blob for `expected`, through a
/// temp file in the store directory and a rename. Skips the write when
/// an intact blob is already there. Refuses ([`StoreError::Changed`]),
/// writing nothing, when the bytes do not hash to `expected` — the
/// caller recorded that digest, and the store must hold exactly those
/// bytes or nothing.
///
/// "Already stored" is decided by [`stored_blob`], which never follows a
/// link: a pre-existing symlink (or any non-file) at the blob path is
/// replaced by the rename, which swaps the link itself, never its target.
pub fn store(repo: &Path, expected: &str, mut reader: impl Read) -> Result<StoreOutcome, StoreError> {
    if !is_sha256_hex(expected) {
        return Err(StoreError::InvalidDigest(expected.to_string()));
    }
    let target = blob_path(repo, expected);
    if stored_blob(repo, expected).matches(expected) {
        return Ok(StoreOutcome::AlreadyStored);
    }
    let dir = store_dir(repo);
    std::fs::create_dir_all(&dir)?;
    if !store_dir_is_real(repo) {
        return Err(StoreError::UnsafeStoreDir);
    }
    let nanos = std::time::SystemTime::now().duration_since(std::time::UNIX_EPOCH).map(|d| d.as_nanos()).unwrap_or(0);
    let temp = dir.join(format!(".tmp-{expected}-{}-{nanos}", std::process::id()));
    let result = (|| {
        let mut file = File::options().write(true).create_new(true).open(&temp)?;
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
        let actual = format!("{:x}", hasher.finalize());
        if actual != expected {
            return Err(StoreError::Changed { expected: expected.to_string(), actual });
        }
        file.sync_all()?;
        drop(file);
        std::fs::rename(&temp, &target)?;
        Ok(StoreOutcome::Stored)
    })();
    if result.is_err() {
        let _ = std::fs::remove_file(&temp);
    }
    result
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

/// The latest record per task and per `(project_id, scenario_id)`
/// (module doc), each once, in ledger order.
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
    ctx.evidence.iter().filter(|r| !r.attachments.is_empty() && latest.contains(&(*r as *const _))).collect()
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

/// One attachment on a latest record whose bytes exist only in the
/// working tree (module doc's back-compat path): not a violation, an
/// advisory pointing at `canon evidence vault`.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UnstoredAttachment {
    pub subject: String,
    pub path: String,
    pub sha256: String,
}

impl UnstoredAttachment {
    pub fn line(&self) -> String {
        format!("{} {} (sha256 {}) is verified against the working tree only", self.subject, self.path, self.sha256)
    }
}

/// The advisories `canon gate check` prints below its violations: every
/// attachment on a latest record with no stored blob whose working-tree
/// file still matches. Sorted by subject, then path.
pub fn unstored_attachments(ctx: &GateContext) -> Vec<UnstoredAttachment> {
    let repo = &ctx.ctx.repo;
    let mut out = Vec::new();
    for record in latest_records(ctx) {
        for attachment in distinct_files(record) {
            if stored_blob(repo, &attachment.sha256) != Observed::Missing {
                continue;
            }
            if working_tree(repo, &attachment.path).matches(&attachment.sha256) {
                out.push(UnstoredAttachment { subject: subject_of(record), path: attachment.path.clone(), sha256: attachment.sha256.clone() });
            }
        }
    }
    out.sort_by(|a, b| (&a.subject, &a.path).cmp(&(&b.subject, &b.path)));
    out
}

/// The `canon gate check` arm (module doc).
pub struct ArtifactStoreCheck;

impl GateCheck for ArtifactStoreCheck {
    fn name(&self) -> &'static str {
        "evidence-artifacts"
    }

    fn run(&self, ctx: &GateContext) -> Vec<Violation> {
        let repo = &ctx.ctx.repo;
        let mut violations = Vec::new();
        for record in latest_records(ctx) {
            for attachment in distinct_files(record) {
                let blob = stored_blob(repo, &attachment.sha256);
                if blob.matches(&attachment.sha256) {
                    continue;
                }
                let tree = working_tree(repo, &attachment.path);
                if blob == Observed::Missing && tree.matches(&attachment.sha256) {
                    continue;
                }
                violations.push(Violation::new(
                    FailureClass::StaleEvidence,
                    subject_of(record),
                    format!(
                        "bound file `{}` recorded sha256 {}; stored blob {}: {}; working tree: {} — the bound bytes are gone, re-run and re-attest with `canon evidence add`",
                        attachment.path,
                        attachment.sha256,
                        blob_display(&attachment.sha256),
                        blob.as_str(),
                        tree.as_str()
                    ),
                ));
            }
        }
        violations.sort_by(|a, b| (&a.subject, &a.detail).cmp(&(&b.subject, &b.detail)));
        violations
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
            scenarios: Vec::new(),
            divergences: Vec::new(),
            subjects: Vec::new(),
            reviews: Vec::new(),
            findings: Vec::new(),
            violations: Vec::new(),
            corpus_violations: Vec::new(),
            unreadable_kinds: Vec::new(),
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
        assert_eq!(unstored_attachments(&ctx), vec![UnstoredAttachment { subject: "a.b.01".into(), path: "smoke.json".into(), sha256: digest }]);
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
        assert!(!dir.path().join(".canon").exists(), "nothing is created for an invalid digest");
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
}
