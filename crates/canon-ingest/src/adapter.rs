//! The `SessionAdapter` trait + `UnifiedRow` normalization target (S3
//! Wave 1, frozen for Wave 2's claude/codex/hermes adapters).
//!
//! `UnifiedRow` mirrors the donor's per-message unified row:
//! one row per billable model call, carrying client/model/provider/
//! session identity, optional-by-format workspace context, a 5-bucket
//! token breakdown, a cost + provenance tag, and reconciliation
//! bookkeeping (`dedup_key`, `is_turn_start`) Wave 2's Claude Code
//! (streaming-duplicate merge) and Codex (cumulative-delta + fork
//! detection) adapters need — see `openspec/changes/s3-session-ingest/
//! design.md` decision D6.

use std::path::{Path, PathBuf};

use serde::{Deserialize, Serialize};

/// A per-message token count, split by billing bucket — ported 1:1
/// from the donor's `TokenBreakdown`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenBreakdown {
    pub input: i64,
    pub output: i64,
    pub cache_read: i64,
    pub cache_write: i64,
    pub reasoning: i64,
}

impl TokenBreakdown {
    /// Saturating sum across all five buckets so a corrupt source
    /// can't overflow-wrap the total — ported from the donor
    /// session-parser project.
    pub fn total(&self) -> i64 {
        self.input.saturating_add(self.output).saturating_add(self.cache_read).saturating_add(self.cache_write).saturating_add(self.reasoning)
    }
}

/// Which provenance a `UnifiedRow`'s `cost` field carries — ported 1:1
/// from the donor's `CostSource`.
/// Gates whether a later, cross-cutting canon pricing pass (out of S3
/// scope) may overwrite `cost`: a parser that already knows the
/// provider-billed dollar figure marks `ProviderReported`; one that
/// only extracted token counts leaves the default `Unknown`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum CostSource {
    #[default]
    Unknown,
    ProviderReported,
    Estimated,
}

/// The shared normalization target every `SessionAdapter::parse` call
/// emits — one row per billable model call. Mirrors the donor's
/// per-message unified row;
/// deliberately generic (`client`/`model_id`/`provider_id`/
/// `session_id` are plain strings, not enums) so a Wave 2 adapter never
/// requires a schema migration to this type.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct UnifiedRow {
    /// The adapter's own `client_id()` (e.g. `"omp"`), never a
    /// display name.
    pub client: String,
    pub model_id: String,
    pub provider_id: String,
    /// The adapter-derived join key — validated into
    /// `canon_model::ids::SessionId` by `crate::normalize`, never
    /// trusted as pre-validated here. Derivation is PER-ADAPTER (see
    /// each adapter module's doc comment): omp/pi reads the in-file
    /// `session` header's `id` field, never the filename.
    pub session_id: String,
    /// `None` when the source format carries no project/cwd context.
    pub workspace_key: Option<String>,
    pub workspace_label: Option<String>,
    /// Unix milliseconds — the source's own event timestamp, or (when
    /// absent) the transcript file's mtime.
    pub timestamp_ms: i64,
    pub tokens: TokenBreakdown,
    pub cost: f64,
    pub cost_source: CostSource,
    pub duration_ms: Option<i64>,
    /// A per-adapter dedup identity for source-level reconciliation
    /// (design D6) — `None` when the adapter's format has no
    /// duplicate-write problem to guard against (e.g. omp/pi, which
    /// writes each assistant turn exactly once).
    pub dedup_key: Option<String>,
    /// True when this row is the first assistant response after a
    /// user turn. `false` for adapters whose source format doesn't
    /// carry turn-boundary information (omp/pi's `pi.rs` donor never
    /// sets this either — ported behavior, not an omission).
    pub is_turn_start: bool,
}

