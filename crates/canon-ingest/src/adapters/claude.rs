//! Claude Code session adapter (S3 Wave 2) — parses
//! `~/.claude/projects/<sanitized-cwd>/*.jsonl` transcripts.
//!
//! ATTRIBUTED PORT (operator directive 2026-07-10, superseding this
//! surface's earlier "clean-room" recommendation) of the donor's
//! Claude Code parser.
//! This adapter ports FIVE units the S3 Wave 2 task + this fix scoped
//! in — each carries its own `Ported from …:LN-LN` comment at its
//! definition below:
//!
//!   1. the reused-buffer streaming JSONL scan loop (`claudecode.rs:393-433`)
//!   2. filename-stem `session_id` (`claudecode.rs:370-374`) with the
//!      sidechain-parent `sessionId` override (`claudecode.rs:436-442`)
//!   3. the 3/5/2-window path-component workspace scan
//!      (`claudecode.rs:674-705`, `claude_workspace_from_path`)
//!   4. the streaming-duplicate dedup-key + per-field-max token merge
//!      (`claudecode.rs:528-573` dedup-key construction,
//!      `claudecode.rs:789-826` `merge_claude_duplicate`)
//!
//! **Unit 5 (ReviewS3Full finding 1, cost-parity fix):** the `user`/
//! `tool_result` billable-token path — `entry.entry_type == "user" ||
//! "tool_result"` routing (`claudecode.rs:452-496`) and
//! `extract_claude_tool_result_message` + its tool-result-block
//! walk/token-estimate helpers (`claudecode.rs:857-1089`). Every
//! non-`assistant` line was previously dropped before its usage was
//! ever inspected, undercounting sessions with `tool_result` records
//! (a `user` turn's tool output, billed as input tokens on the NEXT
//! model call) — this port closes that gap, trimmed of the donor's
//! provider-hint ladder/sidechain-agent-name fields (still excluded,
//! see below): model comes from the already-typed
//! `entry.message.model` (never a raw-JSON top-level fallback), and
//! `provider_id` is always `DEFAULT_PROVIDER_ID` like every other row
//! this adapter emits.
//!
//! Deliberately NOT ported in this Wave 2 slice (the donor's full
//! feature inventory covers several units, none of which this task's Change
//! section calls out): the headless `--output-format json` fast path
//! (`claudecode.rs:378-391,1108-1249`), subagent
//! display-name resolution (`resolve_subagent_name`,
//! `claudecode.rs:89-129` — `UnifiedRow` has no `agent` field to carry
//! it), the `cc-mirror` wrapper's variant-metadata lookup
//! (`claudecode.rs:750-776` — its bare path-window shape is still
//! ported as part of unit 3 above, verbatim, since skipping it would
//! mean re-deriving `claude_workspace_from_path` rather than porting
//! it), the multi-tier provider-confidence merge
//! (`claudecode.rs:1309-1406`), and `is_turn_start`/`duration_ms`
//! derivation (`claudecode.rs:409-411,469-476,619-623,806-824`) —
//! including the `user`-entry `is_human_turn` turn-start marker unit 5
//! also touches upstream of (`claudecode.rs:474-476`) — stay excluded;
//! this fix ports the TOKEN path only, not the turn-boundary one.
//! `provider_id` defaults to Claude Code's own backend (`"anthropic"`)
//! rather than porting `claude_provider_choice`'s hint/confidence
//! ladder.
//!
//! **s31 design D4 (user-directive capture, added alongside the five
//! ATTRIBUTED-PORT units above — no donor line range, the donor never
//! modeled directive text):** every `entry_type == "user"` line's
//! `message.content` may carry a plain string or an array of typed
//! blocks (a real human turn's `{"type":"text","text":"…"}`
//! interleaved with e.g. `tool_result` blocks the SAME line's
//! billable-token path (unit 5) already walks independently).
//! [`flatten_claude_content`] concatenates every text block and skips
//! every non-text block; a tool_result-only line (no text block at
//! all) yields no directive, same "ordinary filtering" rule unit 5's
//! own `None`-on-no-tool_result path already uses. This runs on EVERY
//! `user` line regardless of `isSidechain` — a subagent's own prompt
//! is still a directive, resolved to the SAME parent `session_id`
//! unit 2's sidechain override already attributes its billable rows
//! to.

use std::collections::{HashMap, HashSet};
use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

use crate::adapter::{CostSource, DirectiveRow, ParseOutcome, SessionAdapter, TokenBreakdown, UnifiedRow};
use crate::normalize::{normalize_workspace_key, workspace_label_from_key};

/// Env var consulted only when `use_env_roots` is `true` (mirrors
/// `adapters::omp::EXTRA_SESSIONS_DIR_ENV`'s precedent) — an
/// additional scan root unioned alongside the default
/// `~/.claude/projects` root. Never consulted when `use_env_roots` is
/// `false`, so a deterministic test never picks up an ambient value
/// from the calling shell.
pub const EXTRA_SESSIONS_DIR_ENV: &str = "CANON_INGEST_CLAUDE_SESSIONS_DIR";

/// Claude Code's own backend — the `provider_id` this port uses
/// unconditionally, since the donor's hint/confidence provider ladder
/// (`claudecode.rs:1309-1406`) is out of scope (module doc above).
const DEFAULT_PROVIDER_ID: &str = "anthropic";

pub struct ClaudeCodeAdapter;

/// Claude Code JSONL entry (one line). Ported from `claudecode.rs:22-43`
/// (`ClaudeEntry`), trimmed to the fields this port's scoped units
/// need: `request_id`/`is_sidechain`/`session_id` for the sidechain
/// override (unit 2) and dedup-key construction (unit 4).
#[derive(Debug, Deserialize)]
struct ClaudeEntry {
    #[serde(rename = "type")]
    entry_type: String,
    timestamp: Option<String>,
    message: Option<ClaudeMessage>,
    /// Request ID for deduplication (used with `message.id`).
    #[serde(rename = "requestId")]
    request_id: Option<String>,
    /// True for subagent (sidechain) transcript lines.
    #[serde(rename = "isSidechain", default)]
    is_sidechain: bool,
    /// Parent session UUID (present on every sidechain line).
    #[serde(rename = "sessionId")]
    session_id: Option<String>,
}

