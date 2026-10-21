import "./style.css";
import { loadSnapshot } from "./snapshot";
import { renderNav } from "./nav";
import { fetchSnapshotStatus } from "./status";
import { renderFreshnessBanner, renderErrorBanner, renderUnavailableBanner as renderUnavailable } from "./banner";
import { renderTrustMatrix } from "./panels/trust-matrix";
import { renderSessionCosts } from "./panels/session-costs";
import { renderRoleMemory } from "./panels/role-memory";
import { renderFlywheelFunnel } from "./panels/flywheel-funnel";
import { renderReviewBurndown } from "./panels/review-burndown";
import { renderReviewRounds } from "./panels/review-rounds";
import { renderReviewTotals } from "./panels/review-totals";

// Snapshot base defaults to this app's own committed fixture
// (fixtures/snapshot/, served at /snapshot/ — see vite.config.ts's
// `publicDir`); `?snapshot=<url>` overrides it, e.g. when `canon
// dashboard --snapshot <dir>` (task 6.1) serves a real snapshot at a
// different path alongside the built app.
const params = new URLSearchParams(window.location.search);
const snapshotParam = params.get("snapshot");

function panelBody(sectionId: string): HTMLElement {
  const el = document.querySelector<HTMLElement>(`#${sectionId} .panel-body`);
  if (!el) throw new Error(`missing panel container: #${sectionId} .panel-body`);
  return el;
}

async function main(): Promise<void> {
  const banner = document.querySelector<HTMLElement>("#banner");
  if (!banner) throw new Error("missing #banner element");
  const nav = document.querySelector<HTMLElement>("#nav");
  if (nav) renderNav(nav, "panels");

  // Under `canon dashboard` the server says whether the repo has a snapshot.
  // When it has none the page says why instead of showing panels, and never
  // falls back to the demo fixture; any other server has no status endpoint
  // and the page reads its own fixture as before.
  const status = await fetchSnapshotStatus();
  if (status && !status.available) {
    renderUnavailable(banner, status.reason, status.fix);
    document.querySelector<HTMLElement>("#panels")?.remove();
    document.body.dataset.status = "no-snapshot";
    return;
  }
  const snapshotBaseUrl = snapshotParam ?? (status ? "/live-snapshot/" : "snapshot/");

  try {
    const { manifest, conn } = await loadSnapshot(snapshotBaseUrl);
    renderFreshnessBanner(banner, manifest);

    // Sequential, not Promise.all: DuckDB-Wasm serializes commands over
    // one Worker message channel per connection, so overlapping queries
    // on the same AsyncDuckDBConnection are not a supported concurrency
    // pattern — seven small SELECTs have no meaningful latency cost from
    // running one after another.
    await renderTrustMatrix(conn, panelBody("panel-trust-matrix"));
    await renderSessionCosts(conn, panelBody("panel-session-costs"));
    await renderRoleMemory(conn, panelBody("panel-role-memory"));
    await renderFlywheelFunnel(conn, panelBody("panel-flywheel-funnel"));
    await renderReviewBurndown(conn, panelBody("panel-review-burndown"));
    await renderReviewRounds(conn, panelBody("panel-review-rounds"));
    await renderReviewTotals(conn, panelBody("panel-review-totals"));

    // Deterministic hook for the headless smoke test (test/smoke.test.ts)
    // to wait on instead of polling the DOM for table rows.
    document.body.dataset.status = "ready";
  } catch (error) {
    renderErrorBanner(banner, error);
    document.body.dataset.status = "error";
    throw error;
  }
}

void main();