/// One USER-role message extracted verbatim from a transcript (s31
/// design D4 — user-directive capture). Adapters emit one
/// `DirectiveRow` per user-role message they encounter, NEVER for
/// system/tool/assistant content — see each adapter's own parse
/// function for the exact per-format role/type gate (e.g. omp/pi's
/// `message.role == "user"`, Claude Code's `entry_type == "user"`,
/// Codex's `event_msg`/`user_message` payload). `text` is stored
/// verbatim — command/paste blobs ARE the directive, no truncation
/// this wave (design D4) — flattened from a structured content-block
/// array when the source format uses one (concatenating every `text`
/// block, skipping every non-text block) or used as-is when the
/// source's own content is already a plain string.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct DirectiveRow {
    /// The adapter's own `client_id()` — same rule as
    /// [`UnifiedRow::client`].
    pub client: String,
    /// The adapter-derived join key — same derivation rule as
    /// [`UnifiedRow::session_id`] (see each adapter's module doc).
    pub session_id: String,
    /// Unix milliseconds — the source's own event timestamp, or (when
    /// absent) the transcript file's mtime, same fallback
    /// [`UnifiedRow::timestamp_ms`] uses.
    pub timestamp_ms: i64,
    pub text: String,
    /// `None` when the source format carries no project/cwd context —
    /// same rule as [`UnifiedRow::workspace_key`].
    pub workspace_key: Option<String>,
    pub workspace_label: Option<String>,
}

/// The result of one [`SessionAdapter::parse`] call: the rows it
/// successfully extracted from the file, plus a count of
/// lines/records/whole-file failures encountered along the way —
/// design §7's "malformed evidence is no evidence" made VISIBLE
/// (`skipped`) rather than silently discarded. `skipped` counts
/// genuinely unparseable content this adapter could not extract a row
/// from at all (a corrupt JSON line, an unrecognized/malformed file
/// header, an unopenable or query-failing database) — never a
/// well-formed record this adapter simply has no billable use for
/// (e.g. a `user`-role message with no token usage, a `tool_use`
/// event): those are ordinary filtering, not evidence of corruption,
/// and are NOT counted here.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ParseOutcome {
    pub rows: Vec<UnifiedRow>,
    pub skipped: usize,
    /// User-directive rows this parse extracted (s31 design D4,
    /// additive — `#[serde(default)]` so a pre-s31-shaped
    /// `ParseOutcome` still deserializes). Empty for every call site
    /// that never constructs one, e.g. every early-return malformed-
    /// file path via [`ParseOutcome::new`].
    #[serde(default)]
    pub directives: Vec<DirectiveRow>,
}

impl ParseOutcome {
    pub fn new(rows: Vec<UnifiedRow>, skipped: usize) -> Self {
        Self { rows, skipped, directives: Vec::new() }
    }

    /// Full constructor for an adapter that also extracted directive
    /// rows in the same pass.
    pub fn with_directives(rows: Vec<UnifiedRow>, skipped: usize, directives: Vec<DirectiveRow>) -> Self {
        Self { rows, skipped, directives }
    }
}

/// One session-source adapter (S3 design D1's "trait + static table",
/// frozen for Wave 2). `client_id()` names the adapter
/// (`"claude-code"` | `"codex"` | `"omp"` | `"hermes"`); `scan_roots`
/// resolves the on-disk root(s) to walk (a `Vec` because some clients
/// union more than one root — e.g. Codex's live + archived session
/// directories, design D5); `parse` converts one already-discovered
/// file into a [`ParseOutcome`], skipping unparseable content as a
/// violation rather than panicking (design §7) — and COUNTING it,
/// rather than dropping it silently (Wave 2 amendment: the frozen
/// Wave-1 `Vec<UnifiedRow>` return type undercounted malformed
/// evidence by never surfacing it).
pub trait SessionAdapter: Send + Sync {
    /// The adapter's stable identity — also `UnifiedRow.client`'s
    /// value for every row this adapter emits.
    fn client_id(&self) -> &'static str;

    /// Resolve this adapter's scan root(s) under `home`.
    /// `use_env_roots` gates whether adapter-specific environment
    /// overrides are consulted (Wave 1's omp adapter honors
    /// `CANON_INGEST_OMP_SESSIONS_DIR`) — `false` pins resolution to
    /// pure `home`-relative paths so two ingest runs over the same
    /// fixture home produce byte-identical scan roots regardless of
    /// the ambient shell environment (S3 acceptance: "identical
    /// normalized output across two runs").
    fn scan_roots(&self, home: &Path, use_env_roots: bool) -> Vec<PathBuf>;

    /// Parse one already-discovered file into a [`ParseOutcome`]. A
    /// file this adapter's format doesn't recognize (e.g. an
    /// unrecognized header) returns empty `rows` plus a `skipped`
    /// count, never an error — malformed content is a violation to
    /// skip AND count, not a crash (design §7, "malformed evidence is
    /// no evidence").
    fn parse(&self, path: &Path) -> ParseOutcome;
}
