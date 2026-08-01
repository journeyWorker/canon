//! s43 `findings-are-records` round 6, the two findings this file
//! exists for.
//!
//! **Finding 1 — the guarantee was false across processes.** The
//! `## Review totals` panel says the per-change total and the
//! per-round rows it totals "cannot disagree". That was argued from
//! the SQL — `mart_review_totals`' only `FROM` is `mart_review_rounds`
//! — and the argument is sound about the QUERY and silent about the
//! READ. `report()` fetched the two marts with two `run_query` calls,
//! each a fresh `duckdb` process re-globbing a LIVE ledger, so a
//! finding written between them reached the later panel and not the
//! earlier one. Nine marts meant nine such windows, and `snapshot()`
//! copied nine parquet files the same way.
//!
//! `canon_report::query`'s pinned batch closes it: one process, and a
//! corpus materialized once before any mart is computed
//! (`PIN_CORPUS_SQL`). The first test below does not RACE for that —
//! races prove nothing when they pass. It interleaves the write
//! DIRECTLY, using DuckDB itself to append a finding to the ledger
//! between the rounds read and the totals read, in the same batch.
//! Unpinned, that batch is exactly the defect: the rounds table
//! reports one round and two findings while the totals row reports
//! two rounds and three.
//!
//! **Finding 2 — the panel inferred a clean round from a missing
//! label.** It said `highest_round > rounds_recorded` witnessed a
//! round that ran and found nothing. `round` is author-supplied and
//! canon enforces no contiguous numbering, so the gap witnesses only
//! that some lower label has no row. The second test builds the
//! counter-example the corrected prose names: one change, one finding,
//! labelled round 7. Same gap, no silent round anywhere.

mod support;

use canon_model::envelope::{Actor, Envelope, RecordKind};
use canon_model::ids::{ChangeId, RoleId};
use canon_model::records::{Finding, FindingSeverity};
use canon_report::roots::Roots;
use canon_report::{marts, query, ReportInputs};
use canon_store::git_tier::GitTier;
use canon_store::tier::Tier;
use chrono::{TimeZone, Utc};

fn actor() -> Actor {
    Actor::new("one-corpus-fixture", RoleId::parse("reviewer").unwrap())
}

fn finding(change: &ChangeId, round: u32, seq: u32, hour: u32, summary: &str) -> Finding {
    Finding::new(
        Envelope::new(1, RecordKind::Finding, Utc.with_ymd_and_hms(2026, 7, 1, hour, 0, 0).single().unwrap(), actor()),
        change.clone(),
        round,
        seq,
        FindingSeverity::Blocker,
        "reviewer1",
        summary,
    )
}

fn n(row: &query::Row, column: &str) -> i64 {
    row.get(column).and_then(|value| value.as_i64()).unwrap_or_else(|| panic!("{column} is not an integer in {row:?}"))
}

/// A `Finding` JSON document, as a single-line SQL string literal —
/// the payload the interleaved `COPY` writes into the git tier's
/// `kind=finding/` directory mid-batch.
///
/// Written through `GitTier` first, into a throwaway root, so the
/// bytes are a REAL record envelope produced by the same writer
/// `canon finding` uses, never a hand-typed JSON blob that
/// `stg_git_records` might parse differently.
fn record_json_literal(scratch: &std::path::Path, record: &Finding) -> String {
    GitTier::new(scratch).write(record).unwrap();
    let path = walk_one_json(scratch);
    std::fs::read_to_string(path).unwrap().replace('\n', " ").replace('\'', "''")
}

fn walk_one_json(dir: &std::path::Path) -> std::path::PathBuf {
    for entry in std::fs::read_dir(dir).unwrap().flatten() {
        let path = entry.path();
        if path.is_dir() {
            return walk_one_json(&path);
        }
        if path.extension().is_some_and(|ext| ext == "json") {
            return path;
        }
    }
    panic!("no JSON record written under {}", dir.display());
}

