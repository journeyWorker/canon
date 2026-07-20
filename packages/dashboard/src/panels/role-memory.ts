import type { AsyncDuckDBConnection } from "@duckdb/duckdb-wasm";
import { renderTable, type ColumnDef } from "../render-table";

// Panel 3: role memory (strategies, hit rate, effect) — thin SELECT over
// mart_role_memory (crates/canon-store/sql/views.sql:298-310).
// `avg_source_trajectories` is the view's own named "effect" proxy; kept
// under its honest column name, not relabeled "effect" here.
const QUERY = `
  SELECT
    role,
    regime_key,
    strategy_count,
    active_count,
    demoted_count,
    hit_rate,
    avg_source_trajectories,
    CAST(latest_recorded_at AS VARCHAR) AS latest_recorded_at
  FROM mart_role_memory
  ORDER BY role, regime_key
`;

const COLUMNS: ColumnDef[] = [
  { key: "role", label: "Role" },
  { key: "regime_key", label: "Regime" },
  { key: "strategy_count", label: "Strategies" },
  { key: "active_count", label: "Active" },
  { key: "demoted_count", label: "Demoted" },
  { key: "hit_rate", label: "Hit rate", format: (v) => (typeof v === "number" ? `${(v * 100).toFixed(1)}%` : String(v)) },
  { key: "avg_source_trajectories", label: "Avg source trajectories" },
  { key: "latest_recorded_at", label: "Latest recorded at" },
];

export async function renderRoleMemory(conn: AsyncDuckDBConnection, container: HTMLElement): Promise<void> {
  const result = await conn.query(QUERY);
  const rows = result.toArray().map((row) => row.toJSON() as Record<string, unknown>);
  renderTable(container, COLUMNS, rows);
}