/// Ported from `claudecode.rs:65-74` (`ClaudeMessage`), trimmed to the
/// fields the assistant-message/dedup path uses, extended with
/// `content` (s31 D4 — the donor's own unified row never modeled a
/// user turn's text at all, so this field has no donor line range to
/// cite).
#[derive(Debug, Deserialize)]
struct ClaudeMessage {
    model: Option<String>,
    usage: Option<ClaudeUsage>,
    /// Message ID for deduplication (used with `requestId`).
    id: Option<String>,
    /// A plain string, or an array of typed content blocks — see the
    /// module doc's D4 paragraph and [`flatten_claude_content`].
    content: Option<Value>,
}

/// Ported from `claudecode.rs:76-82` (`ClaudeUsage`).
#[derive(Debug, Deserialize)]
struct ClaudeUsage {
    input_tokens: Option<i64>,
    output_tokens: Option<i64>,
    cache_read_input_tokens: Option<i64>,
    cache_creation_input_tokens: Option<i64>,
}

impl SessionAdapter for ClaudeCodeAdapter {
    fn client_id(&self) -> &'static str {
        "claude-code"
    }

    fn scan_roots(&self, home: &Path, use_env_roots: bool) -> Vec<PathBuf> {
        let mut roots = vec![home.join(".claude/projects")];

        if use_env_roots {
            if let Ok(extra) = std::env::var(EXTRA_SESSIONS_DIR_ENV) {
                if !extra.trim().is_empty() {
                    roots.push(PathBuf::from(extra));
                }
            }
        }

        roots
    }

    fn parse(&self, path: &Path) -> ParseOutcome {
        parse_claude_file(path)
    }
}

/// Parse one Claude Code JSONL transcript. Ported from
/// `claudecode.rs:351-433` (`parse_claude_file_with_cache_and_home`):
/// the filename-stem `session_id` seed (`370-374`) and the file-open +
/// reused-buffer `BufReader::lines()` loop shape (`393-433`). The
/// headless-JSON fast path (`378-391`) and workflow-journal guard
/// (`356-359`) are excluded donor features (module doc above).
fn parse_claude_file(path: &Path) -> ParseOutcome {
    let (workspace_key, workspace_label) = claude_workspace_from_path(path);

    // Filename-stem session_id, ported from `claudecode.rs:370-374` —
    // overridden below by the sidechain parent `sessionId`, ported
    // from `claudecode.rs:436-442`, once the first parseable line is
    // seen.
    let mut session_id = path.file_stem().and_then(|s| s.to_str()).unwrap_or("unknown").to_string();

    let fallback_timestamp = file_modified_timestamp_ms(path);

    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return ParseOutcome::default(),
    };

    let reader = BufReader::new(file);
    let mut rows: Vec<UnifiedRow> = Vec::with_capacity(64);
    let mut directives: Vec<DirectiveRow> = Vec::new();
    let mut skipped = 0usize;
    // Maps dedup_key -> index in `rows` of the first occurrence.
    // Ported from `claudecode.rs:401-406` (`processed_hashes`): Claude
    // Code's streaming API writes the same messageId:requestId
    // multiple times as the response streams in, each write carrying
    // more complete token counts than the last. SHARED across the
    // assistant-message path (unit 4) and the `tool_result` path
    // (unit 5) — ported from `claudecode.rs:406,479-489`, one map for
    // both, since the two key shapes (`"message:…"`/`"…:…"` vs.
    // `"claude-code:tool_result:…"`) never collide.
    let mut processed_hashes: HashMap<String, usize> = HashMap::new();
    let mut buffer = Vec::with_capacity(4096);
    // Sidechain detection is resolved lazily on the first parseable
    // entry, ported from `claudecode.rs:415-417,434-450`.
    let mut sidechain_detected = false;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        // Per-line skip-on-parse-failure, ported behavior from
        // `claudecode.rs:433`'s `if let Ok(entry) = …` gate: a
        // corrupt/truncated line never aborts an otherwise-good
        // transcript — it IS counted (Wave-2 amendment) rather than
        // vanishing silently.
        buffer.clear();
        buffer.extend_from_slice(trimmed.as_bytes());
        let Ok(entry) = simd_json::from_slice::<ClaudeEntry>(&mut buffer) else {
            skipped += 1;
            continue;
        };

        // Sidechain-parent session_id override, ported from
        // `claudecode.rs:434-450`: every line of a subagent transcript
        // carries `isSidechain: true`, so this is checked once, on the
        // first parseable entry of ANY type — the assistant-only
        // filter below happens after this check, matching the donor's
        // own ordering (`434` precedes `452`/`499`).
        if !sidechain_detected {
            sidechain_detected = true;
            if entry.is_sidechain {
                if let Some(parent_id) = entry.session_id.clone() {
                    session_id = parent_id;
                }
            }
        }

        // Unit 5 (ReviewS3Full finding 1): `user`/`tool_result` lines
        // carry billable input tokens (a tool's output, billed on the
        // next model call) that a naive assistant-only filter drops
        // entirely. Ported from `claudecode.rs:452-496`'s routing —
        // extraction returning `None` (a plain-text `user` turn with
        // no tool_result block at all) is ordinary, not malformed, so
        // it is NOT counted as skipped.
        //
        // s31 D4: the SAME line's `message.content` may also carry
        // real human text (a text block alongside/instead of a
        // tool_result block) — extracted independently of the
        // billable-token path above, since a line can be BOTH (a
        // human message with an inline tool_result) or EITHER alone.
        if entry.entry_type == "user" || entry.entry_type == "tool_result" {
            if let Some(text) = entry.message.as_ref().and_then(|m| flatten_claude_content(m.content.as_ref())) {
                let timestamp_ms = parse_claude_entry_timestamp(entry.timestamp.as_deref()).unwrap_or(fallback_timestamp);
                directives.push(DirectiveRow {
                    client: "claude-code".to_string(),
                    session_id: session_id.clone(),
                    timestamp_ms,
                    text,
                    workspace_key: workspace_key.clone(),
                    workspace_label: workspace_label.clone(),
                });
            }
            if let Some(row) = extract_claude_tool_result_row(trimmed, &entry, &session_id, fallback_timestamp, workspace_key.clone(), workspace_label.clone()) {
                match row.dedup_key.clone() {
                    Some(dedup_key) => match processed_hashes.get(&dedup_key) {
                        Some(&existing_idx) => merge_claude_tool_result_duplicate(&mut rows[existing_idx], row.tokens.input, row.timestamp_ms),
                        None => {
                            processed_hashes.insert(dedup_key, rows.len());
                            rows.push(row);
                        }
                    },
                    None => rows.push(row),
                }
            }
            continue;
        }

        // Only assistant messages carry the streaming per-turn usage
        // this unit-4 path reconciles.
        if entry.entry_type != "assistant" {
            continue;
        }

        let Some(message) = entry.message else { continue };
        let Some(usage) = message.usage else { continue };
        let Some(model) = message.model else { continue };

        let parsed_timestamp = parse_claude_entry_timestamp(entry.timestamp.as_deref());

        // Dedup-key construction + streaming-duplicate merge, ported
        // from `claudecode.rs:528-573` (`messageId:requestId` /
        // `message:messageId` composite key, looked up before every
        // push).
        let pending_hash = match (&message.id, &entry.request_id) {
            (Some(msg_id), Some(req_id)) => {
                let hash = format!("{msg_id}:{req_id}");
                if let Some(&existing_idx) = processed_hashes.get(&hash) {
                    merge_claude_duplicate(&mut rows[existing_idx], &usage, parsed_timestamp);
                    continue;
                }
                Some(hash)
            }
            (Some(msg_id), None) => {
                let hash = format!("message:{msg_id}");
                if let Some(&existing_idx) = processed_hashes.get(&hash) {
                    merge_claude_duplicate(&mut rows[existing_idx], &usage, parsed_timestamp);
                    continue;
                }
                Some(hash)
            }
            _ => None,
        };

        let timestamp_ms = parsed_timestamp.unwrap_or(fallback_timestamp);

        // Insert the dedup index only after all checks pass, right
        // before push — ported from `claudecode.rs:596-598`.
        let dedup_key = pending_hash.inspect(|hash| {
            processed_hashes.insert(hash.clone(), rows.len());
        });

        rows.push(UnifiedRow {
            client: "claude-code".to_string(),
            model_id: model,
            provider_id: DEFAULT_PROVIDER_ID.to_string(),
            session_id: session_id.clone(),
            workspace_key: workspace_key.clone(),
            workspace_label: workspace_label.clone(),
            timestamp_ms,
            tokens: TokenBreakdown {
                input: usage.input_tokens.unwrap_or(0).max(0),
                output: usage.output_tokens.unwrap_or(0).max(0),
                cache_read: usage.cache_read_input_tokens.unwrap_or(0).max(0),
                cache_write: usage.cache_creation_input_tokens.unwrap_or(0).max(0),
                reasoning: 0,
            },
            cost: 0.0,
            cost_source: CostSource::Unknown,
            // Excluded donor feature — see module doc.
            duration_ms: None,
            dedup_key,
            // Excluded donor feature — see module doc.
            is_turn_start: false,
        });
    }

    ParseOutcome::with_directives(rows, skipped, directives)
}

