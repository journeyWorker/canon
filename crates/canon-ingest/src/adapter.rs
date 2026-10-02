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

/// A per-message token count, split by billing bucket.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct TokenBreakdown {
    pub input: i64,
    pub output: i64,
    pub cache_read: i64,
    pub cache_write: i64,
    #[serde(default, skip_serializing_if = "is_zero")]
    pub cache_write_1h: i64,
    pub reasoning: i64,
}

fn is_zero(value: &i64) -> bool {
    *value == 0
}

impl TokenBreakdown {
    /// Saturating sum across the billing buckets. `cache_write_1h` is a
    /// subset of `cache_write`, not an additional bucket.
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
    /// (design D6). Rows sharing a key for the same client are collapsed
    /// before session grouping; `None` means the source could not provide
    /// a conservative stable identity.
    pub dedup_key: Option<String>,
    /// True when this row is the first assistant response after a
    /// user turn. `false` for adapters whose source format doesn't
    /// carry turn-boundary information (omp/pi's `pi.rs` donor never
    /// sets this either — ported behavior, not an omission).
    pub is_turn_start: bool,
    /// This row's own agent identity WITHIN its `session_id` — a
    /// stable per-agent handle when the source format distinguishes a
    /// dispatched subagent from the main agent (Claude Code's
    /// `isSidechain` transcripts: the subagent's own transcript-file
    /// stem), paired with [`Self::parent_agent_id`] naming whoever
    /// dispatched it. `crate::normalize` turns each distinct
    /// `agent_id` in a session into its own child
    /// `canon_model::records::Run` under the session's root run,
    /// reconstructing the dispatch tree an earlier ingest collapsed.
    ///
    /// BOTH stay `None` for a plain single-agent session — the
    /// overwhelmingly common case, which must normalize to exactly the
    /// single root run it did before these fields existed. An adapter
    /// whose format carries no agent-delegation edge leaves them
    /// `None` rather than inventing one from an intra-session
    /// message-threading pointer (see `adapters::omp`'s
    /// `PiSessionEntry` doc for that exact trap).
    /// `#[serde(default, skip_serializing_if = "Option::is_none")]`,
    /// NOT the bare `Option<String>` the surrounding
    /// `workspace_key`/`dedup_key` fields use: those were in this
    /// struct's shape from the start, so their `null` is part of the
    /// established wire form, whereas these two are ADDITIVE — skipping
    /// them when unset is what keeps an already-serialized row's bytes
    /// (and therefore `crate::normalize::content_digest` over anything
    /// derived from it) identical to its pre-s37 form. The same
    /// discipline `canon_model::records::Run::parent_run_id` states at
    /// length.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    /// The agent that DISPATCHED this row's agent — see
    /// [`Self::agent_id`]. Names either another row's `agent_id`
    /// (nested subagent) or the session itself (a subagent dispatched
    /// by the main agent), which `crate::normalize` resolves to the
    /// session's root run.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_agent_id: Option<String>,
}

/// One USER-role message extracted from a transcript (s31
/// design D4 — user-directive parsing). Adapters emit one
/// `DirectiveRow` per user-role message they encounter, NEVER for
/// system/tool/assistant content — see each adapter's own parse
/// function for the exact per-format role/type gate (e.g. omp/pi's
/// `message.role == "user"`, Claude Code's `entry_type == "user"`,
/// Codex's `event_msg`/`user_message` payload). `text` is retained
/// verbatim only in this in-memory parser row; the shared
/// post-adapter privacy boundary decides whether a bounded prefix
/// reaches normalized records. It is flattened from a structured
/// content-block array when the source format uses one (concatenating
/// every `text` block, skipping every non-text block) or used as-is
/// when the source's own content is already a plain string.
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
    /// This directive's own agent identity within its `session_id` —
    /// same rule and same `None`-for-single-agent default as
    /// [`UnifiedRow::agent_id`]. A subagent's own prompt is a
    /// directive, and it belongs to the SUBAGENT's run, not the
    /// dispatcher's.
    /// Same additive `skip_serializing_if` discipline
    /// [`UnifiedRow::agent_id`] documents.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub agent_id: Option<String>,
    /// The agent that dispatched this directive's agent — same rule as
    /// [`UnifiedRow::parent_agent_id`].
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub parent_agent_id: Option<String>,
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

/// The prefix `canon-cli`'s `plans::plan_source_cursor_id` claims for
/// every PLAN-import cursor (`plan-<dialect>-v<n>-<digest12>`). Session
/// and plan cursors share one `.canon/ingest/cursors/` directory, so a
/// session adapter whose `client_id` began with this would render a
/// filename inside the plan family's namespace — hence the grammar
/// below reserves it (s40 (`plan-vs-actual-diff`) review follow-up).
pub const PLAN_CURSOR_ID_PREFIX: &str = "plan-";

