import type { AsyncDuckDBConnection } from "@duckdb/duckdb-wasm";
import { renderTable, type ColumnDef } from "../render-table";

// Panel 5: review-feedback burn-down over time — thin SELECT over
// mart_review_burndown (crates/canon-store/sql/views.sql:377-402).
// `divergence_open_running_total` is already the view's own running-sum
// window column; this panel does not recompute it.
const QUERY = `
  SELECT
    CAST(day AS VARCHAR) AS day,
    evidence_faithful,
    evidence_divergent,
    evidence_not_applicable,
    divergence_opened,
    divergence_resolved,
    divergence_open_running_total
  FROM mart_review_burndown
  ORDER BY day
`;

const COLUMNS: ColumnDef[] = [
  { key: "day", label: "Day" },
  { key: "evidence_faithful", label: "Evidence: faithful" },
  { key: "evidence_divergent", label: "Evidence: divergent" },
  { key: "evidence_not_applicable", label: "Evidence: N/A" },
  { key: "divergence_opened", label: "Divergence opened" },
  { key: "divergence_resolved", label: "Divergence resolved" },
  { key: "divergence_open_running_total", label: "Open (running total)" },
];

export async function renderReviewBurndown(conn: AsyncDuckDBConnection, container: HTMLElement): Promise<void> {
  const result = await conn.query(QUERY);
  const rows = result.toArray().map((row) => row.toJSON() as Record<string, unknown>);
  renderTable(container, COLUMNS, rows);
}
