import type { AsyncDuckDBConnection } from "@duckdb/duckdb-wasm";
import { renderTable, type ColumnDef } from "../render-table";

// Panel 4: flywheel health funnel (verdicts -> distilled -> retrieved ->
// applied) — thin SELECT over the `mart_flywheel_funnel` view
// (`crates/canon-store/sql/views.sql`). One row per role; the funnel
// counts are already fully aggregated by the view.
//
// s42 (`close-the-open-loops`) task 3.3 split `applied` by the RULE that
// admitted each count, and the two parts partition it exactly.
//
// s42 re-review, two corrections to what this file used to say:
//
//   * The unit. This comment described the two rules as applying "per
//     run". They apply per DISTINCT (role, strategy) pair — the last
//     three funnel stages all count STRATEGIES, which is why the funnel
//     narrows by construction, and `applied_strategies` in the view
//     assigns each counted pair exactly ONE rule (attribution winning)
//     rather than tallying two overlapping sets. One run injecting two
//     still-distilled strategies of the same role contributes two
//     counts, not one.
//   * The ceiling on the claim. Neither rule is causation; both are
//     CO-OCCURRENCE inside one run. canon stores no edge from a
//     strategy to a verdict, so this join cannot separate guidance that
//     was followed from guidance that was ignored in an otherwise
//     identical run. Closing that gap needs a record change canon has
//     not made (a `StrategyId` stamped on the `Trajectory`/`VerdictRow`
//     at judgment time), not a re-reading of these columns.
//
// The markdown report's Flywheel funnel panel
// (`canon_report::render::FLYWHEEL_FUNNEL_PANEL`) states both; this
// surface reads the same mart and now states both too.
const QUERY = `
  SELECT role, verdicts, distilled, retrieved, applied, applied_attributed, applied_proxy
  FROM mart_flywheel_funnel
  ORDER BY role
`;

/**
 * The panel's caveat, exported so the honesty property this string
 * carries is testable on its own — the same posture as
 * `canon_report::render::FLYWHEEL_FUNNEL_PANEL`, whose full paragraph
 * this condenses without weakening any of its claims.
 */
export const NOTE =
  "The last three stages count distinct STRATEGIES, not runs, so the funnel narrows by construction: `applied` ≤ `retrieved` ≤ `distilled`. `applied_attributed` and `applied_proxy` partition `applied` exactly — each counted `(role, strategy)` pair is admitted under one rule, attribution winning. Both rules are CO-OCCURRENCE inside one run, never causation: canon stores no edge from a strategy to a verdict, so neither can separate guidance that was followed from guidance that was ignored.";

export const COLUMNS: ColumnDef[] = [
  { key: "role", label: "Role" },
  {
    key: "verdicts",
    label: "Verdicts",
    description:
      "VerdictRows across this role's raw trajectories — the evidence the distiller consumes. The one stage not counted in strategies, and the funnel's natural upper bound.",
  },
  {
    key: "distilled",
    label: "Distilled",
    description: "Strategies distilled for this role (stg_strategy_items rows).",
  },
  {
    key: "retrieved",
    label: "Retrieved",
    description:
      "Distinct strategies of this role named in at least one Run.injected_guidance AND still distilled today. Both halves are load-bearing; 0 means no run's recorded guidance names a strategy that exists now.",
  },
  {
    key: "applied",
    label: "Applied",
    description:
      "That same distinct strategy set, narrowed to the ones whose recipient run has something recorded about how it ended — not a count of resolved trajectories. Partitioned exactly by the two columns to its right.",
  },
  {
    key: "applied_attributed",
    label: "Applied (attributed)",
    description:
      "Per distinct strategy, not per run: the strategy is named in some run's injected_guidance, and that SAME run has at least one trajectory of the SAME role stamped with that run id (Trajectory.run_id, set only by `canon ingest artifacts --run`) whose outcome is resolved. The stronger of the two rules — still co-occurrence in one run, not causation.",
  },
  {
    key: "applied_proxy",
    label: "Applied (proxy)",
    description:
      "No such trajectory exists, so all that is recorded is the recipient run's own terminal Run.status (succeeded/failed/aborted). The weaker rule: a repo that never passes `canon ingest artifacts --run` reads its whole Applied here.",
  },
];

export async function renderFlywheelFunnel(conn: AsyncDuckDBConnection, container: HTMLElement): Promise<void> {
  const result = await conn.query(QUERY);
  const rows = result.toArray().map((row) => row.toJSON() as Record<string, unknown>);
  renderTable(container, COLUMNS, rows, NOTE);
}
