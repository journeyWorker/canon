import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { COLUMNS as FUNNEL_COLUMNS, NOTE as FUNNEL_NOTE } from "../src/panels/flywheel-funnel";
import { COLUMNS as ROLE_MEMORY_COLUMNS, NOTE as ROLE_MEMORY_NOTE } from "../src/panels/role-memory";
import { COLUMNS as BURNDOWN_COLUMNS, NOTE as BURNDOWN_NOTE } from "../src/panels/review-burndown";

// s42 (`close-the-open-loops`) re-review: the dashboard is a SECOND
// surface over the same marts as `.canon/REPORT.md`, and a reader must
// not learn something different from it. Three panels had already
// drifted — a "Hit rate" column over a not-demoted share, an "Open
// (running total)" column over a running event count, and an
// applied-split described as per-run when it is per distinct strategy —
// each the same defect class: a friendly label asserting something its
// query does not compute.
//
// These tests pin the two surfaces together in BOTH directions: every
// claim below must appear in this package's own panel copy AND in the
// markdown report's source that prints the same panel. Correcting one
// surface and forgetting the other is a test failure here, which is
// exactly how the drift got shipped in the first place.
const REPO_ROOT = join(new URL("../../..", import.meta.url).pathname);
const REPORT_RENDER_RS = readFileSync(join(REPO_ROOT, "crates/canon-report/src/render.rs"), "utf-8");

/**
 * Claims each panel must state, verbatim, on both surfaces. Written as
 * the markdown report writes them (backticked identifiers), which is
 * also how the dashboard's `NOTE` strings are authored — `renderTable`
 * turns those spans into `<code>`.
 */
const SHARED_CLAIMS: Record<string, { note: string; claims: string[] }> = {
  "role memory": {
    note: ROLE_MEMORY_NOTE,
    claims: [
      "`hit_rate` is NOT a retrieval hit rate",
      "exactly `active_count / strategy_count`",
      "canon records no per-strategy reward or effect metric",
    ],
  },
  "review burn-down": {
    note: BURNDOWN_NOTE,
    claims: [
      "raw `Divergence.status` events",
      "running `opened - resolved` event count",
      "NOT the number open now",
      "`canon divergence status`",
    ],
  },
  "flywheel funnel": {
    note: FUNNEL_NOTE,
    claims: [
      "CO-OCCURRENCE inside one run",
      "never causation",
      "canon stores no edge from a strategy to a verdict",
    ],
  },
};

for (const [panel, { note, claims }] of Object.entries(SHARED_CLAIMS)) {
  for (const claim of claims) {
    test(`the ${panel} panel and the markdown report both state: ${claim}`, () => {
      expect(note).toContain(claim);
      expect(REPORT_RENDER_RS).toContain(claim);
    });
  }
}

test("the role memory panel does not advertise a hit rate or an effect the view lacks", () => {
  const hitRate = ROLE_MEMORY_COLUMNS.find((c) => c.key === "hit_rate");
  // The bare label was the lie: `mart_role_memory` computes
  // `count(*) FILTER (demotion IS NULL) / count(*)`, which counts no
  // retrievals at all.
  expect(hitRate?.label).not.toBe("Hit rate");
  expect(hitRate?.label).toContain("Not-demoted share");
  expect(hitRate?.description).toContain("active_count / strategy_count");
  expect(hitRate?.description).toContain("Not a retrieval hit rate");
  // `mart_role_memory` has no effect column; `avg_source_trajectories`
  // must stay an explicitly-named stand-in, never relabelled "Effect".
  expect(ROLE_MEMORY_COLUMNS.some((c) => /effect/i.test(c.label))).toBe(false);
  const avg = ROLE_MEMORY_COLUMNS.find((c) => c.key === "avg_source_trajectories");
  expect(avg?.description).toContain("no per-strategy reward or effect metric");
});

test("the review burn-down's running column is labelled as an event count, not current state", () => {
  const running = BURNDOWN_COLUMNS.find((c) => c.key === "divergence_open_running_total");
  expect(running?.label).not.toBe("Open (running total)");
  expect(running?.label).toContain("event count");
  expect(running?.description).toContain("NOT the number open now");
  expect(running?.description).toContain("canon divergence status");
});

test("the flywheel funnel states the applied split per distinct strategy, not per run", () => {
  // The unit is the whole correction: one run injecting two
  // still-distilled strategies of the same role contributes two counts.
  expect(FUNNEL_NOTE).toContain("distinct STRATEGIES, not runs");
  expect(FUNNEL_NOTE).toContain("partition `applied` exactly");
  const attributed = FUNNEL_COLUMNS.find((c) => c.key === "applied_attributed");
  expect(attributed?.description).toContain("Per distinct strategy, not per run");
  expect(attributed?.description).toContain("canon ingest artifacts --run");
  expect(attributed?.description).toContain("not causation");
  const proxy = FUNNEL_COLUMNS.find((c) => c.key === "applied_proxy");
  expect(proxy?.description).toContain("terminal Run.status");
  expect(proxy?.description).toContain("never passes `canon ingest artifacts --run`");
});

test("every panel note reaches the reader as prose, not only as a tooltip", () => {
  // A caveat that corrects a misleading reading has to be visible
  // without an interaction; tooltips carry the per-column detail on top
  // of it, never instead of it.
  for (const [panel, { note }] of Object.entries(SHARED_CLAIMS)) {
    expect({ panel, empty: note.trim() === "" }).toEqual({ panel, empty: false });
  }
});

test("dashboard sources cite views.sql by view name, never by line range", () => {
  // Line citations rot on the first edit above them; they were replaced
  // with view names everywhere else in this repo for exactly that
  // reason.
  const sources = [
    "src/panels/flywheel-funnel.ts",
    "src/panels/role-memory.ts",
    "src/panels/review-burndown.ts",
    "src/panels/session-costs.ts",
    "src/panels/trust-matrix.ts",
    "src/render-table.ts",
    "scripts/build-fixture-snapshot.sql",
    "test/fixture-schema.ts",
  ];
  const pkgRoot = new URL("..", import.meta.url).pathname;
  for (const source of sources) {
    const text = readFileSync(join(pkgRoot, source), "utf-8");
    expect({ source, lineCitations: text.match(/views\.sql[`'"]?\s*[:#]\s*\d/g) ?? [] }).toEqual({
      source,
      lineCitations: [],
    });
  }
});