/// Flatten a `user`/`tool_result` line's `message.content` into a
/// verbatim directive string (s31 design D4) — see the module doc's
/// D4 paragraph. Same string-or-typed-block-array shape and the same
/// text-block-concatenation/non-text-block-skip rule as
/// `adapters::omp::flatten_pi_content` (no shared crate-wide utils
/// module exists yet — see that function's own doc comment for the
/// established precedent every adapter in this crate follows).
fn flatten_claude_content(content: Option<&Value>) -> Option<String> {
    let text = match content? {
        Value::String(s) => s.clone(),
        Value::Array(blocks) => blocks
            .iter()
            .filter_map(|block| {
                if block.get("type").and_then(Value::as_str) != Some("text") {
                    return None;
                }
                block.get("text").and_then(Value::as_str)
            })
            .collect::<Vec<_>>()
            .join("\n"),
        _ => return None,
    };
    if text.is_empty() { None } else { Some(text) }
}

/// One `user`/`tool_result` line's extracted billable-token usage.
/// Ported from `claudecode.rs:839-842` (`ClaudeToolResultUsage`).
struct ClaudeToolResultUsage {
    input_tokens: i64,
    dedup_key: Option<String>,
}

/// Extract a billable-token row from a `user`/`tool_result` JSONL
/// line, or `None` when it carries no tool_result content (a plain
/// human message) or no positive token count. Ported from
/// `claudecode.rs:857-917` (`extract_claude_tool_result_message`),
/// trimmed of the provider-hint ladder / `last_model` fallback /
/// sidechain-agent-name fields this port's scope excludes (module doc
/// above).
fn extract_claude_tool_result_row(trimmed: &str, entry: &ClaudeEntry, session_id: &str, fallback_timestamp: i64, workspace_key: Option<String>, workspace_label: Option<String>) -> Option<UnifiedRow> {
    let value: Value = serde_json::from_str(trimmed).ok()?;
    let usage = extract_claude_tool_result_usage(&value)?;

    let model = entry.message.as_ref().and_then(|m| m.model.clone()).unwrap_or_else(|| "unknown".to_string());
    let timestamp_ms = parse_claude_entry_timestamp(entry.timestamp.as_deref()).unwrap_or(fallback_timestamp);

    Some(UnifiedRow {
        client: "claude-code".to_string(),
        model_id: model,
        provider_id: DEFAULT_PROVIDER_ID.to_string(),
        session_id: session_id.to_string(),
        workspace_key,
        workspace_label,
        timestamp_ms,
        tokens: TokenBreakdown { input: usage.input_tokens, output: 0, cache_read: 0, cache_write: 0, reasoning: 0 },
        cost: 0.0,
        cost_source: CostSource::Unknown,
        duration_ms: None,
        // Ported from `claudecode.rs:906-911`: client-id/session-id
        // scoped so a `tool_result` dedup id never collides with an
        // assistant-message dedup key.
        dedup_key: usage.dedup_key.map(|key| format!("claude-code:tool_result:{session_id}:{key}")),
        is_turn_start: false,
    })
}

