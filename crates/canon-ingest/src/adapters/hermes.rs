//! Hermes Agent session adapter (S3 Wave 2).
//!
//! Ported/adapted from the donor's Hermes parser and its scan-root
//! registration (which populates `ScanResult::hermes_db`, then unions
//! it with any profile databases).
//!
//! **SQLite, not JSONL**: unlike every other Wave 1/2 adapter, Hermes
//! aggregates a whole session as one ROW in a `sessions` table inside
//! one SQLite database per install/profile, rather than one JSONL
//! transcript file per session. `scan_roots()` therefore returns the
//! resolved `state.db` FILE paths directly (not directory roots to
//! walk for many files each) — `crate::scanner::scan_roots`'s
//! `WalkDir` walk still works unchanged when rooted at a single file
//! (it yields exactly that one entry), so `registry::AdapterEntry`'s
//! `file_suffix` ("state.db") is a documentation-only no-op filter
//! here: every root this adapter emits already IS a `state.db` path,
//! never a directory `scan_dir` needs to filter inside.
//!
//! `session_id` = the `sessions.id` primary key column, also reused
//! as `UnifiedRow.dedup_key`: each Hermes session row is already a
//! pre-aggregated whole-session total (not a per-turn stream), so
//! there is no per-turn merge/dedup problem to solve — the dedup key
//! is a re-ingest guard, not a streaming-duplicate merge key the way
//! Claude Code's adapter uses one.
//!
//! The donor's unified row carries two fields this port drops
//! because `UnifiedRow` (the frozen S3 contract) has no equivalent
//! slot: `message_count` (an aggregate session stat, not a per-row
//! dimension `UnifiedRow` models) and `agent` (always the constant
//! `"Hermes Agent"` display label in the donor — display-only, not a
//! join key). Both are still SELECTed (to keep the ported query
//! byte-identical to the donor) but discarded after decoding.
//!
//! **s31 design D4 (user-directive capture) — documented format gap,
//! not an oversight**: `SESSIONS_QUERY` reads the `sessions` table's
//! pre-aggregated per-session TOTALS; the underlying Hermes database
//! carries no per-turn message/role table this adapter's query (or
//! any query over `sessions` alone) could read a human-authored
//! directive text out of. `parse_hermes_sqlite` therefore never
//! constructs a `DirectiveRow` — `ParseOutcome.directives` is always
//! empty for this adapter, same as every other field this port has no
//! source data to fill (module doc above). A future revision that
//! wants Hermes directive capture needs a NEW query against whatever
//! per-turn table the Hermes CLI itself writes (out of this adapter's
//! current `sessions`-table scope), not a change to this one.

use std::path::{Path, PathBuf};

use rusqlite::{Connection, OpenFlags};

use crate::adapter::{CostSource, ParseOutcome, SessionAdapter, TokenBreakdown, UnifiedRow};

/// Env var consulted only when `use_env_roots` is `true` — ported name
/// from the donor's `ClientId::Hermes` root (a
/// `PathRoot::EnvVar { var: "HERMES_HOME", fallback_relative: ".hermes" }`).
/// Unlike the donor's EXCLUSIVE env-OR-fallback resolution
/// (`resolve_path_with_env_strategy` picks one), this adapter follows
/// the Wave 1 omp/pi precedent instead
/// (`adapters::omp::EXTRA_SESSIONS_DIR_ENV`): the env root is ADDITIVE
/// to the home-relative default, never a replacement, so a
/// `use_env_roots: false` test run never has to unset the ambient
/// variable to exercise the default path, and a `HERMES_HOME` install
/// never silently hides a `~/.hermes/state.db` that also happens to
/// exist.
pub const HERMES_HOME_ENV: &str = "HERMES_HOME";

pub struct HermesAdapter;

