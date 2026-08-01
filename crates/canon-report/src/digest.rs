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
//!
//! # The open half: namespaced overlay kinds
//! `RecordKind` is closed; the git tier's `kind=<x>/` namespace is
//! NOT. `GitTier::write_namespaced` stores plugin overlay records
//! (`<namespace>.<kind>`, e.g. `porting.coverage`) in the same tier
//! beside the core kinds; `canon-store`'s `stg_git_records` view globs
//! `kind=*/**/*.json` without distinguishing them; and
//! `mart_scope_status` selects `kind = 'porting.coverage'` straight
//! out of the result, which the report RENDERS and `--snapshot`
//! EXPORTS to `mart_scope_status.parquet`. A digest that partitions
//! only `RecordKind::ALL` therefore let a corpus differing ONLY in a
//! coverage overlay row ship DIFFERENT parquet bytes under an
//! IDENTICAL `source_digest` (s43 round 3, finding 1).
//!
//! **Which overlays are digested: all of them.** Two readings were
//! available. "Digest what this snapshot describes" would cover only
//! the overlay kinds some exported mart reads today; "digest the
//! corpus this snapshot came from" covers every overlay file the tier
//! holds. The second is chosen, for two reasons. The mart→overlay
//! coupling lives in SQL text (`sql/views.sql`'s literal
//! `kind = 'porting.coverage'`), out of this crate's reach, so the
//! first reading is not computable here without a SECOND hand-kept
//! list — and a hand-kept list is the exact mechanism that produced
//! this finding and finding 8 before it. And the two failure modes are
//! not symmetric: over-covering costs a spurious digest change on an
//! overlay nothing reads, while under-covering is a false provenance
//! claim, which is the only thing this value exists to make.
//!
//! Overlays land on the CORPUS side. An overlay is authored,
//! git-tracked, PR-reviewable declaration hanging off a core record
//! (`porting.coverage`'s declared `core_kind` is `scenario`) — the
//! module's own membership test for the declarative side. The ledger
//! side stays exactly canon's four closed verdict kinds, so "ledger
//! head" keeps meaning "canon's own attestations" rather than
//! "whatever a plugin wrote".

use std::path::{Path, PathBuf};

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

/// Which digest a CORE record kind feeds, or why it feeds neither.
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
///
/// This closes the CLOSED half only. A `match` over an enum cannot
/// see the open `kind=<x>/` namespace beside it, which is precisely
/// how the whole namespaced-overlay space slipped past a construct
/// written to make slipping past impossible (s43 round 3, finding 1).
/// The open half is covered by DISCOVERY instead — see
/// [`overlay_kind_dirs`], which has no list to fall out of date.
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

/// The CORE record kinds [`corpus_hash`] digests, derived from
/// [`digest_side`] so the two can never disagree. The corpus digest
/// additionally covers every namespaced overlay ([`overlay_texts`]),
/// which no `RecordKind` list can name.
pub(crate) fn corpus_kinds() -> Vec<RecordKind> {
    kinds_on(DigestSide::Corpus)
}

/// The record kinds [`ledger_hash`] digests, derived from
/// [`digest_side`] so the two can never disagree.
pub(crate) fn ledger_kinds() -> Vec<RecordKind> {
    kinds_on(DigestSide::Ledger)
}

fn kinds_on(side: DigestSide) -> Vec<RecordKind> {
    RecordKind::ALL.into_iter().filter(|kind| digest_side(*kind) == side).collect()
}

/// Every namespaced-overlay `kind=<x>/` directory the git tier holds:
/// every `kind=` directory whose `<x>` is not one of the closed core
/// kinds (module doc, "the open half"), as `(kind, absolute path)`
/// sorted by kind.
///
/// DISCOVERED from the tier's own directory listing, never enumerated
/// from a list this module maintains, and deliberately mirroring
/// `canon-store`'s `stg_git_records` glob (`kind=*/**/*.json`,
/// `sql/views.sql`) rather than any Rust-side registry: that glob IS
/// the read surface every mart is built over, so matching it is what
/// makes "the digest covers what the report reads" a checkable
/// statement instead of an aspiration. A future overlay kind cannot
/// slip past the way `Subject`/`Finding` slipped past the hand-kept
/// arrays, because nothing here has to be told the kind exists.
fn overlay_kind_dirs(git_root: &Path) -> Vec<(String, PathBuf)> {
    let Ok(entries) = std::fs::read_dir(git_root) else { return Vec::new() };
    let mut dirs: Vec<(String, PathBuf)> = entries
        .flatten()
        .filter_map(|entry| {
            let path = entry.path();
            if !path.is_dir() {
                return None;
            }
            let name = entry.file_name().to_string_lossy().into_owned();
            let kind = name.strip_prefix("kind=")?.to_string();
            (!RecordKind::ALL.iter().any(|core| core.as_str() == kind)).then_some((kind, path))
        })
        .collect();
    dirs.sort();
    dirs
}

