import type { AsyncDuckDBConnection } from "@duckdb/duckdb-wasm";
import { renderTable, type ColumnDef } from "../render-table";

// Panel 4: flywheel health funnel (verdicts -> distilled -> retrieved ->
// applied) — thin SELECT over mart_flywheel_funnel
// (crates/canon-store/sql/views.sql). One row per role; the
// funnel counts are already fully aggregated by the view.
//
// s42 (`close-the-open-loops`) task 3.3: `applied` is admitted by one of
// two rules per run, and the panel shows WHICH — an attribution (the run
// has a resolved trajectory of that role stamped with its own run_id) and
// a proxy (the run merely reached a terminal status) make different
// claims, so a single mixed number would assert more than the data
// carries. The two split columns partition `applied` exactly.
const QUERY = `
  SELECT role, verdicts, distilled, retrieved, applied, applied_attributed, applied_proxy
  FROM mart_flywheel_funnel
  ORDER BY role
`;

const COLUMNS: ColumnDef[] = [
  { key: "role", label: "Role" },
  { key: "verdicts", label: "Verdicts" },
  { key: "distilled", label: "Distilled" },
  { key: "retrieved", label: "Retrieved" },
  { key: "applied", label: "Applied" },
  { key: "applied_attributed", label: "Applied (attributed)" },
  { key: "applied_proxy", label: "Applied (proxy)" },
];

export async function renderFlywheelFunnel(conn: AsyncDuckDBConnection, container: HTMLElement): Promise<void> {
  const result = await conn.query(QUERY);
  const rows = result.toArray().map((row) => row.toJSON() as Record<string, unknown>);
  renderTable(container, COLUMNS, rows);
}
