// Fixture-schema-lock helper (ReviewS9b finding 2): reads a committed
// Parquet fixture's ACTUAL on-disk schema (column name + order + type)
// via the real `duckdb` CLI's `DESCRIBE SELECT * FROM
// read_parquet(...)` (`-json` mode) — the identical binary
// `scripts/build-fixture-snapshot.ts` already requires on PATH to
// author `fixtures/snapshot/*.parquet` in the first place (that
// script's own module header: "Requires the `duckdb` CLI on PATH"), so
// `fixture-schema.test.ts` adds no new external dependency beyond what
// this package's own dev workflow already needs.
//
// `fixture-schema.test.ts` uses `describeParquetSchema` to compare
// EVERY committed mart fixture against `EXPECTED_MART_SCHEMA` below —
// the S9 SHARED SNAPSHOT CONTRACT `crates/canon-store/sql/views.sql`'s
// own mart_* `SELECT` lists produce, independently re-derived from that
// file (not copied from the fixture-authoring SQL) and cross-checked
// against both `crates/canon-report/tests/snapshot.rs`'s own
// `EXPECTED_CONTRACT` (column names+order) and a live `DESCRIBE SELECT
// * FROM <view>` run over the real views.sql (column types), 2026-07-11.
// A rename/reorder/add/drop/type-change in the committed fixture is a
// bun test FAILURE here — writer (`canon-report`) == fixture is
// enforced on the dashboard side too, not just "renders a row".

import { spawnSync } from "node:child_process";

export interface ParquetColumn {
  name: string;
  type: string;
}

/** True when the native `duckdb` CLI is reachable on PATH. */
export function duckdbAvailable(): boolean {
  return spawnSync("duckdb", ["--version"]).status === 0;
}

/**
 * Runs `DESCRIBE SELECT * FROM read_parquet('<path>')` against the real
 * `duckdb` CLI and returns the parquet file's ACTUAL column
 * `{name, type}` list, in on-disk order — never a value inferred from
 * this repo's SQL sources, only what the committed bytes themselves
 * declare.
 */
export function describeParquetSchema(path: string): ParquetColumn[] {
  const escaped = path.replace(/'/g, "''");
  const result = spawnSync("duckdb", ["-json", "-c", `DESCRIBE SELECT * FROM read_parquet('${escaped}')`], {
    encoding: "utf-8",
  });
  if (result.status !== 0) {
    throw new Error(`duckdb DESCRIBE failed for ${path}: ${result.stderr}`);
  }
  const trimmed = result.stdout.trim();
  const rows = (trimmed ? JSON.parse(trimmed) : []) as Array<{ column_name: string; column_type: string }>;
  return rows.map((r) => ({ name: r.column_name, type: r.column_type }));
}

/**
 * The S9 SHARED SNAPSHOT CONTRACT, per mart — column `{name, type}` in
 * declared `SELECT` order, one entry per `crates/canon-store/sql/
 * views.sql` `mart_*` view (design D5's own panel order). Edit this
 * ONLY alongside a matching, deliberate `views.sql` change (and its
 * `crates/canon-report/tests/snapshot.rs` `EXPECTED_CONTRACT`
 * counterpart) — never to make a drifted fixture pass.
 */
export const EXPECTED_MART_SCHEMA: Record<string, ParquetColumn[]> = {
  mart_trust_matrix: [
    { name: "task_id", type: "VARCHAR" },
    { name: "change_id", type: "VARCHAR" },
    { name: "title", type: "VARCHAR" },
    { name: "task_status", type: "VARCHAR" },
    { name: "covered", type: "BOOLEAN" },
    { name: "green", type: "BOOLEAN" },
    { name: "who", type: "VARCHAR" },
    { name: "evidence_count", type: "BIGINT" },
    { name: "latest_at", type: "TIMESTAMP" },
  ],
  mart_session_costs: [
    { name: "session_id", type: "VARCHAR" },
    { name: "client", type: "VARCHAR" },
    { name: "role", type: "VARCHAR" },
    { name: "workspace_label", type: "VARCHAR" },
    { name: "run_count", type: "BIGINT" },
    { name: "total_cost", type: "DOUBLE" },
    { name: "total_tokens", type: "BIGINT" },
    { name: "first_event_at", type: "TIMESTAMP" },
    { name: "last_event_at", type: "TIMESTAMP" },
  ],
  mart_role_memory: [
    { name: "role", type: "VARCHAR" },
    { name: "regime_key", type: "VARCHAR" },
    { name: "strategy_count", type: "BIGINT" },
    { name: "active_count", type: "BIGINT" },
    { name: "demoted_count", type: "BIGINT" },
    { name: "hit_rate", type: "DOUBLE" },
    { name: "avg_source_trajectories", type: "DOUBLE" },
    { name: "latest_recorded_at", type: "TIMESTAMP" },
  ],
  mart_flywheel_funnel: [
    { name: "role", type: "VARCHAR" },
    { name: "verdicts", type: "BIGINT" },
    { name: "distilled", type: "BIGINT" },
    { name: "retrieved", type: "BIGINT" },
    { name: "applied", type: "BIGINT" },
  ],
  mart_review_burndown: [
    { name: "day", type: "TIMESTAMP" },
    { name: "evidence_faithful", type: "BIGINT" },
    { name: "evidence_divergent", type: "BIGINT" },
    { name: "evidence_not_applicable", type: "BIGINT" },
    { name: "divergence_opened", type: "BIGINT" },
    { name: "divergence_resolved", type: "BIGINT" },
    { name: "divergence_open_running_total", type: "BIGINT" },
  ],
  mart_scope_status: [
    { name: "task_id", type: "VARCHAR" },
    { name: "scenario_id", type: "VARCHAR" },
    { name: "task_status", type: "VARCHAR" },
    { name: "evidence_covered", type: "BOOLEAN" },
    { name: "green", type: "BOOLEAN" },
    { name: "spec_covered", type: "BOOLEAN" },
  ],
  mart_subjects: [
    { name: "domain", type: "VARCHAR" },
    { name: "subject_id", type: "VARCHAR" },
    { name: "title", type: "VARCHAR" },
    { name: "status", type: "VARCHAR" },
    { name: "scenario_count", type: "BIGINT" },
    { name: "covered_scenarios", type: "BIGINT" },
  ],
};
