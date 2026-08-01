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
//! = `change`/`task`/`scenario`/`subject` records (the declarative
//! "what exists to be verified" side); **ledger head** =
//! `evidence_record`/`review`/`divergence`/`finding` records (the
//! dynamic verdict/attestation side). Which side each kind lands on
//! is [`digest_side`]'s exhaustive `match`, never a hand-kept list —
//! see its doc for the two kinds that went missing when the lists were
//! hand-kept. Both
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

/// Which digest a record kind feeds, or why it feeds neither.
///
/// An exhaustive `match` rather than two hand-maintained arrays,
/// because the arrays were wrong. The digest was written against
/// twelve kinds; `Subject` (s36) and `Finding` (s43) were each added
/// as a reviewed, breaking `canon-model` change and neither list
/// heard about it, so a corpus differing ONLY in a subject or a
/// finding produced an IDENTICAL `source_digest` and the snapshot
/// claimed provenance over content it did not cover (s43 round 2,
/// finding 8). `RecordKind::ALL` walks every kind through here, and
/// adding a fifteenth stops compiling until someone says which side
/// it lands on — the same "a new kind is a reviewed change, never a
/// silent default" posture `canon-model` already takes.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum DigestSide {
    /// The declarative "what exists to be verified" side
    /// ([`corpus_hash`], module doc).
    Corpus,
    /// The dynamic verdict/attestation side ([`ledger_hash`], module
    /// doc).
    Ledger,
    /// Digested by neither, with the reason stated rather than left to
    /// the reader to reconstruct from an absence.
    Neither,
}

/// Every kind's side, exhaustively (see [`DigestSide`]).
const fn digest_side(kind: RecordKind) -> DigestSide {
    match kind {
        // Declarative, git-tracked, PR-reviewed: what exists to be
        // verified. `Subject` is the product/management unit the rest
        // of the corpus hangs off, authored and diffable exactly like
        // a `Change` — it belongs here and was simply missed.
        RecordKind::Change | RecordKind::Task | RecordKind::Scenario | RecordKind::Subject => DigestSide::Corpus,
        // Verdict/attestation over that corpus. `Finding` is review
        // evidence committed alongside the code it judges (canon.yaml
        // routes it `local`, beside `review`/`divergence`/
        // `evidence_record`), and it is the SOURCE a release
        // narrative's issue counts are derived from — a snapshot whose
        // digest ignored it would claim to describe a corpus it could
        // not distinguish.
        RecordKind::EvidenceRecord | RecordKind::Review | RecordKind::Divergence | RecordKind::Finding => DigestSide::Ledger,
        // Ingested execution telemetry: machine-derived from agent
        // transcripts, re-derivable by re-running `canon ingest
        // sessions`, and routed off the git tier this module reads.
        // Not something a `git diff` shows a reviewer, which is the
        // module doc's own membership test.
        RecordKind::Session | RecordKind::Run | RecordKind::Event => DigestSide::Neither,
        // In-flight coordination state, routed `hot` and aged to
        // `cold`; a handoff is a message between agents, not a claim
        // about the corpus.
        RecordKind::Handoff => DigestSide::Neither,
        // The `canon-learn` memory tier the module doc excludes by
        // name: a trajectory is raw reward-eligible telemetry, and a
        // strategy item is re-derived FROM trajectories. Neither
        // asserts anything about what exists to be verified or how it
        // was judged.
        RecordKind::Trajectory | RecordKind::StrategyItem => DigestSide::Neither,
    }
}

/// The record kinds [`corpus_hash`] digests, derived from
/// [`digest_side`] so the two can never disagree.
fn corpus_kinds() -> Vec<RecordKind> {
    kinds_on(DigestSide::Corpus)
}

/// The record kinds [`ledger_hash`] digests, derived from
/// [`digest_side`] so the two can never disagree.
fn ledger_kinds() -> Vec<RecordKind> {
    kinds_on(DigestSide::Ledger)
}

