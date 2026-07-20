import type { AsyncDuckDBConnection } from "@duckdb/duckdb-wasm";
import { renderTable, type ColumnDef } from "../render-table";

// Panel 4: flywheel health funnel (verdicts -> distilled -> retrieved ->
// applied) — thin SELECT over mart_flywheel_funnel
// (crates/canon-store/sql/views.sql:327-368). One row per role; the
// funnel counts are already fully aggregated by the view.
const QUERY = `
  SELECT role, verdicts, distilled, retrieved, applied
  FROM mart_flywheel_funnel
  ORDER BY role
`;

const COLUMNS: ColumnDef[] = [
  { key: "role", label: "Role" },
  { key: "verdicts", label: "Verdicts" },
  { key: "distilled", label: "Distilled" },
  { key: "retrieved", label: "Retrieved" },
  { key: "applied", label: "Applied" },
];

export async function renderFlywheelFunnel(conn: AsyncDuckDBConnection, container: HTMLElement): Promise<void> {
  const result = await conn.query(QUERY);
  const rows = result.toArray().map((row) => row.toJSON() as Record<string, unknown>);
  renderTable(container, COLUMNS, rows);
}
