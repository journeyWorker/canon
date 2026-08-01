import type { AsyncDuckDBConnection } from "@duckdb/duckdb-wasm";
import { renderTable, type ColumnDef } from "../render-table";
import { FIX_OF_FIX_MEANING } from "./review-rounds";

// Panel 9 (s43 `findings-are-records`, round 5): per-change review
// totals — a thin SELECT over the `mart_review_totals` view
// (`crates/canon-store/sql/views.sql`), whose own only `FROM` is
// `mart_review_rounds`. This panel recomputes nothing, and neither does
// the view: every column is a `sum()`/`count(*)`/`max()` over the rows
// panel 8 prints.
//
// Why it exists. s43 made the per-ROUND numbers generated so a release
// narrative would stop being typed from memory. It did not finish the
// job: the sentence a release note actually contains is per CHANGE
// ("N findings across R rounds"), and getting one off a per-round table
// meant adding the rows up by hand — the same hand arithmetic that put
// four wrong figures into this release line's published notes. A
// generated table you have to do arithmetic over is not a generated
// number.
//
// The traps this copy has to avoid, each already shipped once on this
// line:
//  1. `rounds_recorded` is NOT rounds run. `mart_review_rounds` holds a
//     row only for a round that recorded a finding, so a clean round is
//     invisible here as it is there — and canon has no record kind that
//     marks a round as run, so the other number is unavailable rather
//     than merely unrendered. The column is named `rounds_recorded` and
//     labelled with the denial for the same reason the fix-of-fix
//     column is: a reader who copies a cell may never read this note.
//  2. `highest_round` witnesses NOTHING about rounds. A gap above
//     `rounds_recorded` says only that some lower round label has no
//     row: `round` is author-supplied and canon enforces no
//     contiguous numbering, so one finding labelled round 7 makes the
//     same gap six silent rounds would. Reading the gap as a clean
//     round was s43 round 6's second finding, shipped by the panel
//     added to make the numbers trustworthy.
//  3. `fix_of_fix` is DERIVED and bounds nothing — the whole of that
//     claim is `FIX_OF_FIX_MEANING`, imported from panel 8 rather than
//     restated, because every restatement of it so far has drifted.
//  4. A per-change total is the shape most readily misread as a verdict
//     ON the change. `findings` counts what reviewers RECORDED, which
//     moves with how hard a change was reviewed; there is no rate and
//     no score here, and the copy says so.
//
// The markdown report's Review totals panel
// (`crates/canon-report/src/render.rs`, `REVIEW_TOTALS_PANEL`) carries
// the same statement; `test/panel-copy.test.ts` pins the two together.
const QUERY = `
  SELECT
    change_id,
    rounds_recorded,
    highest_round,
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
  FROM mart_review_totals
  ORDER BY change_id
`;

/**
 * The panel's caveat, exported so the honesty properties this string
 * carries are testable on their own and pinnable against the markdown
 * report's `REVIEW_TOTALS_PANEL`.
 */
export const NOTE =
  "One row per `change_id`, and every column a `sum()`, `count(*)` or `max()` over the `mart_review_rounds` rows above. That view is this one's only `FROM`, and every panel in one report — or one `--snapshot` export — is computed in a single DuckDB process over one materialized read of the corpus, so the total and the rows it totals are one computation over one input and cannot disagree: a record written to the ledger mid-run reaches BOTH tables or neither, never one and not the other — though the digest header beside these panels is a SEPARATE read outside that pin, taken before it in a report and after it in a `--snapshot`, so header and panels can still describe the corpus a moment apart. This panel exists so a release note is a COPY rather than a computation: the sentence a release note wants is two cells of one row here, and adding the per-round table up by hand is the operation that put four wrong figures into this release line's published notes. `rounds_recorded` is `count(*)` over those rows, and `mart_review_rounds` holds a row only for a round that RECORDED a finding — a round that found nothing wrote no `Finding` and is in neither table. So this column counts the rounds that FOUND something, never the rounds RUN. s42's round 12 returned MERGEABLE with zero findings and is absent from both; canon has no record kind for a review round, so the rounds-RUN number is not derivable from this corpus at all. `highest_round` is `max(round)` over the same rows, the greatest round NUMBER that recorded a finding, and it witnesses nothing about rounds. A `highest_round` above `rounds_recorded` says only that some round number below it has no row, and canon cannot say why: `round` is a number the author supplies and nothing requires a change's rounds to be numbered from 1 or without gaps, so a change whose only finding is labelled round 7 shows the same gap six silent rounds would. Neither the gap nor its absence evidences a round that RAN — a round that found nothing writes no `Finding`, and canon records nothing else about one. Neither column is the rounds-RUN count: `highest_round` is a round NUMBER rather than a count, and it equals the rounds run only if a change's rounds are numbered from 1 without gaps AND its last round found something — canon requires neither. Each split is a `sum()` of the per-round `count(*) FILTER` columns over a closed `Finding` enum, so `severity_blocker + severity_should_fix + severity_note = findings`, `disposition_open + disposition_fixed + disposition_rejected + disposition_deferred = findings` and `introduced_by_sourced + introduced_by_unsourced = findings`, all by construction; the dispositions are the LATEST recorded state of each finding, never its history. `fix_of_fix` is `sum(fix_of_fix)`, and the relationship is DERIVED, never recorded: no `Finding` carries a fix-of-fix field, and the only place the edge exists is the round view's commit-id equality join, already scoped to one `change_id`. `fix_of_fix` bounds NOTHING — not from below, not from above: it UNDER-counts, because an unsourced finding is never counted and a fix in one change that breaks something first found while reviewing a DIFFERENT change is not counted at all; it OVER-counts, because a `resolution_sha` commit may carry work BEYOND the fix and every finding recording that commit is counted regardless; and for any individual match the data cannot say whether the fix or the other work in that commit introduced the defect. None of these columns is a claim about the change. `findings` is how many findings reviewers RECORDED against it, `severity_blocker` is the severity a reviewer TYPED, and `disposition_rejected` records that a finding was rejected rather than that it was wrong. There is no defect rate, no quality score and no comparison between changes here: a row is one change's review HISTORY.";