impl SessionAdapter for HermesAdapter {
    fn client_id(&self) -> &'static str {
        "hermes"
    }

    fn scan_roots(&self, home: &Path, use_env_roots: bool) -> Vec<PathBuf> {
        // Default root — ported from `scanner.rs:1265-1272`
        // (`ClientId::Hermes.data().resolve_path_with_env_strategy`'s
        // home-relative fallback branch, `clients.rs:322-328`'s
        // `fallback_relative: ".hermes"` + `relative: "state.db"`).
        // Always present regardless of on-disk existence —
        // `scanner::scan_dir`'s `!root.exists()` guard makes an absent
        // default a non-fatal, zero-record skip rather than something
        // this adapter needs to pre-check itself (same precedent as
        // Wave 1's `adapters::omp::scan_roots`, which always returns
        // both `.omp` and `.pi` regardless of which fork is actually
        // installed).
        let mut roots = vec![home.join(".hermes/state.db")];

        // Profile databases — ported from `hermes_db_paths`
        // (`scanner.rs:147-172`), which unions the default db with
        // every db the donor's `ScanResult::get(ClientId::Hermes)`
        // collected from `scanner.extraScanPaths.hermes`-configured
        // profile roots (a settings.json-driven list, donor
        // `scanner.rs` test `test_scan_all_clients_with_scanner_settings_merges_hermes_extra_profile_db`).
        // canon-ingest has no settings.json equivalent to source that
        // list from, so this adapter instead walks the conventional
        // `~/.hermes/profiles/<name>/state.db` layout the donor's own
        // test fixture uses directly — same shape, no config surface
        // needed on this side.
        let profiles_dir = home.join(".hermes/profiles");
        if let Ok(entries) = std::fs::read_dir(&profiles_dir) {
            let mut profile_dbs: Vec<PathBuf> = entries
                .filter_map(|entry| entry.ok())
                .filter(|entry| entry.path().is_dir())
                .map(|entry| entry.path().join("state.db"))
                .collect();
            // Deterministic order (S3 acceptance: identical output
            // across two runs) — `read_dir`'s own order is platform-
            // dependent.
            profile_dbs.sort_unstable();
            roots.extend(profile_dbs);
        }

        if use_env_roots {
            if let Ok(hermes_home) = std::env::var(HERMES_HOME_ENV) {
                let trimmed = hermes_home.trim();
                if !trimmed.is_empty() {
                    roots.push(PathBuf::from(trimmed).join("state.db"));
                }
            }
        }

        roots
    }

    fn parse(&self, path: &Path) -> ParseOutcome {
        parse_hermes_sqlite(path)
    }
}

/// The `sessions` table query — ported VERBATIM (column list, `WHERE`
/// clause, and all) from `hermes.rs:47-72`.
const SESSIONS_QUERY: &str = r#"
        SELECT
            id,
            model,
            billing_provider,
            started_at,
            message_count,
            input_tokens,
            output_tokens,
            cache_read_tokens,
            cache_write_tokens,
            reasoning_tokens,
            estimated_cost_usd,
            actual_cost_usd
        FROM sessions
        WHERE model IS NOT NULL
          AND TRIM(model) != ''
          AND (
            COALESCE(input_tokens, 0) > 0 OR
            COALESCE(output_tokens, 0) > 0 OR
            COALESCE(cache_read_tokens, 0) > 0 OR
            COALESCE(cache_write_tokens, 0) > 0 OR
            COALESCE(reasoning_tokens, 0) > 0 OR
            COALESCE(actual_cost_usd, estimated_cost_usd, 0) > 0
          )
    "#;

