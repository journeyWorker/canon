//! The omp/pi reference adapter (S3 Wave 1) — parses
//! `~/.omp/agent/sessions/<encoded-cwd>/*.jsonl` and
//! `~/.pi/agent/sessions/<encoded-cwd>/*.jsonl` (badlogic/pi-mono and
//! its Oh My Pi fork share one JSONL transcript format; both home
//! directories are unioned into one logical `omp` adapter identity —
//! the same shape as design D5's Codex live+archived root union,
//! applied here to omp's dual-home precedent).
//!
//! Ported/adapted from the donor's pi parser (full module) and its
//! dual-root scan registration.
//!
//! **`session_id` derivation is content-only, never the filename**: the
//! adapter reads the in-file `session` header's `id` field. This is
//! the opposite rule from Claude Code/Codex (filename-stem-derived) —
//! see the donor's session-parser audit §3.4 for
//! why the per-adapter split is load-bearing, not an oversight.
//!
//! **s31 design D4 (user-directive capture)**: every `message.role ==
//! "user"` entry emits a [`crate::adapter::DirectiveRow`] — never a
//! `UnifiedRow` (a user turn carries no token usage). `message.content`
//! is either a plain string or an array of typed blocks
//! (`{"type":"text","text":"…"}` interleaved with e.g. tool-call
//! blocks); [`flatten_pi_content`] concatenates every `text` block and
//! skips every non-text block, same as the source's own conversation
//! view would render it. An absent/empty result is ordinary filtering
//! (a user turn carrying no rendered text), never a malformed-content
//! skip.

use std::io::{BufRead, BufReader};
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

use crate::adapter::{CostSource, DirectiveRow, ParseOutcome, SessionAdapter, TokenBreakdown, UnifiedRow};
use crate::normalize::{normalize_workspace_key, workspace_label_from_key};

/// Env var consulted only when `use_env_roots` is `true` — an
/// additional scan root unioned alongside the two default home-
/// relative roots (mirrors the donor's additive extra-dirs
/// override in spirit, scoped to this one adapter rather than a
/// crate-wide multi-client parser). Never consulted when
/// `use_env_roots` is `false`, so a deterministic test never picks up
/// an ambient value from the calling shell.
pub const EXTRA_SESSIONS_DIR_ENV: &str = "CANON_INGEST_OMP_SESSIONS_DIR";

pub struct OmpAdapter;

/// Pi session header (first line of JSONL).
/// Ported from `pi.rs:16-26`.
#[derive(Debug, Deserialize)]
struct PiSessionHeader {
    #[allow(dead_code)]
    #[serde(rename = "type")]
    entry_type: String,
    id: String,
    cwd: Option<String>,
}

/// Loose type-only probe for a JSONL line, used to identify
/// pre-session metadata records without requiring their full schema.
/// Ported from `pi.rs:28-34`.
#[derive(Debug, Deserialize)]
struct PiEntryTypeProbe {
    #[serde(rename = "type")]
    entry_type: String,
}

/// Record types OMP may write before the `session` header (e.g. an
/// auto-generated-title record). The parser skips these while looking
/// for `session` rather than discarding the whole file. Any other
/// unrecognized type before `session` is still treated as a malformed
/// file. Ported from `pi.rs:36-40`.
const PRE_SESSION_METADATA_TYPES: &[&str] = &["title"];

/// Pi session entry (subsequent lines of JSONL). Ported from
/// `pi.rs:42-54`.
#[derive(Debug, Deserialize)]
struct PiSessionEntry {
    #[serde(rename = "type")]
    entry_type: String,
    timestamp: Option<String>,
    message: Option<PiMessage>,
}

/// Ported from `pi.rs:56-62`, extended with `content` (s31 D4 —
/// the donor's own unified row never modeled a user turn's text at
/// all, so this field has no donor line range to cite).
#[derive(Debug, Deserialize)]
struct PiMessage {
    role: Option<String>,
    usage: Option<PiUsage>,
    model: Option<String>,
    provider: Option<String>,
    /// A plain string, or an array of typed content blocks — see the
    /// module doc's D4 paragraph and [`flatten_pi_content`].
    content: Option<Value>,
}

