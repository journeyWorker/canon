//! Per-source ingest watermark cursors (S3 §3, tasks 3.1/3.2).
//!
//! A `canon ingest sessions` pass persists one [`SourceCursor`] per
//! adapter source through canon-store AFTER a successful, durable
//! write, so the next pass can GATE its scan — skipping the
//! parse/normalize/persist of files whose bytes are provably unchanged
//! since they were last ingested. This turns `--watch`'s poll loop from
//! "re-parse + re-normalize + re-persist the whole corpus every poll"
//! into "re-parse only what actually changed" (the persist layer was
//! already idempotent per S3 4.2; the watermark removes the wasted
//! parse/normalize work above it).
//!
//! ## Two documented deviations from S3 3.1's literal wording
//!
//! 1. **Storage location.** 3.1 says the cursor persists "through
//!    canon-store". A per-operator scan cursor is machine-local mutable
//!    state — it changes on every poll, differs per checkout, and
//!    gating a local `--watch` loop must not require cloud
//!    credentials — so it is NOT written into the git-committed
//!    `GitTier`, the shared `PgTier`, or the cloud `R2Tier`. It
//!    persists under a local cursor root ([`CursorStore::open`]'s
//!    `root`, by convention `<repo>/.canon/ingest/cursors/`, gitignored
//!    alongside `.canon/learn` + `.canon/r2`). canon-store still OWNS the
//!    cursor type and its atomic IO (via [`crate::write_atomic`]), so
//!    "through canon-store" holds at the persistence-authority level;
//!    only the tier CHOICE (a local root, not git/pg/r2) deviates, and
//!    it deviates deliberately.
//! 2. **Cursor shape + a sound gate.** 3.1 names `{source_id,
//!    last_seen_at, last_seen_digest}`. All three are present and
//!    meaningful. But a single per-source `last_seen_at` high-water
//!    mark is an UNSOUND gate: a never-seen transcript that arrives
//!    with a pre-cursor mtime (a copied/restored file) would be
//!    silently false-skipped, and a same-`(mtime, size)` in-place
//!    rewrite would too. So the cursor additionally carries a per-file
//!    [`FileSeen`] index, and the skip predicate is a full-content
//!    digest match — a file counts as unchanged ONLY when its current
//!    bytes hash to exactly what was ingested before, so there is no
//!    false exclusion (any new path, any byte change, any deletion is a
//!    change). The gate is applied at **source granularity**
//!    ([`SourceCursor::source_unchanged`]): a whole adapter source's
//!    parse is skipped IFF its entire present file set is byte-identical
//!    to the cursor. A single per-FILE skip would be unsound when two
//!    files contribute rows to one `session_id` (a session spanning
//!    transcripts, a Codex fork-replay): skipping one would re-normalize
//!    that session from a partial row set and persist a divergent
//!    record. Re-parsing a source as a complete set whenever ANYTHING
//!    in it changed keeps every session's normalization whole.
//!    ([`SourceCursor::is_ingested_unchanged`] is the per-file predicate
//!    `source_unchanged` is built from, kept public for callers that
//!    genuinely have a one-file-per-session source.) `last_seen_at` /
//!    `last_seen_digest` are DERIVED from the index
//!    ([`SourceCursor::refresh_summary`]: max mtime / digest of the
//!    sorted index) and kept as 3.1's coarse summary fields.
//!
//! The gate still READS every present file each poll (to digest it);
//! avoiding that re-read via an intra-file byte-offset/line-count
//! resume is S3 3.3 — a further optimization layered ON TOP of this
//! cursor, deliberately not implemented here (it is append-only-`.jsonl`
//! and Claude/Codex-specific, and genuinely fragile). Reading + hashing
//! a file is cheap relative to the parse + normalize + persist this
//! cursor skips, so 3.1/3.2's `--watch` win lands without it.

use std::collections::{BTreeMap, BTreeSet};
use std::path::{Path, PathBuf};

use chrono::{DateTime, TimeZone, Utc};
use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};

use crate::atomic::write_atomic;
use crate::tier::StoreError;