/// Parse one Hermes `state.db` SQLite database into a [`ParseOutcome`].
/// Ported from `parse_hermes_sqlite`, `hermes.rs:31-162` — same
/// read-only-connection / prepare / query_map / filter_map / map
/// pipeline shape, same non-panicking "warn and return empty" error
/// handling at every fallible step (a malformed or absent db is a
/// violation to skip, not a crash — design §7) — and now COUNTED
/// (Wave-2 amendment, `ParseOutcome::skipped`): an unopenable db, an
/// unprepareable/unexecutable query, and each individually
/// undecodable row all count as 1 each, matching the finding's
/// "malformed dbs are silently skipped" language directly.
/// `tracing::warn!` in the donor becomes `eprintln!` here: this crate
/// has no `tracing` dependency (S3 Wave 1 never added one), matching
/// the `eprintln!`-for-non-fatal-diagnostics convention `canon-cli`
/// already uses (`crates/canon-cli/src/main.rs`).
pub fn parse_hermes_sqlite(db_path: &Path) -> ParseOutcome {
    let conn = match Connection::open_with_flags(
        db_path,
        OpenFlags::SQLITE_OPEN_READ_ONLY | OpenFlags::SQLITE_OPEN_NO_MUTEX,
    ) {
        Ok(c) => c,
        Err(err) => {
            eprintln!("canon-ingest hermes: failed to open {}: {err}", db_path.display());
            return ParseOutcome::new(Vec::new(), 1);
        }
    };

    let mut stmt = match conn.prepare(SESSIONS_QUERY) {
        Ok(s) => s,
        Err(err) => {
            eprintln!("canon-ingest hermes: failed to prepare session query on {}: {err}", db_path.display());
            return ParseOutcome::new(Vec::new(), 1);
        }
    };

    let query_rows = match stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, String>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, f64>(3)?,
            row.get::<_, Option<i32>>(4)?.unwrap_or(0),
            row.get::<_, Option<i64>>(5)?.unwrap_or(0),
            row.get::<_, Option<i64>>(6)?.unwrap_or(0),
            row.get::<_, Option<i64>>(7)?.unwrap_or(0),
            row.get::<_, Option<i64>>(8)?.unwrap_or(0),
            row.get::<_, Option<i64>>(9)?.unwrap_or(0),
            row.get::<_, Option<f64>>(10)?,
            row.get::<_, Option<f64>>(11)?,
        ))
    }) {
        Ok(r) => r,
        Err(err) => {
            eprintln!("canon-ingest hermes: failed to execute session query on {}: {err}", db_path.display());
            return ParseOutcome::new(Vec::new(), 1);
        }
    };

    let mut skipped = 0usize;
    let rows: Vec<UnifiedRow> = query_rows
        .filter_map(|row| match row {
            Ok(row) => Some(row),
            Err(err) => {
                eprintln!("canon-ingest hermes: failed to decode session row from {}: {err}", db_path.display());
                skipped += 1;
                None
            }
        })
        .map(
            |(
                session_id,
                model_id,
                billing_provider,
                started_at,
                _message_count,
                input,
                output,
                cache_read,
                cache_write,
                reasoning,
                estimated_cost,
                actual_cost,
            )| {
                let provider_id = resolved_provider(billing_provider, &model_id);

                // Cost precedence + provenance — ported from
                // `hermes.rs:153` (`actual_cost.or(estimated_cost)`).
                // Both figures are Hermes's OWN billed/estimated dollar
                // amounts (never a canon-side derivation), so either one
                // present marks `ProviderReported`; neither present (a
                // token-only row the `WHERE` clause still let through via
                // its token-count arms) leaves the default `Unknown` — no
                // cost figure exists yet for a later pricing pass to
                // confirm or overwrite.
                let cost_source =
                    if actual_cost.is_some() || estimated_cost.is_some() { CostSource::ProviderReported } else { CostSource::Unknown };
                let cost = actual_cost.or(estimated_cost).unwrap_or(0.0).max(0.0);

                UnifiedRow {
                    client: "hermes".to_string(),
                    model_id,
                    provider_id,
                    session_id: session_id.clone(),
                    workspace_key: None,
                    workspace_label: None,
                    timestamp_ms: timestamp_secs_to_ms(started_at),
                    tokens: TokenBreakdown {
                        input: input.max(0),
                        output: output.max(0),
                        cache_read: cache_read.max(0),
                        cache_write: cache_write.max(0),
                        reasoning: reasoning.max(0),
                    },
                    cost,
                    cost_source,
                    duration_ms: None,
                    dedup_key: Some(session_id),
                    is_turn_start: false,
                }
            },
        )
        .collect();

    ParseOutcome::new(rows, skipped)
}

/// Normalize a `sessions.started_at` value to Unix milliseconds.
/// Ported verbatim from `hermes.rs:15-21`: values already in
/// milliseconds (`> 1e12`, i.e. any timestamp past ~2001 expressed in
/// ms) pass through; smaller values are assumed to be Unix seconds and
/// scaled up.
fn timestamp_secs_to_ms(timestamp: f64) -> i64 {
    if timestamp > 1e12 { timestamp as i64 } else { (timestamp * 1000.0) as i64 }
}

