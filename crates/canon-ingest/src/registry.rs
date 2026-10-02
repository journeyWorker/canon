//! The static adapter registry — INSPIRED by the donor's `clients.rs`
//! `define_clients!` static-array shape as
//! a design pattern (a static, declaration-ordered table; no dynamic
//! plugin loading), reimplemented as plain data over the frozen
//! `SessionAdapter` trait rather than macro-expanded — S3 design D1:
//! "`canon-ingest::registry()` returns the static `[omp, claude,
//! codex]` array in adapter-declaration order … S10's plugin.yaml-
//! driven extension mechanism is a later, separate spec." D1's own
//! text (`design.md`) still names its original three-adapter Wave 1
//! snapshot; the registry BELOW is the current source of truth and
//! ships four adapters — `omp`, `hermes`, `claude`, `codex`, in that
//! declaration order (ReviewS3Full finding 6: this doc comment and
//! `specs/session-adapter-registry/spec.md` are kept aligned with the
//! actual shipped set, even where `design.md`'s own D1 prose has not
//! been updated).
//!
//! Wave 1 shipped only the `omp` entry; Wave 2 appended `claude`,
//! `codex`, and `hermes` here without touching [`SessionAdapter`] or
//! [`UnifiedRow`] (the frozen contract `crate::adapter` documents).

use std::path::{Path, PathBuf};

use crate::adapter::{SessionAdapter, UnifiedRow};
use crate::adapters::omp::OmpAdapter;
use crate::adapters::hermes::HermesAdapter;
use crate::adapters::claude::ClaudeCodeAdapter;
use crate::adapters::codex::CodexAdapter;
use crate::scanner;

/// One registered adapter: its [`SessionAdapter`] handle plus the file
/// glob `scanner::scan_dir`/`scanner::scan_roots` matches scan-root
/// contents against. The glob lives here, NOT on [`SessionAdapter`]
/// itself (design Contract froze the trait at exactly three methods;
/// s40 (`plan-vs-actual-diff`) amended it to four by adding
/// `parse_version`, which describes the adapter's PARSE OUTPUT and so
/// genuinely belongs on the adapter — a scan glob still does not)
/// — `SessionAdapter::parse` already self-rejects a
/// non-matching file by returning an empty `Vec` (e.g. omp/pi's header
/// probe), so the glob here is a scan-efficiency filter, not a
/// correctness dependency.
pub struct AdapterEntry {
    pub adapter: &'static dyn SessionAdapter,
    /// A file name suffix glob, e.g. `".jsonl"` — matched via
    /// `str::ends_with`, the same minimal-vocabulary approach the S3
    /// scanner primitive's doc comment calls out (a real glob crate
    /// would be overkill for a single-suffix match; add one if a
    /// future adapter needs a richer pattern).
    pub file_suffix: &'static str,
}

impl AdapterEntry {
    pub fn client_id(&self) -> &'static str {
        self.adapter.client_id()
    }

    /// The `scanner::scan_dir`/`scan_roots` predicate for this entry's
    /// glob.
    pub fn file_matches(&self, path: &Path) -> bool {
        path.file_name().and_then(|n| n.to_str()).is_some_and(|name| name.ends_with(self.file_suffix))
    }
}

/// The static registry, in declaration order (S3 design D1: "the
/// registry enumerates configured sources" — deterministic order,
/// never `HashMap`-iteration-order dependent). Wave 1 ships `omp`
/// only; Wave 2 (claude/codex/hermes) appends entries here.
pub fn registry() -> &'static [AdapterEntry] {
    static OMP: OmpAdapter = OmpAdapter;
    static HERMES: HermesAdapter = HermesAdapter;
    static CLAUDE: ClaudeCodeAdapter = ClaudeCodeAdapter;
    static CODEX: CodexAdapter = CodexAdapter;
    static REGISTRY: &[AdapterEntry] = &[
        AdapterEntry { adapter: &OMP, file_suffix: ".jsonl" },
        // SQLite adapter: `scan_roots()` already returns resolved
        // `state.db` file paths directly (see `adapters::hermes`
        // module doc) rather than a directory to filter many files
        // inside, so this suffix is a documentation-only no-op —
        // every root this adapter emits already ends with it.
        AdapterEntry { adapter: &HERMES, file_suffix: "state.db" },
        AdapterEntry { adapter: &CLAUDE, file_suffix: ".jsonl" },
        AdapterEntry { adapter: &CODEX, file_suffix: ".jsonl" },
    ];
    REGISTRY
}