/// Ported from `pi.rs:64-73`.
#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
struct PiUsage {
    input: Option<i64>,
    output: Option<i64>,
    cache_read: Option<i64>,
    cache_write: Option<i64>,
}

impl SessionAdapter for OmpAdapter {
    fn client_id(&self) -> &'static str {
        "omp"
    }

    fn scan_roots(&self, home: &Path, use_env_roots: bool) -> Vec<PathBuf> {
        // Dual-root union: `.omp` (Oh My Pi fork) and `.pi`
        // (badlogic/pi-mono upstream) — ported precedent from
        // `scanner.rs:1176-1180`, where the donor's single `Pi`
        // `ClientId` scans BOTH its own `.pi/agent/sessions`
        // `ClientDef` root AND an additional `.omp/agent/sessions`
        // root pushed for the fork ("same JSONL format, different
        // root").
        let mut roots = vec![home.join(".omp/agent/sessions"), home.join(".pi/agent/sessions")];

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
        parse_pi_file(path)
    }
}

/// Parse a Pi/omp JSONL session file. Ported from `pi.rs:75-196`
/// (`parse_pi_file`), adapted to emit `UnifiedRow` instead of
/// the donor's unified row and to use `simd_json` per this crate's
/// own dependency (matching the donor's own reused-buffer parse
/// style, `pi.rs:86,103-105`).
///
/// `skipped` (Wave-2 amendment, see `adapter::ParseOutcome`) counts
/// every genuinely unparseable line/header this pass hits: a body
/// line that fails to deserialize as `PiSessionEntry`, and — since a
/// file under this adapter's scan roots that never resolves a
/// `session` header at all is malformed relative to what omp/pi ever
/// writes there, not merely "not this adapter's format" — an
/// unrecognized leading record type or an undeserializable header.
/// Ordinary business filtering (wrong role, missing usage/model on an
/// otherwise well-formed message) is NOT malformed content and is NOT
/// counted.
fn parse_pi_file(path: &Path) -> ParseOutcome {
    let file = match std::fs::File::open(path) {
        Ok(f) => f,
        Err(_) => return ParseOutcome::default(),
    };

    let fallback_timestamp = file_modified_timestamp_ms(path);

    let reader = BufReader::new(file);
    let mut rows: Vec<UnifiedRow> = Vec::with_capacity(64);
    let mut directives: Vec<DirectiveRow> = Vec::new();
    let mut skipped = 0usize;
    let mut buffer = Vec::with_capacity(4096);

    let mut session_id: Option<String> = None;
    let mut workspace_key: Option<String> = None;
    let mut workspace_label: Option<String> = None;

    for line in reader.lines() {
        let line = match line {
            Ok(l) => l,
            Err(_) => continue,
        };

        let trimmed = line.trim();
        if trimmed.is_empty() {
            continue;
        }

        if session_id.is_none() {
            // Header-type probe with abort-on-unrecognized-first-line
            // (ported from `pi.rs:102-127`): a file that doesn't open
            // with a recognized header is not-a-pi-session-file at
            // all, and the whole file yields zero rows rather than
            // guessing at a fallback identity.
            buffer.clear();
            buffer.extend_from_slice(trimmed.as_bytes());
            let entry_type = match simd_json::from_slice::<PiEntryTypeProbe>(&mut buffer) {
                Ok(probe) => probe.entry_type,
                Err(_) => return ParseOutcome::new(Vec::new(), skipped + 1),
            };

            if entry_type != "session" {
                if PRE_SESSION_METADATA_TYPES.contains(&entry_type.as_str()) {
                    continue;
                }
                return ParseOutcome::new(Vec::new(), skipped + 1);
            }

            buffer.clear();
            buffer.extend_from_slice(trimmed.as_bytes());
            let header = match simd_json::from_slice::<PiSessionHeader>(&mut buffer) {
                Ok(h) => h,
                Err(_) => return ParseOutcome::new(Vec::new(), skipped + 1),
            };

            // Content-derived session_id, never the filename — the
            // omp/pi rule the donor's session-parser audit §3.4
            // documents.
            session_id = Some(header.id);
            workspace_key = header.cwd.as_deref().and_then(normalize_workspace_key);
            workspace_label = workspace_key.as_deref().and_then(workspace_label_from_key);
            continue;
        }

        // Per-line skip-on-parse-failure for the body (ported from
        // `pi.rs:130-158`): a corrupt/truncated line never aborts an
        // otherwise-good transcript — it IS counted (Wave-2
        // amendment) rather than vanishing silently.
        buffer.clear();
        buffer.extend_from_slice(trimmed.as_bytes());
        let entry = match simd_json::from_slice::<PiSessionEntry>(&mut buffer) {
            Ok(e) => e,
            Err(_) => {
                skipped += 1;
                continue;
            }
        };

        if entry.entry_type != "message" {
            continue;
        }

        let Some(message) = entry.message else { continue };

        // Resolved once per line (borrowing `entry.timestamp`, not
        // consuming it) so both the D4 directive path and the
        // existing assistant/billing path below share one fallback
        // rule: the source's own event timestamp, or (when absent)
        // the transcript file's mtime.
        let timestamp_ms = entry
            .timestamp
            .as_deref()
            .and_then(|ts| chrono::DateTime::parse_from_rfc3339(ts).ok())
            .map(|dt| dt.timestamp_millis())
            .unwrap_or(fallback_timestamp);

        // s31 D4: a `user`-role message carries no token usage — it
        // NEVER becomes a `UnifiedRow` — but its (possibly empty)
        // rendered text becomes a `DirectiveRow`. `None` (no text at
        // all, e.g. a tool-only turn) is ordinary filtering, not a
        // malformed-content skip.
        if message.role.as_deref() == Some("user") {
            if let Some(text) = flatten_pi_content(message.content.as_ref()) {
                directives.push(DirectiveRow {
                    client: "omp".to_string(),
                    session_id: session_id.clone().unwrap_or_else(|| "unknown".to_string()),
                    timestamp_ms,
                    text,
                    workspace_key: workspace_key.clone(),
                    workspace_label: workspace_label.clone(),
                });
            }
            continue;
        }

        if message.role.as_deref() != Some("assistant") {
            continue;
        }

        let Some(usage) = message.usage else { continue };
        let Some(model) = message.model else { continue };

        // A missing provider field is recoverable: infer it from the
        // model name (falling back to "pi") rather than dropping a
        // row that carries valid tokens. Ported from `pi.rs:160-168`.
        let provider = message.provider.unwrap_or_else(|| inferred_provider_from_model(&model).unwrap_or("pi").to_string());

        rows.push(UnifiedRow {
            client: "omp".to_string(),
            model_id: model,
            provider_id: provider,
            session_id: session_id.clone().unwrap_or_else(|| "unknown".to_string()),
            workspace_key: workspace_key.clone(),
            workspace_label: workspace_label.clone(),
            timestamp_ms,
            tokens: TokenBreakdown {
                input: usage.input.unwrap_or(0).max(0),
                output: usage.output.unwrap_or(0).max(0),
                cache_read: usage.cache_read.unwrap_or(0).max(0),
                cache_write: usage.cache_write.unwrap_or(0).max(0),
                reasoning: 0,
            },
            cost: 0.0,
            cost_source: CostSource::Unknown,
            duration_ms: None,
            dedup_key: None,
            // pi.rs never computes a turn boundary either (no setter
            // call anywhere in the donor) — ported behavior, not an
            // omission; a future enhancement, not invented here.
            is_turn_start: false,
        });
    }

    ParseOutcome::with_directives(rows, skipped, directives)
}