/// One corpus-digest input line per overlay FILE: its tier-relative
/// path, then a full sha256 over the file's bytes.
///
/// Deliberately NOT `GitTier::scan_namespaced_kind`. That reader first
/// validates the `<namespace>.<kind>` identity grammar — so a
/// directory named `kind=weird` is an `Err` it never reads at all —
/// and then drops an unparseable body as a violation. Both are files
/// `stg_git_records`' glob still hands to `mart_scope_status`, so
/// routing the digest through that reader would reintroduce the same
/// hole one layer down. Hashing raw bytes covers exactly what the glob
/// covers and needs no schema this crate does not have for a foreign
/// namespace. The relative path is part of the input so a file moved
/// between overlay kinds still moves the digest.
fn overlay_texts(git_root: &Path) -> Vec<String> {
    let mut texts = Vec::new();
    let mut files = Vec::new();
    for (_, dir) in overlay_kind_dirs(git_root) {
        files.clear();
        collect_json_files(&dir, &mut files);
        for file in &files {
            let Ok(bytes) = std::fs::read(file) else { continue };
            let relative = file.strip_prefix(git_root).unwrap_or(file).to_string_lossy().replace('\\', "/");
            let mut hasher = Sha256::new();
            hasher.update(&bytes);
            texts.push(format!("{relative}\n{}", hex_prefix(&hasher.finalize(), 64)));
        }
    }
    texts
}

fn collect_json_files(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else { return };
    for entry in entries.flatten() {
        let path = entry.path();
        if path.is_dir() {
            collect_json_files(&path, out);
        } else if path.extension().is_some_and(|ext| ext == "json") {
            out.push(path);
        }
    }
}

/// What the rendered header's corpus row claims to cover, GENERATED
/// from [`corpus_kinds`] and the overlay rule rather than hand-typed
/// beside them (s43 round 3, finding 5: the label still read
/// `change/task/scenario` two kinds after `Subject` was added). A full
/// statement of coverage, never a curated subset — every core kind on
/// this side is named, and the open half is named as the open half,
/// since its members are discovered per repo and cannot be listed
/// here.
pub(crate) fn corpus_coverage_label() -> String {
    format!("{}, plus every namespaced overlay in the git tier", slashed(&corpus_kinds()))
}

/// What the rendered header's ledger row covers — same contract as
/// [`corpus_coverage_label`]; no overlay clause, because overlays are
/// corpus-side (module doc).
pub(crate) fn ledger_coverage_label() -> String {
    slashed(&ledger_kinds())
}

