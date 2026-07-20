//! The Codex CLI adapter (S3 Wave 2) — parses `~/.codex/sessions/*.jsonl`
//! AND its sibling `~/.codex/archived_sessions/*.jsonl` (where Codex CLI
//! rotates older sessions) as one logical `codex` adapter identity.
//!
//! Attributed port of the donor's Codex parser and its live+archived
//! scan-root registration, per operator directive 2026-07-10: an
//! attributed port, not a clean-room re-derivation —
//! every ported unit below carries a `Ported from …:LN-LN` line.
//!
//! **`session_id` is the filename stem** (`codex.rs:232-237`), same rule
//! as Claude Code and the opposite of omp/pi's content-derived id
//! (per the donor's session-parser audit §3.4). A
//! *separate*, content-derived identity (`session_meta.id`,
//! `state.session_id_from_meta`) is tracked internally, but only ever
//! feeds the fork-scoped `dedup_key` (`codex.rs:600-611`) — it never
//! becomes `UnifiedRow.session_id`.
//!
//! **The intricate part** (design D6, audit §3.5): Codex's `token_count`
//! events are *cumulative session totals*, not deltas, and a forked
//! child session replays its parent's history into its own file. This
//! module ports `codex.rs:461-626`'s state machine — cumulative-total
//! vs. `last_token_usage` reconciliation, a stale/regressed-total guard,
//! and forked-child replay detection — faithfully; a naive per-line sum
//! would double-count Claude Code duplicates' Codex cousin problem:
//! grossly over-count cumulative totals and double-count replayed fork
//! history.
//!
//! **Deliberately NOT ported** (out of S3 scope, not an oversight):
//! - Codex's separate headless-CLI-cache root (`scanner.rs:1127-1135`'s
//!   `headless_roots`) and its free-form usage-shape parser
//!   (`parse_codex_headless_line` / `extract_headless_usage`,
//!   `codex.rs:1022-1139`) — design D5 scopes the Codex adapter to the
//!   live+archived `sessions`/`archived_sessions` union only.
//! - the donor's incremental-offset resumable reparse
//!   (`parse_codex_file_incremental`, `codex.rs:960-994`, and the
//!   `ParsedCodexFile`/serde-persisted `CodexParseState` bookkeeping
//!   that supports it) — the frozen `SessionAdapter::parse(path) -> Vec<
//!   UnifiedRow>` contract (Wave 1) is a one-shot full-file parse with
//!   no resumption state threaded in or out; every call starts from
//!   `CodexParseState::default()`.
//! - `agent`/`agent_nickname`/`session_is_headless` tracking
//!   (`codex.rs:561-565` et al.) — `UnifiedRow` (Wave 1's frozen
//!   contract) carries no `agent` field at all, so tracking it here
//!   would be dead state.
//! - **s31 design D4 (user-directive capture)**: `parse_codex_reader`'s
//!   existing `event_msg`/`user_message` gate — `codex_message_is_human_turn`
//!   already distinguishes a real human turn from Codex's own
//!   `<environment_context>`/`<system-reminder>`/`<user_instructions>`
//!   injected context (`CODEX_SYSTEM_INJECTED_PREFIXES`) — now ALSO
//!   emits a `DirectiveRow` carrying that turn's verbatim
//!   `payload.message` text (a plain string; Codex's format has no
//!   structured content-block array to flatten, unlike omp/claude).
//!   Forked-child replays of the parent's prompt are already skipped
//!   entirely by the `forked_child_waiting_for_turn_context` branch
//!   BEFORE this gate ever runs, so a replayed prompt never produces a
//!   duplicate directive either.

use std::collections::HashSet;
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

use crate::adapter::{CostSource, DirectiveRow, ParseOutcome, SessionAdapter, TokenBreakdown, UnifiedRow};
use crate::normalize::{normalize_workspace_key, workspace_label_from_key};

/// Env var Codex CLI itself understands for relocating `~/.codex`;
/// honored only when `use_env_roots` is `true` (Wave 1's
/// `SessionAdapter::scan_roots` contract) so a deterministic test never
/// picks up an ambient value from the calling shell. Overrides the
/// ENTIRE `.codex` base directory, not just `sessions` — ported from
/// `scanner.rs:1101-1136`'s `codex_home` resolution (mirrored by
/// the donor's `ClientId::Codex` `PathRoot::EnvVar` entry,
/// `clients.rs:190-201`).
pub const CODEX_HOME_ENV: &str = "CODEX_HOME";

pub struct CodexAdapter;

/// Ported from `codex.rs:26-31`.
#[derive(Debug, Deserialize)]
struct CodexEntry {
    #[serde(rename = "type")]
    entry_type: String,
    timestamp: Option<String>,
    payload: Option<CodexPayload>,
}

/// Ported from `codex.rs:34-60`, trimmed of `agent_nickname` — `UnifiedRow`
/// carries no `agent` field for it to feed (see module doc).
#[derive(Debug, Deserialize)]
struct CodexPayload {
    id: Option<String>,
    forked_from_id: Option<String>,
    #[serde(rename = "type")]
    payload_type: Option<String>,
    model: Option<String>,
    model_name: Option<String>,
    model_info: Option<CodexModelInfo>,
    info: Option<CodexInfo>,
    turn_id: Option<String>,
    source: Option<Value>,
    /// Thread origin from session_meta. `"user"` marks a human-initiated
    /// fork (e.g. a VS Code "fork conversation"), which replays parent
    /// history but never emits a `task_started` for the child's own turn.
    thread_source: Option<String>,
    /// Current working directory from session_meta.
    cwd: Option<String>,
    /// Provider identity from session_meta (e.g. "openai", "azure").
    model_provider: Option<String>,
    /// Free-text body of an `event_msg` `user_message` payload. Used to
    /// detect human turn boundaries: real human input is plain text,
    /// whereas system-injected context (`<environment_context>`, …)
    /// begins with one of `CODEX_SYSTEM_INJECTED_PREFIXES`.
    message: Option<String>,
}

/// Ported from `codex.rs:62-65`.
#[derive(Debug, Deserialize)]
struct CodexModelInfo {
    slug: Option<String>,
}

/// Ported from `codex.rs:67-73`.
#[derive(Debug, Deserialize)]
struct CodexInfo {
    model: Option<String>,
    model_name: Option<String>,
    last_token_usage: Option<CodexTokenUsage>,
    total_token_usage: Option<CodexTokenUsage>,
}

/// Ported from `codex.rs:75-83`.
#[derive(Debug, Deserialize, Clone)]
struct CodexTokenUsage {
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    cached_input_tokens: Option<i64>,
    cache_read_input_tokens: Option<i64>,
    reasoning_output_tokens: Option<i64>,
    total_tokens: Option<i64>,
}

/// A cumulative-snapshot accumulator (NOT a per-line token count) — the
/// state `token_count` deltas are diffed against. Ported from
/// `codex.rs:85-176`.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
struct CodexTotals {
    input: i64,
    output: i64,
    cached: i64,
    reasoning: i64,
}