/// Look up one registered adapter by `client_id()` — `canon ingest
/// sessions --adapter <id>`-style selection (unused by Wave 1's
/// unconditional-scan-everything CLI path today, but the natural seam
/// for it).
pub fn find(client_id: &str) -> Option<&'static AdapterEntry> {
    registry().iter().find(|entry| entry.client_id() == client_id)
}

/// One adapter's full scan+parse pass: resolve its roots under `home`,
/// walk them for matching files, and parse every match into
/// `UnifiedRow`s. Files are walked in deterministic sorted order
/// (`scanner::scan_roots`); rows preserve that file order and each
/// file's own emission order, so re-running over an unchanged fixture
/// tree yields byte-identical row order (S3 acceptance: "identical
/// normalized output across two runs").
pub struct AdapterScanResult {
    pub client_id: &'static str,
    pub roots: Vec<PathBuf>,
    pub files_scanned: Vec<PathBuf>,
    pub rows: Vec<UnifiedRow>,
    /// Sum of every scanned file's `ParseOutcome::skipped` (Wave-2
    /// amendment) — malformed/unparseable content this adapter hit
    /// but could not extract a row from, surfaced up through
    /// `canon-cli`'s ingest summary rather than silently discarded.
    pub skipped: usize,
}

/// Enumerate one adapter's present matching files: resolve its roots
/// under `home`, then walk them for files this entry's glob matches, in
/// deterministic (byte-lexical) sorted order (`scanner::scan_roots`).
/// Returns `(roots, files)` WITHOUT parsing anything — the seam a
/// watermark-gated caller (`canon-cli`'s `canon ingest sessions`, S3
/// §3) inserts a per-file digest gate into, before deciding which of
/// `files` to hand to [`parse_files`].
pub fn enumerate(entry: &AdapterEntry, home: &Path, use_env_roots: bool) -> (Vec<PathBuf>, Vec<PathBuf>) {
    let roots = entry.adapter.scan_roots(home, use_env_roots);
    let files = scanner::scan_roots(&roots, |p| entry.file_matches(p));
    (roots, files)
}

/// Parse the given `files` (a subset — or all — of [`enumerate`]'s
/// output) into `UnifiedRow`s, returning `(rows, skipped)` where
/// `skipped` sums every file's `ParseOutcome::skipped` (corrupt lines /
/// malformed content the adapter could not extract a row from). `files`
/// order is preserved, and each file's own emission order within it, so
/// the row order is deterministic for a fixed `files` slice.
pub fn parse_files(entry: &AdapterEntry, files: &[PathBuf]) -> (Vec<UnifiedRow>, usize) {
    let mut rows = Vec::new();
    let mut skipped = 0usize;
    for path in files {
        let outcome = entry.adapter.parse(path);
        rows.extend(outcome.rows);
        skipped += outcome.skipped;
    }
    (rows, skipped)
}

