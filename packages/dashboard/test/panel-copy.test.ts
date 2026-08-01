import { expect, test } from "bun:test";
import { readFileSync } from "node:fs";
import { join } from "node:path";
import { COLUMNS as FUNNEL_COLUMNS, NOTE as FUNNEL_NOTE } from "../src/panels/flywheel-funnel";
import { COLUMNS as ROLE_MEMORY_COLUMNS, NOTE as ROLE_MEMORY_NOTE } from "../src/panels/role-memory";
import { COLUMNS as BURNDOWN_COLUMNS, NOTE as BURNDOWN_NOTE } from "../src/panels/review-burndown";
import { COLUMNS as SESSION_COSTS_COLUMNS, NOTE as SESSION_COSTS_NOTE } from "../src/panels/session-costs";
import { COLUMNS as REVIEW_ROUNDS_COLUMNS, NOTE as REVIEW_ROUNDS_NOTE } from "../src/panels/review-rounds";

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
  // s43 (`findings-are-records`): this panel is the one a release note
  // gets copied FROM, so its load-bearing caveats — what the fix-of-fix
  // join actually computes, that its count bounds nothing in either
  // direction, and that a round which found nothing has no row — have
  // to be identical on both surfaces or the number stops being worth
  // copying.
  //
  // The last two arrived as s43 round 1's own findings, raised by
  // reading this panel against the corpus it renders. Both were claims
  // the query cannot support, shipped BY the panel written to stop
  // exactly that.
  "review rounds": {
    note: REVIEW_ROUNDS_NOTE,
    emittedFrom: "REVIEW_ROUNDS_PANEL",
    claims: [
      "a commit-id equality join over two recorded fields",
      "reads no git history",
      "counted there and NEVER as not-a-fix-of-fix",
      "ordered strictly by",
      "the UNKNOWN bucket",
      "NEVER as not-a-fix-of-fix",
      "a fix in one change that breaks something first found while reviewing a DIFFERENT change is not counted at all",
      // Round 1, seq 1 — the count errs in BOTH directions, so it is
      // no kind of bound, and the over-count is worked through the
      // live row.
      "`fix_of_fix` therefore bounds NOTHING — not from below, not from above",
      "a `resolution_sha` commit may carry work BEYOND the fix",
      "`f438c610`, which closed round 8 AND shipped s42's whole feature",
      // Round 1, seq 2 — a clean round writes no record, so the row
      // count is not the round count and canon cannot supply the rest.
      "counts the rounds that FOUND something, never the rounds RUN",
      "s42's round 12 returned MERGEABLE with zero findings",
      "canon has no record kind for a review round",
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
    "src/panels/review-rounds.ts",
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

test("the review rounds panel never sells fix_of_fix as proof of causation", () => {
  // The trap this panel exists inside: `fix_of_fix` is evidence that a
  // fix INTRODUCED a new defect only if `introduced_by` was sourced
  // honestly. The view's predicate is `g.resolution_sha =
  // f.introduced_by` — equality between two RECORDED commit ids — so no
  // surface may upgrade that to a causal claim. `render.rs`'s own
  // `review_rounds_panel_states_the_join_and_never_claims_causation`
  // runs the same phrase list against the markdown side.
  const causal = ["caused", "proves", "produced", "resulted in", "led to", "drove", "demonstrates that"];
  const note = ` ${REVIEW_ROUNDS_NOTE.toLowerCase()}`;
  for (const phrase of causal) {
    expect({ phrase, present: note.includes(` ${phrase}`) }).toEqual({ phrase, present: false });
  }

  const fixOfFix = REVIEW_ROUNDS_COLUMNS.find((c) => c.key === "fix_of_fix");
  // The bare label would be the lie: "Fix-of-fix" reads as global and
  // exact, and the column is neither.
  expect(fixOfFix?.label).not.toBe("Fix-of-fix");
  expect(fixOfFix?.label).toContain("not a bound");
  expect(fixOfFix?.label).toContain("same change");
  expect(fixOfFix?.description).toContain("ordered strictly by (round, seq)");
  expect(fixOfFix?.description).toContain("no git history is read and no blame is computed");
  expect(fixOfFix?.description).toContain("not evidence the earlier fix caused the later defect");
});

test("the review rounds panel gives the unsourced bucket its own name, not a zero", () => {
  // `introduced_by = None` is UNSOURCED. Folding it into
  // not-a-fix-of-fix converts a known unknown into a negative, which is
  // the whole reason the column exists separately.
  const unsourced = REVIEW_ROUNDS_COLUMNS.find((c) => c.key === "introduced_by_unsourced");
  expect(unsourced?.label).toContain("UNSOURCED");
  expect(unsourced?.description).toContain("canon never infers one");
  expect(unsourced?.description).toContain("never as not-a-fix-of-fix");
  // The bound that makes the unknown readable against the count.
  const sourced = REVIEW_ROUNDS_COLUMNS.find((c) => c.key === "introduced_by_sourced");
  expect(sourced?.description).toContain("Bounds fix_of_fix from above");
  // A blank commit is the common case, and the copy must say which
  // direction that holds in rather than reading the blank backwards.
  const reviewed = REVIEW_ROUNDS_COLUMNS.find((c) => c.key === "reviewed_sha");
  expect(reviewed?.description).toContain("uncommitted working tree");
  expect(reviewed?.description).toContain("not evidence of a worktree review");
});

test("the review rounds panel calls fix_of_fix no kind of bound, on either surface", () => {
  // s43 round 1, seq 1. Both surfaces shipped "FLOOR", and the corpus
  // under them disproves the direction: `f438c610` closed s42's round 8
  // AND shipped s42's whole feature, so round 9's six matches are a
  // commit-id coincidence rather than defects the round-8 fixes
  // introduced. Two clauses make the count MISS (the unsourced filter,
  // the same-change scope) and the bare equality predicate makes it
  // OVER-count, so it is exact for what it joins and a bound on
  // nothing.
  //
  // Same idea as the causal list above, applied to direction: the word
  // is banned OUTRIGHT, not merely when asserted. A panel that says
  // "not a floor" makes the reader weigh a denial against the word, and
  // the word wins. `render.rs`'s
  // `review_rounds_panel_calls_the_fix_of_fix_count_no_kind_of_bound`
  // runs the same list against the markdown side.
  const directional = [
    "floor",
    "lower bound",
    "lower-bound",
    "bounds from below",
    "minimum",
    "at least",
    "no fewer than",
    "conservative",
    "understates",
    "underestimate",
    "under-estimate",
    "upper bound",
    "ceiling",
    "at most",
    "no more than",
    "overstates",
  ];
  const fixOfFix = REVIEW_ROUNDS_COLUMNS.find((c) => c.key === "fix_of_fix");
  const surfaces: [string, string][] = [
    ["NOTE", REVIEW_ROUNDS_NOTE],
    ["fix_of_fix label", fixOfFix?.label ?? ""],
    ["fix_of_fix description", fixOfFix?.description ?? ""],
    ["REVIEW_ROUNDS_PANEL", EMITTED_PANELS.get("REVIEW_ROUNDS_PANEL") ?? ""],
  ];
  for (const [where, text] of surfaces) {
    const haystack = ` ${text.toLowerCase()}`;
    for (const phrase of directional) {
      expect({ where, phrase, present: haystack.includes(` ${phrase}`) }).toEqual({ where, phrase, present: false });
    }
  }
});

test("the review rounds panel says a round that found nothing has no row", () => {
  // s43 round 1, seq 2. `WHERE kind = 'finding'` is the view's only
  // source, so a clean round is invisible — s42's round 12 returned
  // MERGEABLE with zero findings and has no row. Reading the row count
  // as review effort is the misreading, and canon cannot even supply
  // the true number: no record kind marks a round as RUN.
  for (const [where, text] of [
    ["NOTE", REVIEW_ROUNDS_NOTE],
    ["REVIEW_ROUNDS_PANEL", EMITTED_PANELS.get("REVIEW_ROUNDS_PANEL") ?? ""],
  ] as [string, string][]) {
    expect({ where, states: text.includes("counts the rounds that FOUND something, never the rounds RUN") }).toEqual({ where, states: true });
    expect({ where, states: text.includes("canon has no record kind for a review round") }).toEqual({ where, states: true });
  }
});

// ── The THIRD surface: `canon/skills/canon-report-dashboard/SKILL.md` ──
//
// s43 follow-up. The two surfaces above were pinned to each other while
// the skill doc — the file an agent reads to LEARN how the report works
// — sat outside the pin and drifted by three changes at once: it said
// "five panels" after `mart_scope_status` (s24), `mart_subjects` (s36)
// and `mart_review_rounds` (s43) had been appended, and its role-memory
// bullet still read "strategies, hit rate, and an effect proxy per role
// namespace" — the exact wording s42's re-review had already corrected
// in `render.rs`, over a view with no effect column and a `hit_rate`
// that is `active_count / strategy_count`. `canon skills install`
// checks COPY FIDELITY between `canon/skills/` and its mirrors; it has
// no view of whether the copied bytes are true.
//
// Pinned the same way as above — against EMITTED report prose only, so
// a claim can never be satisfied by a `render.rs` comment or unit-test
// literal.
const SKILL_SOURCE = "canon/skills/canon-report-dashboard/SKILL.md";
const SKILL_MD = readFileSync(join(REPO_ROOT, SKILL_SOURCE), "utf-8");
const SNAPSHOT_RS = readFileSync(join(REPO_ROOT, "crates/canon-report/src/snapshot.rs"), "utf-8");

/**
 * The panels `render` really pushes, in order: each `## ` heading, the
 * mart it renders a table for, and the prose emitted between the two.
 *
 * Scanning `pub fn render`'s body (never the whole file) keeps this on
 * the same footing as `emittedPanels`: `out.push_str("…")` string
 * literals and `out.push_str(SCREAMING_CASE)` constants are the only
 * things matched, so `//` comments in that body — and everything in
 * `mod tests` below it — are out of scope. Unlike `emittedPanels` this
 * also covers the three panels whose prose is a one-line inline
 * literal rather than a named constant (trust matrix, scope status,
 * subjects), which is exactly where a paraphrase would otherwise be
 * unpinnable.
 */
function renderedPanels(source: string): { heading: string; view: string; prose: string }[] {
  const start = source.indexOf("pub fn render(");
  const end = source.indexOf("#[cfg(test)]", start);
  const body = source.slice(start, end === -1 ? undefined : end);
  const step = /out\.push_str\((?:("(?:[^"\\]|\\.)*")|([A-Z][A-Z0-9_]*))\);|render_table\(&mut out, &marts\.(\w+)\)/g;

  const panels: { heading: string; view: string; prose: string }[] = [];
  let heading = "";
  let prose = "";
  for (const [, literal, constant, field] of body.matchAll(step)) {
    if (field !== undefined) {
      panels.push({ heading, view: `mart_${field}`, prose });
      continue;
    }
    const text = literal !== undefined ? (JSON.parse(literal) as string) : EMITTED_PANELS.get(constant!);
    if (text === undefined) throw new Error(`render pushes ${constant}, which is not a declared *_PANEL constant`);
    if (text.startsWith("## ")) {
      heading = text.slice(3).trim();
      prose = "";
    } else {
      prose += text;
    }
  }
  return panels;
}

const RENDERED_PANELS = renderedPanels(REPORT_RENDER_RS);

/** `snapshot.rs`'s `SNAPSHOT_TABLES`, in declaration order. */
function snapshotTables(source: string): string[] {
  const block = /pub const SNAPSHOT_TABLES: &\[&str\] = &\[([^\]]*)\];/.exec(source);
  if (!block) throw new Error("SNAPSHOT_TABLES not found in snapshot.rs");
  return [...block[1].matchAll(/"([^"]+)"/g)].map(([, table]) => table);
}

const SNAPSHOT_TABLES = snapshotTables(SNAPSHOT_RS);

/** Markdown soft-wraps are not semantic; a claim spanning two source lines is the same claim. */
const unwrap = (text: string) => text.replace(/\s+/g, " ");
const SKILL_TEXT = unwrap(SKILL_MD);

/**
 * Clauses the skill must carry VERBATIM from the report's own prose,
 * keyed by the panel heading `render` pushes.
 *
 * Deliberately clauses, not whole paragraphs. The report's panels run
 * to a screen each (`FLYWHEEL_FUNNEL_PANEL` alone is ~1.4k characters)
 * and a skill that quoted them entire would be a second copy of the
 * report rather than a guide — the pin would then fail on every
 * harmless rewording and get deleted.
 *
 * So what is pinned is the CORRECTIONS: for each panel, the clauses
 * that exist because a reader who believed the obvious reading of a
 * column name would act wrongly. `hit_rate` sounds like a retrieval
 * rate and is not one; `divergence_open_running_total` sounds like
 * current state and is not; `applied` sounds causal and is
 * co-occurrence; `workspace_label` sounds like a repo and splits and
 * merges them; `fix_of_fix` sounds exact and bounds nothing. Each of those
 * was already shipped wrong once. Everything else in a panel —
 * ordering notes, column inventories, worked bounds — may be
 * summarised freely.
 *
 * Every clause is asserted to be a substring of the EMITTED prose too,
 * so this table can never quietly acquire a claim the report does not
 * make.
 */
const SKILL_CLAIMS: Record<string, string[]> = {
  // Thin panels: the report's whole line, minus its trailing view
  // reference, which the skill states in its own bullet header.
  "Trust matrix": ["Change/task coverage × green × who"],
  "Scope status": ["Task done × evidence-verified × spec-covered, per declared scenario ref"],
  Subjects: ["Per-domain subject rollup: status × scenario coverage"],
  "Session costs": [
    "a re-ingested corrected cost REPLACES the superseded figure instead of being summed with it",
    "`workspace_label` is NOT a repo identity",
    "SPLITS one repo whose worktrees sit in differently-named directories",
    "MERGES two different repos sharing a directory name",
  ],
  // The bullet that drifted: every clause here replaces a word of the
  // retired "strategies, hit rate, and an effect proxy" phrasing.
  "Role memory": [
    "`hit_rate` is NOT a retrieval hit rate",
    "exactly `active_count / strategy_count`",
    "canon records no per-strategy reward or effect metric",
  ],
  "Flywheel funnel": [
    "the last three stages count STRATEGIES, so the funnel narrows by construction",
    "Both rules are CO-OCCURRENCE inside one run, never causation",
    "canon stores no edge from a strategy to a verdict",
  ],
  "Review burn-down": [
    "a running `opened - resolved` event count, NOT the number open now",
    "For current state per scenario, run `canon divergence status`",
  ],
  // s43 round 1 reached this surface within the hour: the skill was
  // written from the panel and faithfully copied "FLOOR" into a third
  // place. That is the pin earning its keep — the correction now has
  // to land on all three at once, instead of one of them rotting for
  // three releases the way the role-memory wording did.
  "Review rounds": [
    "a commit-id equality join over two recorded fields and nothing more",
    "reads no git history, computes no blame",
    "counted there and NEVER as not-a-fix-of-fix",
    "`introduced_by_unsourced` is the UNKNOWN bucket",
    "`fix_of_fix` therefore bounds NOTHING — not from below, not from above",
    "a `resolution_sha` commit may carry work BEYOND the fix",
    "counts the rounds that FOUND something, never the rounds RUN",
  ],
};

test("the rendered panel order is the snapshot table order", () => {
  // Guards `renderedPanels` itself: if the scan ever silently matched
  // nothing, or drifted off `render`'s body, every skill assertion
  // below would pass vacuously.
  expect(RENDERED_PANELS.map((panel) => panel.view)).toEqual(SNAPSHOT_TABLES);
});

test("the skill documents every panel the report renders, and no other", () => {
  // The "five panels" drift: `mart_scope_status`, `mart_subjects` and
  // `mart_review_rounds` were each appended to the report without the
  // skill hearing about it. A panel is documented when its bullet
  // header names both the heading a reader sees and the view behind it.
  const documented = [...SKILL_MD.matchAll(/^- \*\*(.+?)\*\* \(`(mart_\w+)`\)/gm)].map(([, heading, view]) => ({ heading, view }));
  expect(documented).toEqual(RENDERED_PANELS.map(({ heading, view }) => ({ heading, view })));
});

test("the skill's snapshot manifest example is the real table contract", () => {
  // The stale example listed five tables, so a reader wiring a consumer
  // against it would have missed three exports outright.
  const fenced = [...SKILL_MD.matchAll(/```json\n([\s\S]*?)```/g)].map(([, json]) => json);
  const manifest = fenced.map((json) => JSON.parse(json) as { tables?: { table: string; file: string }[] }).find((parsed) => parsed.tables);
  expect(manifest?.tables).toEqual(SNAPSHOT_TABLES.map((table) => ({ table, file: `${table}.parquet` })));
});

for (const [heading, claims] of Object.entries(SKILL_CLAIMS)) {
  const panel = RENDERED_PANELS.find((candidate) => candidate.heading === heading);
  for (const claim of claims) {
    test(`the skill and the markdown report both state, for ${heading}: ${claim}`, () => {
      // Emitted-side first: a claim the report does not make is a
      // skill-only fiction, and pinning the skill to it would entrench
      // the fiction rather than catch it.
      expect(unwrap(panel?.prose ?? "")).toContain(unwrap(claim));
      expect(SKILL_TEXT).toContain(unwrap(claim));
    });
  }
}

test("the installed skill mirrors carry the corrected source", () => {
  // `canon skills install` is the only thing that materializes these,
  // and nothing re-runs it automatically — so a correction landing in
  // `canon/skills/` while `.claude/`/`.codex/` still serve the old
  // bytes is drift of exactly the kind this file exists to catch, one
  // copy further out.
  const claude = readFileSync(join(REPO_ROOT, ".claude/skills/canon-report-dashboard/SKILL.md"), "utf-8");
  expect(claude).toBe(SKILL_MD);

  // `.codex/` is the flattened convention: frontmatter promoted to a
  // header block, body verbatim after it.
  const codex = readFileSync(join(REPO_ROOT, ".codex/skills/canon-report-dashboard.md"), "utf-8");
  const body = SKILL_MD.split(/^---$/m)[2] ?? "";
  expect(codex).toContain(body.trimEnd());
});