/// The single way a [`SessionAdapter::client_id`] can violate the
/// adapter-id grammar — a named cause rather than a bare `bool`, so the
/// registry-wide assertion that enforces it can say WHICH rule an id
/// broke (see [`session_adapter_id_violation`]).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SessionAdapterIdViolation {
    /// An empty id renders the cursor filename `.json` — a dotfile with
    /// no stem, shared by every empty-id adapter.
    Empty,
    /// Anything outside `[a-z0-9-]`. This is what rules out a path
    /// separator, `.` (hence `..`), whitespace, NUL, and — because
    /// uppercase is excluded rather than folded — any case-only alias
    /// that two adapters could otherwise resolve to one file on a
    /// case-insensitive filesystem.
    IllegalByte { byte: u8 },
    /// A leading or trailing `-`: not unsafe by itself, but it makes an
    /// id that reads as an option flag and one whose rendered cursor
    /// filename differs from its id only in a character no reader sees.
    OuterHyphen,
    /// The id itself ends `-v<digits>` — the exact suffix
    /// `canon-cli`'s `ingest::session_source_cursor_id` appends for
    /// version ≠ 1, so such an id would collide with ANOTHER adapter's
    /// bumped cursor (`omp-v2` at version 1 versus `omp` at version 2).
    ReservedVersionSuffix,
    /// The id starts with [`PLAN_CURSOR_ID_PREFIX`].
    ReservedPlanNamespace,
}

impl std::fmt::Display for SessionAdapterIdViolation {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        match self {
            Self::Empty => f.write_str("must not be empty"),
            Self::IllegalByte { byte } => write!(f, "contains byte {byte:#04x}, outside the lowercase `[a-z0-9-]` grammar"),
            Self::OuterHyphen => f.write_str("must not start or end with `-`"),
            Self::ReservedVersionSuffix => f.write_str("must not end with the reserved `-v<digits>` parse-version suffix"),
            Self::ReservedPlanNamespace => write!(f, "must not start with the reserved `{PLAN_CURSOR_ID_PREFIX}` plan-cursor prefix"),
        }
    }
}

/// The adapter-id grammar, as the violation `client_id` commits or
/// `None` — a **declared precondition of the cursor identity**, not
/// cosmetics. `canon-cli`'s `ingest::session_source_cursor_id` renders
/// `client_id` at version 1 and `<client_id>-v<version>` from 2 on, and
/// `canon_store::cursor::CursorStore` then joins that straight onto its
/// root as `<id>.json`. Two properties therefore have to hold of the
/// id, and both are properties of its SHAPE:
///
/// 1. **Bare-filename safety.** `[a-z0-9-]` with no outer `-` admits no
///    separator, no `.`/`..`, no whitespace, no NUL, and no uppercase —
///    so the rendered id is a single path component on any filesystem
///    canon targets, and no two ids can case-fold together (which on
///    macOS's default case-insensitive volume would be one file, i.e.
///    one shared watermark).
/// 2. **Injectivity of `(client_id, version) -> id`.** Forbidding an id
///    that ends `-v<digits>` is exactly what makes the rendering
///    one-to-one. Proof: suppose `render(x, m) == render(y, n)`. If
///    `m == n == 1` then `x == y`. If both differ from 1 then
///    `x ++ "-v" ++ m == y ++ "-v" ++ n`; were `|x| < |y|`, the shorter
///    string's `"-v"` marker would have to reappear at some index ≥ 1
///    of `"-v" ++ digits(m)`, whose only `-` sits at index 0 — so
///    `|x| == |y|`, hence `x == y` and `m == n`. The mixed case needs
///    `x == y ++ "-v" ++ digits(n)`, i.e. `x` ends `-v<digits>`, which
///    this grammar rejects. Drop the rule and two adapters silently
///    share one cursor, each skipping the other's transcripts as
///    `unchanged`.
///
/// Enforcement is a registry-wide assertion
/// (`crate::registry`'s tests) rather than a runtime branch, and
/// deliberately so: [`crate::registry::registry`] is a closed static
/// table linked into the binary, so an id that violates this grammar is
/// a compile-time fact about the source tree, not a condition a run can
/// encounter. A runtime check would be unreachable code claiming to
/// guard something a test already decided.
pub fn session_adapter_id_violation(client_id: &str) -> Option<SessionAdapterIdViolation> {
    if client_id.is_empty() {
        return Some(SessionAdapterIdViolation::Empty);
    }
    if let Some(byte) = client_id.bytes().find(|byte| !matches!(*byte, b'a'..=b'z' | b'0'..=b'9' | b'-')) {
        return Some(SessionAdapterIdViolation::IllegalByte { byte });
    }
    if client_id.starts_with('-') || client_id.ends_with('-') {
        return Some(SessionAdapterIdViolation::OuterHyphen);
    }
    if client_id.starts_with(PLAN_CURSOR_ID_PREFIX) {
        return Some(SessionAdapterIdViolation::ReservedPlanNamespace);
    }
    if ends_with_version_suffix(client_id) {
        return Some(SessionAdapterIdViolation::ReservedVersionSuffix);
    }
    None
}