fn slashed(kinds: &[RecordKind]) -> String {
    kinds.iter().map(|kind| kind.as_str()).collect::<Vec<_>>().join("/")
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

/// Canonical, deterministic serialization of `records` as digest-input
/// lines: one line per record, later sorted by [`digest_texts`] so two
/// runs over identical content — regardless of on-disk file iteration
/// order — hash identically (mirrors parity.py's `_ledger_digest`:
/// `sorted(records, key=lambda r: str(r.get("at", "")))`,
/// `sort_keys=True` on the JSON dump; this module sorts on the JSON
/// text itself, since `serde_json::Value`'s own `Ord` is not
/// implemented and per-record JSON text IS already canonical —
/// `RawRecord` is read straight off disk, never re-serialized, so key
/// order is exactly what `serde_json::to_string` on that same `Value`
/// produces deterministically for a `Map` in `BTreeMap` mode — no
/// `preserve_order` feature anywhere in this workspace, matching
/// `canon-ingest::normalize`'s own documented assumption).
fn record_texts(records: &[RawRecord]) -> Vec<String> {
    records.iter().map(|r| serde_json::to_string(&r.0).unwrap_or_default()).collect()
}

/// One digest over a side's input lines, whatever produced them
/// ([`record_texts`] for core kinds, [`overlay_texts`] for the open
/// half): sorted, newline-joined, sha256, first 12 hex.
fn digest_texts(mut texts: Vec<String>) -> String {
    if texts.is_empty() {
        // parity.py's own literal placeholder for "nothing to hash"
        // (`_ledger_digest`: `if not records: return "—"`).
        return "—".to_string();
    }
    texts.sort();
    digest12(&texts.join("\n"))
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

fn kind_texts(git_root: &Path, kinds: &[RecordKind]) -> Result<Vec<String>, ReportError> {
    let mut texts = Vec::new();
    for kind in kinds {
        texts.extend(record_texts(&read_kind(git_root, *kind)?));
    }
    Ok(texts)
}

impl DigestHeader {
    /// Computes every input digest from `repo_root` (for `policy.yaml`)
    /// and `git_root` (`canon.yaml`'s `tiers.git.root` for the
    /// corpus/ledger record scan, module doc: git tier only, never
    /// r2/learn).
    pub fn compute(repo_root: &Path, git_root: &Path) -> Result<Self, ReportError> {
        let mut corpus = kind_texts(git_root, &corpus_kinds())?;
        // The open half (module doc): every namespaced overlay the
        // tier holds, discovered rather than listed.
        corpus.extend(overlay_texts(git_root));
        let corpus_hash = digest_texts(corpus);
        let ledger_hash = digest_texts(kind_texts(git_root, &ledger_kinds())?);
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
    fn a_digest_side_over_an_empty_git_root_is_the_em_dash_placeholder() {
        let dir = tempfile::tempdir().unwrap();
        let hash = digest_texts(kind_texts(dir.path(), &corpus_kinds()).unwrap());
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

    /// A well-formed overlay body for `namespaced_kind`, keyed
    /// `{project_id}__{scenario_id}` exactly as
    /// `GitTier::write_namespaced`'s own natural-key check requires —
    /// hand-built JSON, because an overlay kind has no `canon-model`
    /// type at all (that is the whole reason `digest_side`'s `match`
    /// could not see it).
    fn overlay_body(namespaced_kind: &str, project_id: &str, scenario_id: &str, covered: bool) -> RawRecord {
        RawRecord(serde_json::json!({
            "schema": 1,
            "kind": namespaced_kind,
            "at": "2026-01-02T12:00:00Z",
            "actor": {"agent_id": "porting-sync", "role": "implementer"},
            "project_id": project_id,
            "scenario_id": scenario_id,
            "covered": covered,
        }))
    }

    /// s43 round 3, finding 1. `mart_scope_status` reads
    /// `kind = 'porting.coverage'` straight out of `stg_records`, and
    /// the report both RENDERS that mart and EXPORTS it to
    /// `mart_scope_status.parquet` — but `digest_side` partitions only
    /// `RecordKind::ALL`, so a corpus differing ONLY in a coverage
    /// overlay row shipped different parquet bytes under an identical
    /// `source_digest`. Asserted through `combined_digest`, the value
    /// `manifest.json` actually carries.
    #[test]
    fn a_corpus_differing_only_in_a_porting_coverage_overlay_changes_the_source_digest() {
        let dir = tempfile::tempdir().unwrap();
        let git_root = dir.path().join("ledger");
        let tier = GitTier::new(&git_root);
        let digest = || DigestHeader::compute(dir.path(), &git_root).unwrap().combined_digest();

        let baseline = digest();
        tier.write_namespaced("porting.coverage", "root__s9.fixture.03", overlay_body("porting.coverage", "root", "s9.fixture.03", true))
            .unwrap();
        let with_overlay = digest();
        assert_ne!(baseline, with_overlay, "an overlay-only change must move `source_digest`");

        // And the row's own CONTENT counts, not merely its existence:
        // flipping `covered` is exactly what moves a
        // `mart_scope_status` cell.
        tier.write_namespaced("porting.coverage", "root__s9.fixture.03", overlay_body("porting.coverage", "root", "s9.fixture.03", false))
            .unwrap();
        assert_ne!(with_overlay, digest(), "flipping an overlay row's `covered` must move `source_digest`");
    }

    /// The mechanism, exercised directly. Nothing in this crate has
    /// ever heard of `future.widget`; the digest covers it anyway,
    /// because [`overlay_kind_dirs`] DISCOVERS `kind=` directories
    /// instead of consulting a list. This is the property that stops
    /// the next namespaced kind repeating finding 1 — there is no list
    /// for it to be missing from.
    #[test]
    fn an_overlay_kind_this_crate_has_never_heard_of_still_moves_the_source_digest() {
        let dir = tempfile::tempdir().unwrap();
        let git_root = dir.path().join("ledger");
        let tier = GitTier::new(&git_root);
        let digest = || DigestHeader::compute(dir.path(), &git_root).unwrap().combined_digest();

        let baseline = digest();
        tier.write_namespaced("future.widget", "root__w1", overlay_body("future.widget", "root", "w1", true)).unwrap();
        assert_ne!(baseline, digest(), "an unknown overlay kind must still move `source_digest`");
    }

    /// The other half of the mechanism: a `kind=` directory whose name
    /// is not a LEGAL overlay identity at all. `stg_git_records`' glob
    /// (`kind=*/**/*.json`) reads its files into `stg_records`
    /// regardless, so the digest must too — which is why
    /// [`overlay_texts`] hashes bytes rather than routing through
    /// `GitTier::scan_namespaced_kind`, whose grammar check (asserted
    /// here) would have refused this directory outright.
    #[test]
    fn a_foreign_kind_directory_that_is_not_a_legal_overlay_identity_is_still_digested() {
        let dir = tempfile::tempdir().unwrap();
        let git_root = dir.path().join("ledger");
        let digest = || DigestHeader::compute(dir.path(), &git_root).unwrap().combined_digest();

        std::fs::create_dir_all(git_root.join("kind=weird")).unwrap();
        let baseline = digest();
        assert!(
            GitTier::new(&git_root).scan_namespaced_kind("weird").is_err(),
            "this test is only meaningful while `scan_namespaced_kind` refuses the name — if it starts accepting it, `overlay_texts` may be rerouted through it"
        );

        std::fs::write(git_root.join("kind=weird/thing__000000000000.json"), "{\"kind\":\"weird\"}\n").unwrap();
        let with_file = digest();
        assert_ne!(baseline, with_file, "a file under an ungrammatical foreign kind dir must move `source_digest`");

        std::fs::write(git_root.join("kind=weird/thing__000000000000.json"), "{\"kind\":\"weird\",\"x\":1}\n").unwrap();
        assert_ne!(with_file, digest(), "editing that file's bytes must move `source_digest`");
    }

    /// [`overlay_kind_dirs`] is exactly "every `kind=` directory that
    /// is not a core kind" — no core kind leaks in (they are digested
    /// through `digest_side`, and counting them twice would be a
    /// silent double-hash), and no non-`kind=` directory does.
    #[test]
    fn overlay_kind_dirs_is_every_kind_directory_that_is_not_a_core_kind() {
        let dir = tempfile::tempdir().unwrap();
        for kind in RecordKind::ALL {
            std::fs::create_dir_all(dir.path().join(format!("kind={}", kind.as_str()))).unwrap();
        }
        for foreign in ["porting.coverage", "future.widget", "weird"] {
            std::fs::create_dir_all(dir.path().join(format!("kind={foreign}"))).unwrap();
        }
        std::fs::create_dir_all(dir.path().join("not-a-kind-dir")).unwrap();
        std::fs::write(dir.path().join("kind=stray-file"), "").unwrap();

        let found: Vec<String> = overlay_kind_dirs(dir.path()).into_iter().map(|(kind, _)| kind).collect();
        assert_eq!(found, vec!["future.widget".to_string(), "porting.coverage".to_string(), "weird".to_string()]);
    }

    /// The exclusion half of `manifest.rs`'s account, asserted rather
    /// than asserted-about: a record of an excluded kind must leave
    /// `source_digest` alone. Without this, "excludes exactly six core
    /// kinds" is a claim only the `match` arms support, and the two
    /// findings above are both cases of a claim no test held down.
    #[test]
    fn a_record_of_an_excluded_kind_does_not_move_the_source_digest() {
        use canon_model::envelope::{Actor, Envelope};
        use canon_model::ids::{RoleId, SessionId};
        use canon_model::records::Session;

        let dir = tempfile::tempdir().unwrap();
        let git_root = dir.path().join("ledger");
        let tier = GitTier::new(&git_root);
        let at = chrono::Utc::now();

        let baseline = DigestHeader::compute(dir.path(), &git_root).unwrap().combined_digest();
        tier.write(&Session::new(
            Envelope::new(1, RecordKind::Session, at, Actor::new("agent1", RoleId::parse("dev").unwrap())),
            SessionId::parse("11111111-1111-4111-8111-111111111111").unwrap(),
            "omp",
            at,
            None,
        ))
        .unwrap();
        assert_eq!(
            baseline,
            DigestHeader::compute(dir.path(), &git_root).unwrap().combined_digest(),
            "`session` is a documented exclusion (manifest.rs); a session-only change must NOT move `source_digest`"
        );
    }
}
