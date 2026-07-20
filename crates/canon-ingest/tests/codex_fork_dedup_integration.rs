//! End-to-end reproduction of ReviewS3Full finding 2 (CRITICAL,
//! fork-dedup): two REAL, on-disk Codex fork/replay `.jsonl` files —
//! each forked from the SAME parent session, each parsed through the
//! actual `CodexAdapter` — produce rows whose `dedup_key` is scoped to
//! the shared fork-parent identity
//! (`adapters::codex::set_codex_dedup_key`), not to either child's own
//! (filename-derived) `session_id`. Before the fix, `normalize_rows`
//! grouped/summed purely by `session_id`, so both children's rows
//! survived and the shared history was counted twice; after the fix,
//! the cross-file `dedup_key` consumption collapses them to one.

use canon_ingest::adapter::SessionAdapter;
use canon_ingest::adapters::codex::CodexAdapter;

fn write_fork_child(dir: &std::path::Path, file_name: &str, meta_id: &str, ts_prefix: &str) -> std::path::PathBuf {
    let content = format!(
        concat!(
            r#"{{"timestamp":"{ts}00:00:00Z","type":"session_meta","payload":{{"id":"{meta}","forked_from_id":"parent-session-shared","source":{{"subagent":{{"thread_spawn":{{"parent_thread_id":"parent-session-shared","depth":1}}}}}},"model_provider":"openai","cwd":"/repo/{meta}"}}}}"#,
            "\n",
            r#"{{"timestamp":"{ts}00:00:01Z","type":"turn_context","payload":{{"model":"gpt-5.5"}}}}"#,
            "\n",
            // Both children report the IDENTICAL fork-parent-scoped
            // cumulative total on their first post-fork token_count
            // event (a replayed/inherited snapshot that both siblings
            // happen to still be carrying) — same provider/model/
            // totals, so `codex_token_count_dedup_key`'s total-based
            // branch produces the SAME dedup_key string for both
            // files despite their different filename-derived
            // `session_id`s.
            r#"{{"timestamp":"{ts}00:00:02Z","type":"event_msg","payload":{{"type":"token_count","info":{{"total_token_usage":{{"input_tokens":5000,"cached_input_tokens":4000,"output_tokens":300,"total_tokens":5300}},"last_token_usage":{{"input_tokens":50,"cached_input_tokens":40,"output_tokens":30,"total_tokens":80}}}}}}}}"#,
        ),
        ts = ts_prefix,
        meta = meta_id,
    );
    let path = dir.join(file_name);
    std::fs::write(&path, content).unwrap();
    path
}

#[test]
fn two_codex_fork_files_sharing_a_parent_dedup_key_are_not_double_counted() {
    let dir = tempfile::tempdir().unwrap();
    let path_a = write_fork_child(dir.path(), "fork-child-a.jsonl", "child-a-meta", "2026-06-01T10:");
    let path_b = write_fork_child(dir.path(), "fork-child-b.jsonl", "child-b-meta", "2026-06-01T11:");

    let adapter = CodexAdapter;
    let outcome_a = adapter.parse(&path_a);
    let outcome_b = adapter.parse(&path_b);

    assert_eq!(outcome_a.skipped, 0);
    assert_eq!(outcome_b.skipped, 0);
    assert_eq!(outcome_a.rows.len(), 1, "child A must emit its (replayed) token_count row: {:?}", outcome_a.rows);
    assert_eq!(outcome_b.rows.len(), 1, "child B must emit its (replayed) token_count row: {:?}", outcome_b.rows);

    // Different filename-derived session_ids...
    assert_ne!(outcome_a.rows[0].session_id, outcome_b.rows[0].session_id);
    // ...but the SAME fork-parent-scoped dedup_key, proving the
    // cross-file collision this fix must resolve is real, not
    // contrived at the normalize layer alone.
    assert_eq!(outcome_a.rows[0].dedup_key, outcome_b.rows[0].dedup_key);
    assert!(outcome_a.rows[0].dedup_key.as_deref().unwrap().contains("parent-session-shared"));

    let mut all_rows = outcome_a.rows;
    all_rows.extend(outcome_b.rows);
    let normalized = canon_ingest::normalize_rows(&all_rows);

    // Without the fix: two sessions, tokens counted twice. With the
    // fix: exactly one session survives, the replayed history summed
    // once.
    assert_eq!(normalized.sessions.len(), 1, "the second fork child's duplicate row must collapse away: {:?}", normalized.sessions.iter().map(|s| s.session.session_id.as_str()).collect::<Vec<_>>());
    assert_eq!(normalized.sessions[0].events.len(), 1);
}
