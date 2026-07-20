-- Builds packages/dashboard's canonical fixture snapshot: seven small, hand-
-- authored tables whose column NAME + ORDER + TYPE match
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

-- Panel 1: mart_trust_matrix (crates/canon-store/sql/views.sql:189-226)
CREATE OR REPLACE TABLE mart_trust_matrix AS
SELECT * FROM (
    VALUES
        ('add-json-export#1.1',      'add-json-export',    'Implement JSON export writer',   'done',        true,  true,  'agent-s9b', 2::BIGINT, TIMESTAMP '2026-07-08 14:03:00'),
        ('add-json-export#1.2',      'add-json-export',    'Add --format flag to CLI',        'done',        true,  false, 'agent-s9b', 1::BIGINT, TIMESTAMP '2026-07-09 09:12:00'),
        ('add-json-export#2.1',      'add-json-export',    'Write integration test',          'in_progress', false, false, NULL,        0::BIGINT, CAST(NULL AS TIMESTAMP)),
        ('fix-retry-backoff#1.1',    'fix-retry-backoff',  'Exponential backoff on 5xx',      'done',        true,  true,  'agent-s3',  3::BIGINT, TIMESTAMP '2026-07-10 18:45:00'),
        ('fix-retry-backoff#1.2',    'fix-retry-backoff',  'Circuit breaker after 5 failures','done',        true,  true,  'agent-s3',  1::BIGINT, TIMESTAMP '2026-07-10 19:02:00')
) AS t(task_id, change_id, title, task_status, covered, green, who, evidence_count, latest_at);

-- Panel 2: mart_session_costs (crates/canon-store/sql/views.sql:247-285)
CREATE OR REPLACE TABLE mart_session_costs AS
SELECT * FROM (
    VALUES
        ('sess-0001', 'claude-code', 'unattributed', 'canon-wt/impl',   4::BIGINT, 1.284500::DOUBLE, 182340::BIGINT, TIMESTAMP '2026-07-08 13:55:00', TIMESTAMP '2026-07-08 15:40:00'),
        ('sess-0002', 'codex',       'unattributed', 'canon-wt/impl',   2::BIGINT, 0.412300::DOUBLE,  63210::BIGINT, TIMESTAMP '2026-07-09 08:50:00', TIMESTAMP '2026-07-09 09:20:00'),
        ('sess-0003', 'claude-code', 'reviewer',     'canon-wt/review', 3::BIGINT, 0.902100::DOUBLE, 140012::BIGINT, TIMESTAMP '2026-07-10 17:30:00', TIMESTAMP '2026-07-10 19:10:00')
) AS t(session_id, client, role, workspace_label, run_count, total_cost, total_tokens, first_event_at, last_event_at);

-- Panel 3: mart_role_memory (crates/canon-store/sql/views.sql:298-310)
CREATE OR REPLACE TABLE mart_role_memory AS
SELECT * FROM (
    VALUES
        ('implementer', 'rust-crate-scaffold',       6::BIGINT, 5::BIGINT, 1::BIGINT, 0.8333::DOUBLE, 2.50::DOUBLE, TIMESTAMP '2026-07-10 20:00:00'),
        ('reviewer',    'spec-compliance-review',    4::BIGINT, 4::BIGINT, 0::BIGINT, 1.0000::DOUBLE, 3.00::DOUBLE, TIMESTAMP '2026-07-09 16:20:00'),
        ('fixer',       'review-finding-remediation',3::BIGINT, 2::BIGINT, 1::BIGINT, 0.6667::DOUBLE, 1.67::DOUBLE, TIMESTAMP '2026-07-11 07:45:00')
) AS t(role, regime_key, strategy_count, active_count, demoted_count, hit_rate, avg_source_trajectories, latest_recorded_at);

-- Panel 4: mart_flywheel_funnel (crates/canon-store/sql/views.sql:327-368)
CREATE OR REPLACE TABLE mart_flywheel_funnel AS
SELECT * FROM (
    VALUES
        ('implementer', 18::BIGINT, 6::BIGINT, 9::BIGINT, 14::BIGINT),
        ('reviewer',    11::BIGINT, 4::BIGINT, 5::BIGINT,  9::BIGINT),
        ('fixer',        7::BIGINT, 3::BIGINT, 4::BIGINT,  5::BIGINT)
) AS t(role, verdicts, distilled, retrieved, applied);

-- Panel 5: mart_review_burndown (crates/canon-store/sql/views.sql:377-402).
-- `day` is `date_trunc('day', "at")` over a TIMESTAMP column, so it stays
-- TIMESTAMP (not DATE) — matched here. `divergence_open_running_total` is
-- the running sum of (opened - resolved); verified by hand below.
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
-- task-scenario-join). `spec_covered` is a nullable BOOLEAN (an honest
-- NULL when no porting.coverage overlay exists for the scenario).
CREATE OR REPLACE TABLE mart_scope_status AS
SELECT * FROM (
    VALUES
        ('add-json-export#1.1', 'export.json.01', 'done',        true,  true,  true),
        ('add-json-export#2.1', 'export.json.02', 'in_progress', false, false, CAST(NULL AS BOOLEAN))
) AS t(task_id, scenario_id, task_status, evidence_covered, green, spec_covered);

-- Panel 7: mart_subjects (crates/canon-store/sql/views.sql, s36
-- subject-domain-loop). Per-domain rollup: subject status x scenario
-- coverage. `covered_scenarios` <= `scenario_count`.
CREATE OR REPLACE TABLE mart_subjects AS
SELECT * FROM (
    VALUES
        ('dev',      'add-json-export',   'JSON export',   'building', 2::BIGINT, 1::BIGINT),
        ('planning', 'fix-retry-backoff', 'Retry backoff', 'shipped',  3::BIGINT, 3::BIGINT)
) AS t(domain, subject_id, title, status, scenario_count, covered_scenarios);

COPY "mart_trust_matrix"    TO 'fixtures/snapshot/mart_trust_matrix.parquet'    (FORMAT parquet);
COPY "mart_session_costs"   TO 'fixtures/snapshot/mart_session_costs.parquet'   (FORMAT parquet);
COPY "mart_role_memory"     TO 'fixtures/snapshot/mart_role_memory.parquet'     (FORMAT parquet);
COPY "mart_flywheel_funnel" TO 'fixtures/snapshot/mart_flywheel_funnel.parquet' (FORMAT parquet);
COPY "mart_review_burndown" TO 'fixtures/snapshot/mart_review_burndown.parquet' (FORMAT parquet);
COPY "mart_scope_status"    TO 'fixtures/snapshot/mart_scope_status.parquet'    (FORMAT parquet);
COPY "mart_subjects"        TO 'fixtures/snapshot/mart_subjects.parquet'        (FORMAT parquet);
