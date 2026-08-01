import { expect, test } from "bun:test";
import { join } from "node:path";
import { duckdbAvailable, queryJson, sqlLiteral } from "./duckdb-cli";

// s42 (`close-the-open-loops`) re-review: `fixture-schema.test.ts` locks
// the committed fixtures' column NAME/ORDER/TYPE, and `smoke.test.ts`
// asserts `rowCount > 0`. Neither reads a single VALUE, so a fixture row
// could contradict the very arithmetic the panels' new prose asserts
// (`applied_attributed + applied_proxy = applied`, `hit_rate =
// active_count / strategy_count`, `divergence_open_running_total` as a
// running sum) and still pass — and a prior review round found exactly
// that: demo rows violating the funnel invariant outright.
//
// These tests read the REAL committed bytes back through the `duckdb`
// CLI and assert the arithmetic. A fixture that lies about what the
// marts compute makes the dashboard's own screenshot a
// counter-example to its own caveats.
const SNAPSHOT = join(new URL("..", import.meta.url).pathname, "fixtures", "snapshot");

function rows<T>(mart: string, select: string): T[] {
  return queryJson<T>(`SELECT ${select} FROM read_parquet(${sqlLiteral(join(SNAPSHOT, `${mart}.parquet`))})`);
}

function skipWithoutDuckdb(): boolean {
  if (duckdbAvailable()) return false;
  console.error("skipping: `duckdb` CLI not found on PATH");
  return true;
}

interface FunnelRow {
  role: string;
  verdicts: number;
  distilled: number;
  retrieved: number;
  applied: number;
  applied_attributed: number;
  applied_proxy: number;
}

test("mart_flywheel_funnel fixture rows satisfy the funnel invariants the panel asserts", () => {
  if (skipWithoutDuckdb()) return;
  const funnel = rows<FunnelRow>("mart_flywheel_funnel", "*");
  expect(funnel.length).toBeGreaterThan(0);

  for (const row of funnel) {
    // The split PARTITIONS `applied` — the view assigns each counted
    // `(role, strategy)` pair exactly one rule, attribution winning.
    expect({ role: row.role, sum: row.applied_attributed + row.applied_proxy }).toEqual({
      role: row.role,
      sum: row.applied,
    });
    // The last three stages count the same narrowing strategy set.
    expect(row.applied).toBeLessThanOrEqual(row.retrieved);
    expect(row.retrieved).toBeLessThanOrEqual(row.distilled);
    // `verdicts` is the evidence the distiller consumes: at most one
    // strategy per verdict, so it bounds `distilled` from above.
    expect(row.distilled).toBeLessThanOrEqual(row.verdicts);
    expect(row.applied_attributed).toBeGreaterThanOrEqual(0);
    expect(row.applied_proxy).toBeGreaterThanOrEqual(0);
  }
});

test("mart_flywheel_funnel fixture exercises all three shapes of the applied split", () => {
  if (skipWithoutDuckdb()) return;
  const funnel = rows<FunnelRow>("mart_flywheel_funnel", "*");
  const shape = (r: FunnelRow) =>
    r.applied_attributed > 0 && r.applied_proxy > 0
      ? "mixed"
      : r.applied_attributed > 0
        ? "all-attributed"
        : "all-proxy";
  // A fixture where every row reads the same way would render the split
  // columns without ever demonstrating that they split anything —
  // "all-proxy" in particular is the shape of a repo that never passes
  // `canon ingest artifacts --run`, which the panel's tooltip names.
  const shapes = [...new Set(funnel.map(shape))].sort();
  expect(shapes).toEqual(["all-attributed", "all-proxy", "mixed"]);
});

interface RoleMemoryRow {
  role: string;
  strategy_count: number;
  active_count: number;
  demoted_count: number;
  hit_rate: number;
}

test("mart_role_memory fixture rows make hit_rate exactly the not-demoted share", () => {
  if (skipWithoutDuckdb()) return;
  const memory = rows<RoleMemoryRow>("mart_role_memory", "*");
  expect(memory.length).toBeGreaterThan(0);

  for (const row of memory) {
    // Every strategy is either demoted or not — the view's two FILTERed
    // counts are complements over the same `count(*)`.
    expect({ role: row.role, sum: row.active_count + row.demoted_count }).toEqual({
      role: row.role,
      sum: row.strategy_count,
    });
    // The claim the panel now makes in prose: `hit_rate` IS
    // `active_count / strategy_count`, rounded to 4 places by the view.
    const expected = Math.round((row.active_count / row.strategy_count) * 1e4) / 1e4;
    expect({ role: row.role, hit_rate: row.hit_rate }).toEqual({ role: row.role, hit_rate: expected });
  }
  // A fixture of all-1.0 rows would never distinguish "not-demoted
  // share" from a constant.
  expect(memory.some((r) => r.demoted_count > 0)).toBe(true);
  expect(memory.some((r) => r.demoted_count === 0)).toBe(true);
});

