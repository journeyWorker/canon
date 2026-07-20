//! Full scan -> parse -> normalize pipeline over a fixture `$HOME`
//! layout (S3 Change item 8): the omp adapter's dual `.omp`/`.pi` root
//! union, content-derived `session_id` (never the filename), per-line
//! skip on a corrupt line, and idempotence across two full ingest
//! runs.

use std::path::PathBuf;

fn fixtures_home() -> PathBuf {
    PathBuf::from(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures/home")
}

fn omp_entry() -> &'static canon_ingest::AdapterEntry {
    &canon_ingest::registry()[0]
}

#[test]
fn omp_adapter_scans_dual_root_home_and_derives_session_ids_from_content() {
    let home = fixtures_home();
    let result = canon_ingest::registry::scan_and_parse(omp_entry(), &home, false);

    assert_eq!(result.client_id, "omp");
    // Three fixture files: two under `.omp/agent/sessions/…`, one under
    // the legacy `.pi/agent/sessions/…` root — proves the dual-root
    // union (`adapters::omp` module doc).
    assert_eq!(result.files_scanned.len(), 3, "found: {:?}", result.files_scanned);

    let session_ids: std::collections::BTreeSet<_> = result.rows.iter().map(|r| r.session_id.clone()).collect();
    assert_eq!(session_ids, std::collections::BTreeSet::from(["omp_ses_alpha_7c3f9a".to_string(), "omp_ses_beta_c91e2d".to_string(), "pi_ses_gamma_2f88b1".to_string(),]));

    // Content-derived, never the filename: none of the fixture
    // filenames (`session-alpha`, `session-beta-corrupt`,
    // `session-gamma`) match any derived session_id.
    for row in &result.rows {
        assert!(!row.session_id.contains("session-"), "session_id `{}` looks filename-derived, not content-derived", row.session_id);
    }

    // `session-beta-corrupt.jsonl` has 3 message lines: one truncated/
    // corrupt line sits between two valid assistant messages — the
    // corrupt line is skipped, the two valid ones survive.
    let beta_rows: Vec<_> = result.rows.iter().filter(|r| r.session_id == "omp_ses_beta_c91e2d").collect();
    assert_eq!(beta_rows.len(), 2, "corrupt line should be skipped, not abort the file: {beta_rows:?}");
    assert_eq!(beta_rows[0].tokens.input, 88);
    assert_eq!(beta_rows[1].tokens.input, 95);

    // Missing `provider` on the `.pi`-root fixture is inferred from
    // the model name (`claude-opus-4` -> `anthropic`), matching the
    // ported `pi.rs:163-168` fallback.
    let gamma_row = result.rows.iter().find(|r| r.session_id == "pi_ses_gamma_2f88b1").unwrap();
    assert_eq!(gamma_row.provider_id, "anthropic");
}

#[test]
fn full_pipeline_is_idempotent_across_two_ingest_runs() {
    let home = fixtures_home();

    let run_once = || {
        let scan = canon_ingest::registry::scan_and_parse(omp_entry(), &home, false);
        canon_ingest::normalize_rows(&scan.rows)
    };

    let first = run_once();
    let second = run_once();

    assert!(!first.sessions.is_empty());
    assert_eq!(first.skipped_rows, second.skipped_rows);

    let first_json = serde_json::to_value(&first).unwrap();
    let second_json = serde_json::to_value(&second).unwrap();
    assert_eq!(first_json, second_json, "re-ingesting an unchanged fixture home must yield byte-identical normalized output");

    let first_bytes = serde_json::to_vec(&first_json).unwrap();
    let second_bytes = serde_json::to_vec(&second_json).unwrap();
    assert_eq!(first_bytes, second_bytes);
}

#[test]
fn normalized_sessions_carry_the_content_derived_session_id_through_to_canon_model() {
    let home = fixtures_home();
    let scan = canon_ingest::registry::scan_and_parse(omp_entry(), &home, false);
    let outcome = canon_ingest::normalize_rows(&scan.rows);

    let alpha = outcome.sessions.iter().find(|s| s.session.session_id.as_str() == "omp_ses_alpha_7c3f9a").expect("alpha session normalized");
    assert_eq!(alpha.session.client, "omp");
    assert_eq!(alpha.run.session_id.as_ref().unwrap().as_str(), "omp_ses_alpha_7c3f9a");
    assert_eq!(alpha.events.len(), 2);
    for (idx, event) in alpha.events.iter().enumerate() {
        assert_eq!(event.run_id, alpha.run.run_id);
        assert_eq!(event.seq, (idx + 1) as u64);
        assert_eq!(event.label, canon_ingest::normalize::TOKEN_USAGE_LABEL);
    }
}