impl CodexTotals {
    fn from_usage(usage: &CodexTokenUsage) -> Self {
        Self {
            input: usage.input_tokens.unwrap_or(0).max(0),
            output: usage.output_tokens.unwrap_or(0).max(0),
            cached: usage.cached_input_tokens.unwrap_or(0).max(usage.cache_read_input_tokens.unwrap_or(0)).max(0),
            reasoning: usage.reasoning_output_tokens.unwrap_or(0).max(0),
        }
    }

    fn delta_from(self, previous: Self) -> Option<Self> {
        if self.input < previous.input || self.output < previous.output || self.cached < previous.cached || self.reasoning < previous.reasoning {
            return None;
        }

        Some(Self {
            input: self.input - previous.input,
            output: self.output - previous.output,
            cached: self.cached - previous.cached,
            reasoning: self.reasoning - previous.reasoning,
        })
    }

    fn saturating_add(self, other: Self) -> Self {
        Self {
            input: self.input.saturating_add(other.input),
            output: self.output.saturating_add(other.output),
            cached: self.cached.saturating_add(other.cached),
            reasoning: self.reasoning.saturating_add(other.reasoning),
        }
    }

    fn total(self) -> i64 {
        self.input.saturating_add(self.output).saturating_add(self.cached).saturating_add(self.reasoning)
    }

    fn is_within(self, baseline: Self) -> bool {
        self.input <= baseline.input && self.output <= baseline.output && self.cached <= baseline.cached && self.reasoning <= baseline.reasoning
    }

    /// Some Codex `token_count` snapshots arrive slightly out of order:
    /// the cumulative total regresses by roughly one recent increment,
    /// then resumes from the true higher watermark on the next row.
    /// Treat those as stale snapshots rather than hard resets so
    /// `last_token_usage` doesn't get counted twice.
    fn looks_like_stale_regression(self, previous: Self, last: Self) -> bool {
        let previous_total = previous.total();
        let current_total = self.total();
        let last_total = last.total();

        if previous_total <= 0 || current_total <= 0 || last_total <= 0 {
            return false;
        }

        current_total.saturating_mul(100) >= previous_total.saturating_mul(98) || current_total.saturating_add(last_total.saturating_mul(2)) >= previous_total
    }

    fn into_tokens(self) -> TokenBreakdown {
        // Clamp cached to not exceed input to prevent inflated totals
        // when malformed data reports more cached tokens than input.
        let clamped_cached = self.cached.min(self.input).max(0);
        TokenBreakdown { input: (self.input - clamped_cached).max(0), output: self.output.max(0), cache_read: clamped_cached, cache_write: 0, reasoning: self.reasoning.max(0) }
    }
}

/// Ported from `codex.rs:178-219`, trimmed of `session_is_headless`/
/// `session_agent` (see module doc) and the `#[serde(default)]`/
/// `Serialize`/`Deserialize` plumbing the donor's incremental-reparse
/// cache needed — this crate's `parse()` always starts fresh from
/// `CodexParseState::default()`.
#[derive(Debug, Clone, Default)]
struct CodexParseState {
    current_model: Option<String>,
    current_turn_start_ms: Option<i64>,
    previous_totals: Option<CodexTotals>,
    session_id_from_meta: Option<String>,
    session_forked_from_id: Option<String>,
    forked_child_session_id: Option<String>,
    forked_child_replay_session_id: Option<String>,
    session_provider: Option<String>,
    session_workspace_key: Option<String>,
    session_workspace_label: Option<String>,
    forked_child_waiting_for_turn_context: bool,
    forked_child_inherited_baseline: Option<CodexTotals>,
    forked_child_inherited_reported_total: Option<i64>,
    /// Set when a human `user_message` event is seen; consumed by the
    /// next token_count-derived row to mark it `is_turn_start`.
    pending_turn_start: bool,
    /// `turn_id`s announced by a `task_started` event while a forked
    /// child is still skipping its replayed parent history — used only
    /// to disambiguate a same-millisecond turn (see
    /// `forked_child_turn_starts_own_session`).
    forked_child_task_started_turn_ids: HashSet<String>,
    /// Set when the active forked child is a human-initiated
    /// (`thread_source: "user"`) fork.
    forked_child_is_user_fork: bool,
}

impl SessionAdapter for CodexAdapter {
    fn client_id(&self) -> &'static str {
        "codex"
    }

    /// Live+archived root union — ported from `scanner.rs:1101-1136`
    /// (design D5). `${CODEX_HOME:-~/.codex}/sessions` is Codex CLI's
    /// live transcript dir; `archived_sessions` is Codex CLI's own
    /// session-rotation behavior, not a donor invention — an adapter
    /// that scans only the live directory silently under-counts any
    /// session Codex has rotated out.
    fn scan_roots(&self, home: &Path, use_env_roots: bool) -> Vec<PathBuf> {
        let codex_home = if use_env_roots {
            std::env::var(CODEX_HOME_ENV).ok().filter(|value| !value.trim().is_empty()).map(PathBuf::from).unwrap_or_else(|| home.join(".codex"))
        } else {
            home.join(".codex")
        };

        vec![codex_home.join("sessions"), codex_home.join("archived_sessions")]
    }

    fn parse(&self, path: &Path) -> ParseOutcome {
        parse_codex_file(path)
    }
}

/// Ported from `codex.rs:232-237` — the row's SURFACE `session_id` is
/// always the filename stem, never `session_meta.id` (see module doc).
fn session_id_from_path(path: &Path) -> String {
    path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown").to_string()
}

/// Ported from `codex.rs:239-248`.
fn codex_workspace_from_cwd(cwd: &str) -> (Option<String>, Option<String>) {
    let workspace_key = normalize_codex_workspace_key(cwd);
    let workspace_label = workspace_key.as_deref().and_then(workspace_label_from_key);

    if workspace_label.is_none() {
        return (None, None);
    }

    (workspace_key, workspace_label)
}

/// Ported from `codex.rs:250-261`, reusing this crate's own
/// `normalize_workspace_key`/`workspace_label_from_key`
/// (`crate::normalize`, verbatim ports of the donor's workspace-key
/// helpers) rather than the donor's `super::` import.
fn normalize_codex_workspace_key(raw: &str) -> Option<String> {
    let normalized = normalize_workspace_key(raw)?;
    if normalized.chars().any(char::is_control) {
        return None;
    }

    if looks_like_explicit_workspace_path(&normalized) { Some(normalized) } else { None }
}

/// Ported from `codex.rs:263-270`.
fn looks_like_explicit_workspace_path(path: &str) -> bool {
    if path.starts_with("//") || path.starts_with('/') {
        return true;
    }

    let bytes = path.as_bytes();
    bytes.len() >= 3 && bytes[0].is_ascii_alphabetic() && bytes[1] == b':' && bytes[2] == b'/'
}