/// Sum every `tool_result` block's input-token estimate in one
/// `user`/`tool_result` JSONL line, deduping repeated `tool_use_id`s
/// within the SAME line. Ported from `claudecode.rs:919-945`
/// (`extract_claude_tool_result_usage`).
fn extract_claude_tool_result_usage(value: &Value) -> Option<ClaudeToolResultUsage> {
    let mut total_tokens: i64 = 0;
    let mut first_dedup_id: Option<String> = None;
    let mut seen_ids: HashSet<String> = HashSet::new();

    for tool_result in claude_tool_result_values(value) {
        let tool_result_id = extract_tool_result_id(tool_result);
        if let Some(id) = tool_result_id.as_ref() {
            if !seen_ids.insert(id.clone()) {
                continue;
            }
        }
        if first_dedup_id.is_none() {
            first_dedup_id = tool_result_id;
        }
        total_tokens += extract_tool_result_input_tokens(tool_result).unwrap_or(0);
    }

    if total_tokens <= 0 {
        return None;
    }

    Some(ClaudeToolResultUsage { input_tokens: total_tokens, dedup_key: first_dedup_id.map(|id| format!("tool_result:{id}")) })
}

/// Collect every `tool_result`-shaped value reachable from a
/// `user`/`tool_result` JSONL line — a bare top-level `tool_result`
/// entry, a `tool_result`/`message.tool_result` field, or a
/// `tool_result`-typed block inside `message.content`/`content`.
/// Ported from `claudecode.rs:947-978` (`claude_tool_result_values`).
fn claude_tool_result_values(value: &Value) -> Vec<&Value> {
    let mut results = Vec::new();

    if value.get("type").and_then(|kind| kind.as_str()).is_some_and(|kind| kind == "tool_result") {
        results.push(value);
    }

    if let Some(tool_result) = value.get("tool_result") {
        results.push(tool_result);
    }

    if let Some(message_tool_result) = value.get("message").and_then(|message| message.get("tool_result")) {
        results.push(message_tool_result);
    }

    if let Some(content) = value.get("message").and_then(|message| message.get("content")).or_else(|| value.get("content")) {
        collect_tool_result_blocks(content, &mut results);
    }

    results
}

/// Ported from `claudecode.rs:980-992` (`collect_tool_result_blocks`).
fn collect_tool_result_blocks<'a>(value: &'a Value, results: &mut Vec<&'a Value>) {
    if let Some(blocks) = value.as_array() {
        for block in blocks {
            if block.get("type").and_then(|kind| kind.as_str()).is_some_and(|kind| kind == "tool_result") {
                results.push(block);
            }
        }
    }
}

/// Ported from `claudecode.rs:994-998` (`extract_tool_result_id`).
fn extract_tool_result_id(tool_result: &Value) -> Option<String> {
    extract_string(tool_result.get("tool_use_id")).or_else(|| extract_string(tool_result.get("id"))).or_else(|| extract_string(tool_result.get("tool_result_id")))
}

/// Ported from `claudecode.rs:1000-1005` (`extract_tool_result_input_tokens`).
fn extract_tool_result_input_tokens(tool_result: &Value) -> Option<i64> {
    explicit_tool_result_input_tokens(tool_result).or_else(|| {
        let chars = tool_result_output_char_count(tool_result);
        (chars > 0).then(|| estimate_tokens_from_chars(chars))
    })
}

/// Ported from `claudecode.rs:1007-1034` (`explicit_tool_result_input_tokens`).
fn explicit_tool_result_input_tokens(tool_result: &Value) -> Option<i64> {
    for candidate in [
        tool_result.get("input_tokens"),
        tool_result.get("token_count"),
        tool_result.get("tokens"),
        tool_result.get("usage").and_then(|usage| usage.get("input_tokens")),
        tool_result.get("tool_output").and_then(|tool_output| tool_output.get("input_tokens")),
        tool_result.get("tool_output").and_then(|tool_output| tool_output.get("token_count")),
        tool_result.get("tool_output").and_then(|tool_output| tool_output.get("tokens")),
        tool_result.get("tool_output").and_then(|tool_output| tool_output.get("usage")).and_then(|usage| usage.get("input_tokens")),
    ] {
        if let Some(tokens) = extract_i64(candidate) {
            return Some(tokens.max(0));
        }
    }
    None
}

/// Ported from `claudecode.rs:1036-1062` (`tool_result_output_char_count`).
fn tool_result_output_char_count(tool_result: &Value) -> usize {
    let mut chars = 0;

    if let Some(output) = tool_result.get("tool_output").and_then(|tool_output| tool_output.get("output")).and_then(|output| output.as_str()) {
        chars += output.chars().count();
    }

    match tool_result.get("content") {
        Some(content) if content.is_string() => {
            chars += content.as_str().map(str::chars).map(Iterator::count).unwrap_or(0);
        }
        Some(content) => {
            chars += tool_result_content_output_chars(content);
        }
        None => {}
    }

    chars
}

/// Ported from `claudecode.rs:1064-1083` (`tool_result_content_output_chars`).
fn tool_result_content_output_chars(content: &Value) -> usize {
    content
        .as_array()
        .map(|blocks| {
            blocks
                .iter()
                .map(|block| {
                    block
                        .get("tool_output")
                        .and_then(|tool_output| tool_output.get("output"))
                        .and_then(|output| output.as_str())
                        .or_else(|| block.get("text").and_then(|text| text.as_str()))
                        .map(str::chars)
                        .map(Iterator::count)
                        .unwrap_or(0)
                })
                .sum()
        })
        .unwrap_or(0)
}

