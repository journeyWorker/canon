-- Builds packages/dashboard's canonical fixture snapshot: nine small tables
-- (eight hand-authored, one DERIVED — see panel 9) whose column NAME + ORDER + TYPE match
-- crates/canon-store/sql/views.sql's mart_* SELECT lists exactly (S9
-- SHARED SNAPSHOT CONTRACT). Each is exported with the same
-- `COPY "<table>" TO '<table>.parquet' (FORMAT parquet)` shape
-- `canon report --snapshot` uses (design.md D3) — byte-identical filename to
-- table name, no `EXPORT DATABASE`.
--
-- Run via `bun run fixture:build` (packages/dashboard/scripts/
-- build-fixture-snapshot.ts), never by hand — that script also writes the
-- companion manifest.json. Regenerate this file's rows only by editing the
-- CREATE TABLE statements below and re-running the build script; the
-- generated .parquet files are committed binary artifacts.

-- Panel 1: mart_trust_matrix (crates/canon-store/sql/views.sql)
CREATE OR REPLACE TABLE mart_trust_matrix AS
SELECT * FROM (
    VALUES
        ('add-json-export#1.1',      'add-json-export',    'Implement JSON export writer',   'done',        true,  true,  'agent-s9b', 2::BIGINT, TIMESTAMP '2026-07-08 14:03:00'),
        ('add-json-export#1.2',      'add-json-export',    'Add --format flag to CLI',        'done',        true,  false, 'agent-s9b', 1::BIGINT, TIMESTAMP '2026-07-09 09:12:00'),
        ('add-json-export#2.1',      'add-json-export',    'Write integration test',          'in_progress', false, false, NULL,        0::BIGINT, CAST(NULL AS TIMESTAMP)),
        ('fix-retry-backoff#1.1',    'fix-retry-backoff',  'Exponential backoff on 5xx',      'done',        true,  true,  'agent-s3',  3::BIGINT, TIMESTAMP '2026-07-10 18:45:00'),
        ('fix-retry-backoff#1.2',    'fix-retry-backoff',  'Circuit breaker after 5 failures','done',        true,  true,  'agent-s3',  1::BIGINT, TIMESTAMP '2026-07-10 19:02:00')
) AS t(task_id, change_id, title, task_status, covered, green, who, evidence_count, latest_at);

-- Panel 2: mart_session_costs (crates/canon-store/sql/views.sql)
CREATE OR REPLACE TABLE mart_session_costs AS
SELECT * FROM (
    VALUES
        ('sess-0001', 'claude-code', 'unattributed', 'canon-wt/impl',   4::BIGINT, 1.284500::DOUBLE, 182340::BIGINT, TIMESTAMP '2026-07-08 13:55:00', TIMESTAMP '2026-07-08 15:40:00'),
        ('sess-0002', 'codex',       'unattributed', 'canon-wt/impl',   2::BIGINT, 0.412300::DOUBLE,  63210::BIGINT, TIMESTAMP '2026-07-09 08:50:00', TIMESTAMP '2026-07-09 09:20:00'),
        ('sess-0003', 'claude-code', 'reviewer',     'canon-wt/review', 3::BIGINT, 0.902100::DOUBLE, 140012::BIGINT, TIMESTAMP '2026-07-10 17:30:00', TIMESTAMP '2026-07-10 19:10:00')
) AS t(session_id, client, role, workspace_label, run_count, total_cost, total_tokens, first_event_at, last_event_at);

