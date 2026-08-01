//! Acceptance: "report generation on a completely fresh repo (no
//! strategies/trajectories, no r2 export, no git-tier records at all)
//! succeeds and renders empty panels" — the P1 regression guard for
//! `Roots::ensure_seeded` seeding `strategies/`/`trajectories/` at the
//! WRONG glob depth (`crates/canon-store/sql/views.sql`'s
//! `stg_strategy_items`/`stg_trajectories` read `read_parquet` with a
//! FOUR-directory-level glob; a seed at three levels leaves that glob
//! matching zero files, which `read_parquet` hard-errors on — aborting
//! `canon report` on the very first real-world run against a brand new
//! consumer repo, never exercised by the fixture-corpus-driven tests
//! elsewhere in this crate since that fixture always populates the
//! learn store).

mod support;

use canon_report::{report, ReportInputs, Roots};

#[test]
fn report_generation_on_a_completely_fresh_repo_succeeds_with_empty_panels() {
    if !support::duckdb_available() {
        eprintln!("skipping: `duckdb` CLI not found on PATH");
        return;
    }
    let dir = tempfile::tempdir().unwrap();
    // No fixture corpus at all — every root is a bare, never-written-to
    // directory (some don't even exist yet on disk), exactly a brand
    // new `canon init`-ed repo before anything has ever routed a
    // record to git/r2, or distilled a strategy/trajectory.
    let roots = Roots::new(dir.path().join("ledger"), dir.path().join("r2"), dir.path().join("learn"));
    let inputs = ReportInputs::new(dir.path(), roots);

    let content = report(&inputs).expect(
        "report generation over a completely fresh repo must not crash on an empty read_parquet glob — \
         Roots::ensure_seeded must seed strategies/trajectories at the exact 4-level depth sql/views.sql globs",
    );

    assert!(content.starts_with("# canon report\n"));
    // Every one of the nine marts has zero rows over a corpus this
    // empty — each panel renders the documented "no rows" placeholder,
    // never a missing section or a panic.
    assert_eq!(content.matches("_No rows._").count(), 9, "all nine mart panels must render empty, not crash:\n{content}");
    assert!(content.contains("## Role memory\n\n"));
    assert!(content.contains("## Flywheel funnel\n\n"));
    assert!(content.contains("## Scope status\n\n"));
    assert!(content.contains("## Subjects\n\n"));
    // s43 (`findings-are-records`): a repo with no `finding` records at
    // all must still render the panel and its caveats. The caveats
    // describe the METRIC, so the reader most likely to misread an
    // empty fix-of-fix column is exactly the one who sees zero rows —
    // including the reader who assumes zero rows means zero rounds,
    // which is the one reading a corpus this empty.
    assert!(content.contains("## Review rounds\n\n"));
    assert!(
        content.contains(canon_report::render::FIX_OF_FIX_MEANING),
        "the empty review-rounds panel must still state the one canonical sentence verbatim"
    );
    assert!(
        content.contains("counts the rounds that FOUND something, never the rounds RUN"),
        "the empty review-rounds panel must still say a clean round writes no row"
    );
    // s43 round 5: the totals panel is the one a release note copies
    // from, so an empty corpus must still render it with the caveat
    // that makes its headline column safe to copy — `rounds_recorded`
    // is rounds that recorded a finding, and zero of them is not
    // evidence that zero rounds ran.
    assert!(content.contains("## Review totals\n\n"));
    assert!(
        content.contains("this column counts the rounds that FOUND something, never the rounds RUN"),
        "the empty review-totals panel must still say `rounds_recorded` is not the rounds run"
    );
    assert!(
        content.contains(canon_report::render::FIX_OF_FIX_MEANING),
        "the empty review-totals panel must still state the one canonical sentence verbatim"
    );
}