/// One token per four characters, rounded up — matches the donor's
/// own fallback for tool outputs that carry no explicit token
/// metadata. Ported from `claudecode.rs:1085-1089`
/// (`estimate_tokens_from_chars`).
fn estimate_tokens_from_chars(chars: usize) -> i64 {
    chars.div_ceil(4) as i64
}

/// Ported from the donor session-parser project's `extract_i64` — a
/// local copy scoped to this
/// adapter, same as `adapters::omp`'s own `contains_delimited`/
/// `inferred_provider_from_model` local-copy precedent (no shared
/// crate-wide `utils` module exists yet).
fn extract_i64(value: Option<&Value>) -> Option<i64> {
    value.and_then(|val| val.as_i64().or_else(|| val.as_u64().map(|v| v as i64)).or_else(|| val.as_str().and_then(|s| s.parse::<i64>().ok())))
}

/// Ported from the donor session-parser project's `extract_string`.
fn extract_string(value: Option<&Value>) -> Option<String> {
    value.and_then(|val| val.as_str().map(|s| s.to_string()))
}

/// Per-`tool_result`-line duplicate merge (mirrors unit 4's
/// `merge_claude_duplicate` shape, but for the single `input`-tokens
/// field a tool_result row carries). Ported from
/// `claudecode.rs:828-837` (`merge_claude_tool_result_duplicate`).
fn merge_claude_tool_result_duplicate(existing: &mut UnifiedRow, input_tokens: i64, timestamp_ms: i64) {
    existing.tokens.input = existing.tokens.input.max(input_tokens.max(0));
    if timestamp_ms >= existing.timestamp_ms {
        existing.timestamp_ms = timestamp_ms;
    }
}

/// Per-field max merge across a streaming-duplicate write. Ported
/// from `claudecode.rs:789-826` (`merge_claude_duplicate`), minus the
/// request-start-timestamp duration recovery (`806-824`): that logic
/// depends on the excluded `user`-entry `pending_request_start_timestamp_ms`
/// tracking (module doc above), so this port keeps the token-max-merge
/// core (`795-804`) and a plain newer-timestamp update in its place.
fn merge_claude_duplicate(existing: &mut UnifiedRow, usage: &ClaudeUsage, parsed_timestamp: Option<i64>) {
    let t = &mut existing.tokens;
    t.input = t.input.max(usage.input_tokens.unwrap_or(0).max(0));
    t.output = t.output.max(usage.output_tokens.unwrap_or(0).max(0));
    t.cache_read = t.cache_read.max(usage.cache_read_input_tokens.unwrap_or(0).max(0));
    t.cache_write = t.cache_write.max(usage.cache_creation_input_tokens.unwrap_or(0).max(0));

    if let Some(timestamp_ms) = parsed_timestamp {
        if timestamp_ms >= existing.timestamp_ms {
            existing.timestamp_ms = timestamp_ms;
        }
    }
}

/// Ported from `claudecode.rs:674-705` (`claude_workspace_from_path`)
/// — a 3-window scan for `.claude/projects/<key>`, a 5-window scan for
/// `.cc-mirror/…/config/projects/<key>` (the cc-mirror wrapper's own
/// variant-metadata *lookup*, `claudecode.rs:750-776`, is excluded —
/// module doc above — but this bare path-window branch is ported
/// unmodified since it is this same function, not a separate unit),
/// and a final 2-window fallback for a bare trailing `projects/<key>`.
fn claude_workspace_from_path(path: &Path) -> (Option<String>, Option<String>) {
    let components: Vec<String> = path.components().map(|component| component.as_os_str().to_string_lossy().to_string()).collect();

    for window in components.windows(3) {
        if window[0] == ".claude" && window[1] == "projects" {
            let key = normalize_workspace_key(&window[2]);
            let label = key.as_deref().and_then(workspace_label_from_key);
            return (key, label);
        }
    }

    for window in components.windows(5) {
        if window[0] == ".cc-mirror" && window[2] == "config" && window[3] == "projects" {
            let key = normalize_workspace_key(&window[4]);
            let label = key.as_deref().and_then(workspace_label_from_key);
            return (key, label);
        }
    }

    for window in components.windows(2).rev() {
        if window[0] == "projects" {
            let key = normalize_workspace_key(&window[1]);
            let label = key.as_deref().and_then(workspace_label_from_key);
            return (key, label);
        }
    }

    (None, None)
}

