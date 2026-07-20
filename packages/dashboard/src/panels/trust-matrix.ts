import type { AsyncDuckDBConnection } from "@duckdb/duckdb-wasm";
import { renderTable, type ColumnDef } from "../render-table";

// Panel 1: change/task trust matrix — thin SELECT over mart_trust_matrix
// (crates/canon-store/sql/views.sql:189-226). No mart re-derivation:
// `covered`/`green`/`who` are computed columns of the view itself, not
// recomputed here. `latest_at` is cast to text in SQL to sidestep Arrow
// timestamp-unit ambiguity in the JS layer (display formatting, not
// aggregation).
const QUERY = `
  SELECT
    change_id,
    task_id,
    title,
    task_status,
    covered,
    green,
    who,
    evidence_count,
    CAST(latest_at AS VARCHAR) AS latest_at
  FROM mart_trust_matrix
  ORDER BY change_id, task_id
`;

const COLUMNS: ColumnDef[] = [
  { key: "change_id", label: "Change" },
  { key: "task_id", label: "Task" },
  { key: "title", label: "Title" },
  { key: "task_status", label: "Status" },
  { key: "covered", label: "Covered" },
  { key: "green", label: "Green" },
  { key: "who", label: "Who" },
  { key: "evidence_count", label: "Evidence #" },
  { key: "latest_at", label: "Latest at" },
];

export async function renderTrustMatrix(conn: AsyncDuckDBConnection, container: HTMLElement): Promise<void> {
  const result = await conn.query(QUERY);
  const rows = result.toArray().map((row) => row.toJSON() as Record<string, unknown>);
  renderTable(container, COLUMNS, rows);
}