fn kinds_on(side: DigestSide) -> Vec<RecordKind> {
    RecordKind::ALL.into_iter().filter(|kind| digest_side(*kind) == side).collect()
}

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
        let corpus_hash = digest_kinds(git_root, &corpus_kinds())?;
        let ledger_hash = digest_kinds(git_root, &ledger_kinds())?;
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
        let hash = digest_kinds(dir.path(), &corpus_kinds()).unwrap();
        assert_eq!(hash, "—");
    }

    /// Every kind lands on exactly one side, and the side that digests
    /// NOTHING is exactly the six documented exclusions. Without this,
    /// `digest_side`'s exhaustive `match` still forces a decision for a
    /// fifteenth kind but nothing stops that decision being
    /// `Neither` by reflex — which is how `Subject` and `Finding` went
    /// missing under the hand-kept arrays.
    #[test]
    fn every_record_kind_is_assigned_a_digest_side() {
        let corpus = corpus_kinds();
        let ledger = ledger_kinds();
        let neither: Vec<RecordKind> = kinds_on(DigestSide::Neither);
        assert_eq!(corpus.len() + ledger.len() + neither.len(), RecordKind::ALL.len(), "the three sides must partition RecordKind::ALL");

        assert_eq!(corpus, vec![RecordKind::Change, RecordKind::Task, RecordKind::Scenario, RecordKind::Subject]);
        assert_eq!(ledger, vec![RecordKind::Review, RecordKind::Divergence, RecordKind::EvidenceRecord, RecordKind::Finding]);
        assert_eq!(
            neither,
            vec![
                RecordKind::Session,
                RecordKind::Run,
                RecordKind::Event,
                RecordKind::Handoff,
                RecordKind::Trajectory,
                RecordKind::StrategyItem,
            ],
            "an exclusion is a decision with a reason in `digest_side`, never a kind nobody classified"
        );
    }

    /// s43 round 2, finding 8. `Finding` was outside both digest lists,
    /// so a corpus differing ONLY in a finding produced a byte-identical
    /// `source_digest` — the snapshot's `manifest.json` asserting
    /// provenance over a corpus it could not tell apart. `Subject` (s36)
    /// had the same hole for two releases. Both are asserted through
    /// `combined_digest`, the value `manifest.json` actually carries,
    /// rather than through the sub-hash, because the sub-hash could
    /// change while the combination did not.
    #[test]
    fn a_corpus_differing_only_in_a_finding_or_a_subject_changes_the_source_digest() {
        use canon_model::envelope::{Actor, Envelope};
        use canon_model::ids::{ChangeId, RoleId, SubjectId};
        use canon_model::records::{Finding, FindingSeverity, Subject, SubjectStatus};
        use canon_store::git_tier::GitTier;

        let dir = tempfile::tempdir().unwrap();
        let git_root = dir.path().join("ledger");
        let tier = GitTier::new(&git_root);
        let actor = || Actor::new("reviewer1", RoleId::parse("reviewer").unwrap());
        let at = chrono::Utc::now();

        let baseline = DigestHeader::compute(dir.path(), &git_root).unwrap().combined_digest();

        tier.write(&Finding::new(
            Envelope::new(1, RecordKind::Finding, at, actor()),
            ChangeId::parse("s43-findings-are-records").unwrap(),
            1,
            1,
            FindingSeverity::Blocker,
            "reviewer1",
            "the snapshot claimed provenance it no longer had",
        ))
        .unwrap();
        let with_finding = DigestHeader::compute(dir.path(), &git_root).unwrap().combined_digest();
        assert_ne!(baseline, with_finding, "a finding-only change must move `source_digest`");

        tier.write(&Subject::new(
            Envelope::new(1, RecordKind::Subject, at, actor()),
            SubjectId::parse("digest-coverage").unwrap(),
            "digest coverage",
            "a subject-only change must move `source_digest` too",
            "canon",
            SubjectStatus::Building,
            RoleId::parse("dev").unwrap(),
        ))
        .unwrap();
        let with_subject = DigestHeader::compute(dir.path(), &git_root).unwrap().combined_digest();
        assert_ne!(with_finding, with_subject, "a subject-only change must move `source_digest`");
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