/// Ported from the donor session-parser project
/// (the same helper `adapters::omp::file_modified_timestamp_ms` already
/// ports independently — no shared `utils` module exists yet in this
/// crate, so each adapter carries its own private copy, matching Wave
/// 1's precedent).
fn file_modified_timestamp_ms(path: &Path) -> i64 {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(std::time::SystemTime::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_else(|| chrono::Utc::now().timestamp_millis())
}

/// Ported from `claudecode.rs:778-782` (`parse_claude_entry_timestamp`).
fn parse_claude_entry_timestamp(timestamp: Option<&str>) -> Option<i64> {
    timestamp.and_then(|ts| chrono::DateTime::parse_from_rfc3339(ts).ok()).map(|dt| dt.timestamp_millis())
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
    fn parses_valid_assistant_message_with_filename_stem_session_id() {
        let content = r#"{"type":"user","message":{"role":"user"}}
{"type":"assistant","timestamp":"2026-01-01T00:00:01.000Z","requestId":"req_a","message":{"id":"msg_a","model":"claude-sonnet-5","usage":{"input_tokens":100,"output_tokens":50,"cache_read_input_tokens":10,"cache_creation_input_tokens":5}}}"#;
        let file = write_fixture(content);

        let outcome = parse_claude_file(file.path());
        let rows = outcome.rows;

        assert_eq!(outcome.skipped, 0);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].client, "claude-code");
        assert_eq!(rows[0].model_id, "claude-sonnet-5");
        assert_eq!(rows[0].provider_id, "anthropic");
        assert_eq!(rows[0].session_id, file.path().file_stem().unwrap().to_string_lossy());
        assert_eq!(rows[0].tokens.input, 100);
        assert_eq!(rows[0].tokens.output, 50);
        assert_eq!(rows[0].tokens.cache_read, 10);
        assert_eq!(rows[0].tokens.cache_write, 5);
        assert_eq!(rows[0].dedup_key, Some("msg_a:req_a".to_string()));
    }

    #[test]
    fn streaming_duplicate_merge_keeps_per_field_max_tokens_and_latest_timestamp() {
        // Same messageId:requestId written twice — the second write's
        // token counts are strictly larger, mirroring Claude Code's
        // streaming API completing a response over multiple lines.
        let content = r#"{"type":"assistant","timestamp":"2026-01-01T00:00:01.000Z","requestId":"req_b","message":{"id":"msg_b","model":"claude-sonnet-5","usage":{"input_tokens":10,"output_tokens":5,"cache_read_input_tokens":0,"cache_creation_input_tokens":0}}}
{"type":"assistant","timestamp":"2026-01-01T00:00:02.500Z","requestId":"req_b","message":{"id":"msg_b","model":"claude-sonnet-5","usage":{"input_tokens":10,"output_tokens":180,"cache_read_input_tokens":5,"cache_creation_input_tokens":0}}}"#;
        let file = write_fixture(content);

        let rows = parse_claude_file(file.path()).rows;

        assert_eq!(rows.len(), 1, "the two streaming duplicate lines must merge into one row: {rows:?}");
        assert_eq!(rows[0].tokens.input, 10);
        assert_eq!(rows[0].tokens.output, 180, "per-field max keeps the larger of the two writes");
        assert_eq!(rows[0].tokens.cache_read, 5);
        assert_eq!(rows[0].dedup_key, Some("msg_b:req_b".to_string()));
        let expected_ts = chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:02.500Z").unwrap().timestamp_millis();
        assert_eq!(rows[0].timestamp_ms, expected_ts, "timestamp advances to the later write");
    }

    #[test]
    fn message_only_dedup_key_used_when_request_id_is_absent() {
        let content = r#"{"type":"assistant","timestamp":"2026-01-01T00:00:01.000Z","message":{"id":"msg_c","model":"claude-sonnet-5","usage":{"input_tokens":1,"output_tokens":1,"cache_read_input_tokens":0,"cache_creation_input_tokens":0}}}
{"type":"assistant","timestamp":"2026-01-01T00:00:02.000Z","message":{"id":"msg_c","model":"claude-sonnet-5","usage":{"input_tokens":1,"output_tokens":9,"cache_read_input_tokens":0,"cache_creation_input_tokens":0}}}"#;
        let file = write_fixture(content);

        let rows = parse_claude_file(file.path()).rows;

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].dedup_key, Some("message:msg_c".to_string()));
        assert_eq!(rows[0].tokens.output, 9);
    }

    #[test]
    fn sidechain_entry_overrides_session_id_with_parent_session_id() {
        // The fixture's own tempfile stem is random/unrelated — proves
        // session_id resolves to the sidechain's parent `sessionId`
        // field, never the subagent transcript's own filename.
        let content = r#"{"type":"user","isSidechain":true,"sessionId":"parent-session-xyz","message":{"role":"user"}}
{"type":"assistant","isSidechain":true,"sessionId":"parent-session-xyz","timestamp":"2026-01-01T00:05:00.000Z","message":{"id":"msg_sub","model":"claude-sonnet-5","usage":{"input_tokens":3,"output_tokens":2,"cache_read_input_tokens":0,"cache_creation_input_tokens":0}}}"#;
        let file = write_fixture(content);

        let rows = parse_claude_file(file.path()).rows;

        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].session_id, "parent-session-xyz");
        assert_ne!(rows[0].session_id, file.path().file_stem().unwrap().to_string_lossy());
    }

    #[test]
    fn skips_malformed_json_lines_but_keeps_the_rest() {
        let content = r#"{"type":"assistant","timestamp":"2026-01-01T00:00:01.000Z","message":{"id":"msg_d","model":"claude-sonnet-5","usage":{"input_tokens":10,"output_tokens":5,"cache_read_input_tokens":0,"cache_creation_input_tokens":0}}}
not valid json at all
{"type":"assistant","timestamp":"2026-01-01T00:00:02.000Z","message":{"id":"msg_e","model":"claude-sonnet-5","usage":{"input_tokens":20,"output_tokens":8,"cache_read_input_tokens":0,"cache_creation_input_tokens":0}}}"#;
        let file = write_fixture(content);

        let outcome = parse_claude_file(file.path());
        let rows = outcome.rows;

        assert_eq!(outcome.skipped, 1, "the corrupt line must be COUNTED, not just silently skipped");
        assert_eq!(rows.len(), 2, "the corrupt middle line is skipped, both valid entries survive: {rows:?}");
        assert_eq!(rows[0].model_id, "claude-sonnet-5");
        assert_eq!(rows[1].tokens.input, 20);
    }

    /// ReviewS3Full finding 1 (CRITICAL, cost-parity): a `tool_result`
    /// block's output counts as billable INPUT tokens on the row it
    /// produces — previously dropped entirely because the assistant-
    /// only filter ran before any `user`/`tool_result` line was ever
    /// inspected. Fixture + expected token/dedup-key shape adapted
    /// verbatim from the donor's own
    /// `test_tool_result_output_counts_as_input`
    /// (`claudecode.rs:1957-1980`): a 16-character tool output with no
    /// explicit token count estimates to 4 tokens (`div_ceil(4)`).
    #[test]
    fn tool_result_output_counts_as_billable_input_tokens() {
        let content = r#"{"type":"user","timestamp":"2026-05-27T10:00:00.000Z","message":{"model":"claude-sonnet-5","content":[{"type":"tool_result","tool_use_id":"toolu_input","tool_output":{"output":"abcdefghijklmnop"}}]}}"#;
        let file = write_fixture(content);

        let outcome = parse_claude_file(file.path());
        let rows = outcome.rows;

        assert_eq!(outcome.skipped, 0, "a tool_result line is billable evidence, not malformed content");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].client, "claude-code");
        assert_eq!(rows[0].model_id, "claude-sonnet-5");
        assert_eq!(rows[0].provider_id, "anthropic");
        assert_eq!(rows[0].tokens.input, 4, "16 chars / 4 = 4 estimated tokens, matching the donor fixture");
        assert_eq!(rows[0].tokens.output, 0);
        assert_eq!(rows[0].tokens.cache_read, 0);
        assert_eq!(rows[0].tokens.cache_write, 0);
        let session_id = file.path().file_stem().unwrap().to_string_lossy();
        assert_eq!(rows[0].dedup_key.as_deref(), Some(format!("claude-code:tool_result:{session_id}:tool_result:toolu_input").as_str()));
    }

    /// A second `tool_result` line with the SAME `tool_use_id` merges
    /// into the first row (per-field max on `tokens.input`) rather
    /// than double-counting — mirrors unit 4's streaming-duplicate
    /// merge, ported from `claudecode.rs:478-489`.
    #[test]
    fn duplicate_tool_result_id_merges_by_max_input_tokens() {
        let content = concat!(
            r#"{"type":"user","timestamp":"2026-05-27T10:00:00.000Z","message":{"model":"claude-sonnet-5","content":[{"type":"tool_result","tool_use_id":"toolu_dup","tool_output":{"output":"abcd"}}]}}"#,
            "\n",
            r#"{"type":"user","timestamp":"2026-05-27T10:00:01.000Z","message":{"model":"claude-sonnet-5","content":[{"type":"tool_result","tool_use_id":"toolu_dup","tool_output":{"output":"abcdefghijklmnop"}}]}}"#,
        );
        let file = write_fixture(content);

        let rows = parse_claude_file(file.path()).rows;

        assert_eq!(rows.len(), 1, "the repeated tool_use_id must merge into one row, not double-count: {rows:?}");
        assert_eq!(rows[0].tokens.input, 4, "max across both writes (1 char/4 vs 16 chars/4)");
    }

    #[test]
    fn parsing_the_same_file_twice_is_idempotent() {
        let content = r#"{"type":"assistant","timestamp":"2026-01-01T00:00:01.000Z","requestId":"req_f","message":{"id":"msg_f","model":"claude-sonnet-5","usage":{"input_tokens":1,"output_tokens":1,"cache_read_input_tokens":0,"cache_creation_input_tokens":0}}}
{"type":"assistant","timestamp":"2026-01-01T00:00:02.000Z","requestId":"req_f","message":{"id":"msg_f","model":"claude-sonnet-5","usage":{"input_tokens":1,"output_tokens":9,"cache_read_input_tokens":0,"cache_creation_input_tokens":0}}}
not valid json at all"#;
        let file = write_fixture(content);

        let first = parse_claude_file(file.path());
        let second = parse_claude_file(file.path());

        assert_eq!(first, second, "re-parsing an unchanged file must yield byte-identical rows");
    }

    #[test]
    fn claude_workspace_from_path_matches_the_dot_claude_projects_window() {
        let path = Path::new("/home/example/.claude/projects/-Users-example-project/session-alpha.jsonl");
        let (key, label) = claude_workspace_from_path(path);
        assert_eq!(key, Some("-Users-example-project".to_string()));
        assert_eq!(label, Some("-Users-example-project".to_string()));
    }

    #[test]
    fn claude_workspace_from_path_falls_back_to_a_bare_trailing_projects_window() {
        let path = Path::new("/home/example/some/other/projects/my-key/session.jsonl");
        let (key, label) = claude_workspace_from_path(path);
        assert_eq!(key, Some("my-key".to_string()));
        assert_eq!(label, Some("my-key".to_string()));
    }

    #[test]
    fn claude_workspace_from_path_is_none_outside_any_known_layout() {
        let path = Path::new("/home/example/unrelated/session.jsonl");
        assert_eq!(claude_workspace_from_path(path), (None, None));
    }

    #[test]
    fn scan_roots_targets_dot_claude_projects() {
        let adapter = ClaudeCodeAdapter;
        let home = Path::new("/home/example");
        let roots = adapter.scan_roots(home, false);
        assert_eq!(roots, vec![home.join(".claude/projects")]);
    }

    #[test]
    fn scan_roots_ignores_env_override_when_use_env_roots_is_false() {
        // SAFETY: test-only, single-threaded within this process's
        // test harness invocation for this var name.
        unsafe { std::env::set_var(EXTRA_SESSIONS_DIR_ENV, "/should/not/appear") };
        let adapter = ClaudeCodeAdapter;
        let roots = adapter.scan_roots(Path::new("/home/example"), false);
        unsafe { std::env::remove_var(EXTRA_SESSIONS_DIR_ENV) };
        assert!(!roots.iter().any(|r| r == Path::new("/should/not/appear")));
    }

    #[test]
    fn interleaved_user_and_assistant_turns_emit_directives_in_order_with_verbatim_text() {
        let content = concat!(
            r#"{"type":"user","timestamp":"2026-01-01T00:00:01.000Z","message":{"role":"user","content":"first question"}}"#,
            "\n",
            r#"{"type":"assistant","timestamp":"2026-01-01T00:00:02.000Z","requestId":"req_x","message":{"id":"msg_x","model":"claude-sonnet-5","usage":{"input_tokens":10,"output_tokens":5,"cache_read_input_tokens":0,"cache_creation_input_tokens":0}}}"#,
            "\n",
            r#"{"type":"user","timestamp":"2026-01-01T00:00:03.000Z","message":{"role":"user","content":[{"type":"text","text":"second question"}]}}"#,
            "\n",
            r#"{"type":"assistant","timestamp":"2026-01-01T00:00:04.000Z","requestId":"req_y","message":{"id":"msg_y","model":"claude-sonnet-5","usage":{"input_tokens":20,"output_tokens":8,"cache_read_input_tokens":0,"cache_creation_input_tokens":0}}}"#,
        );
        let file = write_fixture(content);
        let outcome = parse_claude_file(file.path());

        assert_eq!(outcome.skipped, 0);
        assert_eq!(outcome.rows.len(), 2, "a plain user turn must never itself become a UnifiedRow");
        assert_eq!(outcome.directives.len(), 2);
        assert_eq!(outcome.directives[0].text, "first question", "a plain-string content must be used as-is, verbatim");
        assert_eq!(outcome.directives[1].text, "second question", "an array-of-blocks content must flatten to its text block, verbatim");
        assert!(outcome.directives[0].timestamp_ms < outcome.directives[1].timestamp_ms, "transcript order preserved");
        for directive in &outcome.directives {
            assert_eq!(directive.client, "claude-code");
            assert_eq!(directive.session_id, file.path().file_stem().unwrap().to_string_lossy());
        }
    }

    /// A line that carries BOTH a real text block and a `tool_result`
    /// block must produce BOTH a directive AND a billable row — the
    /// two extraction paths (unit 5's token path, D4's directive path)
    /// run independently over the same line.
    #[test]
    fn text_and_tool_result_blocks_on_the_same_line_produce_a_directive_and_a_billable_row() {
        let content = r#"{"type":"user","timestamp":"2026-05-27T10:00:00.000Z","message":{"model":"claude-sonnet-5","content":[{"type":"text","text":"here's the tool output"},{"type":"tool_result","tool_use_id":"toolu_mixed","tool_output":{"output":"abcdefghijklmnop"}}]}}"#;
        let file = write_fixture(content);
        let outcome = parse_claude_file(file.path());

        assert_eq!(outcome.directives.len(), 1);
        assert_eq!(outcome.directives[0].text, "here's the tool output", "the tool_result block is skipped by the flattener, only the text block survives");
        assert_eq!(outcome.rows.len(), 1, "the same line's tool_result block still produces its billable row");
        assert_eq!(outcome.rows[0].tokens.input, 4);
    }

    #[test]
    fn tool_result_only_line_emits_no_directive() {
        let content = r#"{"type":"user","timestamp":"2026-05-27T10:00:00.000Z","message":{"model":"claude-sonnet-5","content":[{"type":"tool_result","tool_use_id":"toolu_only","tool_output":{"output":"abcdefghijklmnop"}}]}}"#;
        let file = write_fixture(content);
        let outcome = parse_claude_file(file.path());

        assert!(outcome.directives.is_empty(), "a tool_result-only line carries no human text, so no directive");
        assert_eq!(outcome.rows.len(), 1);
    }

    /// A sidechain (subagent) transcript's own `user` entry is still a
    /// real directive — attributed to the SAME parent `session_id` its
    /// billable rows already resolve to (module doc's D4 paragraph).
    #[test]
    fn sidechain_user_entry_emits_a_directive_attributed_to_the_parent_session() {
        let content = r#"{"type":"user","isSidechain":true,"sessionId":"parent-session-xyz","message":{"role":"user","content":"investigate the flaky test"}}
{"type":"assistant","isSidechain":true,"sessionId":"parent-session-xyz","timestamp":"2026-01-01T00:05:00.000Z","message":{"id":"msg_sub","model":"claude-sonnet-5","usage":{"input_tokens":3,"output_tokens":2,"cache_read_input_tokens":0,"cache_creation_input_tokens":0}}}"#;
        let file = write_fixture(content);
        let outcome = parse_claude_file(file.path());

        assert_eq!(outcome.directives.len(), 1);
        assert_eq!(outcome.directives[0].session_id, "parent-session-xyz");
        assert_eq!(outcome.directives[0].text, "investigate the flaky test");
    }

    /// s31 D4's digest-dedup invariant at the adapter layer: re-parsing
    /// a GROWN fixture must re-emit byte-identical earlier directives.
    #[test]
    fn reparsing_a_grown_fixture_reemits_byte_identical_earlier_directives() {
        let base = r#"{"type":"user","timestamp":"2026-01-01T00:00:01.000Z","message":{"role":"user","content":"first ask"}}"#;
        let first_file = write_fixture(base);
        let first = parse_claude_file(first_file.path());
        assert_eq!(first.directives.len(), 1);

        let grown = format!("{base}\n{}", r#"{"type":"user","timestamp":"2026-01-01T00:00:02.000Z","message":{"role":"user","content":"second ask"}}"#);
        let grown_file = write_fixture(&grown);
        let second = parse_claude_file(grown_file.path());
        assert_eq!(second.directives.len(), 2);

        // Both fixtures resolve to the SAME (tempfile-stem-derived)
        // session_id only if the file paths matched, so compare
        // everything except session_id directly (verbatim text,
        // timestamp, workspace context all still must match).
        assert_eq!(first.directives[0].text, second.directives[0].text);
        assert_eq!(first.directives[0].timestamp_ms, second.directives[0].timestamp_ms);
        assert_eq!(first.directives[0].workspace_key, second.directives[0].workspace_key);
        assert_eq!(second.directives[1].text, "second ask");
    }

    #[test]
    fn flatten_claude_content_handles_string_array_and_absent_content() {
        assert_eq!(flatten_claude_content(Some(&serde_json::json!("hello"))), Some("hello".to_string()));
        assert_eq!(flatten_claude_content(Some(&serde_json::json!([{"type": "text", "text": "a"}, {"type": "text", "text": "b"}]))), Some("a\nb".to_string()));
        assert_eq!(flatten_claude_content(Some(&serde_json::json!([{"type": "tool_result", "tool_use_id": "x"}]))), None);
        assert_eq!(flatten_claude_content(None), None);
        assert_eq!(flatten_claude_content(Some(&serde_json::json!(""))), None);
    }
}