/// The full 64-hex sha256 of a file's raw bytes — the cursor's
/// per-file content fingerprint. Distinct from
/// [`crate::partition::content_digest12`]'s 12-hex git-path suffix: a
/// cursor is machine-local operator state, never a git path, so it uses
/// the full digest for a vanishing collision probability. Standard
/// FIPS 180-4 sha256, byte-for-byte reproducible.
pub fn file_digest(bytes: &[u8]) -> String {
    let hash = Sha256::digest(bytes);
    let mut hex = String::with_capacity(64);
    for byte in hash.iter() {
        hex.push_str(&format!("{byte:02x}"));
    }
    hex
}

/// One already-ingested file's stat + content digest.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct FileSeen {
    /// File mtime in epoch-millis at the time it was last ingested —
    /// informational only (it feeds 3.1's `last_seen_at`). The gate
    /// decides on `digest`, NEVER mtime, so a preserved-mtime rewrite
    /// is still caught.
    pub mtime_ms: i64,
    /// File size in bytes at last ingest — informational (paired with
    /// `mtime_ms` for a human-legible cursor).
    pub size: u64,
    /// Full sha256 of the file's raw bytes at last ingest — the SOLE
    /// skip predicate (module doc deviation 2).
    pub digest: String,
}

/// One adapter source's persisted watermark cursor — S3 3.1's
/// `{source_id, last_seen_at, last_seen_digest}` plus the per-file
/// soundness index (module doc deviation 2).
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceCursor {
    /// The adapter's static `client_id` (`omp`/`claude`/`codex`/
    /// `hermes`) — one cursor per source.
    pub source_id: String,
    /// The max `FileSeen.mtime_ms` across `files`, as a timestamp —
    /// 3.1's coarse "last_seen_at" high-water mark (informational; the
    /// gate never reads it).
    #[serde(default)]
    pub last_seen_at: Option<DateTime<Utc>>,
    /// A digest over the sorted `files` index — 3.1's
    /// "last_seen_digest"; changes whenever any file is added, changed,
    /// or removed. A cheap "did anything change at all since the last
    /// pass?" fingerprint.
    #[serde(default)]
    pub last_seen_digest: String,
    /// Per-file `path -> FileSeen` index (deviation 2). The key is
    /// `Path::to_string_lossy` — a non-UTF-8 transcript path is
    /// astronomically rare for these CLIs, and a lossy collision only
    /// forces a re-scan of the colliding path, never a false skip.
    #[serde(default)]
    pub files: BTreeMap<String, FileSeen>,
}

/// [`SourceCursor::diff`]'s per-file partition result (s31 D1) —
/// deliberately a THIRD gate beside (never replacing)
/// [`SourceCursor::source_unchanged`]'s source-granularity skip and
/// [`SourceCursor::is_ingested_unchanged`]'s single-file predicate
/// they're built from: a caller whose adapter contract guarantees one
/// file per session (module doc's `SessionAdapter` note) can gate at
/// FILE granularity instead of re-parsing an entire source whenever
/// any one of its files changes.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct CursorDiff {
    /// Present paths that are new to this cursor OR whose digest no
    /// longer matches what was indexed — must be (re)parsed this pass.
    pub changed_or_new: BTreeSet<String>,
    /// Present paths whose digest exactly matches the index — already
    /// durably ingested, safe to skip parsing entirely.
    pub unchanged: BTreeSet<String>,
}

impl SourceCursor {
    /// A fresh, empty cursor for `source_id` (first run / after reset).
    pub fn empty(source_id: impl Into<String>) -> Self {
        Self { source_id: source_id.into(), last_seen_at: None, last_seen_digest: String::new(), files: BTreeMap::new() }
    }

    fn key(path: &Path) -> String {
        path.to_string_lossy().into_owned()
    }

    /// The SOLE skip decision (module doc deviation 2): `true` iff
    /// `path`'s current `digest` equals what was ingested before, i.e.
    /// the file's bytes are provably unchanged since the last durable
    /// write, so its records are already stored and the scan may skip
    /// parsing/normalizing/persisting it. Any unseen path, or any byte
    /// change, returns `false` (no false exclusion).
    pub fn is_ingested_unchanged(&self, path: &Path, digest: &str) -> bool {
        self.files.get(&Self::key(path)).is_some_and(|f| f.digest == digest)
    }

