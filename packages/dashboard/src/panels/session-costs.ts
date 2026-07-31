import type { AsyncDuckDBConnection } from "@duckdb/duckdb-wasm";
import { renderTable, type ColumnDef } from "../render-table";

// Panel 2: session costs — thin SELECT over the `mart_session_costs`
// view (`crates/canon-store/sql/views.sql`).
//
// s42 (`close-the-open-loops`) re-review: this header used to read
// "session costs by role/repo/session". The view has no repo column.
// It groups by `(session_id, client, role, workspace_label)`, and
// `workspace_label` is the `token_usage` event's own field standing in
// for the repo the mart cannot name — the view's own comment says so.
// The column keeps that honest name here; this panel does not rename or
// reinterpret it.
const QUERY = `
  SELECT
    session_id,
    client,
    role,
    workspace_label,
    run_count,
    total_cost,
    total_tokens,
    CAST(first_event_at AS VARCHAR) AS first_event_at,
    CAST(last_event_at AS VARCHAR) AS last_event_at
  FROM mart_session_costs
  ORDER BY session_id, workspace_label
`;

const COLUMNS: ColumnDef[] = [
  { key: "session_id", label: "Session" },
  { key: "client", label: "Client" },
  { key: "role", label: "Role" },
  { key: "workspace_label", label: "Workspace" },
  { key: "run_count", label: "Runs" },
  { key: "total_cost", label: "Cost ($)", format: (v) => (typeof v === "number" ? v.toFixed(4) : String(v)) },
  { key: "total_tokens", label: "Tokens" },
  { key: "first_event_at", label: "First event" },
  { key: "last_event_at", label: "Last event" },
];

export async function renderSessionCosts(conn: AsyncDuckDBConnection, container: HTMLElement): Promise<void> {
  const result = await conn.query(QUERY);
  const rows = result.toArray().map((row) => row.toJSON() as Record<string, unknown>);
  renderTable(container, COLUMNS, rows);
}