/// `true` iff `id` ends with `-v` followed by at least one digit and
/// nothing else — the rendering `session_source_cursor_id` produces for
/// a version ≠ 1, matched with `rsplit_once` so `a-v1-v2` (which DOES
/// end that way) is caught too.
fn ends_with_version_suffix(id: &str) -> bool {
    match id.rsplit_once("-v") {
        Some((_, digits)) => !digits.is_empty() && digits.bytes().all(|byte| byte.is_ascii_digit()),
        None => false,
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
/// evidence by never surfacing it); `parse_version()` declares the
/// generation of that parse output (s40 amendment — see its own doc).
pub trait SessionAdapter: Send + Sync {
    /// The adapter's stable identity — also `UnifiedRow.client`'s
    /// value for every row this adapter emits, and the stem of the
    /// session-ingest cursor's filename. It MUST satisfy
    /// [`session_adapter_id_violation`]'s grammar (lowercase
    /// `[a-z0-9-]`, no outer `-`, and neither the reserved `-v<digits>`
    /// suffix nor the `plan-` prefix); that is what keeps the rendered
    /// cursor id a safe bare filename AND keeps `(client_id, version)`
    /// one-to-one, so no two adapters can ever share one watermark.
    fn client_id(&self) -> &'static str;

    /// This adapter's PARSE-OUTPUT generation (s40
    /// (`plan-vs-actual-diff`), task 4.1) — bumped whenever this
    /// adapter's parse output changes for IDENTICAL input bytes: a
    /// newly-extracted field, a corrected mapping, a changed
    /// role/usage gate, a different `dedup_key`. NOT a version of the
    /// foreign transcript format itself, and NOT bumped for a refactor
    /// that leaves every emitted [`UnifiedRow`]/[`DirectiveRow`]
    /// byte-identical.
    ///
    /// It exists because of what the session-ingest cursor's gate
    /// actually compares: per-file CONTENT digests, under a cursor
    /// identity that named this adapter's `client_id` and nothing
    /// else. Transcript bytes are byte-stable by design — an
    /// already-ingested session re-ingests idempotently — so a change
    /// to THIS code was invisible to that gate: every unchanged
    /// transcript was still reported `skipped unchanged (watermark)`
    /// and the corpus went silently stale under the new
    /// normalization. `canon-cli`'s `ingest::session_source_cursor_id`
    /// folds this value into the cursor IDENTITY (never into a
    /// per-file digest), so a bump lands on a DIFFERENT cursor id,
    /// finds no cursor there at all, and re-reads the whole source
    /// exactly as if every transcript had been edited — with no
    /// `--full` and no cursor deletion, the stale cursor simply
    /// orphaned rather than mutated. This is the session-side half of
    /// the fix s38 (`evidence-bearing-memory`) shipped for
    /// [`crate::PlanAdapter::parse_version`] and explicitly left open
    /// as its own task 5.3.
    ///
    /// REQUIRED, never defaulted, for s38's stated reason: a new
    /// adapter must decide its own generation deliberately, because a
    /// silent `1` inherited from a trait default is indistinguishable
    /// from "this adapter has never changed its output" — a claim only
    /// its author can make.
    ///
    /// The shipped adapters are at `1` EXCEPT `claude-code`, which is
    /// at `2` (s37 (`execution-graph-topology`)): its sidechain parse
    /// now populates `agent_id`/`parent_agent_id`, the very fields
    /// [`crate::normalize`] turns into child runs, so an unchanged
    /// `.jsonl` transcript genuinely normalizes to something new and a
    /// stored `claude-code` cursor would hide the whole lineage
    /// backfill behind its watermark. omp/codex/hermes gained the same
    /// two fields as a constant `None` — a `skip_serializing_if`-elided
    /// `Option` no downstream consumer can distinguish from its
    /// absence — so for them `1` is not a placeholder awaiting a real
    /// value but the correct one: the only value that leaves every
    /// cursor already on disk valid and re-reads nothing. Unlike s38's
    /// plan dialects (whose `depends_on` extraction changed, so both
    /// went to `2`), a version here moves per adapter, never in
    /// lockstep.
    fn parse_version(&self) -> u32;

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