    /// The SOURCE-granularity gate (module doc deviation 2): `true` iff
    /// every present file is already ingested at its current digest AND
    /// the present set is exactly the cursor's set (no additions, no
    /// deletions). `present` maps each present file's
    /// `Path::to_string_lossy` key to its current content digest. When
    /// `true` the whole source's parse/normalize/persist is skipped;
    /// when `false` the source is re-parsed as a complete set so no
    /// multi-file session is ever partially re-normalized.
    pub fn source_unchanged(&self, present: &BTreeMap<String, String>) -> bool {
        self.files.len() == present.len() && self.files.iter().all(|(k, seen)| present.get(k) == Some(&seen.digest))
    }

    /// Per-FILE partition of `present` against this cursor (s31 D1,
    /// beside — never replacing — [`Self::source_unchanged`]'s
    /// source-granularity gate): `unchanged` holds every present path
    /// whose digest exactly matches what's indexed here; every OTHER
    /// present path (a content change, or a path never seen before)
    /// lands in `changed_or_new`. A path this cursor has indexed but
    /// that is ABSENT from `present` — a deleted transcript — appears
    /// in NEITHER set: s31 design D1 needs no tombstone work for a
    /// deletion (the records already persisted for it are append-only
    /// history, untouched by this pass); [`Self::retain_present`]
    /// remains the sole place a deletion is acted on, pruning the
    /// cursor's own index so it does not grow unbounded. The caller
    /// (`canon-cli`'s pass layer) parses only `changed_or_new` and
    /// skips `unchanged` entirely — never handing it to parse.
    pub fn diff(&self, present: &BTreeMap<String, String>) -> CursorDiff {
        let mut changed_or_new = BTreeSet::new();
        let mut unchanged = BTreeSet::new();
        for (path, digest) in present {
            if self.files.get(path).is_some_and(|f| &f.digest == digest) {
                unchanged.insert(path.clone());
            } else {
                changed_or_new.insert(path.clone());
            }
        }
        CursorDiff { changed_or_new, unchanged }
    }

    /// Record `path` as ingested at the given stat + digest (call for
    /// every file actually persisted this pass).
    pub fn record(&mut self, path: &Path, mtime_ms: i64, size: u64, digest: String) {
        self.files.insert(Self::key(path), FileSeen { mtime_ms, size, digest });
    }

    /// Drop any indexed path NOT in `present` — a deleted transcript
    /// must not keep the cursor growing forever, and a path later
    /// recreated with different content must not shadow a stale entry
    /// (it would not anyway, since the digest would differ — this is
    /// belt-and-suspenders + unbounded-growth control).
    pub fn retain_present(&mut self, present: &BTreeSet<String>) {
        self.files.retain(|k, _| present.contains(k));
    }

    /// Recompute 3.1's summary fields (`last_seen_at`,
    /// `last_seen_digest`) from the current `files` index — call once
    /// after a batch of [`record`](Self::record) /
    /// [`retain_present`](Self::retain_present) mutations, before
    /// persisting.
    pub fn refresh_summary(&mut self) {
        self.last_seen_at = self.files.values().map(|f| f.mtime_ms).max().and_then(|ms| Utc.timestamp_millis_opt(ms).single());
        // BTreeMap iterates in sorted key order, so this digest is
        // deterministic across runs over an unchanged index.
        let mut hasher = Sha256::new();
        for (path, seen) in &self.files {
            hasher.update(path.as_bytes());
            hasher.update(b"\0");
            hasher.update(seen.digest.as_bytes());
            hasher.update(b"\n");
        }
        let hash = hasher.finalize();
        let mut hex = String::with_capacity(64);
        for byte in hash.iter() {
            hex.push_str(&format!("{byte:02x}"));
        }
        self.last_seen_digest = hex;
    }
}

/// Reads/writes per-source cursors under a local cursor `root`, one
/// `<source_id>.json` per source, each written atomically.
pub struct CursorStore {
    root: PathBuf,
}

impl CursorStore {
    /// A cursor store rooted at `root` (by convention `<repo>/canon/
    /// ingest/cursors/`). The directory is created lazily on the first
    /// [`write`](Self::write).
    pub fn open(root: impl Into<PathBuf>) -> Self {
        Self { root: root.into() }
    }

    /// The local cursor root this store persists under.
    pub fn root(&self) -> &Path {
        &self.root
    }

    fn path_for(&self, source_id: &str) -> PathBuf {
        // `source_id` is a static adapter `client_id` (`omp`/`claude`/
        // `codex`/`hermes`) — never attacker-controlled or path-bearing,
        // so a plain `<source_id>.json` filename is safe.
        self.root.join(format!("{source_id}.json"))
    }