/// The interleaving, run directly rather than raced: `SELECT` the
/// rounds table, WRITE a new finding to the ledger, `SELECT` the
/// totals row — three statements, in that order, in one batch.
///
/// Unpinned this is the round-6 blocker in miniature and the numbers
/// come out inconsistent (verified against `duckdb` v1.5.4: the rounds
/// read returns round 1 with 2 findings, the totals read returns
/// `rounds_recorded = 2`, `findings = 3`). Pinned, both reads see the
/// corpus as it was before the batch began, so they agree — and the
/// mid-run write reaches NEITHER, which is the honest half of the
/// claim the panel makes.
#[test]
fn a_write_landing_between_two_panel_reads_reaches_neither_of_them() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let git_root = dir.join_git();
    let change = ChangeId::parse("s43-atomic-read").unwrap();

    let tier = GitTier::new(&git_root);
    tier.write(&finding(&change, 1, 1, 9, "round 1, seq 1")).unwrap();
    tier.write(&finding(&change, 1, 2, 10, "round 1, seq 2")).unwrap();

    // The record the interleaved write appends: a SECOND round, so it
    // moves `rounds_recorded` and `findings` at once and a panel that
    // saw it could not be mistaken for one that did not.
    let scratch = dir.path().join("scratch");
    let interloper = record_json_literal(&scratch, &finding(&change, 2, 1, 11, "written mid-read"));
    let interloper_path = git_root.join("kind=finding").join("zz-interleaved.json");

    let roots = Roots::new(&git_root, dir.path().join("r2"), dir.path().join("learn"));
    let statements = [
        marts::REVIEW_ROUNDS.sql(),
        // `COPY … (FORMAT csv, QUOTE '')` writes the document verbatim
        // — one line, no quoting, no header — which is exactly what
        // `stg_git_records`' `read_text` then reads back. The trailing
        // `SELECT` keeps this element one result set, so the batch's
        // arity still matches.
        format!(
            "COPY (SELECT '{interloper}') TO '{}' (FORMAT csv, HEADER false, QUOTE '', DELIMITER '\\x01'); SELECT 1 AS wrote;",
            interloper_path.display()
        ),
        marts::REVIEW_TOTALS.sql(),
    ];

    let sets = query::run_pinned_queries(&roots, &statements).unwrap();
    let (rounds, totals) = (&sets[0], &sets[2]);

    // The write really happened — otherwise this test would pass
    // against a `COPY` that silently did nothing.
    assert!(interloper_path.is_file(), "the interleaved write must actually land on disk");
    assert!(
        std::fs::read_to_string(&interloper_path).unwrap().contains("written mid-read"),
        "the interleaved file must be the finding, not an empty or quoted stand-in"
    );

    assert_eq!(rounds.len(), 1, "the rounds read must see the corpus as it was at the start of the batch: {rounds:?}");
    assert_eq!(totals.len(), 1, "one change, one totals row: {totals:?}");
    let total = &totals[0];

    assert_eq!(n(total, "rounds_recorded"), rounds.len() as i64, "the total must count exactly the rounds the reader is shown");
    assert_eq!(
        n(total, "findings"),
        rounds.iter().map(|row| n(row, "findings")).sum::<i64>(),
        "the total must sum exactly the findings the reader is shown"
    );
    assert_eq!(n(total, "highest_round"), 1, "the mid-read write added round 2; neither panel may show it");
    assert_eq!(n(total, "findings"), 2, "two findings existed when the batch began, and a third was written during it");
}

