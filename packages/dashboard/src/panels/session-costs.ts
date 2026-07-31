import type { AsyncDuckDBConnection } from "@duckdb/duckdb-wasm";
import { renderTable, type ColumnDef } from "../render-table";

// Panel 2: session costs — thin SELECT over the `mart_session_costs`
// view (`crates/canon-store/sql/views.sql`).
//
// s42 (`close-the-open-loops`) re-review: this header used to read
// "session costs by role/repo/session". The view has no repo column.
// It groups by `(session_id, client, role, workspace_label)`. The
// column keeps that honest name here; this panel does not rename or
// reinterpret it.
//
// Round-9 re-review: two statements the markdown report makes about
// this panel had reached only source comments, so the dashboard reader
// got neither. They live in `NOTE` below now.
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

/**
 * The panel's caveat, exported so the honesty property this string
 * carries is testable on its own — the same posture as
 * `canon_report::render::SESSION_COSTS_PANEL` on the Rust side, whose
 * claims this string must repeat verbatim.
 *
 * The fold sentence matters because it changes what the headline
 * numbers MEAN: `mart_session_costs` folds each `run`/`session`/
 * `token_usage` record to its latest version before summing, so a
 * corrected cost supersedes rather than doubles.
 *
 * The `workspace_label` sentences are deliberately not softened. The
 * field is `canon_ingest::normalize::workspace_label_from_key` — the
 * last non-empty path segment of the `token_usage` event's workspace
 * key and nothing more — so it SPLITS one repo whose worktrees sit in
 * differently-named directories and MERGES two different repos sharing
 * a directory name. Two stronger fields sit on this exact join and
 * neither is read: the same event's own `workspace_key` (the full key,
 * at this panel's grain) and `Session.project_key`, which `canon-cli`
 * stamps to the main worktree's key precisely so linked worktrees
 * aggregate as one project (`crates/canon-model/src/records.rs`'
 * `Session` doc). The mart's GROUP BY deliberately stays on
 * `workspace_label` — its multi-workspace row split is pinned by its
 * own test — so this is a named gap, not a pending change.
 */
export const NOTE =
  "Totals fold each `run`/`session`/`token_usage` record to its latest version first, so a re-ingested corrected cost REPLACES the superseded figure instead of being summed with it. `workspace_label` is NOT a repo identity: it is the last non-empty path segment of the `token_usage` event's workspace key and nothing more, so it SPLITS one repo whose worktrees sit in differently-named directories and MERGES two different repos sharing a directory name. Two stronger fields exist and this mart reads neither: the same event's own `workspace_key`, at this panel's exact grain, and `Session.project_key`, which `canon-cli` stamps to the main worktree's key so a repo's linked worktrees aggregate as one project.";

export const COLUMNS: ColumnDef[] = [
  { key: "session_id", label: "Session" },
  { key: "client", label: "Client" },
  { key: "role", label: "Role" },
  {
    key: "workspace_label",
    label: "Workspace",
    description:
      "Last non-empty path segment of the token_usage event's workspace key, and nothing more. Not a repo identity: it splits one repo whose worktrees sit in differently-named directories and merges two different repos sharing a directory name. The same event's workspace_key and Session.project_key are both stronger; this mart reads neither.",
  },
  {
    key: "run_count",
    label: "Runs",
    description: "count(DISTINCT run_id) over this session's token_usage events, so it is unaffected by the version fold the cost and token totals need.",
  },
  {
    key: "total_cost",
    label: "Cost ($)",
    description: "Sum over the latest version of each token_usage event. A re-ingested corrected cost replaces the figure it corrects rather than adding to it.",
    format: (v) => (typeof v === "number" ? v.toFixed(4) : String(v)),
  },
  {
    key: "total_tokens",
    label: "Tokens",
    description: "Sum over the latest version of each token_usage event, same fold as Cost.",
  },
  { key: "first_event_at", label: "First event" },
  { key: "last_event_at", label: "Last event" },
];

export async function renderSessionCosts(conn: AsyncDuckDBConnection, container: HTMLElement): Promise<void> {
  const result = await conn.query(QUERY);
  const rows = result.toArray().map((row) => row.toJSON() as Record<string, unknown>);
  renderTable(container, COLUMNS, rows, NOTE);
}