    /// The persisted cursor for `source_id`. `None` means either this
    /// source has never been ingested (first run) OR the cursor file is
    /// missing / unreadable / corrupt — a missing-or-corrupt cursor
    /// DEGRADES to a full rescan (fail-soft), never an error, because
    /// S3 4.2's digest-idempotent write path keeps a full rescan
    /// correct. A lost cursor costs one poll's performance, never
    /// correctness.
    pub fn read(&self, source_id: &str) -> Option<SourceCursor> {
        let bytes = std::fs::read(self.path_for(source_id)).ok()?;
        serde_json::from_slice(&bytes).ok()
    }

    /// Persist `cursor` atomically ([`crate::write_atomic`]: a mid-write
    /// kill never leaves a torn cursor for a concurrent `--watch`
    /// reader; a torn cursor would fail-soft to a full rescan on read
    /// anyway).
    pub fn write(&self, cursor: &SourceCursor) -> Result<(), StoreError> {
        let json = serde_json::to_vec_pretty(cursor)?;
        write_atomic(&self.path_for(&cursor.source_id), &json)?;
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::path::PathBuf;

    fn p(s: &str) -> PathBuf {
        PathBuf::from(s)
    }

    #[test]
    fn digest_is_deterministic_and_changes_with_content() {
        assert_eq!(file_digest(b"hello"), file_digest(b"hello"));
        assert_ne!(file_digest(b"hello"), file_digest(b"hellp"));
        // full 64-hex sha256
        assert_eq!(file_digest(b"hello").len(), 64);
    }

    #[test]
    fn unchanged_digest_is_skipped_new_or_changed_is_not() {
        let mut c = SourceCursor::empty("omp");
        let d = file_digest(b"session-1 bytes");
        c.record(&p("/x/a.jsonl"), 1000, 15, d.clone());

        // same path, same digest -> skip
        assert!(c.is_ingested_unchanged(&p("/x/a.jsonl"), &d));
        // same path, DIFFERENT digest (a same-mtime+size rewrite would
        // land here) -> NOT skipped
        assert!(!c.is_ingested_unchanged(&p("/x/a.jsonl"), &file_digest(b"session-1 REWRITTEN")));
        // unseen path (even one that "looks old") -> NOT skipped
        assert!(!c.is_ingested_unchanged(&p("/x/b.jsonl"), &d));
    }

    #[test]
    fn retain_present_prunes_absent_paths() {
        let mut c = SourceCursor::empty("codex");
        c.record(&p("/x/a.jsonl"), 1, 1, file_digest(b"a"));
        c.record(&p("/x/b.jsonl"), 2, 1, file_digest(b"b"));
        let mut present = BTreeSet::new();
        present.insert("/x/a.jsonl".to_string());
        c.retain_present(&present);
        assert!(c.files.contains_key("/x/a.jsonl"));
        assert!(!c.files.contains_key("/x/b.jsonl"));
    }

    #[test]
    fn source_unchanged_only_when_the_whole_present_set_matches() {
        let mut c = SourceCursor::empty("omp");
        c.record(&p("/x/a.jsonl"), 1, 3, file_digest(b"aaa"));
        c.record(&p("/x/b.jsonl"), 2, 3, file_digest(b"bbb"));

        let mut present: BTreeMap<String, String> = BTreeMap::new();
        present.insert("/x/a.jsonl".to_string(), file_digest(b"aaa"));
        present.insert("/x/b.jsonl".to_string(), file_digest(b"bbb"));
        assert!(c.source_unchanged(&present), "identical present set + digests => skip source");

        // one file's content changed => re-parse the whole source
        present.insert("/x/b.jsonl".to_string(), file_digest(b"bbb-CHANGED"));
        assert!(!c.source_unchanged(&present));

        // a new file appeared => re-parse
        present.insert("/x/b.jsonl".to_string(), file_digest(b"bbb"));
        present.insert("/x/c.jsonl".to_string(), file_digest(b"ccc"));
        assert!(!c.source_unchanged(&present));

        // a file was deleted => re-parse (set sizes differ)
        present.remove("/x/c.jsonl");
        present.remove("/x/b.jsonl");
        assert!(!c.source_unchanged(&present));
    }

    #[test]
    fn summary_fields_derive_from_the_index() {
        let mut c = SourceCursor::empty("omp");
        c.record(&p("/x/a.jsonl"), 5000, 3, file_digest(b"aaa"));
        c.record(&p("/x/b.jsonl"), 9000, 3, file_digest(b"bbb"));
        c.refresh_summary();
        assert_eq!(c.last_seen_at, Utc.timestamp_millis_opt(9000).single());
        assert!(!c.last_seen_digest.is_empty());
        // deterministic across repeated recompute over the same index
        let d1 = c.last_seen_digest.clone();
        c.refresh_summary();
        assert_eq!(d1, c.last_seen_digest);
    }

    #[test]
    fn store_round_trips_and_fail_softs_on_corrupt() {
        let dir = tempfile::tempdir().unwrap();
        let store = CursorStore::open(dir.path().join("cursors"));
        assert!(store.read("omp").is_none(), "never-written source reads as None (first run)");

        let mut c = SourceCursor::empty("omp");
        c.record(&p("/x/a.jsonl"), 1000, 5, file_digest(b"hello"));
        c.refresh_summary();
        store.write(&c).unwrap();

        let back = store.read("omp").expect("round-trips");
        assert_eq!(back, c);

        // a corrupt cursor file fail-softs to None (=> full rescan),
        // never an error/panic
        std::fs::write(store.root().join("omp.json"), b"{ not json").unwrap();
        assert!(store.read("omp").is_none());
    }

    #[test]
    fn diff_partitions_growing_new_deleted_and_unchanged_files() {
        let mut c = SourceCursor::empty("omp");
        c.record(&p("/x/a.jsonl"), 1, 3, file_digest(b"aaa"));
        c.record(&p("/x/b.jsonl"), 2, 3, file_digest(b"bbb"));
        c.record(&p("/x/gone.jsonl"), 3, 3, file_digest(b"ggg"));

        let mut present: BTreeMap<String, String> = BTreeMap::new();
        // `a.jsonl` grew: same path, new digest.
        present.insert("/x/a.jsonl".to_string(), file_digest(b"aaa-GROWN"));
        // `b.jsonl` is byte-identical to what's indexed.
        present.insert("/x/b.jsonl".to_string(), file_digest(b"bbb"));
        // `c.jsonl` is a brand-new path, never indexed before.
        present.insert("/x/c.jsonl".to_string(), file_digest(b"ccc"));
        // `gone.jsonl` is indexed but absent from `present` (deleted).

        let diff = c.diff(&present);
        assert_eq!(
            diff.changed_or_new,
            BTreeSet::from(["/x/a.jsonl".to_string(), "/x/c.jsonl".to_string()]),
            "a grown file and a brand-new file both need (re)parsing"
        );
        assert_eq!(diff.unchanged, BTreeSet::from(["/x/b.jsonl".to_string()]), "a byte-identical file is safe to skip");
        // A deleted file (indexed, but absent from `present`) appears
        // in NEITHER set — s31 D1 needs no tombstone work for it.
        assert!(!diff.changed_or_new.contains("/x/gone.jsonl"));
        assert!(!diff.unchanged.contains("/x/gone.jsonl"));
    }

    #[test]
    fn diff_of_a_fully_unchanged_present_set_is_all_unchanged() {
        let mut c = SourceCursor::empty("omp");
        c.record(&p("/x/a.jsonl"), 1, 3, file_digest(b"aaa"));
        c.record(&p("/x/b.jsonl"), 2, 3, file_digest(b"bbb"));

        let mut present: BTreeMap<String, String> = BTreeMap::new();
        present.insert("/x/a.jsonl".to_string(), file_digest(b"aaa"));
        present.insert("/x/b.jsonl".to_string(), file_digest(b"bbb"));

        let diff = c.diff(&present);
        assert!(diff.changed_or_new.is_empty());
        assert_eq!(diff.unchanged.len(), 2);
    }

    #[test]
    fn diff_of_an_empty_cursor_reports_every_present_file_as_new() {
        let c = SourceCursor::empty("omp");
        let mut present: BTreeMap<String, String> = BTreeMap::new();
        present.insert("/x/a.jsonl".to_string(), file_digest(b"aaa"));

        let diff = c.diff(&present);
        assert_eq!(diff.changed_or_new, BTreeSet::from(["/x/a.jsonl".to_string()]));
        assert!(diff.unchanged.is_empty());
    }
}