/// The same interleaving through the PUBLIC surface: `fetch_all` is
/// what `report()` calls, and a caller must not have to know about
/// pinning to get panels that agree.
#[test]
fn fetch_all_returns_every_panel_from_one_corpus() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let git_root = dir.join_git();
    let change = ChangeId::parse("s43-atomic-read").unwrap();

    let tier = GitTier::new(&git_root);
    tier.write(&finding(&change, 1, 1, 9, "round 1, seq 1")).unwrap();
    tier.write(&finding(&change, 2, 1, 10, "round 2, seq 1")).unwrap();
    tier.write(&finding(&change, 2, 2, 11, "round 2, seq 2")).unwrap();

    let roots = Roots::new(&git_root, dir.path().join("r2"), dir.path().join("learn"));
    let all = marts::fetch_all(&roots).unwrap();

    // Every field must carry ITS OWN mart. The batch is consumed
    // positionally, so a spec list and a struct that drifted apart
    // would mislabel whole panels without any query failing.
    assert_eq!(all.trust_matrix.columns, marts::TRUST_MATRIX_COLUMNS);
    assert_eq!(all.session_costs.columns, marts::SESSION_COSTS_COLUMNS);
    assert_eq!(all.role_memory.columns, marts::ROLE_MEMORY_COLUMNS);
    assert_eq!(all.flywheel_funnel.columns, marts::FLYWHEEL_FUNNEL_COLUMNS);
    assert_eq!(all.review_burndown.columns, marts::REVIEW_BURNDOWN_COLUMNS);
    assert_eq!(all.scope_status.columns, marts::SCOPE_STATUS_COLUMNS);
    assert_eq!(all.subjects.columns, marts::SUBJECTS_COLUMNS);
    assert_eq!(all.review_rounds.columns, marts::REVIEW_ROUNDS_COLUMNS);
    assert_eq!(all.review_totals.columns, marts::REVIEW_TOTALS_COLUMNS);

    assert_eq!(all.review_rounds.rows.len(), 2, "two rounds recorded a finding: {:?}", all.review_rounds.rows);
    let total = &all.review_totals.rows[0];
    assert_eq!(n(total, "rounds_recorded"), 2);
    assert_eq!(n(total, "findings"), all.review_rounds.rows.iter().map(|row| n(row, "findings")).sum::<i64>());
}

/// Round 6 finding 2's counter-example, built rather than argued.
///
/// One change, one finding, labelled round 7. `rounds_recorded = 1`
/// and `highest_round = 7` — the exact shape the retired prose read as
/// six rounds that ran and found nothing. Nothing ran. The gap is a
/// numbering choice, and the rendered report may not claim otherwise.
#[test]
fn a_lone_round_7_finding_makes_the_same_gap_six_silent_rounds_would() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    let git_root = dir.join_git();
    let change = ChangeId::parse("s43-round-7-only").unwrap();

    GitTier::new(&git_root).write(&finding(&change, 7, 1, 9, "the only finding this change ever drew")).unwrap();

    let roots = Roots::new(&git_root, dir.path().join("r2"), dir.path().join("learn"));
    let totals = marts::fetch_review_totals(&roots).unwrap();
    assert_eq!(totals.rows.len(), 1, "one change, one totals row: {:?}", totals.rows);
    let total = &totals.rows[0];

    assert_eq!(n(total, "rounds_recorded"), 1, "one round recorded a finding");
    assert_eq!(n(total, "highest_round"), 7, "the author labelled it 7, and canon stores the label it was given");
    assert!(
        n(total, "highest_round") > n(total, "rounds_recorded"),
        "this corpus must reproduce the gap the retired prose read as silent rounds"
    );

    let report = canon_report::report(&ReportInputs::new(dir.path(), roots)).unwrap();
    assert!(report.contains("| s43-round-7-only | 1 | 7 |"), "the totals row must render the gap: {report}");

    // The rendered page may not tell a reader that six rounds ran, in
    // any of the shapes it used to.
    for claim in [
        "witnesses a round in between that recorded nothing",
        "witnesses a silent round",
        "the only signal in the corpus that a clean round happened",
    ] {
        assert!(!report.contains(claim), "the report still infers a round from a numbering gap: {claim:?}");
    }
    assert!(
        report.contains("Neither the gap nor its absence evidences a round that RAN"),
        "the report must say outright what this row's gap does not mean"
    );
}

/// `tempfile::TempDir` has no `join`; every test above wants the same
/// `<tmp>/ledger` git root, so the extension keeps that literal in one
/// place rather than in four.
trait GitRoot {
    fn join_git(&self) -> std::path::PathBuf;
}

impl GitRoot for tempfile::TempDir {
    fn join_git(&self) -> std::path::PathBuf {
        self.path().join("ledger")
    }
}