/// Reused-buffer `read_line` loop — ported from `codex.rs:272-308`. The
/// per-entry state machine below (session_meta capture, fork detection,
/// cumulative-delta reconciliation) is ported from `codex.rs:308-706`
/// with the donor's headless-line fallback and incremental-offset/
/// `parse_succeeded` bookkeeping dropped (see module doc) — a line
/// that fails to parse as `CodexEntry` is skipped AND counted
/// (`skipped`, Wave-2 amendment); a line that parses but whose
/// `payload` is absent is skipped WITHOUT counting (a well-formed
/// entry this adapter has no use for, not corrupt content) — neither
/// ever aborts the whole file.
fn parse_codex_reader<R: BufRead>(mut reader: R, session_id: &str, fallback_timestamp: i64, mut state: CodexParseState) -> ParseOutcome {
    let mut rows: Vec<UnifiedRow> = Vec::with_capacity(64);
    let mut directives: Vec<DirectiveRow> = Vec::new();
    let mut skipped = 0usize;
    let mut buffer = Vec::with_capacity(4096);
    let mut line = String::with_capacity(4096);
    let mut pending_model_messages: Vec<(UnifiedRow, bool)> = Vec::new();

    loop {
        line.clear();
        match reader.read_line(&mut line) {
            Ok(0) => break,
            Ok(_) => {}
            Err(_) => break,
        }

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        buffer.clear();
        buffer.extend_from_slice(trimmed.as_bytes());
        let Ok(entry) = simd_json::from_slice::<CodexEntry>(&mut buffer) else {
            skipped += 1;
            continue;
        };
        let Some(payload) = entry.payload else {
            continue;
        };

        let payload_model = extract_model(&payload);
        let is_token_count = entry.entry_type == "event_msg" && payload.payload_type.as_deref() == Some("token_count");
        let info_model = if is_token_count { payload.info.as_ref().and_then(extract_model_from_info) } else { None };
        let event_model = payload_model.clone().or_else(|| info_model.clone());

        // Ported from `codex.rs:320-369` — while skipping a forked
        // child's replayed parent history, every entry is either the
        // one that "starts the child's own turn" (falls through, no
        // `continue`, to the ordinary per-entry handling below — same
        // fallthrough the donor's own `handled = true` without an early
        // `continue` produces at `codex.rs:331-332`) or is consumed
        // here via `continue`.
        if state.forked_child_waiting_for_turn_context {
            let ends_wait = entry.entry_type == "turn_context" && forked_child_turn_starts_own_session(&state, payload.turn_id.as_deref());
            if ends_wait {
                state.forked_child_waiting_for_turn_context = false;
                state.forked_child_replay_session_id = None;
                state.forked_child_task_started_turn_ids.clear();
                state.forked_child_is_user_fork = false;
                let carried_child_id = state.forked_child_session_id.clone();
                if let Some(id) = carried_child_id {
                    state.session_id_from_meta = Some(id);
                }
                state.current_model = payload_model.clone();
            } else {
                if entry.entry_type == "event_msg" && payload.payload_type.as_deref() == Some("task_started") {
                    if let Some(turn_id) = payload.turn_id.as_deref() {
                        state.forked_child_task_started_turn_ids.insert(turn_id.to_string());
                    }
                }
                if entry.entry_type == "session_meta" {
                    if let Some(id) = payload.id.as_deref() {
                        if state.forked_child_session_id.as_deref().is_some_and(|child_id| child_id != id) {
                            // Newer Codex fork logs can embed the parent
                            // session metadata before replaying parent
                            // token_count history. Keep skipping while
                            // that copied upstream transcript is active.
                            state.forked_child_replay_session_id = Some(id.to_string());
                        }
                    }
                }
                if is_token_count {
                    if let Some(info) = payload.info.as_ref() {
                        remember_forked_child_inherited_baseline(&mut state, info);
                    }
                }
                continue;
            }
        }

        if !pending_model_messages.is_empty() && event_model.is_none() && !is_token_count && entry.entry_type != "session_meta" {
            flush_pending_model_messages_as_unknown(&mut pending_model_messages, &mut rows);
        }

        // Ported from `codex.rs:384-423`.
        if entry.entry_type == "session_meta" {
            if let Some(id) = payload.id.as_deref() {
                state.session_id_from_meta = Some(id.to_string());
            }
            let forked_from_id = payload.forked_from_id.as_deref().filter(|id| !id.is_empty()).or_else(|| forked_from_id_from_source(payload.source.as_ref()));
            if let Some(forked_from_id) = forked_from_id {
                let repeated_active_child_meta =
                    !state.forked_child_waiting_for_turn_context && payload.id.as_deref().is_some() && state.forked_child_session_id.as_deref() == payload.id.as_deref();
                state.session_forked_from_id = Some(forked_from_id.to_string());
                state.forked_child_session_id = payload.id.clone();
                if !repeated_active_child_meta {
                    state.forked_child_waiting_for_turn_context = true;
                    state.forked_child_replay_session_id = None;
                    state.forked_child_inherited_baseline = None;
                    state.forked_child_inherited_reported_total = None;
                    state.forked_child_task_started_turn_ids.clear();
                    state.forked_child_is_user_fork = payload.thread_source.as_deref() == Some("user");
                }
            }
            if let Some(provider) = &payload.model_provider {
                state.session_provider = Some(provider.clone());
            }
            if let Some(cwd) = &payload.cwd {
                let (workspace_key, workspace_label) = codex_workspace_from_cwd(cwd);
                state.session_workspace_key = workspace_key;
                state.session_workspace_label = workspace_label;
            }
        }

        // Extract model from turn_context — ported from `codex.rs:425-439`.
        if entry.entry_type == "turn_context" {
            state.current_model = payload_model.clone();
            state.current_turn_start_ms = parse_codex_entry_timestamp(entry.timestamp.as_deref());
            let flushed_model = state.current_model.clone();
            if let Some(model) = flushed_model {
                flush_pending_model_messages(&mut pending_model_messages, &mut rows, &model);
            }
        }

        // A human `user_message` event starts a new turn — ported from
        // `codex.rs:441-459`. The event itself carries no tokens, so the
        // flag is deferred to the next token_count-derived row.
        if entry.entry_type == "event_msg" && payload.payload_type.as_deref() == Some("user_message") && codex_message_is_human_turn(payload.message.as_deref()) {
            state.pending_turn_start = true;
            // Forked-child replays of the parent prompt arrive before
            // turn_context and are already skipped by the
            // `forked_child_waiting_for_turn_context` branch above, so
            // they never reach here.

            // s31 D4: the event itself carries no tokens, but it IS
            // the directive — `payload.message` is a plain string
            // (Codex's format has no structured content-block array
            // to flatten, unlike omp/claude). Already guarded by
            // `codex_message_is_human_turn` above, so this is always
            // `Some`.
            if let Some(text) = payload.message.clone() {
                let timestamp_ms = parse_codex_entry_timestamp(entry.timestamp.as_deref()).unwrap_or(fallback_timestamp);
                directives.push(DirectiveRow {
                    client: "codex".to_string(),
                    session_id: session_id.to_string(),
                    timestamp_ms,
                    text,
                    workspace_key: state.session_workspace_key.clone(),
                    workspace_label: state.session_workspace_label.clone(),
                });
            }
        }

        // Process token_count events — ported from `codex.rs:461-625`,
        // the cumulative-total/delta-reconciliation state machine.
        if is_token_count {
            let Some(info) = payload.info else {
                continue;
            };

            let model = payload_model.or(info_model).or_else(|| state.current_model.clone());
            if let Some(model_name) = &model {
                state.current_model = Some(model_name.clone());
                flush_pending_model_messages(&mut pending_model_messages, &mut rows, model_name);
            }

            // Use last_token_usage as the primary increment source.
            // Upstream totals are mutable snapshots (compaction,
            // context-window capping can rewrite them), so total_token_
            // usage is only used for dedup and monotonicity checks —
            // never as a direct delta source.
            let total_usage = info.total_token_usage.as_ref().map(CodexTotals::from_usage);
            let last_usage = info.last_token_usage.as_ref().map(CodexTotals::from_usage);

            // Forked child logs can replay more than one parent
            // token_count row after the first child turn_context, often
            // with child-local timestamps. Keep the inherited baseline
            // active until totals move beyond it.
            if forked_child_should_skip_inherited_snapshot(&state, info.total_token_usage.as_ref(), total_usage) {
                continue;
            }
            state.forked_child_inherited_baseline = None;
            state.forked_child_inherited_reported_total = None;

            let (tokens, next_totals) = match (total_usage, last_usage, state.previous_totals) {
                // Both present with previous baseline (standard path).
                (Some(total), Some(last), Some(previous)) => {
                    if total == previous {
                        continue;
                    }
                    if total.delta_from(previous).is_none() && total.looks_like_stale_regression(previous, last) {
                        continue;
                    }
                    (last.into_tokens(), Some(total))
                }
                // Both present, first event — use last (NOT full total)
                // to avoid overcounting tokens carried from a resumed
                // session.
                (Some(total), Some(last), None) => (last.into_tokens(), Some(total)),
                // Only total, have previous (defensive — upstream schema
                // requires both when info is present).
                (Some(total), None, Some(previous)) => {
                    if total == previous {
                        continue;
                    }
                    if let Some(delta) = total.delta_from(previous) {
                        (delta.into_tokens(), Some(total))
                    } else {
                        state.previous_totals = Some(total);
                        continue;
                    }
                }
                // Only total, first event, no last — legacy/degraded path.
                (Some(total), None, None) => (total.into_tokens(), Some(total)),
                // Only last, have previous.
                (None, Some(last), Some(previous)) => (last.into_tokens(), Some(previous.saturating_add(last))),
                // Only last, no previous.
                (None, Some(last), None) => (last.into_tokens(), None),
                // Neither.
                (None, None, _) => continue,
            };

            // Skip zero-token snapshots without advancing the baseline
            // so post-compaction zero totals don't inflate later deltas.
            if tokens.input == 0 && tokens.output == 0 && tokens.cache_read == 0 && tokens.reasoning == 0 {
                continue;
            }

            state.previous_totals = next_totals;

            let parsed_timestamp = parse_codex_entry_timestamp(entry.timestamp.as_deref());
            let timestamp_ms = parsed_timestamp.unwrap_or(fallback_timestamp);
            let duration_ms = duration_between_ms(state.current_turn_start_ms, parsed_timestamp);

            let provider = state.session_provider.as_deref().or_else(|| model.as_deref().and_then(inferred_provider_from_model)).unwrap_or("openai").to_string();

            let mut row = UnifiedRow {
                client: "codex".to_string(),
                model_id: model.clone().unwrap_or_else(|| "unknown".to_string()),
                provider_id: provider,
                session_id: session_id.to_string(),
                workspace_key: state.session_workspace_key.clone(),
                workspace_label: state.session_workspace_label.clone(),
                timestamp_ms,
                tokens,
                cost: 0.0,
                cost_source: CostSource::Unknown,
                duration_ms,
                dedup_key: None,
                is_turn_start: false,
            };

            if state.pending_turn_start {
                row.is_turn_start = true;
                state.pending_turn_start = false;
            }

            if parsed_timestamp.is_some() || total_usage.is_some() {
                // Fork/subagent children replay the same upstream
                // token_count history into many sibling files. Those
                // replays carry identical cumulative totals but a
                // distinct per-file session id, so a session-scoped key
                // never collapses them and the totals get counted once
                // per sibling. Scope the key to the fork parent instead
                // so sibling replays share one key. Unrelated sessions
                // keep their own id and never merge. Ported from
                // `codex.rs:591-611`.
                let dedup_scope_id = state.session_forked_from_id.as_deref().or(state.session_id_from_meta.as_deref()).unwrap_or(session_id);
                set_codex_dedup_key(&mut row, model.as_deref().unwrap_or("unknown"), dedup_scope_id, total_usage);
            }

            if model.is_some() {
                rows.push(row);
            } else {
                pending_model_messages.push((row, parsed_timestamp.is_none()));
            }
        }
    }

    flush_pending_model_messages_as_unknown(&mut pending_model_messages, &mut rows);
    ParseOutcome::with_directives(rows, skipped, directives)
}

