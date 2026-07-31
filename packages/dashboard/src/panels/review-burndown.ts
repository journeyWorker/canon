import type { AsyncDuckDBConnection } from "@duckdb/duckdb-wasm";
import { renderTable, type ColumnDef } from "../render-table";

// Panel 5: review-feedback burn-down over time — thin SELECT over the
// `mart_review_burndown` view (`crates/canon-store/sql/views.sql`).
// `divergence_open_running_total` is already the view's own running-sum
// window column; this panel does not recompute it.
//
// s42 (`close-the-open-loops`) re-review: the last column used to be
// labelled "Open (running total)" with no caveat, which reads as
// current state. It is not. Every row of this mart is a per-day tally
// of RAW `Divergence.status` events, so the running column is a running
// `opened - resolved` EVENT count — a divergence opened and resolved on
// different days moves it twice, and nothing re-reads the divergence's
// state today. `canon divergence status` is the current-state surface.
// The markdown report's Review burn-down panel
// (`crates/canon-report/src/render.rs`) already carries that caveat;
// this surface reads the same mart and now carries it too.
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

/**
 * The panel's caveat, exported so the honesty property this string
 * carries is testable on its own. Same claim as the markdown report's
 * Review burn-down paragraph, including the pointer to the surface that
 * DOES answer "what is open now".
 */
export const NOTE =
  "A per-day trend over raw `Divergence.status` events, so `divergence_open_running_total` is a running `opened - resolved` event count, NOT the number open now. For current state per scenario, run `canon divergence status`.";

export const COLUMNS: ColumnDef[] = [
  { key: "day", label: "Day" },
  { key: "evidence_faithful", label: "Evidence: faithful" },
  { key: "evidence_divergent", label: "Evidence: divergent" },
  { key: "evidence_not_applicable", label: "Evidence: N/A" },
  {
    key: "divergence_opened",
    label: "Divergence opened",
    description: "Count of raw Divergence records with status 'open' recorded on this day.",
  },
  {
    key: "divergence_resolved",
    label: "Divergence resolved",
    description: "Count of raw Divergence records with status 'resolved' recorded on this day.",
  },
  {
    key: "divergence_open_running_total",
    label: "Running opened − resolved (event count)",
    description:
      "Running sum of (opened - resolved) over raw Divergence.status events, oldest day through this one. NOT the number open now: run `canon divergence status` for current state per scenario.",
  },
];

export async function renderReviewBurndown(conn: AsyncDuckDBConnection, container: HTMLElement): Promise<void> {
  const result = await conn.query(QUERY);
  const rows = result.toArray().map((row) => row.toJSON() as Record<string, unknown>);
  renderTable(container, COLUMNS, rows, NOTE);
}
