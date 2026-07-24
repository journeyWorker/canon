//! [`DigestHeader`]: the report's TIMESTAMP-FREE input-digest table
//! (design D2/decision 11, tasks.md 1.2) — lifted near-verbatim from
//! the donor parity harness's `_digest`/`_corpus_digest`/
//! `_ledger_digest`/`_render_report` (verified against the donor
//! source directly, 2026-07-11): sha256, hex, truncated to 12
//! characters; `—` for an absent input (the donor's own literal
//! placeholder).
//!
//! # Mapping parity.py's two-sided model onto canon's own record kinds
//! parity.py splits "corpus" (`.feature` files: the declarative,
//! git-tracked spec) from "ledger" (review/clear/run JSONL: the
//! dynamic, verdict-bearing evidence stream) — this module generalizes
//! that SAME split onto canon-model's own closed kind set rather than
//! introducing a THIRD, canon-specific notion of "corpus": **corpus**
//! = `change`/`task`/`scenario` records (the declarative "what exists
//! to be verified" side); **ledger head** = `evidence_record`/`review`/
//! `divergence` records (the dynamic verdict/attestation side). Both
//! read through `canon_store::git_tier::GitTier` only — the git-
//! tracked, PR-reviewed tier (never the r2 cold tier or the
//! `canon-learn` operator-local store) — because a digest is only
//! meaningful over content a `git diff` could actually show a
//! reviewer, exactly parity.py's own model (its corpus/ledger are both
//! plain git-tracked files, never a cache/cold-tier export).

use std::path::Path;

use canon_model::envelope::RecordKind;
use canon_model::evidence::RawRecord;
use canon_store::git_tier::GitTier;
use canon_store::tier::{Tier, TierQuery};
use sha2::{Digest, Sha256};

use crate::error::ReportError;

/// `policy.yaml`'s fixed on-disk location relative to a repo root
/// (`crates/canon-gate/src/policy.rs::POLICY_YAML_RELATIVE_PATH`,
/// duplicated here as a bare path constant — never the CEL-evaluation
/// logic that constant's owning module implements — since this crate
/// depends on `canon-model`/`canon-store` only, task 1.1).
pub const POLICY_YAML_RELATIVE_PATH: &str = canon_model::paths::POLICY_FILE;

/// The record kinds [`corpus_hash`] digests (module doc).
const CORPUS_KINDS: [RecordKind; 3] = [RecordKind::Change, RecordKind::Task, RecordKind::Scenario];

/// The record kinds [`ledger_hash`] digests (module doc).
const LEDGER_KINDS: [RecordKind; 3] = [RecordKind::EvidenceRecord, RecordKind::Review, RecordKind::Divergence];

/// The three input digests every rendered report header embeds — no
/// `generated_at` or other timestamp field anywhere on this type
/// (decision 11: a timestamp in a git-committed generated file is
/// exactly the drift/conflict source that decision forbids).
///
/// Deliberately excludes `source_git_sha`: a committed `canon/
/// REPORT.md` can never contain the hash of the commit that adds it
/// (that commit's hash is a function of the report's own bytes), so
/// embedding `git rev-parse HEAD` here would make every committed
/// report drift on the very next `--check` (design D2, reconciled).
/// The commit sha this report was generated FROM belongs to the
/// `--snapshot` `manifest.json` instead (D3) — that artifact is never
/// drift-checked, so it can safely carry output-inclusive provenance.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DigestHeader {
    pub corpus_hash: String,
    pub policy_hash: String,
    pub ledger_hash: String,
}

fn digest12(text: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(text.as_bytes());
    let full = hasher.finalize();
    hex_prefix(&full, 12)
}

fn hex_prefix(bytes: &[u8], chars: usize) -> String {
    let mut out = String::with_capacity(chars);
    for byte in bytes {
        if out.len() >= chars {
            break;
        }
        out.push_str(&format!("{byte:02x}"));
    }
    out.truncate(chars);
    out
}

/// Canonical, deterministic serialization of `records` for hashing:
/// sorted by `(at, raw JSON text)` so two runs over identical content —
/// regardless of on-disk file iteration order — hash identically
/// (mirrors parity.py's `_ledger_digest`: `sorted(records, key=lambda
/// r: str(r.get("at", "")))`, `sort_keys=True` on the JSON dump; this
/// module additionally breaks same-`at` ties on the JSON text itself,
/// since `serde_json::Value`'s own `Ord` is not implemented and
/// per-record JSON text IS already canonical — `RawRecord` is read
/// straight off disk, never re-serialized, so key order is exactly
/// what `serde_json::to_string` on that same `Value` produces
/// deterministically for a `Map` in `BTreeMap` mode — no
/// `preserve_order` feature anywhere in this workspace, matching
/// `canon-ingest::normalize`'s own documented assumption).
fn canonical_blob(records: &[RawRecord]) -> Option<String> {
    if records.is_empty() {
        return None;
    }
    let mut texts: Vec<String> = records.iter().map(|r| serde_json::to_string(&r.0).unwrap_or_default()).collect();
    texts.sort();
    Some(texts.join("\n"))
}