/// One adapter's full, UNGATED scan+parse pass — [`enumerate`] then
/// [`parse_files`] over every present file. The convenience entry point
/// for callers that want a full rescan (the pre-S3-§3 behaviour, and
/// the non-`--watch` one-shot path); the watermark-gated path composes
/// `enumerate` + a digest gate + `parse_files` itself.
pub fn scan_and_parse(entry: &AdapterEntry, home: &Path, use_env_roots: bool) -> AdapterScanResult {
    let (roots, files_scanned) = enumerate(entry, home, use_env_roots);
    let (rows, skipped) = parse_files(entry, &files_scanned);
    AdapterScanResult { client_id: entry.client_id(), roots, files_scanned, rows, skipped }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn registry_contains_omp() {
        // Wave 1 shipped `omp` alone; Wave 2 (claude/codex/hermes)
        // appends entries here without ever removing or reordering
        // it before `omp` — an exact `len() == 1` assertion would
        // break the moment any Wave 2 adapter registers, so this
        // checks presence + first-declared position, not count.
        let entries = registry();
        assert!(!entries.is_empty());
        assert_eq!(entries[0].client_id(), "omp");
        assert!(entries.iter().any(|e| e.client_id() == "omp"));
    }

    #[test]
    fn find_looks_up_by_client_id() {
        assert!(find("omp").is_some());
        assert!(find("does-not-exist").is_none());
    }

    #[test]
    fn file_matches_is_suffix_based() {
        let entry = &registry()[0];
        assert!(entry.file_matches(Path::new("/home/x/.omp/agent/sessions/abc/one.jsonl")));
        assert!(!entry.file_matches(Path::new("/home/x/.omp/agent/sessions/abc/notes.txt")));
    }

    /// Each shipped adapter's parse generation is pinned to the value
    /// its own rustdoc argues for, as a DECLARED table rather than a
    /// blanket "all at 1" — the blanket form is what let s37
    /// (`execution-graph-topology`) change the Claude adapter's output
    /// while its assertion still passed.
    ///
    /// `claude-code` is at `3`: nested Claude cache-creation accounting
    /// changes normalized output for identical transcript bytes. The other
    /// adapters remain at `1`; their new bucket is always zero.
    #[test]
    fn every_shipped_adapters_parse_generation_matches_its_declared_value() {
        let declared: &[(&str, u32)] = &[("omp", 1), ("hermes", 1), ("claude-code", 3), ("codex", 1)];

        for entry in registry() {
            let expected = declared.iter().find(|(id, _)| *id == entry.client_id()).map(|(_, version)| *version);
            assert_eq!(
                Some(entry.adapter.parse_version()),
                expected,
                "adapter `{}` reports parse generation {} — declare it here WITH the reason its output changed (or did not)",
                entry.client_id(),
                entry.adapter.parse_version()
            );
        }

        for (client_id, _) in declared {
            assert!(find(client_id).is_some(), "declared adapter `{client_id}` is no longer registered — drop it from the table or restore it");
        }
    }

    /// The registry-wide enforcement point for
    /// [`crate::adapter::session_adapter_id_violation`]'s grammar, which
    /// is a precondition of the session cursor's identity rather than
    /// cosmetics: `canon-cli`'s `ingest::session_source_cursor_id`
    /// renders a `client_id` straight into a `<id>.json` filename, and
    /// its `(client_id, version) -> id` mapping is one-to-one ONLY over
    /// ids that do not themselves end `-v<digits>`. Because this table
    /// is a closed static set linked into the binary, this assertion —
    /// not a runtime branch — is what makes the property hold of every
    /// adapter that will ever ship.
    #[test]
    fn every_registered_adapter_id_satisfies_the_cursor_filename_grammar() {
        for entry in registry() {
            let client_id = entry.client_id();
            assert_eq!(
                crate::adapter::session_adapter_id_violation(client_id),
                None,
                "adapter id `{client_id}` breaks the cursor-filename grammar: it {}",
                crate::adapter::session_adapter_id_violation(client_id).map(|violation| violation.to_string()).unwrap_or_default()
            );
        }
    }

    /// The grammar's rules, each witnessed by the id it exists to
    /// reject. `omp-v2` is the load-bearing one: it is a perfectly
    /// plausible adapter name that would render the SAME cursor id as
    /// `omp` at parse version 2, so the two adapters would share one
    /// watermark and each report the other's transcripts `unchanged`.
    #[test]
    fn the_adapter_id_grammar_rejects_every_unsafe_or_ambiguous_shape() {
        use crate::adapter::{SessionAdapterIdViolation as Violation, session_adapter_id_violation as violation};

        assert_eq!(violation("claude-code"), None);
        assert_eq!(violation("omp2"), None);
        assert_eq!(violation("omp-v2x"), None, "`-v` followed by a non-digit tail is not the reserved suffix");
        assert_eq!(violation("omp-v"), None, "a bare `-v` with no digits cannot be a rendered version suffix");

        assert_eq!(violation(""), Some(Violation::Empty));
        assert_eq!(violation("omp-v2"), Some(Violation::ReservedVersionSuffix));
        assert_eq!(violation("omp-v1-v10"), Some(Violation::ReservedVersionSuffix), "the suffix is matched at the END, not at the first `-v`");
        assert_eq!(violation("plan-openspec"), Some(Violation::ReservedPlanNamespace));
        assert_eq!(violation("-omp"), Some(Violation::OuterHyphen));
        assert_eq!(violation("omp-"), Some(Violation::OuterHyphen));
        assert_eq!(violation("OMP"), Some(Violation::IllegalByte { byte: b'O' }), "uppercase is rejected, not folded: on a case-insensitive volume `OMP` and `omp` are ONE cursor file");
        assert_eq!(violation(".."), Some(Violation::IllegalByte { byte: b'.' }));
        assert_eq!(violation("a/b"), Some(Violation::IllegalByte { byte: b'/' }));
        assert_eq!(violation("a b"), Some(Violation::IllegalByte { byte: b' ' }));
    }
}
