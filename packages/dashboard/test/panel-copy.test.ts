import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { COLUMNS as FUNNEL_COLUMNS, NOTE as FUNNEL_NOTE } from "../src/panels/flywheel-funnel";
import { COLUMNS as ROLE_MEMORY_COLUMNS, NOTE as ROLE_MEMORY_NOTE } from "../src/panels/role-memory";
import { COLUMNS as BURNDOWN_COLUMNS, NOTE as BURNDOWN_NOTE } from "../src/panels/review-burndown";
import { COLUMNS as SESSION_COSTS_COLUMNS, NOTE as SESSION_COSTS_NOTE } from "../src/panels/session-costs";

// s42 (`close-the-open-loops`) re-review: the dashboard is a SECOND
// surface over the same marts as `.canon/REPORT.md`, and a reader must
// not learn something different from it. Four panels had drifted — a
// "Hit rate" column over a not-demoted share, an "Open (running total)"
// column over a running event count, an applied-split described as
// per-run when it is per distinct strategy, and a session-costs
// paragraph calling `workspace_label` the closest available stand-in
// for a repo when two stronger fields sit unread on the same join —
// each the same defect class: a friendly label asserting something its
// query does not compute.
//
// These tests pin the two surfaces together in BOTH directions: every
// claim below must appear in this package's own panel copy AND in the
// markdown report's rendered prose. Correcting one surface and
// forgetting the other is a test failure here, which is exactly how the
// drift got shipped in the first place.
const REPO_ROOT = join(new URL("../../..", import.meta.url).pathname);
const REPORT_RENDER_RS = readFileSync(join(REPO_ROOT, "crates/canon-report/src/render.rs"), "utf-8");

/**
 * Round-9 re-review, the defect this replaced: the claim search ran
 * over the WHOLE `render.rs` source. `canon stores no edge from a
 * strategy to a verdict` is also a literal inside `render.rs`'s own
 * `#[test]` block, so deleting it from the panel a reader actually
 * sees left this file green — the test passed while the two surfaces
 * disagreed, which is the single thing it exists to prevent. Comments
 * are the same hazard: every correction on this line was first written
 * as a comment.
 *
 * So match against EMITTED text only. Each panel's prose is a named
 * `pub const … _PANEL` in `render.rs`, and this parses those literals
 * out; nothing in a comment or a `#[test]` body is in scope. The
 * remaining gap — a constant kept but no longer pushed — is closed by
 * `every panel constant this file binds to is pushed into the report`
 * below, and by `render.rs`'s own
 * `every_named_panel_constant_reaches_the_rendered_report_verbatim`.
 *
 * The alternative was reading `.canon/REPORT.md`, which is literally
 * rendered output. Rejected: it is a GENERATED artifact regenerated on
 * its own cadence, so a correct `render.rs` change would fail here
 * until someone reran `canon report` — a false failure on the report's
 * refresh latency rather than on the drift this test is about.
 */
function emittedPanels(source: string): Map<string, string> {
  const declaration = /^pub const (\w+_PANEL): &str = ("(?:[^"\\]|\\.)*");$/gm;
  const panels = new Map<string, string>();
  for (const [, name, literal] of source.matchAll(declaration)) {
    // Rust's escapes here (`\n`, `\"`, `\\`) are a subset of JSON's.
    panels.set(name, JSON.parse(literal) as string);
  }
  return panels;
}

const EMITTED_PANELS = emittedPanels(REPORT_RENDER_RS);

/**
 * Claims each panel must state, verbatim, on both surfaces. Written as
 * the markdown report writes them (backticked identifiers), which is
 * also how the dashboard's `NOTE` strings are authored — `renderTable`
 * turns those spans into `<code>`.
 */
