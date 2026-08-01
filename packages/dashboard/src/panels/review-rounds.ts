import type { AsyncDuckDBConnection } from "@duckdb/duckdb-wasm";
import { renderTable, type ColumnDef } from "../render-table";

// Panel 8 (s43 `findings-are-records`): findings per review round —
// thin SELECT over the `mart_review_rounds` view
// (`crates/canon-store/sql/views.sql`). Every count, including
// `fix_of_fix`, is already the view's own aggregate; this panel
// recomputes nothing.
//
// This is the panel with the shortest fuse in the whole app. It exists
// because a hand-written release summary claimed "49 real issues, four
// of them defects in the previous round's fix" and both numbers were
// wrong — so a friendly label here that asserts more than the view
// computes reproduces the exact defect the mart was built to stop.
// Four specific traps, and how the copy below avoids each:
//  1. `fix_of_fix` is NOT proof that the earlier fix caused the later
//     defect. The view's predicate is `g.resolution_sha =
//     f.introduced_by` — commit-id equality between two RECORDED
//     fields, scoped to one change and ordered strictly by
//     `(round, seq)`. No git history is read and no blame is computed,
//     so the column is exactly as sound as the sourcing of
//     `introduced_by` — which canon never INFERS, and which a reviewer
//     may leave unset. Canon asks for that discipline; it cannot check
//     it.
//  2. `introduced_by_unsourced` is a KNOWN UNKNOWN, not a zero. A
//     finding with no sourced introducing commit is never counted as
//     not-a-fix-of-fix.
//  3. `fix_of_fix` bounds NOTHING (s43 round 1, seq 1 — this panel and
//     its markdown twin both shipped "FLOOR", and the corpus below
//     them disproved the direction). It MISSES: unsourced findings,
//     and a fix in one change that broke something first found while
//     reviewing another. It also OVER-counts: a `resolution_sha`
//     commit may carry work beyond the fix, and then every finding
//     recording that commit matches regardless. Live in v0.4.0 —
//     `f438c610` closed s42's round 8 AND shipped s42's feature, so
//     round 9's six matches are a commit-id coincidence, while rounds
//     10 and 11 matched pure fix commits.
//  4. A round that found nothing wrote no `Finding`, so it has no row
//     (s43 round 1, seq 2). The table's unit is rounds THAT FOUND
//     SOMETHING; nothing in canon records a round as merely RUN.
//
// The markdown report's Review rounds panel
// (`crates/canon-report/src/render.rs`) carries the same statement;
// `test/panel-copy.test.ts` pins the two together so neither surface
// can drift.
const QUERY = `
  SELECT
    change_id,
    "round",
    reviewed_sha,
    findings,
    severity_blocker,
    severity_should_fix,
    severity_note,
    disposition_open,
    disposition_fixed,
    disposition_rejected,
    disposition_deferred,
    fix_of_fix,
    introduced_by_sourced,
    introduced_by_unsourced
  FROM mart_review_rounds
  ORDER BY change_id, "round"
`;

/**
 * The panel's caveat, exported so the honesty property this string
 * carries is testable on its own. Same claim as the markdown report's
 * Review rounds paragraph: what the join computes, what it does not
 * establish, that its unit is rounds that FOUND something, and that
 * the count bounds nothing in either direction.
 */
export const NOTE =
  "One row per `(change_id, round)` over `Finding` records, folded to the latest version of each finding first. A round that found NOTHING wrote no `Finding` and so has no row: this table counts the rounds that FOUND something, never the rounds RUN. s42's round 12 returned MERGEABLE with zero findings and is absent. canon has no record kind for a review round, so the rounds-run count is not derivable from this corpus at all. `fix_of_fix` counts the findings in this round whose SOURCED `introduced_by` equals the `resolution_sha` of a finding earlier in the SAME change, ordered strictly by `(round, seq)` — a commit-id equality join over two recorded fields, which reads no git history and is exactly as sound as the sourcing of `introduced_by`, a field canon never infers and a reviewer may leave unsourced. `introduced_by_unsourced` is the UNKNOWN bucket: a finding with no sourced `introduced_by` is counted there and NEVER as not-a-fix-of-fix. `fix_of_fix` therefore bounds NOTHING — not from below, not from above. It UNDER-counts, because an unsourced finding could belong to it and is never counted, and a fix in one change that breaks something first found while reviewing a DIFFERENT change is not counted at all. It OVER-counts, because a `resolution_sha` commit may carry work BEYOND the fix, and every finding recording that commit is counted whether the fix or the other work introduced it. Both are live in canon's own v0.4.0 rows: round 9's six counted findings record `f438c610`, which closed round 8 AND shipped s42's whole feature, so they are NOT attributable to round 8's fixes, while rounds 10 and 11's introducing commits were pure fix commits, so theirs are.";

export const COLUMNS: ColumnDef[] = [
  { key: "change_id", label: "Change" },
  { key: "round", label: "Round" },
  {
    key: "reviewed_sha",
    label: "Reviewed commit",
    description:
      "The greatest reviewed_sha any of the round's findings recorded, or — when none recorded one. A round that reviewed an uncommitted working tree is the usual reason and the common case, but nothing here distinguishes that from a round whose findings simply left the field unset, so a blank is not evidence of a worktree review. The view does not require a round's findings to agree on it.",
  },
  { key: "findings", label: "Findings" },
  { key: "severity_blocker", label: "Blocker" },
  { key: "severity_should_fix", label: "Should fix" },
  { key: "severity_note", label: "Note" },
  { key: "disposition_open", label: "Open" },
  { key: "disposition_fixed", label: "Fixed" },
  { key: "disposition_rejected", label: "Rejected" },
  { key: "disposition_deferred", label: "Deferred" },
  {
    key: "fix_of_fix",
    label: "Fix-of-fix (same change, not a bound)",
    description:
      "Findings whose sourced introduced_by equals the resolution_sha of a finding earlier in the SAME change, ordered strictly by (round, seq). Commit-id equality between two recorded fields: no git history is read and no blame is computed, so this is not evidence the earlier fix caused the later defect. It bounds nothing in either direction. It misses — unsourced findings are excluded as unknown, and a cross-change fix-of-fix is not counted at all. It also over-counts — a resolution commit that carries work beyond the fix matches every finding recording it, fix-induced or not.",
  },
  {
    key: "introduced_by_sourced",
    label: "Introducing commit sourced",
    description:
      "Findings carrying an introduced_by at all. Bounds fix_of_fix from above: a sourced commit that matches no earlier resolution_sha is counted here and not there.",
  },
  {
    key: "introduced_by_unsourced",
    label: "Introducing commit UNSOURCED (unknown)",
    description:
      "Findings whose introducing commit could not be sourced. canon never infers one, so these are counted as UNKNOWN and never as not-a-fix-of-fix. sourced + unsourced = findings.",
  },
];

export async function renderReviewRounds(conn: AsyncDuckDBConnection, container: HTMLElement): Promise<void> {
  const result = await conn.query(QUERY);
  const rows = result.toArray().map((row) => row.toJSON() as Record<string, unknown>);
  renderTable(container, COLUMNS, rows, NOTE);
}