/// Ported from `codex.rs:712-719`.
fn forked_from_id_from_source(source: Option<&Value>) -> Option<&str> {
    source?.get("subagent")?.get("thread_spawn")?.get("parent_thread_id")?.as_str().filter(|id| !id.is_empty())
}

/// Ported from `codex.rs:721-773`.
fn forked_child_turn_starts_own_session(state: &CodexParseState, turn_id: Option<&str>) -> bool {
    if state.forked_child_replay_session_id.is_none() {
        return true;
    }

    let Some(child_session_id) = state.forked_child_session_id.as_deref() else {
        return true;
    };

    match (turn_id, codex_uuid_v7_order_key(child_session_id)) {
        (Some(turn_id), Some(child_key)) => {
            let Some(turn_key) = codex_uuid_v7_order_key(turn_id) else {
                return true;
            };
            // Compare only the UUID v7 48-bit millisecond timestamp (the
            // first 12 hex of the order key), not the full id. The
            // child's own turn is minted at or after its session_meta
            // and the replayed parent turns strictly earlier, so the
            // millisecond prefix is the causal signal; the version
            // nibble + random tail of two independently-minted v7 UUIDs
            // is a coin flip.
            match turn_key[..12].cmp(&child_key[..12]) {
                std::cmp::Ordering::Greater => true,
                std::cmp::Ordering::Less => false,
                // Same millisecond: only a task-started turn_id (or a
                // human-initiated user fork, which never emits
                // task_started but whose replayed parent turns carry
                // the *parent's* millisecond prefix) ends the skip —
                // see `codex.rs:756-764` for the accepted residual.
                std::cmp::Ordering::Equal => state.forked_child_is_user_fork || state.forked_child_task_started_turn_ids.contains(turn_id),
            }
        }
        _ => true,
    }
}

/// Ported from `codex.rs:775-802`.
fn codex_uuid_v7_order_key(id: &str) -> Option<String> {
    let mut parts = id.split('-');
    let first = parts.next()?;
    let second = parts.next()?;
    let third = parts.next()?;
    let fourth = parts.next()?;
    let fifth = parts.next()?;

    if parts.next().is_some() || first.len() != 8 || second.len() != 4 || third.len() != 4 || fourth.len() != 4 || fifth.len() != 12 || !third.starts_with('7') {
        return None;
    }

    let mut key = String::with_capacity(32);
    for part in [first, second, third, fourth, fifth] {
        if !part.bytes().all(|byte| byte.is_ascii_hexdigit()) {
            return None;
        }
        key.push_str(&part.to_ascii_lowercase());
    }
    Some(key)
}