const SHARED_CLAIMS: Record<string, { note: string; emittedFrom: string; claims: string[] }> = {
  "session costs": {
    note: SESSION_COSTS_NOTE,
    emittedFrom: "SESSION_COSTS_PANEL",
    claims: [
      "a re-ingested corrected cost REPLACES the superseded figure",
      "`workspace_label` is NOT a repo identity",
      "SPLITS one repo whose worktrees sit in differently-named directories",
      "MERGES two different repos sharing a directory name",
      "Two stronger fields exist and this mart reads neither",
      "the same event's own `workspace_key`, at this panel's exact grain",
      "`Session.project_key`, which `canon-cli` stamps to the main worktree's key",
    ],
  },
  "role memory": {
    note: ROLE_MEMORY_NOTE,
    emittedFrom: "ROLE_MEMORY_PANEL",
    claims: [
      "`hit_rate` is NOT a retrieval hit rate",
      "exactly `active_count / strategy_count`",
      "canon records no per-strategy reward or effect metric",
    ],
  },
  "review burn-down": {
    note: BURNDOWN_NOTE,
    emittedFrom: "REVIEW_BURNDOWN_PANEL",
    claims: [
      "raw `Divergence.status` events",
      "running `opened - resolved` event count",
      "NOT the number open now",
      "`canon divergence status`",
    ],
  },
  "flywheel funnel": {
    note: FUNNEL_NOTE,
    emittedFrom: "FLYWHEEL_FUNNEL_PANEL",
    claims: [
      "CO-OCCURRENCE inside one run",
      "never causation",
      "canon stores no edge from a strategy to a verdict",
    ],
  },
};

test("every panel constant this file binds to is pushed into the report", () => {
  // Extraction finding a constant proves only that it is DECLARED. A
  // constant that `render` stopped pushing would let both surfaces
  // agree about text no reader ever sees.
  for (const [panel, { emittedFrom }] of Object.entries(SHARED_CLAIMS)) {
    expect({ panel, declared: EMITTED_PANELS.has(emittedFrom) }).toEqual({ panel, declared: true });
    expect(REPORT_RENDER_RS).toContain(`out.push_str(${emittedFrom});`);
  }
});

test("claim matching sees emitted prose only, never comments or Rust unit-test literals", () => {
  // Guards the narrowing itself: these markers exist in `render.rs` and
  // must NOT be in scope, or the whole-source grep is back.
  const emitted = [...EMITTED_PANELS.values()].join("\n");
  for (const marker of ["#[test]", "s42 (`close-the-open-loops`)", "assert!("]) {
    expect({ marker, inSource: REPORT_RENDER_RS.includes(marker), inEmitted: emitted.includes(marker) }).toEqual({
      marker,
      inSource: true,
      inEmitted: false,
    });
  }
});

for (const [panel, { note, emittedFrom, claims }] of Object.entries(SHARED_CLAIMS)) {
  const emitted = EMITTED_PANELS.get(emittedFrom) ?? "";
  for (const claim of claims) {
    test(`the ${panel} panel and the markdown report both state: ${claim}`, () => {
      expect(note).toContain(claim);
      expect(emitted).toContain(claim);
    });
  }
}

test("the session costs panel never sells workspace_label as a repo identity", () => {
  // `workspace_label` is `workspace_label_from_key`'s last non-empty
  // path segment, so it SPLITS one repo whose worktrees sit in
  // differently-named directories and MERGES two repos sharing a
  // directory name. It is not the best field available either: the same
  // `token_usage` event carries `workspace_key` at the identical grain,
  // and `Session.project_key` (`crates/canon-model/src/records.rs`'
  // `Session` doc) carries repo identity outright. This mart selects
  // neither, so no surface may rank `workspace_label` as the closest
  // available repo proxy.
  expect(EMITTED_PANELS.get("SESSION_COSTS_PANEL")).not.toContain("closest available stand-in");
  expect(SESSION_COSTS_NOTE).not.toContain("closest available");
  const workspace = SESSION_COSTS_COLUMNS.find((c) => c.key === "workspace_label");
  expect(workspace?.label).not.toBe("Repo");
  expect(workspace?.description).toContain("Not a repo identity");
  expect(workspace?.description).toContain("workspace_key and Session.project_key are both stronger; this mart reads neither");
  // The fold is what makes the money column a corrected figure rather
  // than a sum over superseded versions; the reader needs that at the
  // column too, not only in the note.
  const cost = SESSION_COSTS_COLUMNS.find((c) => c.key === "total_cost");
  expect(cost?.description).toContain("latest version of each token_usage event");
  expect(cost?.description).toContain("replaces the figure it corrects");
});

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