-- Panel 3: mart_role_memory (crates/canon-store/sql/views.sql).
-- `hit_rate` is the view's `round(active_count::DOUBLE / strategy_count,
-- 4)` — NOT a retrieval hit rate — so every row here must carry that
-- exact value, and `active_count + demoted_count = strategy_count`.
-- One demoted-carrying row and one all-active row keep the column from
-- reading like a constant.
CREATE OR REPLACE TABLE mart_role_memory AS
SELECT * FROM (
    VALUES
        ('implementer', 'rust-crate-scaffold',       6::BIGINT, 5::BIGINT, 1::BIGINT, 0.8333::DOUBLE, 2.50::DOUBLE, TIMESTAMP '2026-07-10 20:00:00'),
        ('reviewer',    'spec-compliance-review',    4::BIGINT, 4::BIGINT, 0::BIGINT, 1.0000::DOUBLE, 3.00::DOUBLE, TIMESTAMP '2026-07-09 16:20:00'),
        ('fixer',       'review-finding-remediation',3::BIGINT, 2::BIGINT, 1::BIGINT, 0.6667::DOUBLE, 1.67::DOUBLE, TIMESTAMP '2026-07-11 07:45:00')
) AS t(role, regime_key, strategy_count, active_count, demoted_count, hit_rate, avg_source_trajectories, latest_recorded_at);

-- Panel 4: mart_flywheel_funnel (crates/canon-store/sql/views.sql).
-- s42 (`close-the-open-loops`) task 3.3 split `applied` by the RULE
-- that admitted each count, and the two parts PARTITION it:
-- `applied = applied_attributed + applied_proxy` must hold for every
-- row here, exactly as the view guarantees. `implementer` is mixed,
-- `reviewer` is all-proxy (the shape of a repo that never passes
-- `canon ingest artifacts --run`), `fixer` is all-attributed.
CREATE OR REPLACE TABLE mart_flywheel_funnel AS
SELECT * FROM (
    VALUES
        ('implementer', 18::BIGINT, 6::BIGINT, 5::BIGINT, 3::BIGINT, 2::BIGINT, 1::BIGINT),
        ('reviewer',    11::BIGINT, 4::BIGINT, 3::BIGINT, 2::BIGINT, 0::BIGINT, 2::BIGINT),
        ('fixer',        7::BIGINT, 3::BIGINT, 2::BIGINT, 1::BIGINT, 1::BIGINT, 0::BIGINT)
) AS t(role, verdicts, distilled, retrieved, applied, applied_attributed, applied_proxy);

-- Panel 5: mart_review_burndown (crates/canon-store/sql/views.sql).
-- `day` is `date_trunc('day', "at")` over a TIMESTAMP column, so it stays
-- TIMESTAMP (not DATE) — matched here. `divergence_open_running_total` is
-- the running sum of (opened - resolved) EVENTS, not the count open now,
-- so at least one day must resolve more than it opens: a monotonically
-- rising fixture would read exactly like the current state the panel
-- says it is not.
CREATE OR REPLACE TABLE mart_review_burndown AS
SELECT * FROM (
    VALUES
        (TIMESTAMP '2026-07-07 00:00:00', 3::BIGINT, 1::BIGINT, 0::BIGINT, 2::BIGINT, 0::BIGINT, 2::BIGINT),
        (TIMESTAMP '2026-07-08 00:00:00', 4::BIGINT, 0::BIGINT, 1::BIGINT, 1::BIGINT, 1::BIGINT, 2::BIGINT),
        (TIMESTAMP '2026-07-09 00:00:00', 5::BIGINT, 2::BIGINT, 0::BIGINT, 0::BIGINT, 2::BIGINT, 0::BIGINT),
        (TIMESTAMP '2026-07-10 00:00:00', 6::BIGINT, 1::BIGINT, 0::BIGINT, 3::BIGINT, 1::BIGINT, 2::BIGINT),
        (TIMESTAMP '2026-07-11 00:00:00', 2::BIGINT, 0::BIGINT, 0::BIGINT, 0::BIGINT, 1::BIGINT, 1::BIGINT)
) AS t(day, evidence_faithful, evidence_divergent, evidence_not_applicable, divergence_opened, divergence_resolved, divergence_open_running_total);

-- Panel 6: mart_scope_status (crates/canon-store/sql/views.sql, s24
-- task-scenario-join, re-driven by s45 report-scenario-driver). Driven
-- by the SPEC corpus: one row per authored (project_id, scenario_id),
-- so `project_id` is non-null on every row and `task_id` is the
-- NULLABLE side — a NULL there is a specified scenario no plan task
-- declares, which is the worklist entry. `spec_covered` stays an
-- honest NULL when no porting.coverage overlay exists.
CREATE OR REPLACE TABLE mart_scope_status AS
SELECT * FROM (
    VALUES
        ('root', 'export.json.01', 'JSON export round-trips', 'json-export', 'add-json-export#1.1', CAST(1 AS BIGINT), 'done',        true,  true,  CAST(NULL AS VARCHAR), true),
        ('root', 'export.json.02', 'JSON export rejects a bad schema', CAST(NULL AS VARCHAR), 'add-json-export#2.1', CAST(1 AS BIGINT), 'in_progress', false, false, CAST(NULL AS VARCHAR), CAST(NULL AS BOOLEAN)),
        ('root', 'export.json.03', 'JSON export streams a large corpus', CAST(NULL AS VARCHAR), CAST(NULL AS VARCHAR), CAST(0 AS BIGINT), CAST(NULL AS VARCHAR), true, true, 'faithful', CAST(NULL AS BOOLEAN))
) AS t(project_id, scenario_id, title, subject_id, task_id, declaring_task_count, task_status, evidence_covered, green, scenario_verdict, spec_covered);

-- Panel 7: mart_subjects (crates/canon-store/sql/views.sql, s36
-- subject-domain-loop). Per-domain rollup: subject status x scenario
-- coverage. `covered_scenarios` <= `scenario_count`.
CREATE OR REPLACE TABLE mart_subjects AS
SELECT * FROM (
    VALUES
        ('dev',      'add-json-export',   'JSON export',   'building', 2::BIGINT, 1::BIGINT),
        ('planning', 'fix-retry-backoff', 'Retry backoff', 'shipped',  3::BIGINT, 3::BIGINT)
) AS t(domain, subject_id, title, status, scenario_count, covered_scenarios);

-- Panel 8: mart_review_rounds (crates/canon-store/sql/views.sql, s43
-- findings-are-records). `reviewed_sha` is nullable: a round that reviewed an
-- uncommitted working tree has none, which is the COMMON case, so at least one
-- row must carry NULL or the fixture would show a shape the model calls rare.
--
-- The arithmetic the panel asserts must hold on every row, or the dashboard's
-- own screenshot is a counter-example to its own caveats:
--   introduced_by_sourced + introduced_by_unsourced = findings
--   fix_of_fix <= introduced_by_sourced
--   severity_blocker + severity_should_fix + severity_note = findings
--   disposition_open + fixed + rejected + deferred = findings
--
-- And the rows must exercise all three shapes `introduced_by` can take, or the
-- columns render without demonstrating that they distinguish anything:
--   add-json-export round 1 — sourced 5, unsourced 0, fix_of_fix 0: FULLY
--              sourced and still no match, so a reader cannot read fix_of_fix
--              as "the sourced ones".
--   add-json-export round 2 — fix_of_fix 1 < sourced 3, unsourced 3: a MATCHED
--              introducing commit beside sourced-but-unmatched ones and
--              unsourced ones. This is the row that separates "matched" from
--              "sourced".
--   add-json-export round 4 — sourced 0, unsourced 4: an ALL-UNKNOWN round.
--              fix_of_fix 0 here means "canon does not know", never "no
--              fix-of-fix happened", which is the first of the two
--              UNDER-counts the panel's canonical sentence names.
-- fix-retry-backoff round 1 is a second change, so the table also shows the
-- per-change grain the fix-of-fix scope depends on.
--
-- add-json-export jumps from round 2 to round 4 ON PURPOSE. Round 3 returned
-- MERGEABLE with zero findings, wrote no `Finding`, and so has no row — the
-- live phenomenon both review panels describe (s42's round 12). The fixture
-- carries it so `highest_round` and `rounds_recorded` are not equal on every
-- row, which would leave the column's copy undemonstrated. What the fixture
-- CANNOT show is why the label is missing: this gap comes from a silent
-- round, and an identical gap comes from a change whose only finding is
-- labelled round 7. That is exactly why the panel claims nothing from it
-- (s43 round 6, finding 2).
CREATE OR REPLACE TABLE mart_review_rounds AS
SELECT * FROM (
    VALUES
        ('add-json-export',   1::BIGINT, 'f438c610d9b4a71e0c53e2b8a19d7c46f0b3e185', 5::BIGINT, 1::BIGINT, 2::BIGINT, 2::BIGINT, 0::BIGINT, 4::BIGINT, 1::BIGINT, 0::BIGINT, 0::BIGINT, 5::BIGINT, 0::BIGINT),
        ('add-json-export',   2::BIGINT, CAST(NULL AS VARCHAR),                      6::BIGINT, 2::BIGINT, 1::BIGINT, 3::BIGINT, 1::BIGINT, 4::BIGINT, 0::BIGINT, 1::BIGINT, 1::BIGINT, 3::BIGINT, 3::BIGINT),
        ('add-json-export',   4::BIGINT, CAST(NULL AS VARCHAR),                      4::BIGINT, 0::BIGINT, 1::BIGINT, 3::BIGINT, 2::BIGINT, 1::BIGINT, 0::BIGINT, 1::BIGINT, 0::BIGINT, 0::BIGINT, 4::BIGINT),
        ('fix-retry-backoff', 1::BIGINT, '9c1d0a7b4e6f28315d0ab9c7e4f1268a35bd90c2', 3::BIGINT, 1::BIGINT, 1::BIGINT, 1::BIGINT, 0::BIGINT, 3::BIGINT, 0::BIGINT, 0::BIGINT, 1::BIGINT, 2::BIGINT, 1::BIGINT)
) AS t(change_id, "round", reviewed_sha, findings, severity_blocker, severity_should_fix, severity_note, disposition_open, disposition_fixed, disposition_rejected, disposition_deferred, fix_of_fix, introduced_by_sourced, introduced_by_unsourced);

-- Panel 9: mart_review_totals (crates/canon-store/sql/views.sql, s43
-- findings-are-records round 5). The per-change total a release note copies
-- instead of adding the rows above up by hand.
--
-- The ONLY table in this file that is not hand-authored, and deliberately so.
-- The real view's whole claim is that every column is a sum over
-- mart_review_rounds; a hand-typed VALUES list here would be a SECOND place
-- the same number lives, which is the defect class the view exists to remove
-- — the fixture would then be able to show a total that disagrees with the
-- rows beside it, in a dashboard whose caveats say that cannot happen. So the
-- statement below is the real view's own body, over the fixture rows above.
CREATE OR REPLACE TABLE mart_review_totals AS
SELECT
    change_id,
    CAST(count(*) AS BIGINT)                     AS rounds_recorded,
    CAST(max("round") AS BIGINT)                 AS highest_round,
    CAST(sum(findings) AS BIGINT)                AS findings,
    CAST(sum(severity_blocker) AS BIGINT)        AS severity_blocker,
    CAST(sum(severity_should_fix) AS BIGINT)     AS severity_should_fix,
    CAST(sum(severity_note) AS BIGINT)           AS severity_note,
    CAST(sum(disposition_open) AS BIGINT)        AS disposition_open,
    CAST(sum(disposition_fixed) AS BIGINT)       AS disposition_fixed,
    CAST(sum(disposition_rejected) AS BIGINT)    AS disposition_rejected,
    CAST(sum(disposition_deferred) AS BIGINT)    AS disposition_deferred,
    CAST(sum(fix_of_fix) AS BIGINT)              AS fix_of_fix,
    CAST(sum(introduced_by_sourced) AS BIGINT)   AS introduced_by_sourced,
    CAST(sum(introduced_by_unsourced) AS BIGINT) AS introduced_by_unsourced
FROM mart_review_rounds
GROUP BY change_id
ORDER BY change_id;

COPY "mart_trust_matrix"    TO 'fixtures/snapshot/mart_trust_matrix.parquet'    (FORMAT parquet);
COPY "mart_session_costs"   TO 'fixtures/snapshot/mart_session_costs.parquet'   (FORMAT parquet);
COPY "mart_role_memory"     TO 'fixtures/snapshot/mart_role_memory.parquet'     (FORMAT parquet);
COPY "mart_flywheel_funnel" TO 'fixtures/snapshot/mart_flywheel_funnel.parquet' (FORMAT parquet);
COPY "mart_review_burndown" TO 'fixtures/snapshot/mart_review_burndown.parquet' (FORMAT parquet);
COPY "mart_scope_status"    TO 'fixtures/snapshot/mart_scope_status.parquet'    (FORMAT parquet);
COPY "mart_subjects"        TO 'fixtures/snapshot/mart_subjects.parquet'        (FORMAT parquet);
COPY "mart_review_rounds"   TO 'fixtures/snapshot/mart_review_rounds.parquet'   (FORMAT parquet);
COPY "mart_review_totals"   TO 'fixtures/snapshot/mart_review_totals.parquet'   (FORMAT parquet);