/// Ported from `codex.rs:804-808`.
fn parse_codex_entry_timestamp(timestamp: Option<&str>) -> Option<i64> {
    timestamp.and_then(|ts| chrono::DateTime::parse_from_rfc3339(ts).ok()).map(|dt| dt.timestamp_millis())
}

/// Ported from `codex.rs:810-813`.
fn duration_between_ms(start_ms: Option<i64>, end_ms: Option<i64>) -> Option<i64> {
    let duration = end_ms?.saturating_sub(start_ms?);
    (duration > 0).then_some(duration)
}

/// Ported from `codex.rs:815-849`, adapted to `UnifiedRow`'s field names.
fn codex_token_count_dedup_key(row: &UnifiedRow, model: &str, upstream_session_id: &str, total_usage: Option<CodexTotals>) -> String {
    if let Some(total) = total_usage {
        // Codex fork/subagent logs can replay the same upstream
        // token_count history into many child files with child-local
        // timestamps. The cumulative total is the stable upstream
        // identity; timestamp is only a fallback when older rows do not
        // carry totals.
        return format!("codex:token_count-total:{}:{}:{}:{}:{}:{}:{}", upstream_session_id, row.provider_id, model, total.input, total.output, total.cached, total.reasoning);
    }

    format!(
        "codex:token_count:{}:{}:{}:{}:{}:{}:{}:{}",
        row.timestamp_ms, row.provider_id, model, row.tokens.input, row.tokens.output, row.tokens.cache_read, row.tokens.cache_write, row.tokens.reasoning
    )
}

/// Ported from `codex.rs:851-865`.
fn set_codex_dedup_key(row: &mut UnifiedRow, model: &str, upstream_session_id: &str, total_usage: Option<CodexTotals>) {
    if row.dedup_key.is_none() {
        row.dedup_key = Some(codex_token_count_dedup_key(row, model, upstream_session_id, total_usage));
    }
}

/// Ported from `codex.rs:867-884` (`fallback_timestamp_indices`
/// bookkeeping dropped — this crate's `parse()` return doesn't surface
/// it). NOTE: faithfully preserves the donor's own asymmetry — the
/// dedup key set here scopes to the row's OWN `session_id`, not the
/// fork-scoped id the primary token_count path uses (`codex.rs:600-608`),
/// because at this point (a token_count event resolved BEFORE any model
/// was known) the fork state isn't reliably attributable yet.
fn flush_pending_model_messages(pending: &mut Vec<(UnifiedRow, bool)>, rows: &mut Vec<UnifiedRow>, model: &str) {
    for (mut row, used_fallback_timestamp) in pending.drain(..) {
        if !used_fallback_timestamp {
            let upstream_session_id = row.session_id.clone();
            set_codex_dedup_key(&mut row, model, &upstream_session_id, None);
        }
        row.model_id = model.to_string();
        rows.push(row);
    }
}

/// Ported from `codex.rs:886-903`.
fn flush_pending_model_messages_as_unknown(pending: &mut Vec<(UnifiedRow, bool)>, rows: &mut Vec<UnifiedRow>) {
    if pending.is_empty() {
        return;
    }
    flush_pending_model_messages(pending, rows, "unknown");
}

/// Ported from `codex.rs:905-923`, adapted to always start from
/// `CodexParseState::default()` (see module doc — no incremental resume).
fn parse_codex_file(path: &Path) -> ParseOutcome {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return ParseOutcome::default(),
    };

    let session_id = session_id_from_path(path);
    let fallback_timestamp = file_modified_timestamp_ms(path);
    let reader = BufReader::new(file);
    parse_codex_reader(reader, &session_id, fallback_timestamp, CodexParseState::default())
}

/// Ported from `codex.rs:925-927`.
fn reported_total_tokens(usage: &CodexTokenUsage) -> Option<i64> {
    usage.total_tokens.filter(|total| *total >= 0)
}

/// Ported from `codex.rs:929-938`.
fn remember_forked_child_inherited_baseline(state: &mut CodexParseState, info: &CodexInfo) {
    let Some(total_usage) = info.total_token_usage.as_ref() else {
        return;
    };

    let totals = CodexTotals::from_usage(total_usage);
    state.previous_totals = Some(totals);
    state.forked_child_inherited_baseline = Some(totals);
    state.forked_child_inherited_reported_total = reported_total_tokens(total_usage);
}

/// Ported from `codex.rs:940-958`.
fn forked_child_should_skip_inherited_snapshot(state: &CodexParseState, total_usage: Option<&CodexTokenUsage>, totals: Option<CodexTotals>) -> bool {
    if let (Some(usage), Some(baseline)) = (total_usage, state.forked_child_inherited_reported_total) {
        if reported_total_tokens(usage).is_some_and(|total| total <= baseline) {
            return true;
        }
    }

    if let (Some(totals), Some(baseline)) = (totals, state.forked_child_inherited_baseline) {
        return totals.is_within(baseline);
    }

    false
}

/// Ported from `codex.rs:996-1005`.
fn extract_model(payload: &CodexPayload) -> Option<String> {
    payload
        .model_info
        .as_ref()
        .and_then(|mi| mi.slug.clone())
        .filter(|s| !s.is_empty())
        .or(payload.model.clone().filter(|s| !s.is_empty()))
        .or(payload.model_name.clone().filter(|s| !s.is_empty()))
        .or(payload.info.as_ref().and_then(extract_model_from_info))
}

/// Ported from `codex.rs:1007-1012`.
fn extract_model_from_info(info: &CodexInfo) -> Option<String> {
    info.model.clone().filter(|s| !s.is_empty()).or(info.model_name.clone().filter(|s| !s.is_empty()))
}

/// Prefixes Codex prepends to context it injects as `user_message`
/// events. These are the bodies that must NOT be counted as human
/// turns. Ported from `codex.rs:1141-1147`.
const CODEX_SYSTEM_INJECTED_PREFIXES: [&str; 3] = ["<environment_context>", "<system-reminder>", "<user_instructions>"];

/// Returns true when a Codex `user_message` payload represents real
/// human input rather than system-injected context. Ported from
/// `codex.rs:1149-1168`.
fn codex_message_is_human_turn(message: Option<&str>) -> bool {
    match message {
        Some(text) => {
            let trimmed = text.trim_start();
            !CODEX_SYSTEM_INJECTED_PREFIXES.iter().any(|prefix| trimmed.starts_with(prefix))
        }
        None => false,
    }
}

