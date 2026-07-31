import type { AsyncDuckDBConnection } from "@duckdb/duckdb-wasm";
import { renderTable, type ColumnDef } from "../render-table";

// Panel 3: per-`(role, regime_key)` strategy counts — thin SELECT over
// the `mart_role_memory` view (`crates/canon-store/sql/views.sql`).
//
// s42 (`close-the-open-loops`) re-review: this header used to read
// "role memory (strategies, hit rate, effect)" and the table used to
// label its sixth column "Hit rate". Both name quantities the view does
// not compute. `mart_role_memory` has NO effect column at all, and
// `hit_rate` is `count(*) FILTER (demotion IS NULL) / count(*)` rounded
// to 4 places — i.e. exactly `active_count / strategy_count`, the
// not-demoted share of the namespace, with nothing retrieval-side in
// it. The markdown report's own Role memory panel
// (`crates/canon-report/src/render.rs`) was corrected to say so; this
// surface reads the same marts and now says the same thing, because a
// reader must not learn something different from the dashboard than
// from `.canon/REPORT.md`. `avg_source_trajectories` keeps its honest
// column name here rather than being relabelled "effect".
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

/**
 * The panel's caveat, exported so the honesty property this string
 * carries is testable on its own — the same posture as
 * `canon_report::render::FLYWHEEL_FUNNEL_PANEL` on the Rust side.
 * States the same computation as the markdown report's Role memory
 * paragraph, in a web surface's shorter register.
 */
export const NOTE =
  "`hit_rate` is NOT a retrieval hit rate: it is the fraction of that namespace's distilled strategies carrying no `demotion` flag, i.e. exactly `active_count / strategy_count`. `avg_source_trajectories` is the mean number of source trajectories a strategy was distilled from — an explicitly-named stand-in, because canon records no per-strategy reward or effect metric.";

export const COLUMNS: ColumnDef[] = [
  { key: "role", label: "Role" },
  { key: "regime_key", label: "Regime" },
  { key: "strategy_count", label: "Strategies" },
  { key: "active_count", label: "Active" },
  { key: "demoted_count", label: "Demoted" },
  {
    key: "hit_rate",
    label: "Not-demoted share (hit_rate)",
    description:
      "Exactly active_count / strategy_count, rounded to 4 places: the share of this namespace's distilled strategies carrying no demotion flag. Not a retrieval hit rate — nothing here counts retrievals.",
    format: (v) => (typeof v === "number" ? `${(v * 100).toFixed(1)}%` : String(v)),
  },
  {
    key: "avg_source_trajectories",
    label: "Avg source trajectories",
    description:
      "Mean number of source trajectories a strategy in this namespace was distilled from. An explicitly-named stand-in for effect, because canon records no per-strategy reward or effect metric.",
  },
  { key: "latest_recorded_at", label: "Latest recorded at" },
];

export async function renderRoleMemory(conn: AsyncDuckDBConnection, container: HTMLElement): Promise<void> {
  const result = await conn.query(QUERY);
  const rows = result.toArray().map((row) => row.toJSON() as Record<string, unknown>);
  renderTable(container, COLUMNS, rows, NOTE);
}
