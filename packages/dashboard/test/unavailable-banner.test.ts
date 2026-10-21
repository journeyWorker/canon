import { afterEach, expect, mock, test } from "bun:test";

// report.dashboard.04: when `canon dashboard` says the repo has no snapshot,
// the panels page shows why and the fix instead of panels, and loads nothing.

class FakeElement {
  className = "";
  textContent = "";
  style: Record<string, string> = {};
  dataset: Record<string, string> = {};
  children: FakeElement[] = [];
  constructor(readonly tag: string) {}
  replaceChildren(...nodes: FakeElement[]): void {
    this.children = nodes;
  }
  append(...nodes: FakeElement[]): void {
    this.children.push(...nodes);
  }
  remove(): void {
    removed.push(this.tag);
  }
  texts(): string[] {
    return this.children.map((c) => c.textContent);
  }
}

const removed: string[] = [];
const loaded: string[] = [];
const banner = new FakeElement("banner");
const panels = new FakeElement("panels");
const body = new FakeElement("body");

function installDom(): void {
  const selectors: Record<string, FakeElement> = { "#banner": banner, "#panels": panels };
  Object.assign(globalThis, {
    document: {
      createElement: (tag: string) => new FakeElement(tag),
      querySelector: (selector: string) => selectors[selector] ?? null,
      body,
    },
    window: { location: { search: "" } },
  });
}

const status = { available: false, reason: "duckdb is not on PATH", fix: "install DuckDB (https://duckdb.org/docs/installation)" };

afterEach(() => {
  mock.restore();
});

test("report.dashboard.04 the unavailable-snapshot page shows the reason and the fix and loads no panel", async () => {
  installDom();
  const trackLoad = (name: string) => async () => {
    loaded.push(name);
  };
  mock.module("../src/style.css", () => ({}));
  mock.module("../src/nav", () => ({ renderNav: () => {} }));
  mock.module("../src/status", () => ({ fetchSnapshotStatus: async () => status }));
  mock.module("../src/snapshot", () => ({
    loadSnapshot: async () => {
      loaded.push("snapshot");
      throw new Error("the snapshot must not be loaded");
    },
  }));
  for (const [file, fn] of [
    ["trust-matrix", "renderTrustMatrix"],
    ["session-costs", "renderSessionCosts"],
    ["role-memory", "renderRoleMemory"],
    ["flywheel-funnel", "renderFlywheelFunnel"],
    ["review-burndown", "renderReviewBurndown"],
    ["review-rounds", "renderReviewRounds"],
    ["review-totals", "renderReviewTotals"],
  ]) {
    mock.module(`../src/panels/${file}`, () => ({ [fn]: trackLoad(file) }));
  }

  await import("../src/main");
  await new Promise((resolve) => setTimeout(resolve, 20));

  expect(banner.className).toContain("banner-error");
  const shown = banner.texts();
  expect(shown).toContain("No snapshot to show.");
  expect(shown).toContain(status.reason);
  expect(shown).toContain(status.fix);
  expect(removed).toEqual(["panels"]);
  expect(body.dataset.status).toBe("no-snapshot");
  expect(loaded).toEqual([]);
});