fn read_kind(git_root: &Path, kind: RecordKind) -> Result<Vec<RawRecord>, ReportError> {
    let tier = GitTier::new(git_root);
    match tier.read(&TierQuery::kind(kind)) {
        Ok(result) => Ok(result.records),
        // A missing/unreadable git root degrades to "no records for
        // this kind" rather than aborting digest computation — mirrors
        // `PolicyResolution::resolve`'s own "fail-soft load" posture
        // for a repo that has not yet routed anything to this kind.
        Err(_) => Ok(Vec::new()),
    }
}

fn digest_kinds(git_root: &Path, kinds: &[RecordKind]) -> Result<String, ReportError> {
    let mut all = Vec::new();
    for kind in kinds {
        all.extend(read_kind(git_root, *kind)?);
    }
    Ok(match canonical_blob(&all) {
        Some(blob) => digest12(&blob),
        // parity.py's own literal placeholder for "nothing to hash"
        // (`_ledger_digest`: `if not records: return "—"`).
        None => "—".to_string(),
    })
}

impl DigestHeader {
    /// Computes every input digest from `repo_root` (for `policy.yaml`)
    /// and `git_root` (`canon.yaml`'s `tiers.git.root` for the
    /// corpus/ledger record scan, module doc: git tier only, never
    /// r2/learn).
    pub fn compute(repo_root: &Path, git_root: &Path) -> Result<Self, ReportError> {
        let corpus_hash = digest_kinds(git_root, &CORPUS_KINDS)?;
        let ledger_hash = digest_kinds(git_root, &LEDGER_KINDS)?;
        let policy_path = repo_root.join(POLICY_YAML_RELATIVE_PATH);
        let policy_hash = match std::fs::read_to_string(&policy_path) {
            Ok(text) => digest12(&text),
            Err(_) => "—".to_string(),
        };
        Ok(Self { corpus_hash, policy_hash, ledger_hash })
    }

    /// One combined 12-hex digest over all three input digests —
    /// `--snapshot`'s `manifest.json` `source_digest` field (design
    /// D3). Unlike the three sub-digests above, this is NEVER embedded
    /// in the drift-checked markdown header (decision 11/D2's own
    /// reconciliation note): `manifest.json` is not drift-checked, so
    /// it may safely carry this output-inclusive summary fingerprint.
    pub fn combined_digest(&self) -> String {
        digest12(&format!("{}|{}|{}", self.corpus_hash, self.policy_hash, self.ledger_hash))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn digest12_is_deterministic_and_twelve_hex_characters() {
        let a = digest12("hello");
        let b = digest12("hello");
        assert_eq!(a, b);
        assert_eq!(a.len(), 12);
        assert!(a.chars().all(|c| c.is_ascii_hexdigit()));
    }

    #[test]
    fn digest12_differs_for_different_input() {
        assert_ne!(digest12("a"), digest12("b"));
    }

    #[test]
    fn digest_kinds_over_an_empty_git_root_is_the_em_dash_placeholder() {
        let dir = tempfile::tempdir().unwrap();
        let hash = digest_kinds(dir.path(), &CORPUS_KINDS).unwrap();
        assert_eq!(hash, "—");
    }

    #[test]
    fn compute_reads_policy_yaml_when_present() {
        let dir = tempfile::tempdir().unwrap();
        std::fs::create_dir_all(dir.path().join(".canon")).unwrap();
        std::fs::write(dir.path().join(POLICY_YAML_RELATIVE_PATH), "trust_required:\n  p1: human\n").unwrap();

        let header = DigestHeader::compute(dir.path(), &dir.path().join("ledger")).unwrap();
        assert_ne!(header.policy_hash, "—");
        assert_eq!(header.policy_hash.len(), 12);
    }

    #[test]
    fn compute_em_dashes_policy_hash_when_policy_yaml_is_absent() {
        let dir = tempfile::tempdir().unwrap();
        let header = DigestHeader::compute(dir.path(), &dir.path().join("ledger")).unwrap();
        assert_eq!(header.policy_hash, "—");
    }
}