/// Flatten a pi/omp `user` message's `content` field into a verbatim
/// directive string (s31 design D4). `content` is either a plain
/// string (used as-is) or an array of typed blocks — every `{"type":
/// "text","text":"…"}` block's `text` is concatenated in order
/// (`"\n"`-joined when there is more than one), every non-text block
/// (tool-call/tool-result, images, …) is skipped entirely. Returns
/// `None` for an absent `content`, a non-string/non-array shape, or a
/// result with no text at all — same "ordinary filtering, not
/// malformed" rule the rest of this adapter applies to a well-formed
/// record it has no use for.
fn flatten_pi_content(content: Option<&Value>) -> Option<String> {
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

/// Ported from the donor session-parser project.
fn file_modified_timestamp_ms(path: &Path) -> i64 {
    std::fs::metadata(path)
        .and_then(|meta| meta.modified())
        .ok()
        .and_then(|time| time.duration_since(std::time::SystemTime::UNIX_EPOCH).ok())
        .map(|duration| duration.as_millis() as i64)
        .unwrap_or_else(|| chrono::Utc::now().timestamp_millis())
}

/// Ported from the donor session-parser project's provider-identity module
/// (`contains_delimited` + `inferred_provider_from_model`), used only
/// as omp/pi's own missing-provider fallback (`pi.rs:163-168`) — not a
/// general-purpose canon pricing/provider-catalog surface.
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
    fn parses_valid_assistant_message_with_content_derived_session_id() {
        let content = r#"{"type":"session","id":"pi_ses_001","timestamp":"2026-01-01T00:00:00.000Z","cwd":"/tmp"}
{"type":"message","id":"msg_001","parentId":null,"timestamp":"2026-01-01T00:00:01.000Z","message":{"role":"assistant","model":"claude-3-5-sonnet","provider":"anthropic","usage":{"input":100,"output":50,"cacheRead":10,"cacheWrite":5,"totalTokens":165}}}"#;
        let file = write_fixture(content);

        let outcome = parse_pi_file(file.path());
        let rows = outcome.rows;

        assert_eq!(outcome.skipped, 0);
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].client, "omp");
        assert_eq!(rows[0].session_id, "pi_ses_001");
        assert_eq!(rows[0].model_id, "claude-3-5-sonnet");
        assert_eq!(rows[0].provider_id, "anthropic");
        assert_eq!(rows[0].tokens.input, 100);
        assert_eq!(rows[0].tokens.output, 50);
        assert_eq!(rows[0].tokens.cache_read, 10);
        assert_eq!(rows[0].tokens.cache_write, 5);
        assert_eq!(rows[0].workspace_key, Some("/tmp".to_string()));
        assert_eq!(rows[0].workspace_label, Some("tmp".to_string()));
    }

    #[test]
    fn session_id_never_comes_from_the_filename() {
        // The fixture's own tempfile name is random/unrelated — proves
        // session_id is read from the in-file header, not derived from
        // `path.file_stem()`.
        let content = r#"{"type":"session","id":"pi_ses_content_derived","cwd":"/tmp"}
{"type":"message","timestamp":"2026-01-01T00:00:01.000Z","message":{"role":"assistant","model":"gpt-4o-mini","provider":"openai","usage":{"input":1,"output":1}}}"#;
        let file = write_fixture(content);
        let outcome = parse_pi_file(file.path());
        let rows = outcome.rows;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].session_id, "pi_ses_content_derived");
        assert_ne!(rows[0].session_id, file.path().file_stem().unwrap().to_string_lossy());
    }

    #[test]
    fn skips_malformed_json_lines_but_keeps_the_rest() {
        let content = r#"{"type":"session","id":"pi_ses_004","cwd":"/tmp"}
not valid json
{"type":"message","timestamp":"2026-01-01T00:00:01.000Z","message":{"role":"assistant","model":"gpt-4o-mini","provider":"openai","usage":{"input":10,"output":5}}}"#;
        let file = write_fixture(content);
        let outcome = parse_pi_file(file.path());
        let rows = outcome.rows;
        assert_eq!(outcome.skipped, 1, "the corrupt line must be COUNTED, not just silently skipped");
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].model_id, "gpt-4o-mini");
    }

    #[test]
    fn skips_leading_title_record_before_session_header() {
        let content = r#"{"type":"title","v":1,"title":"Comment on GitHub issue"}
{"type":"session","id":"pi_ses_005","cwd":"/tmp"}
{"type":"message","timestamp":"2026-07-02T18:08:53.229Z","message":{"role":"assistant","model":"claude-sonnet-5","provider":"anthropic","usage":{"input":2,"output":180,"cacheWrite":70844}}}"#;
        let file = write_fixture(content);
        let outcome = parse_pi_file(file.path());
        let rows = outcome.rows;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].session_id, "pi_ses_005");
        assert_eq!(rows[0].tokens.cache_write, 70844);
    }

    #[test]
    fn rejects_unrecognized_leading_record_type_as_whole_file_malformed() {
        let content = r#"{"type":"totally_unknown_thing","foo":"bar"}
{"type":"session","id":"pi_ses_007","cwd":"/tmp"}
{"type":"message","timestamp":"2026-07-02T18:08:53.229Z","message":{"role":"assistant","model":"gpt-4o-mini","provider":"openai","usage":{"input":10,"output":5}}}"#;
        let file = write_fixture(content);
        let outcome = parse_pi_file(file.path());
        assert!(outcome.rows.is_empty());
        assert_eq!(outcome.skipped, 1, "an unrecognized leading record type is malformed and must be counted");
    }

    #[test]
    fn missing_provider_is_inferred_from_model_name() {
        let content = r#"{"type":"session","id":"pi_ses_008","cwd":"/tmp"}
{"type":"message","timestamp":"2026-07-02T18:08:53.229Z","message":{"role":"assistant","model":"claude-opus-4","usage":{"input":1,"output":1}}}"#;
        let file = write_fixture(content);
        let rows = parse_pi_file(file.path()).rows;
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].provider_id, "anthropic");
    }

    #[test]
    fn scan_roots_unions_omp_and_pi_home_dirs() {
        let adapter = OmpAdapter;
        let home = Path::new("/home/example");
        let roots = adapter.scan_roots(home, false);
        assert!(roots.contains(&home.join(".omp/agent/sessions")));
        assert!(roots.contains(&home.join(".pi/agent/sessions")));
    }

    #[test]
    fn scan_roots_ignores_env_override_when_use_env_roots_is_false() {
        // SAFETY: test-only, single-threaded within this process's
        // test harness invocation for this var name.
        unsafe { std::env::set_var(EXTRA_SESSIONS_DIR_ENV, "/should/not/appear") };
        let adapter = OmpAdapter;
        let roots = adapter.scan_roots(Path::new("/home/example"), false);
        unsafe { std::env::remove_var(EXTRA_SESSIONS_DIR_ENV) };
        assert!(!roots.iter().any(|r| r == Path::new("/should/not/appear")));
    }

    #[test]
    fn interleaved_user_and_assistant_turns_emit_directives_in_order_with_verbatim_text() {
        let content = concat!(
            r#"{"type":"session","id":"pi_ses_directives","cwd":"/tmp/proj"}"#,
            "\n",
            r#"{"type":"message","timestamp":"2026-01-01T00:00:01.000Z","message":{"role":"user","content":[{"type":"text","text":"first question"}]}}"#,
            "\n",
            r#"{"type":"message","timestamp":"2026-01-01T00:00:02.000Z","message":{"role":"assistant","model":"gpt-4o-mini","provider":"openai","usage":{"input":10,"output":5}}}"#,
            "\n",
            r#"{"type":"message","timestamp":"2026-01-01T00:00:03.000Z","message":{"role":"user","content":"plain string question"}}"#,
            "\n",
            r#"{"type":"message","timestamp":"2026-01-01T00:00:04.000Z","message":{"role":"assistant","model":"gpt-4o-mini","provider":"openai","usage":{"input":20,"output":8}}}"#,
        );
        let file = write_fixture(content);
        let outcome = parse_pi_file(file.path());

        assert_eq!(outcome.skipped, 0);
        assert_eq!(outcome.rows.len(), 2, "a user turn must never itself become a UnifiedRow");
        assert_eq!(outcome.directives.len(), 2);
        assert_eq!(outcome.directives[0].text, "first question", "an array-of-blocks content must flatten to its text block, verbatim");
        assert_eq!(outcome.directives[1].text, "plain string question", "a plain-string content must be used as-is, verbatim");
        // Transcript order, never re-sorted here (normalize.rs owns
        // the timestamp merge) — proves the adapter preserves scan
        // order across interleaved user/assistant lines.
        assert!(outcome.directives[0].timestamp_ms < outcome.directives[1].timestamp_ms);
        for directive in &outcome.directives {
            assert_eq!(directive.client, "omp");
            assert_eq!(directive.session_id, "pi_ses_directives");
            assert_eq!(directive.workspace_key, Some("/tmp/proj".to_string()));
            assert_eq!(directive.workspace_label, Some("proj".to_string()));
        }
    }

    #[test]
    fn non_text_content_blocks_are_skipped_and_text_blocks_are_concatenated() {
        let content = concat!(
            r#"{"type":"session","id":"pi_ses_blocks","cwd":"/tmp"}"#,
            "\n",
            r#"{"type":"message","timestamp":"2026-01-01T00:00:01.000Z","message":{"role":"user","content":[{"type":"tool_result","tool_use_id":"x"},{"type":"text","text":"line one"},{"type":"image","url":"ignored"},{"type":"text","text":"line two"}]}}"#,
        );
        let file = write_fixture(content);
        let outcome = parse_pi_file(file.path());

        assert_eq!(outcome.directives.len(), 1);
        assert_eq!(outcome.directives[0].text, "line one\nline two", "non-text blocks skipped, text blocks concatenated in order");
    }

    #[test]
    fn user_message_with_no_content_emits_no_directive_and_is_not_malformed() {
        let content = concat!(
            r#"{"type":"session","id":"pi_ses_no_content","cwd":"/tmp"}"#,
            "\n",
            r#"{"type":"message","timestamp":"2026-01-01T00:00:01.000Z","message":{"role":"user"}}"#,
            "\n",
            r#"{"type":"message","timestamp":"2026-01-01T00:00:02.000Z","message":{"role":"assistant","model":"gpt-4o-mini","provider":"openai","usage":{"input":1,"output":1}}}"#,
        );
        let file = write_fixture(content);
        let outcome = parse_pi_file(file.path());

        assert_eq!(outcome.skipped, 0, "an absent user content field is ordinary filtering, never a malformed-content skip");
        assert!(outcome.directives.is_empty());
        assert_eq!(outcome.rows.len(), 1);
    }

    /// s31 D4's digest-dedup invariant at the adapter layer: re-parsing
    /// a GROWN fixture (a later user/assistant pair appended) must
    /// re-emit byte-identical earlier directives — this adapter never
    /// reorders or recomputes anything based on total line count.
    #[test]
    fn reparsing_a_grown_fixture_reemits_byte_identical_earlier_directives() {
        let base = concat!(
            r#"{"type":"session","id":"pi_ses_grow","cwd":"/tmp/proj"}"#,
            "\n",
            r#"{"type":"message","timestamp":"2026-01-01T00:00:01.000Z","message":{"role":"user","content":"first ask"}}"#,
            "\n",
            r#"{"type":"message","timestamp":"2026-01-01T00:00:02.000Z","message":{"role":"assistant","model":"gpt-4o-mini","provider":"openai","usage":{"input":10,"output":5}}}"#,
        );
        let first_file = write_fixture(base);
        let first = parse_pi_file(first_file.path());
        assert_eq!(first.directives.len(), 1);

        let grown = format!(
            "{base}\n{}\n{}",
            r#"{"type":"message","timestamp":"2026-01-01T00:00:03.000Z","message":{"role":"user","content":"second ask"}}"#,
            r#"{"type":"message","timestamp":"2026-01-01T00:00:04.000Z","message":{"role":"assistant","model":"gpt-4o-mini","provider":"openai","usage":{"input":20,"output":8}}}"#,
        );
        let grown_file = write_fixture(&grown);
        let second = parse_pi_file(grown_file.path());
        assert_eq!(second.directives.len(), 2);

        assert_eq!(first.directives[0], second.directives[0], "the earlier directive must stay byte-identical after the file grows");
        assert_eq!(second.directives[1].text, "second ask");
    }

    #[test]
    fn flatten_pi_content_handles_string_array_and_absent_content() {
        assert_eq!(flatten_pi_content(Some(&serde_json::json!("hello"))), Some("hello".to_string()));
        assert_eq!(flatten_pi_content(Some(&serde_json::json!([{"type": "text", "text": "a"}, {"type": "text", "text": "b"}]))), Some("a\nb".to_string()));
        assert_eq!(flatten_pi_content(Some(&serde_json::json!([{"type": "tool_use", "name": "read"}]))), None, "an all-non-text block array yields no directive");
        assert_eq!(flatten_pi_content(None), None);
        assert_eq!(flatten_pi_content(Some(&serde_json::json!(""))), None, "an empty string yields no directive");
    }
}
