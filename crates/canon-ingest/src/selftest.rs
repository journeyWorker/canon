//! canon-ingest's shared-contract selftest entry point (Wave-3 `canon
//! selftest` aggregator, per-crate registration — unblocks S3 6.9).
//! Wraps this crate's pure normalize invariants (determinism across two
//! runs + session grouping) as in-memory fixture checks over synthetic
//! [`UnifiedRow`]s — no filesystem or network read, side-effect-free
//! against the real repo by construction, so it runs unconditionally in
//! CI. Mirrors `canon-store`/`canon-vocab`/`canon-policy`'s own
//! `pub fn selftest() -> Result<usize, Vec<String>>` precedent.
//!
//! `Ok(n)` reports how many independent checks passed; `Err(_)` carries
//! one human-readable line per failing check — never panics.

use crate::adapter::{CostSource, TokenBreakdown, UnifiedRow};
use crate::normalize::{content_digest, normalize_rows};

/// One named fixture check (mirrors `canon-cli`'s `selftest::Suite`
/// alias — a `type` def keeps clippy's `type_complexity` lint happy).
type Check = (&'static str, fn() -> Result<(), String>);

/// Run canon-ingest's fixture checks. See module doc.
pub fn selftest() -> Result<usize, Vec<String>> {
    let checks: &[Check] = &[
        ("normalize-determinism", check_normalize_determinism),
        ("session-grouping", check_session_grouping),
    ];
    let mut passed = 0;
    let mut failures = Vec::new();
    for (name, run) in checks {
        match run() {
            Ok(()) => passed += 1,
            Err(e) => failures.push(format!("{name}: {e}")),
        }
    }
    if failures.is_empty() { Ok(passed) } else { Err(failures) }
}

fn row(session_id: &str, ts_ms: i64) -> UnifiedRow {
    UnifiedRow {
        client: "omp".into(),
        model_id: "claude-sonnet-5".into(),
        provider_id: "anthropic".into(),
        session_id: session_id.into(),
        workspace_key: Some("/tmp/proj".into()),
        workspace_label: Some("proj".into()),
        timestamp_ms: ts_ms,
        tokens: TokenBreakdown { input: 10, output: 5, cache_read: 0, cache_write: 0, reasoning: 0 },
        cost: 0.0,
        cost_source: CostSource::Unknown,
        duration_ms: None,
        dedup_key: None,
        is_turn_start: false,
    }
}

/// Two `normalize_rows` passes over identical input produce a
/// byte-identical run + the same `content_digest` (the S3 acceptance
/// "identical normalized output across two runs", the write-identity
/// idempotence rests on).
fn check_normalize_determinism() -> Result<(), String> {
    let rows = vec![row("ses_a", 1_000), row("ses_a", 2_000)];
    let first = normalize_rows(&rows);
    let second = normalize_rows(&rows);
    if first.sessions.is_empty() {
        return Err("normalize produced no sessions".into());
    }
    let a = serde_json::to_value(&first.sessions[0].run).map_err(|e| e.to_string())?;
    let b = serde_json::to_value(&second.sessions[0].run).map_err(|e| e.to_string())?;
    if a != b {
        return Err("two normalize runs produced different `run` records".into());
    }
    if content_digest(&a) != content_digest(&b) {
        return Err("content_digest differs across two identical runs".into());
    }
    Ok(())
}

/// Rows are grouped into one session per distinct `session_id`, with no
/// malformed-row skips for well-formed input.
fn check_session_grouping() -> Result<(), String> {
    let rows = vec![row("ses_b", 2_000), row("ses_a", 1_000), row("ses_a", 1_500)];
    let outcome = normalize_rows(&rows);
    if outcome.skipped_rows != 0 {
        return Err(format!("expected 0 skipped rows, got {}", outcome.skipped_rows));
    }
    if outcome.sessions.len() != 2 {
        return Err(format!("expected 2 sessions (ses_a, ses_b), got {}", outcome.sessions.len()));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn selftest_passes_against_its_own_in_memory_fixtures() {
        assert_eq!(selftest().expect("clean"), 2);
    }
}
