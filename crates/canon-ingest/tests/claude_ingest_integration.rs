//! Full scan -> parse pipeline over a fixture `$HOME` layout for the
//! Claude Code adapter (S3 Wave 2): `.claude/projects/<key>` workspace
//! derivation, filename-stem `session_id` with sidechain-parent
//! override, streaming-duplicate dedup-merge, and per-line
//! skip-on-corrupt-line — see `src/adapters/claude.rs`'s module doc
//! for the exact donor session-parser ranges ported.

use std::path::PathBuf;

fn fixtures_home() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/home")
}

fn claude_entry() -> &'static canon_ingest::AdapterEntry {
    canon_ingest::registry::find("claude-code").expect("claude-code adapter registered")
}

#[test]
fn claude_adapter_scans_dot_claude_projects_and_derives_workspace_from_path() {
    let home = fixtures_home();
    let result = canon_ingest::registry::scan_and_parse(claude_entry(), &home, false);

    assert_eq!(result.client_id, "claude-code");
    // Two fixture files under `.claude/projects/-Users-example-project/`:
    // the main session (`sess-alpha.jsonl`) and the sidechain subagent
    // transcript (`agent-sub01.jsonl`).
    assert_eq!(result.files_scanned.len(), 2, "found: {:?}", result.files_scanned);

    for row in &result.rows {
        assert_eq!(row.client, "claude-code");
        assert_eq!(row.workspace_key, Some("-Users-example-project".to_string()));
        assert_eq!(row.workspace_label, Some("-Users-example-project".to_string()));
        assert_eq!(row.provider_id, "anthropic");
    }
}

#[test]
fn streaming_duplicate_pair_merges_into_one_row_with_max_tokens_and_the_corrupt_line_is_skipped() {
    let home = fixtures_home();
    let result = canon_ingest::registry::scan_and_parse(claude_entry(), &home, false);

    // `sess-alpha.jsonl` has 5 lines: 1 user turn (no row), 2 streaming
    // duplicate assistant writes for msg_001 (merge into 1 row), 1
    // corrupt line (skipped, no row), and 1 distinct msg_002 (1 row).
    // Plus the sidechain file's own 1 assistant row, whose session_id
    // resolves to the SAME parent (`sess-alpha`) — so 3 total rows
    // share session_id "sess-alpha".
    let alpha_rows: Vec<_> = result.rows.iter().filter(|r| r.session_id == "sess-alpha").collect();
    assert_eq!(alpha_rows.len(), 3, "expected msg_001 (merged) + msg_002 + the sidechain row, got: {alpha_rows:?}");

    let merged = alpha_rows.iter().find(|r| r.dedup_key.as_deref() == Some("msg_001:req_001")).expect("merged streaming-duplicate row present");
    assert_eq!(merged.tokens.input, 10, "max across both writes");
    assert_eq!(merged.tokens.output, 180, "max across both writes — the corrupt line between them never resets this");
    assert_eq!(merged.tokens.cache_read, 5, "max across both writes");
    let expected_ts = chrono::DateTime::parse_from_rfc3339("2026-01-01T00:00:02.500Z").unwrap().timestamp_millis();
    assert_eq!(merged.timestamp_ms, expected_ts, "timestamp advances to the later of the two merged writes");

    let msg_002 = alpha_rows.iter().find(|r| r.dedup_key.as_deref() == Some("message:msg_002")).expect("msg_002 row present, proving the corrupt line was skipped not fatal");
    assert_eq!(msg_002.tokens.input, 20);
}

#[test]
fn sidechain_subagent_transcript_session_id_resolves_to_the_parent_not_its_own_filename() {
    let home = fixtures_home();
    let result = canon_ingest::registry::scan_and_parse(claude_entry(), &home, false);

    let sidechain_row = result.rows.iter().find(|r| r.dedup_key.as_deref() == Some("message:msg_101")).expect("sidechain row present");
    assert_eq!(sidechain_row.session_id, "sess-alpha", "isSidechain:true overrides session_id with the parent sessionId field, never `agent-sub01` (the file's own stem)");
    assert_ne!(sidechain_row.session_id, "agent-sub01");

    // No row anywhere carries the subagent file's own stem as its
    // session_id — proves the override applies unconditionally, not
    // just to the row we happened to check above.
    assert!(!result.rows.iter().any(|r| r.session_id == "agent-sub01"));
}