/// Resolve `UnifiedRow.provider_id`. Ported verbatim from
/// `hermes.rs:23-29`: prefer the row's own `billing_provider` column
/// (canonicalized), fall back to inferring from the model id string,
/// fall back to the literal `"hermes"` client id when neither yields
/// anything.
fn resolved_provider(billing_provider: Option<String>, model_id: &str) -> String {
    billing_provider
        .filter(|provider| !provider.trim().is_empty())
        .and_then(|provider| canonical_provider(provider.trim()))
        .or_else(|| inferred_provider_from_model(model_id).map(str::to_string))
        .unwrap_or_else(|| "hermes".to_string())
}

// --- Ported from the donor's provider-identity module ---
//
// `hermes.rs`'s `resolved_provider` (above) calls the donor crate's
// `provider_identity::canonical_provider` +
// `provider_identity::inferred_provider_from_model`. The donor crate
// exposes those as a shared cross-client module; canon-ingest has no
// such shared surface yet (Wave 1's omp adapter made the same call —
// see `adapters::omp`'s own local `contains_delimited` +
// `inferred_provider_from_model` copy, `omp.rs:266-311`), so this
// adapter carries its own copy rather than reaching across adapter
// boundaries for a two-function dependency. Ported functions below
// are otherwise UNCHANGED from the donor.

/// Ported verbatim from `provider_identity.rs:1-34`.
fn canonicalize_provider_segment(segment: &str) -> Option<String> {
    let normalized = segment.trim().trim_end_matches('/').to_lowercase().replace('-', "_");
    if normalized.starts_with('<') && normalized.ends_with('>') {
        return None;
    }

    let canonical = match normalized.as_str() {
        "" | "unknown" => return None,
        "x_ai" | "xai" => "xai",
        "z_ai" | "zai" => "zai",
        "moonshot" | "moonshotai" => "moonshotai",
        "meta" | "meta_llama" => "meta_llama",
        "azure" | "azure_ai" => "azure_ai",
        "anthropic" | "vertex" | "vertex_ai" => "anthropic",
        "together" | "together_ai" => "together_ai",
        "fireworks" | "fireworks_ai" => "fireworks_ai",
        "google" | "gemini" => "google",
        "openai" | "openai_codex" => "openai",
        "minimax" | "minimaxai" | "minimax_ai" => "minimax",
        "mistral" | "mistralai" => "mistralai",
        "ai21" => "ai21",
        // For unknown segments, reject if they contain digits — those are
        // almost certainly model-name fragments (e.g., "gpt-4", "claude-3")
        // rather than provider identifiers.
        other if other.chars().any(|ch| ch.is_ascii_digit()) => return None,
        other => other,
    };

    Some(canonical.into())
}

/// Ported verbatim from `provider_identity.rs:36-38`.
fn canonical_provider(raw: &str) -> Option<String> {
    provider_tags(raw).into_iter().next()
}

/// Ported verbatim from `provider_identity.rs:40-60`.
fn provider_tags(raw: &str) -> Vec<String> {
    let mut tags = Vec::new();
    let mut push = |segment: &str| {
        if let Some(tag) = canonicalize_provider_segment(segment) {
            if !tags.iter().any(|existing| existing == &tag) {
                tags.push(tag);
            }
        }
    };

    for segment in raw.trim().trim_end_matches('/').split('/') {
        push(segment);
        if segment.contains('.') {
            for dotted in segment.split('.') {
                push(dotted);
            }
        }
    }

    tags
}

/// Ported verbatim from `provider_identity.rs:111-122`.
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