/// Ported from the donor session-parser project
/// (same local-copy pattern `adapters::omp::file_modified_timestamp_ms` uses).
fn file_modified_timestamp_ms(path: &Path) -> i64 {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(std::time::SystemTime::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_else(|| chrono::Utc::now().timestamp_millis())
}

/// Ported from the donor session-parser project's provider-identity module
/// (`contains_delimited` + `inferred_provider_from_model`) — a local
/// copy scoped to this adapter, same as `adapters::omp`'s own copy
/// (the donor's version is a crate-wide shared module; canon-ingest has
/// no crate-wide equivalent for a single adapter's fallback to justify
/// one yet).
fn contains_delimited(haystack: &str, needle: &str) -> bool {
    for (pos, _) in haystack.match_indices(needle) {
        let before_ok = pos == 0 || !haystack.as_bytes()[pos - 1].is_ascii_alphanumeric();
        let after_pos = pos + needle.len();
        let after_ok = after_pos == haystack.len() || !haystack.as_bytes()[after_pos].is_ascii_alphanumeric();
        if before_ok && after_ok {
            return true;
        }
    }
    false
}

fn inferred_provider_from_model(model: &str) -> Option<&'static str> {
    let lower = model.to_lowercase();

    if lower.contains("claude") || lower.contains("anthropic") || contains_delimited(&lower, "opus") || contains_delimited(&lower, "sonnet") || contains_delimited(&lower, "haiku") {
        return Some("anthropic");
    }
    if lower.contains("gpt") || lower.contains("openai") || contains_delimited(&lower, "o1") || contains_delimited(&lower, "o3") || contains_delimited(&lower, "o4") {
        return Some("openai");
    }
    if lower.contains("gemini") || lower.contains("google") {
        return Some("google");
    }
    if lower.contains("grok") {
        return Some("xai");
    }
    if lower.contains("deepseek") {
        return Some("deepseek");
    }
    if lower.contains("mistral") || lower.contains("mixtral") {
        return Some("mistral");
    }
    if lower.contains("llama") || contains_delimited(&lower, "meta") {
        return Some("meta");
    }
    if lower.contains("qwen") {
        return Some("qwen");
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::io::Write;

    fn write_fixture(content: &str) -> tempfile::NamedTempFile {
        let mut file = tempfile::NamedTempFile::with_suffix(".jsonl").unwrap();
        file.write_all(content.as_bytes()).unwrap();
        file.flush().unwrap();
        file
    }

    #[test]
    fn session_id_is_the_filename_stem_not_session_meta_id() {
        let content = concat!(
            r#"{"type":"session_meta","payload":{"id":"content-derived-id","source":"interactive","model_provider":"openai","cwd":"/repo/proj"}}"#,
            "\n",
            r#"{"timestamp":"2026-01-01T00:00:01Z","type":"turn_context","payload":{"model":"gpt-5.1"}}"#,
            "\n",
            r#"{"timestamp":"2026-01-01T00:00:02Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":10,"output_tokens":3}}}}"#,
        );
        let file = write_fixture(content);
        let rows = parse_codex_file(file.path()).rows;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].session_id, file.path().file_stem().unwrap().to_string_lossy());
        assert_ne!(rows[0].session_id, "content-derived-id");
    }

    /// Cumulative-total -> per-row delta correctness: three
    /// `token_count` events carrying ONLY `total_token_usage` (no
    /// `last_token_usage`, forcing the `(Some(total), None, Some
    /// (previous))` delta-reconciliation branch, `codex.rs:519-531`) —
    /// a naive per-line sum would report the raw cumulative totals
    /// (100/180/260 input) instead of the true per-turn deltas
    /// (80/60/60 after cache clamping).
    #[test]
    fn cumulative_totals_are_diffed_into_deltas_not_summed_raw() {
        let content = concat!(
            r#"{"timestamp":"2026-01-01T00:00:00Z","type":"session_meta","payload":{"source":"interactive","model_provider":"openai","cwd":"/repo/proj"}}"#,
            "\n",
            r#"{"timestamp":"2026-01-01T00:00:01Z","type":"turn_context","payload":{"model":"gpt-5.1"}}"#,
            "\n",
            r#"{"timestamp":"2026-01-01T00:00:02Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":100,"cached_input_tokens":20,"output_tokens":30,"reasoning_output_tokens":5}}}}"#,
            "\n",
            r#"{"timestamp":"2026-01-01T00:00:05Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":180,"cached_input_tokens":40,"output_tokens":55,"reasoning_output_tokens":8}}}}"#,
            "\n",
            r#"{"timestamp":"2026-01-01T00:00:09Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":260,"cached_input_tokens":60,"output_tokens":90,"reasoning_output_tokens":12}}}}"#,
        );
        let file = write_fixture(content);
        let rows = parse_codex_file(file.path()).rows;

        assert_eq!(rows.len(), 3);
        // Row 1: first event, no previous baseline -> full total (clamped).
        assert_eq!((rows[0].tokens.input, rows[0].tokens.cache_read, rows[0].tokens.output, rows[0].tokens.reasoning), (80, 20, 30, 5));
        // Row 2: delta of totals (180-100=80 raw input, 40-20=20 cached -> 60 net input).
        assert_eq!((rows[1].tokens.input, rows[1].tokens.cache_read, rows[1].tokens.output, rows[1].tokens.reasoning), (60, 20, 25, 3));
        // Row 3: delta of totals (260-180=80 raw input, 60-40=20 cached -> 60 net input).
        assert_eq!((rows[2].tokens.input, rows[2].tokens.cache_read, rows[2].tokens.output, rows[2].tokens.reasoning), (60, 20, 35, 4));
        assert_eq!(rows[0].model_id, "gpt-5.1");
        assert_eq!(rows[0].provider_id, "openai");
        assert_eq!(rows[0].workspace_key.as_deref(), Some("/repo/proj"));
    }

    /// Forked-child replay is NOT double-counted: the child's own file
    /// opens with a session_meta declaring `forked_from_id`, then
    /// replays a parent `user_message` + `token_count` snapshot BEFORE
    /// its own `turn_context` (skipped entirely — never even becomes a
    /// row), then replays the SAME cumulative total again immediately
    /// AFTER its own turn_context (skipped via the inherited-baseline
    /// guard), and only the genuinely new token_count row is emitted.
    /// Adapted from the donor's own `test_forked_child_ignores_inherited_records_before_turn_context`
    /// and `test_forked_child_ignores_replayed_parent_rows_after_turn_context`
    /// (`codex.rs:1817-1871`), combined into one fixture.
    #[test]
    fn forked_child_replay_of_parent_history_is_not_double_counted() {
        let content = concat!(
            r#"{"timestamp":"2026-05-05T21:51:57.991Z","type":"session_meta","payload":{"id":"child-session","forked_from_id":"parent-session","source":{"subagent":{"thread_spawn":{"parent_thread_id":"parent-session","depth":1}}},"model_provider":"openai","cwd":"/repo-child"}}"#,
            "\n",
            r#"{"timestamp":"2026-05-05T21:51:57.992Z","type":"session_meta","payload":{"id":"parent-session","source":"interactive","model_provider":"azure","cwd":"/repo-parent"}}"#,
            "\n",
            r#"{"timestamp":"2026-05-05T21:51:57.993Z","type":"event_msg","payload":{"type":"user_message","message":"parent prompt copied into child log"}}"#,
            "\n",
            r#"{"timestamp":"2026-05-05T21:51:57.994Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":116000,"cached_input_tokens":114000,"output_tokens":1000,"total_tokens":117000},"last_token_usage":{"input_tokens":73000,"cached_input_tokens":72000,"output_tokens":500,"total_tokens":73500}}}}"#,
            "\n",
            r#"{"timestamp":"2026-05-05T21:51:58.947Z","type":"turn_context","payload":{"model":"gpt-5.5"}}"#,
            "\n",
            r#"{"timestamp":"2026-05-05T21:51:58.948Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":116000,"cached_input_tokens":114000,"output_tokens":1000,"total_tokens":117000},"last_token_usage":{"input_tokens":73000,"cached_input_tokens":72000,"output_tokens":500,"total_tokens":73500}}}}"#,
            "\n",
            r#"{"timestamp":"2026-05-05T21:51:59.253Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":117500,"cached_input_tokens":115000,"output_tokens":1200,"reasoning_output_tokens":50,"total_tokens":118700},"last_token_usage":{"input_tokens":1500,"cached_input_tokens":1000,"output_tokens":200,"reasoning_output_tokens":50,"total_tokens":1700}}}}"#,
            "\n",
        );
        let file = write_fixture(content);
        let outcome = parse_codex_file(file.path());
        let rows = outcome.rows;

        assert_eq!(rows.len(), 1, "parent replay (pre- and post-turn_context) must not surface as rows");
        assert_eq!(rows[0].model_id, "gpt-5.5");
        assert_eq!(rows[0].tokens.input, 500);
        assert_eq!(rows[0].tokens.cache_read, 1000);
        assert_eq!(rows[0].tokens.output, 200);
        assert_eq!(rows[0].tokens.reasoning, 50);
        // Workspace comes from the child's OWN session_meta (first
        // line), never the replayed parent's `/repo-parent`.
        assert_eq!(rows[0].workspace_key.as_deref(), Some("/repo-child"));
        // Dedup key is scoped to the fork PARENT id, not the child's own
        // filename-derived session_id, so sibling replay files collapse
        // onto one key.
        assert!(rows[0].dedup_key.as_deref().unwrap().contains("parent-session"));
        // s31 D4: the replayed parent `user_message` sits INSIDE the
        // forked_child_waiting_for_turn_context skip window (before
        // the child's own turn_context) — it must never surface as a
        // directive either, same as it never surfaces as a row.
        assert!(outcome.directives.is_empty(), "a replayed parent prompt must not produce a duplicate directive");
    }

    #[test]
    fn corrupt_line_is_skipped_not_fatal() {
        let content = concat!(
            r#"{"timestamp":"2026-01-01T00:00:00Z","type":"session_meta","payload":{"source":"interactive","model_provider":"openai","cwd":"/repo/proj"}}"#,
            "\n",
            r#"{"timestamp":"2026-01-01T00:00:01Z","type":"turn_context","payload":{"model":"gpt-5.1"}}"#,
            "\n",
            "not valid json at all\n",
            r#"{"timestamp":"2026-01-01T00:00:02Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":10,"output_tokens":3}}}}"#,
            "\n",
        );
        let file = write_fixture(content);
        let outcome = parse_codex_file(file.path());
        assert_eq!(outcome.skipped, 1, "the corrupt line must be COUNTED, not just silently skipped");
        assert_eq!(outcome.rows.len(), 1);
        assert_eq!(outcome.rows[0].model_id, "gpt-5.1");
    }

    #[test]
    fn parsing_the_same_file_twice_is_idempotent() {
        let content = concat!(
            r#"{"timestamp":"2026-01-01T00:00:00Z","type":"session_meta","payload":{"source":"interactive","model_provider":"openai","cwd":"/repo/proj"}}"#,
            "\n",
            r#"{"timestamp":"2026-01-01T00:00:01Z","type":"turn_context","payload":{"model":"gpt-5.1"}}"#,
            "\n",
            r#"{"timestamp":"2026-01-01T00:00:02Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":10,"output_tokens":3}}}}"#,
            "\n",
            r#"{"timestamp":"2026-01-01T00:00:05Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":25,"output_tokens":9}}}}"#,
            "\n",
        );
        let file = write_fixture(content);
        let first = parse_codex_file(file.path());
        let second = parse_codex_file(file.path());
        assert_eq!(first, second);
        assert_eq!(first.rows.len(), 2);
    }

    #[test]
    fn scan_roots_unions_live_and_archived_sessions() {
        let adapter = CodexAdapter;
        let home = Path::new("/home/example");
        let roots = adapter.scan_roots(home, false);
        assert!(roots.contains(&home.join(".codex/sessions")));
        assert!(roots.contains(&home.join(".codex/archived_sessions")));
    }

    #[test]
    fn scan_roots_honors_codex_home_env_override_only_when_enabled() {
        // Both cases share one test (rather than two separate `#[test]`
        // fns) because `CODEX_HOME` is process-global state — two tests
        // independently set/remove the SAME env var and `cargo test`
        // runs tests in parallel threads by default, so two such tests
        // race each other. Sequencing both cases in one test removes
        // the race entirely.
        // SAFETY: test-only, single-threaded within this process's test
        // harness invocation for this var name.
        unsafe { std::env::set_var(CODEX_HOME_ENV, "/should/not/appear") };
        let adapter = CodexAdapter;
        let roots = adapter.scan_roots(Path::new("/home/example"), false);
        assert!(!roots.iter().any(|r| r.starts_with("/should/not/appear")), "use_env_roots=false must ignore CODEX_HOME");

        unsafe { std::env::set_var(CODEX_HOME_ENV, "/custom/codex-home") };
        let roots = adapter.scan_roots(Path::new("/home/example"), true);
        assert!(roots.contains(&PathBuf::from("/custom/codex-home/sessions")));
        assert!(roots.contains(&PathBuf::from("/custom/codex-home/archived_sessions")));

        unsafe { std::env::remove_var(CODEX_HOME_ENV) };
    }

    /// Live+archived union both ingest — a session under
    /// `sessions/` and a ROTATED session under `archived_sessions/`
    /// (Codex CLI's own rotation behavior, design D5) are both scanned
    /// and parsed as one logical `codex` source.
    #[test]
    fn live_and_archived_sessions_are_both_scanned_and_parsed() {
        let home = tempfile::tempdir().unwrap();
        let live_dir = home.path().join(".codex/sessions");
        let archived_dir = home.path().join(".codex/archived_sessions");
        std::fs::create_dir_all(&live_dir).unwrap();
        std::fs::create_dir_all(&archived_dir).unwrap();

        let live_content = concat!(
            r#"{"timestamp":"2026-02-01T00:00:00Z","type":"session_meta","payload":{"source":"interactive","model_provider":"openai","cwd":"/repo/live"}}"#,
            "\n",
            r#"{"timestamp":"2026-02-01T00:00:01Z","type":"turn_context","payload":{"model":"gpt-5.1"}}"#,
            "\n",
            r#"{"timestamp":"2026-02-01T00:00:02Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":12,"output_tokens":4}}}}"#,
            "\n",
        );
        std::fs::write(live_dir.join("live-session.jsonl"), live_content).unwrap();

        // A rotated session Codex CLI moved out of the live directory —
        // same JSONL schema, just a different root.
        let archived_content = concat!(
            r#"{"timestamp":"2025-12-01T00:00:00Z","type":"session_meta","payload":{"source":"interactive","model_provider":"openai","cwd":"/repo/archived"}}"#,
            "\n",
            r#"{"timestamp":"2025-12-01T00:00:01Z","type":"turn_context","payload":{"model":"gpt-4.1"}}"#,
            "\n",
            r#"{"timestamp":"2025-12-01T00:00:02Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":40,"output_tokens":11}}}}"#,
            "\n",
        );
        std::fs::write(archived_dir.join("rotated-session.jsonl"), archived_content).unwrap();

        let adapter = CodexAdapter;
        let roots = adapter.scan_roots(home.path(), false);
        let files = crate::scanner::scan_roots(&roots, |p| p.file_name().and_then(|n| n.to_str()).is_some_and(|n| n.ends_with(".jsonl")));
        assert_eq!(files.len(), 2);

        let rows: Vec<UnifiedRow> = files.iter().flat_map(|path| adapter.parse(path).rows).collect();
        assert_eq!(rows.len(), 2);
        assert!(rows.iter().any(|r| r.session_id == "live-session" && r.workspace_key.as_deref() == Some("/repo/live")));
        assert!(rows.iter().any(|r| r.session_id == "rotated-session" && r.workspace_key.as_deref() == Some("/repo/archived")));
    }

    #[test]
    fn missing_provider_is_inferred_from_model_name() {
        let content = concat!(
            r#"{"timestamp":"2026-01-01T00:00:00Z","type":"turn_context","payload":{"model":"claude-opus-4"}}"#,
            "\n",
            r#"{"timestamp":"2026-01-01T00:00:01Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":1,"output_tokens":1}}}}"#,
        );
        let file = write_fixture(content);
        let rows = parse_codex_file(file.path()).rows;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].provider_id, "anthropic");
    }

    #[test]
    fn human_user_message_marks_the_next_token_count_row_as_turn_start() {
        let content = concat!(
            r#"{"timestamp":"2026-01-01T00:00:00Z","type":"session_meta","payload":{"source":"interactive","model_provider":"openai","cwd":"/repo/proj"}}"#,
            "\n",
            r#"{"timestamp":"2026-01-01T00:00:01Z","type":"turn_context","payload":{"model":"gpt-5.1"}}"#,
            "\n",
            r#"{"timestamp":"2026-01-01T00:00:02Z","type":"event_msg","payload":{"type":"user_message","message":"how do I center a div?"}}"#,
            "\n",
            r#"{"timestamp":"2026-01-01T00:00:03Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":10,"output_tokens":3}}}}"#,
            "\n",
        );
        let file = write_fixture(content);
        let outcome = parse_codex_file(file.path());
        assert_eq!(outcome.rows.len(), 1);
        assert!(outcome.rows[0].is_turn_start);
        assert_eq!(outcome.directives.len(), 1);
        assert_eq!(outcome.directives[0].text, "how do I center a div?");
        assert_eq!(outcome.directives[0].client, "codex");
        assert_eq!(outcome.directives[0].workspace_key.as_deref(), Some("/repo/proj"));
        let expected_ts = chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:02Z").unwrap().timestamp_millis();
        assert_eq!(outcome.directives[0].timestamp_ms, expected_ts);
    }
    #[test]
    fn system_injected_user_message_does_not_mark_turn_start() {
        let content = concat!(
            r#"{"timestamp":"2026-01-01T00:00:00Z","type":"session_meta","payload":{"source":"interactive","model_provider":"openai","cwd":"/repo/proj"}}"#,
            "\n",
            r#"{"timestamp":"2026-01-01T00:00:01Z","type":"turn_context","payload":{"model":"gpt-5.1"}}"#,
            "\n",
            r#"{"timestamp":"2026-01-01T00:00:02Z","type":"event_msg","payload":{"type":"user_message","message":"<environment_context>cwd=/tmp</environment_context>"}}"#,
            "\n",
            r#"{"timestamp":"2026-01-01T00:00:03Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":10,"output_tokens":3}}}}"#,
            "\n",
        );
        let file = write_fixture(content);
        let outcome = parse_codex_file(file.path());
        assert_eq!(outcome.rows.len(), 1);
        assert!(!outcome.rows[0].is_turn_start);
        assert!(outcome.directives.is_empty(), "system-injected context must never produce a directive either");
    }

    #[test]
    fn interleaved_user_messages_emit_directives_in_order() {
        let content = concat!(
            r#"{"timestamp":"2026-01-01T00:00:00Z","type":"session_meta","payload":{"source":"interactive","model_provider":"openai","cwd":"/repo/proj"}}"#,
            "\n",
            r#"{"timestamp":"2026-01-01T00:00:01Z","type":"turn_context","payload":{"model":"gpt-5.1"}}"#,
            "\n",
            r#"{"timestamp":"2026-01-01T00:00:02Z","type":"event_msg","payload":{"type":"user_message","message":"first ask"}}"#,
            "\n",
            r#"{"timestamp":"2026-01-01T00:00:03Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":10,"output_tokens":3}}}}"#,
            "\n",
            r#"{"timestamp":"2026-01-01T00:00:04Z","type":"event_msg","payload":{"type":"user_message","message":"second ask"}}"#,
            "\n",
            r#"{"timestamp":"2026-01-01T00:00:05Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":25,"output_tokens":9}}}}"#,
            "\n",
        );
        let file = write_fixture(content);
        let outcome = parse_codex_file(file.path());

        assert_eq!(outcome.rows.len(), 2);
        assert_eq!(outcome.directives.len(), 2);
        assert_eq!(outcome.directives[0].text, "first ask");
        assert_eq!(outcome.directives[1].text, "second ask");
        assert!(outcome.directives[0].timestamp_ms < outcome.directives[1].timestamp_ms);
    }

    /// s31 D4's digest-dedup invariant at the adapter layer: re-parsing
    /// a GROWN fixture must re-emit byte-identical earlier directives.
    #[test]
    fn reparsing_a_grown_fixture_reemits_byte_identical_earlier_directives() {
        let base = concat!(
            r#"{"timestamp":"2026-01-01T00:00:00Z","type":"session_meta","payload":{"source":"interactive","model_provider":"openai","cwd":"/repo/proj"}}"#,
            "\n",
            r#"{"timestamp":"2026-01-01T00:00:01Z","type":"turn_context","payload":{"model":"gpt-5.1"}}"#,
            "\n",
            r#"{"timestamp":"2026-01-01T00:00:02Z","type":"event_msg","payload":{"type":"user_message","message":"first ask"}}"#,
            "\n",
            r#"{"timestamp":"2026-01-01T00:00:03Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":10,"output_tokens":3}}}}"#,
        );
        let first_file = write_fixture(base);
        let first = parse_codex_file(first_file.path());
        assert_eq!(first.directives.len(), 1);

        let grown = format!(
            "{base}\n{}\n{}",
            r#"{"timestamp":"2026-01-01T00:00:04Z","type":"event_msg","payload":{"type":"user_message","message":"second ask"}}"#,
            r#"{"timestamp":"2026-01-01T00:00:05Z","type":"event_msg","payload":{"type":"token_count","info":{"total_token_usage":{"input_tokens":25,"output_tokens":9}}}}"#,
        );
        let grown_file = write_fixture(&grown);
        let second = parse_codex_file(grown_file.path());
        assert_eq!(second.directives.len(), 2);

        assert_eq!(first.directives[0].text, second.directives[0].text);
        assert_eq!(first.directives[0].timestamp_ms, second.directives[0].timestamp_ms);
        assert_eq!(first.directives[0].workspace_key, second.directives[0].workspace_key);
        assert_eq!(second.directives[1].text, "second ask");
    }
}