interface BurndownRow {
  day: string;
  divergence_opened: number;
  divergence_resolved: number;
  divergence_open_running_total: number;
}

test("mart_review_burndown fixture rows are a running opened - resolved event count", () => {
  if (skipWithoutDuckdb()) return;
  const burndown = rows<BurndownRow>(
    "mart_review_burndown",
    "CAST(day AS VARCHAR) AS day, divergence_opened, divergence_resolved, divergence_open_running_total",
  ).sort((a, b) => a.day.localeCompare(b.day));
  expect(burndown.length).toBeGreaterThan(0);

  let running = 0;
  for (const row of burndown) {
    running += row.divergence_opened - row.divergence_resolved;
    expect({ day: row.day, total: row.divergence_open_running_total }).toEqual({ day: row.day, total: running });
  }
  // The whole point of the caveat: the running total is an event count,
  // so it must be able to fall as well as rise. A monotonically rising
  // fixture would read exactly like the "number open now" the panel
  // says it is not.
  expect(burndown.some((r) => r.divergence_resolved > r.divergence_opened)).toBe(true);
});

interface ReviewRoundsRow {
  change_id: string;
  round: number;
  reviewed_sha: string | null;
  findings: number;
  severity_blocker: number;
  severity_should_fix: number;
  severity_note: number;
  disposition_open: number;
  disposition_fixed: number;
  disposition_rejected: number;
  disposition_deferred: number;
  fix_of_fix: number;
  introduced_by_sourced: number;
  introduced_by_unsourced: number;
}

test("mart_review_rounds fixture rows satisfy the arithmetic the panel asserts", () => {
  if (skipWithoutDuckdb()) return;
  const rounds = rows<ReviewRoundsRow>("mart_review_rounds", "*");
  expect(rounds.length).toBeGreaterThan(0);

  for (const row of rounds) {
    const label = `${row.change_id}#${row.round}`;
    // `severity` and `disposition` are closed enums on `Finding`, so
    // each set of FILTERed counts partitions the round's findings. A
    // fixture violating this would render buckets that do not add up
    // under prose claiming they do.
    expect({ label, sum: row.severity_blocker + row.severity_should_fix + row.severity_note }).toEqual({ label, sum: row.findings });
    expect({
      label,
      sum: row.disposition_open + row.disposition_fixed + row.disposition_rejected + row.disposition_deferred,
    }).toEqual({ label, sum: row.findings });
    // The identity that makes the UNKNOWN bucket readable against the
    // count: every finding either carries a sourced `introduced_by` or
    // does not.
    expect({ label, sum: row.introduced_by_sourced + row.introduced_by_unsourced }).toEqual({ label, sum: row.findings });
    // `fix_of_fix` is the sourced set NARROWED by the join, so it can
    // never exceed it. A fixture where it did would contradict the
    // arithmetic the panel states beside its canonical sentence.
    expect(row.fix_of_fix).toBeLessThanOrEqual(row.introduced_by_sourced);
  }
});

test("mart_review_rounds fixture exercises all three shapes of introduced_by", () => {
  if (skipWithoutDuckdb()) return;
  const rounds = rows<ReviewRoundsRow>("mart_review_rounds", "*");
  const shape = (r: ReviewRoundsRow) =>
    r.introduced_by_sourced === 0
      ? "all-unsourced"
      : r.fix_of_fix === 0
        ? "sourced-none-matched"
        : "sourced-some-matched";
  // Each shape carries a claim the columns would otherwise never
  // demonstrate: `all-unsourced` is the round where `fix_of_fix 0`
  // means "canon does not know" rather than "it did not happen";
  // `sourced-none-matched` stops a reader equating `fix_of_fix` with
  // "the sourced ones"; `sourced-some-matched` is the only shape that
  // shows the join admitting anything at all.
  expect([...new Set(rounds.map(shape))].sort()).toEqual(["all-unsourced", "sourced-none-matched", "sourced-some-matched"]);

  // A round that reviewed an uncommitted worktree has no reviewed
  // commit, and that is the COMMON case — a fixture with a sha on every
  // row would show a shape the model calls rare, and would let a
  // sha-ordered derivation look viable.
  expect(rounds.some((r) => r.reviewed_sha === null)).toBe(true);
  expect(rounds.some((r) => r.reviewed_sha !== null)).toBe(true);
  // More than one change, so the per-change grain the fix-of-fix scope
  // depends on is visible rather than implied.
  expect(new Set(rounds.map((r) => r.change_id)).size).toBeGreaterThan(1);
});