/// Ported verbatim from `provider_identity.rs:124-183`.
fn inferred_provider_from_model(model: &str) -> Option<&'static str> {
    let lower = model.to_lowercase();

    if lower.contains("claude")
        || lower.contains("anthropic")
        || contains_delimited(&lower, "opus")
        || contains_delimited(&lower, "sonnet")
        || contains_delimited(&lower, "haiku")
        || contains_delimited(&lower, "fable")
    {
        return Some("anthropic");
    }

    if lower.contains("gpt")
        || lower.contains("openai")
        || contains_delimited(&lower, "o1")
        || contains_delimited(&lower, "o3")
        || contains_delimited(&lower, "o4")
    {
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

    if lower.contains("minimax") {
        return Some("minimax");
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

    // Sakana's `fugu` / `fugu-ultra` model line. Bare `fugu` is intentionally
    // still mapped to the sakana provider here (provider identity is independent
    // of whether we can price the model — see build_sakana_overrides, which
    // deliberately does NOT price bare `fugu`).
    if lower.contains("fugu") {
        return Some("sakana");
    }

    None
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Build a fresh Hermes `sessions` table at `path`, matching the
    /// donor's own test-fixture schema (ported from
    /// `create_hermes_sqlite_db`)
    /// rather than checking in an opaque binary `.db` file — a
    /// programmatically-built fixture never drifts from whatever
    /// `rusqlite`/libsqlite3 version actually built it.
    fn create_test_db(path: &Path) -> Connection {
        let conn = Connection::open(path).unwrap();
        conn.execute_batch(
            "CREATE TABLE sessions (
                id TEXT PRIMARY KEY,
                source TEXT NOT NULL,
                model TEXT,
                started_at REAL NOT NULL,
                message_count INTEGER DEFAULT 0,
                input_tokens INTEGER DEFAULT 0,
                output_tokens INTEGER DEFAULT 0,
                cache_read_tokens INTEGER DEFAULT 0,
                cache_write_tokens INTEGER DEFAULT 0,
                reasoning_tokens INTEGER DEFAULT 0,
                billing_provider TEXT,
                estimated_cost_usd REAL,
                actual_cost_usd REAL
            );",
        )
        .unwrap();
        conn
    }

    #[allow(clippy::too_many_arguments)]
    fn insert_session(
        conn: &Connection,
        id: &str,
        model: Option<&str>,
        billing_provider: Option<&str>,
        started_at: f64,
        input: i64,
        output: i64,
        estimated_cost: Option<f64>,
        actual_cost: Option<f64>,
    ) {
        conn.execute(
            "INSERT INTO sessions (
                id, source, model, started_at, message_count,
                input_tokens, output_tokens, cache_read_tokens, cache_write_tokens, reasoning_tokens,
                billing_provider, estimated_cost_usd, actual_cost_usd
            ) VALUES (?1, 'cli', ?2, ?3, 1, ?4, ?5, 0, 0, 0, ?6, ?7, ?8)",
            rusqlite::params![id, model, started_at, input, output, billing_provider, estimated_cost, actual_cost],
        )
        .unwrap();
    }

    #[test]
    fn round_trip_parse_yields_expected_rows() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("state.db");
        let conn = create_test_db(&db_path);
        insert_session(&conn, "ses_001", Some("claude-opus-4"), Some("anthropic"), 1_775_001_102.0, 100, 50, None, Some(1.23));
        drop(conn);

        let outcome = parse_hermes_sqlite(&db_path);
        let rows = outcome.rows;

        assert_eq!(outcome.skipped, 0);
        assert_eq!(rows.len(), 1);
        let row = &rows[0];
        assert_eq!(row.client, "hermes");
        assert_eq!(row.session_id, "ses_001");
        assert_eq!(row.dedup_key.as_deref(), Some("ses_001"));
        assert_eq!(row.model_id, "claude-opus-4");
        assert_eq!(row.provider_id, "anthropic");
        assert_eq!(row.tokens.input, 100);
        assert_eq!(row.tokens.output, 50);
        assert!((row.cost - 1.23).abs() < 1e-9);
        assert_eq!(row.cost_source, CostSource::ProviderReported);
        assert_eq!(row.workspace_key, None);
        assert_eq!(row.workspace_label, None);
        assert_eq!(row.duration_ms, None);
        assert!(!row.is_turn_start);
    }

    /// s31 design D4: Hermes's `sessions` table has no per-turn text
    /// to read a directive out of (module doc's D4 paragraph) —
    /// `ParseOutcome.directives` must stay empty even when rows are
    /// otherwise successfully parsed.
    #[test]
    fn parse_never_emits_directives_documented_format_gap() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("state.db");
        let conn = create_test_db(&db_path);
        insert_session(&conn, "ses_no_directives", Some("claude-opus-4"), Some("anthropic"), 1_775_001_102.0, 100, 50, None, Some(1.23));
        drop(conn);

        let outcome = parse_hermes_sqlite(&db_path);
        assert_eq!(outcome.rows.len(), 1, "rows are still parsed normally");
        assert!(outcome.directives.is_empty(), "Hermes has no per-turn table to source directive text from");
    }

    #[test]
    fn where_filter_excludes_zero_token_zero_cost_rows() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("state.db");
        let conn = create_test_db(&db_path);
        // Kept: has tokens.
        insert_session(&conn, "kept-tokens", Some("gpt-4o-mini"), None, 1_700_000_000.0, 5, 0, None, None);
        // Excluded: model present but every token bucket AND both cost
        // columns are zero/absent — no billable evidence at all.
        insert_session(&conn, "excluded-empty", Some("gpt-4o-mini"), None, 1_700_000_000.0, 0, 0, None, None);
        // Excluded: model is NULL outright.
        insert_session(&conn, "excluded-null-model", None, None, 1_700_000_000.0, 10, 10, None, None);
        drop(conn);

        let rows = parse_hermes_sqlite(&db_path).rows;

        let ids: Vec<&str> = rows.iter().map(|r| r.session_id.as_str()).collect();
        assert_eq!(ids, vec!["kept-tokens"]);
    }

    #[test]
    fn actual_cost_takes_precedence_over_estimated() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("state.db");
        let conn = create_test_db(&db_path);
        insert_session(&conn, "both-costs", Some("gpt-4o"), None, 1_700_000_000.0, 1, 1, Some(9.99), Some(2.50));
        insert_session(&conn, "estimated-only", Some("gpt-4o"), None, 1_700_000_000.0, 1, 1, Some(4.44), None);
        drop(conn);

        let rows = parse_hermes_sqlite(&db_path).rows;
        let both = rows.iter().find(|r| r.session_id == "both-costs").unwrap();
        let estimated_only = rows.iter().find(|r| r.session_id == "estimated-only").unwrap();

        assert!((both.cost - 2.50).abs() < 1e-9, "actual_cost_usd must win over estimated_cost_usd");
        assert_eq!(both.cost_source, CostSource::ProviderReported);
        assert!((estimated_only.cost - 4.44).abs() < 1e-9);
        assert_eq!(estimated_only.cost_source, CostSource::ProviderReported);
    }

    #[test]
    fn started_at_normalizes_seconds_and_milliseconds() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("state.db");
        let conn = create_test_db(&db_path);
        // Seconds: 2024-01-01T00:00:00Z == 1704067200.
        insert_session(&conn, "secs", Some("gpt-4o"), None, 1_704_067_200.0, 1, 1, None, Some(0.01));
        // Already milliseconds (> 1e12).
        insert_session(&conn, "ms", Some("gpt-4o"), None, 1_704_067_200_000.0, 1, 1, None, Some(0.01));
        drop(conn);

        let rows = parse_hermes_sqlite(&db_path).rows;
        let secs = rows.iter().find(|r| r.session_id == "secs").unwrap();
        let ms = rows.iter().find(|r| r.session_id == "ms").unwrap();

        assert_eq!(secs.timestamp_ms, 1_704_067_200_000);
        assert_eq!(ms.timestamp_ms, 1_704_067_200_000);
    }

    #[test]
    fn parse_is_idempotent_across_repeated_calls() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("state.db");
        let conn = create_test_db(&db_path);
        insert_session(&conn, "ses_idem", Some("claude-sonnet-4-5"), Some("anthropic"), 1_700_000_000.0, 42, 17, Some(0.05), None);
        drop(conn);

        let first = parse_hermes_sqlite(&db_path);
        let second = parse_hermes_sqlite(&db_path);
        assert_eq!(first, second);
    }

    #[test]
    fn missing_or_absent_database_yields_empty_and_counts_skipped() {
        let dir = tempfile::tempdir().unwrap();
        let missing = dir.path().join("no-such-state.db");
        let missing_outcome = parse_hermes_sqlite(&missing);
        assert!(missing_outcome.rows.is_empty());
        assert_eq!(missing_outcome.skipped, 1, "an unopenable db is malformed/absent evidence and must be counted");

        // A file that exists but isn't a SQLite database at all.
        let garbage = dir.path().join("garbage.db");
        std::fs::write(&garbage, b"not a sqlite database").unwrap();
        let garbage_outcome = parse_hermes_sqlite(&garbage);
        assert!(garbage_outcome.rows.is_empty());
        assert_eq!(garbage_outcome.skipped, 1, "a malformed db file must be counted, not just silently skipped");
    }

    #[test]
    fn missing_provider_is_inferred_from_model_name() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("state.db");
        let conn = create_test_db(&db_path);
        insert_session(&conn, "ses_inferred", Some("claude-opus-4"), None, 1_700_000_000.0, 1, 1, None, Some(0.01));
        drop(conn);

        let rows = parse_hermes_sqlite(&db_path).rows;
        assert_eq!(rows[0].provider_id, "anthropic");
    }

    #[test]
    fn unresolvable_provider_falls_back_to_hermes() {
        let dir = tempfile::tempdir().unwrap();
        let db_path = dir.path().join("state.db");
        let conn = create_test_db(&db_path);
        insert_session(&conn, "ses_fallback", Some("some-unknown-model-xyz"), None, 1_700_000_000.0, 1, 1, None, Some(0.01));
        drop(conn);

        let rows = parse_hermes_sqlite(&db_path).rows;
        assert_eq!(rows[0].provider_id, "hermes");
    }

    #[test]
    fn scan_roots_includes_default_and_profile_dbs() {
        let dir = tempfile::tempdir().unwrap();
        let home = dir.path();
        std::fs::create_dir_all(home.join(".hermes/profiles/work")).unwrap();
        std::fs::create_dir_all(home.join(".hermes/profiles/personal")).unwrap();

        let adapter = HermesAdapter;
        let roots = adapter.scan_roots(home, false);

        assert!(roots.contains(&home.join(".hermes/state.db")));
        assert!(roots.contains(&home.join(".hermes/profiles/work/state.db")));
        assert!(roots.contains(&home.join(".hermes/profiles/personal/state.db")));
    }

    #[test]
    fn scan_roots_hermes_home_env_is_additive_and_gated_by_use_env_roots() {
        // Both assertions share ONE test function deliberately: Rust's
        // default test harness runs functions in parallel across
        // threads, and two separate tests each mutating the same
        // process-global `HERMES_HOME_ENV` var would race each other
        // (observed: flaky under `cargo test`, stable only under
        // `--test-threads=1`) — merging avoids the race entirely
        // rather than papering over it with a serialization crate.
        //
        // SAFETY: test-only, and this function never yields across an
        // await/thread boundary while the var is set.
        unsafe { std::env::set_var(HERMES_HOME_ENV, "/custom/hermes/home") };
        let adapter = HermesAdapter;
        let home = Path::new("/home/example");

        let ignored = adapter.scan_roots(home, false);
        assert!(!ignored.iter().any(|r| r == Path::new("/custom/hermes/home/state.db")));

        let additive = adapter.scan_roots(home, true);
        // Additive, not exclusive: the home-relative default is still present.
        assert!(additive.contains(&home.join(".hermes/state.db")));
        assert!(additive.contains(&PathBuf::from("/custom/hermes/home/state.db")));

        unsafe { std::env::remove_var(HERMES_HOME_ENV) };
    }

    #[test]
    fn hermes_is_registered_in_the_registry() {
        let entry = crate::registry::find("hermes").expect("hermes must be registered in registry::registry()");
        assert_eq!(entry.client_id(), "hermes");
        assert!(entry.file_matches(Path::new("/home/x/.hermes/state.db")));
    }
}