#[test]
fn full_claude_pipeline_is_idempotent_across_two_ingest_runs() {
    let home = fixtures_home();
    let run_once = || canon_ingest::registry::scan_and_parse(claude_entry(), &home, false).rows;

    let first = run_once();
    let second = run_once();

    assert!(!first.is_empty());
    assert_eq!(first.len(), second.len());

    let first_json = serde_json::to_value(&first).unwrap();
    let second_json = serde_json::to_value(&second).unwrap();
    assert_eq!(first_json, second_json, "re-ingesting an unchanged fixture home must yield byte-identical rows");

    let first_bytes = serde_json::to_vec(&first_json).unwrap();
    let second_bytes = serde_json::to_vec(&second_json).unwrap();
    assert_eq!(first_bytes, second_bytes);
}

#[test]
fn normalized_sessions_carry_the_claude_client_id_through_to_canon_model() {
    let home = fixtures_home();
    let scan = canon_ingest::registry::scan_and_parse(claude_entry(), &home, false);
    let outcome = canon_ingest::normalize_rows(&scan.rows);

    let alpha = outcome.sessions.iter().find(|s| s.session.session_id.as_str() == "sess-alpha").expect("sess-alpha session normalized");
    assert_eq!(alpha.session.client, "claude-code");
    assert_eq!(alpha.run.session_id.as_ref().unwrap().as_str(), "sess-alpha");
    assert_eq!(alpha.events.len(), 3, "msg_001 (merged) + msg_002 + the sidechain row");
}

/// s37-execution-graph-topology, end to end over the real fixture home:
/// `agent-sub01.jsonl` is a sidechain transcript whose rows are
/// attributed to `sess-alpha` (unchanged), so `sess-alpha` normalizes to
/// its root run PLUS one child run for that subagent, with the dispatch
/// edge recorded. The sibling assertions that session grouping and event
/// attribution are untouched live in the test above; this one asserts the
/// topology that used to be destroyed at ingest time.
#[test]
fn a_sidechain_transcript_normalizes_to_a_root_run_plus_one_child_run_per_subagent() {
    let home = fixtures_home();
    let scan = canon_ingest::registry::scan_and_parse(claude_entry(), &home, false);
    let outcome = canon_ingest::normalize_rows(&scan.rows);

    let alpha = outcome.sessions.iter().find(|s| s.session.session_id.as_str() == "sess-alpha").expect("sess-alpha session normalized");

    assert_eq!(alpha.run.parent_run_id, None, "the main agent's run is the root");
    assert_eq!(alpha.child_runs.len(), 1, "one child run for the one sidechain subagent transcript: {:?}", alpha.child_runs);
    assert_eq!(alpha.child_runs[0].parent_run_id, Some(alpha.run.run_id), "the subagent's run hangs off the dispatching session's root run");
    assert_eq!(
        alpha.child_runs[0].session_id.as_ref().unwrap().as_str(),
        "sess-alpha",
        "the child run stays on the parent session — lineage capture never re-groups sessions"
    );
    assert_ne!(alpha.child_runs[0].run_id, alpha.run.run_id);
    assert_eq!(alpha.runs().count(), 2);
}

/// s37-execution-graph-topology: the whole normalize pass — root run,
/// child runs, and their parent links — is byte-identical across two
/// ingests of an unchanged fixture home. Without this, the watermark
/// cursor would re-persist every subagent as a brand-new run on each
/// pass.
#[test]
fn normalized_run_topology_is_identical_across_two_ingest_runs() {
    let home = fixtures_home();
    let run_once = || {
        let scan = canon_ingest::registry::scan_and_parse(claude_entry(), &home, false);
        let outcome = canon_ingest::normalize_rows(&scan.rows);
        serde_json::to_value(outcome.sessions.iter().flat_map(canon_ingest::NormalizedSession::runs).collect::<Vec<_>>()).unwrap()
    };

    let first = run_once();
    assert!(first.as_array().is_some_and(|runs| runs.len() >= 2), "sanity: the fixture home must actually produce a child run, or this proves nothing");
    assert_eq!(first, run_once(), "re-ingesting an unchanged fixture home must yield byte-identical run ids and parent links");
}