export const COLUMNS: ColumnDef[] = [
  { key: "change_id", label: "Change" },
  {
    key: "rounds_recorded",
    label: "Rounds that recorded a finding (never rounds run)",
    description:
      "count(*) over this change's mart_review_rounds rows, and that view holds a row only for a round that RECORDED a finding. A round that found nothing wrote no Finding and is in neither table, so this counts the rounds that found something and never the rounds run. canon has no record kind for a review round, so the rounds-run number is not derivable from this corpus at all.",
  },
  {
    key: "highest_round",
    label: "Highest round number recorded (witnesses no round)",
    description:
      "max(round) over the same rows. Above the rounds-recorded count it says only that some lower round number has no row, and canon cannot say why: round is author-supplied and nothing requires a change's rounds to be numbered from 1 or without gaps, so one finding labelled round 7 makes the same gap six silent rounds would. Neither the gap nor its absence evidences a round that RAN. Neither column is the rounds-run count: this is a round NUMBER rather than a count, and it equals the rounds run only if a change's rounds are numbered from 1 without gaps and its last round found something, neither of which canon requires.",
  },
  {
    key: "findings",
    label: "Findings recorded (not a defect count)",
    description:
      "sum(findings) over this change's rounds: how many findings reviewers RECORDED against the change. It moves with how many rounds the change got and how freely its reviewers wrote findings down, so it is a record of the review, not a measure of the change. There is no denominator in this view and no comparison between changes.",
  },
  { key: "severity_blocker", label: "Blocker" },
  { key: "severity_should_fix", label: "Should fix" },
  { key: "severity_note", label: "Note" },
  { key: "disposition_open", label: "Open" },
  { key: "disposition_fixed", label: "Fixed" },
  {
    key: "disposition_rejected",
    label: "Rejected",
    description:
      "Findings whose latest recorded disposition is rejected. That records that a finding was rejected, not that it was wrong — canon stores the disposition a reviewer wrote and checks it against nothing.",
  },
  { key: "disposition_deferred", label: "Deferred" },
  {
    key: "fix_of_fix",
    label: "Fix-of-fix total (same change, not a bound)",
    // The per-column rule, then the canonical sentence — DERIVED from
    // the constant rather than restated. `title` renders literally, so
    // the backticks come out.
    description:
      "sum() of the per-round fix-of-fix counts. Nothing records this relationship: no Finding carries a fix-of-fix field, and the edge exists only as the round view's commit-id equality join between a sourced introduced_by and an earlier finding's resolution_sha in the SAME change. That join is already scoped to one change, so summing it changes the grain and nothing else. " +
      FIX_OF_FIX_MEANING.replaceAll("`", ""),
  },
  {
    key: "introduced_by_sourced",
    label: "Introducing commit sourced",
    description:
      "Findings carrying an introduced_by at all. Bounds fix_of_fix from above at this grain too: a sourced commit matching no earlier resolution_sha is counted here and not there.",
  },
  {
    key: "introduced_by_unsourced",
    label: "Introducing commit UNSOURCED (unknown)",
    description:
      "Findings whose introducing commit could not be sourced. canon never infers one, so these are counted as UNKNOWN and never as not-a-fix-of-fix. sourced + unsourced = findings.",
  },
];

export async function renderReviewTotals(conn: AsyncDuckDBConnection, container: HTMLElement): Promise<void> {
  const result = await conn.query(QUERY);
  const rows = result.toArray().map((row) => row.toJSON() as Record<string, unknown>);
  renderTable(container, COLUMNS, rows, NOTE);
}
