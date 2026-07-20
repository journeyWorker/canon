import { afterAll, beforeAll, expect, test } from "bun:test";
import { spawn, type Subprocess } from "bun";
import { existsSync } from "node:fs";
import puppeteer, { type Browser } from "puppeteer-core";

// End-to-end proof of task 5.6 / design.md D4: the built app instantiates
// DuckDB-Wasm (self-hosted mvp/eh bundle + self-hosted `parquet` core
// extension, src/duckdb-bundles.ts + src/duckdb-extensions.ts), loads this
// package's committed fixture snapshot (fixtures/snapshot/), and renders
// non-empty data in all five panels — while every non-localhost host is
// UNREACHABLE, not merely unobserved.
//
// Network isolation is enforced at the browser-process level via
// `--proxy-server`/`--proxy-bypass-list` (routes everything except
// localhost through a closed port, so a stray external request gets a
// real connection failure) rather than CDP `Page.setRequestInterception`:
// interception proxies every response body through the DevTools protocol,
// which corrupts `WebAssembly.instantiateStreaming`'s streamed read of the
// ~35-40MB core wasm module under CDP's per-request round-trip latency
// (verified empirically — interception produced a nondeterministic "RuntimeError:
// function signature mismatch" wasm trap; the proxy approach does not,
// because Chrome's own network stack denies the request before it ever
// reaches page/DevTools-protocol code). Requests are still recorded
// (passively, no interception) to additionally assert no non-local URL was
// even attempted.
//
// Requires a real browser — DuckDB-Wasm needs Worker + WebAssembly
// execution no DOM-emulation library provides — resolved via
// `PUPPETEER_EXECUTABLE_PATH` or a handful of standard OS install paths
// (no puppeteer browser download).

const PKG_ROOT = new URL("..", import.meta.url).pathname;
const PORT = 4319;
const BASE_URL = `http://localhost:${PORT}/`;

function findChromeExecutable(): string {
  const candidates = [
    process.env.PUPPETEER_EXECUTABLE_PATH,
    "/Applications/Google Chrome.app/Contents/MacOS/Google Chrome",
    "/usr/bin/google-chrome-stable",
    "/usr/bin/google-chrome",
    "/usr/bin/chromium",
    "/usr/bin/chromium-browser",
  ].filter((candidate): candidate is string => Boolean(candidate));
  const found = candidates.find((candidate) => existsSync(candidate));
  if (!found) {
    throw new Error("no Chrome/Chromium executable found for the smoke test; set PUPPETEER_EXECUTABLE_PATH");
  }
  return found;
}

// `vite preview` is spawned as a real child process serving `dist/`; its
// startup time is genuine external-process latency, not something this
// test's own logic can make deterministic, so we await the process's own
// "ready" log line instead of a wall-clock sleep (a real signal, not a
// guessed delay).
function waitForServerReady(server: Subprocess<"ignore", "pipe", "pipe">): Promise<void> {
  const { promise, resolve, reject } = Promise.withResolvers<void>();
  const timeout = setTimeout(() => reject(new Error("vite preview did not become ready in time")), 20_000);

  (async () => {
    const reader = server.stdout.getReader();
    const decoder = new TextDecoder();
    let buffered = "";
    while (true) {
      const { done, value } = await reader.read();
      if (done) break;
      buffered += decoder.decode(value, { stream: true });
      if (buffered.includes("Local:") || buffered.includes(String(PORT))) {
        clearTimeout(timeout);
        resolve();
        return;
      }
    }
    reject(new Error("vite preview stdout closed before signaling readiness"));
  })().catch(reject);

  return promise;
}

let server: Subprocess<"ignore", "pipe", "pipe">;
let browser: Browser;

beforeAll(async () => {
  server = spawn({
    cmd: ["bun", "x", "vite", "preview", "--port", String(PORT), "--strictPort"],
    cwd: PKG_ROOT,
    stdout: "pipe",
    stderr: "pipe",
    stdin: "ignore",
  });
  await waitForServerReady(server);

  browser = await puppeteer.launch({
    executablePath: findChromeExecutable(),
    headless: true,
    args: [
      "--no-sandbox",
      "--disable-gpu",
      // Every host but localhost routes through an unreachable proxy port
      // — any accidental external request fails fast at the network
      // layer instead of merely going unobserved.
      "--proxy-server=http://127.0.0.1:1",
      "--proxy-bypass-list=127.0.0.1,localhost",
    ],
  });
});

afterAll(async () => {
  await browser?.close();
  server?.kill();
});

test("renders all 5 panels from the fixture snapshot with zero third-party network", async () => {
  const page = await browser.newPage();
  const requestedUrls: string[] = [];
  page.on("request", (req) => requestedUrls.push(req.url()));

  await page.goto(BASE_URL, { waitUntil: "networkidle0" });
  await page.waitForSelector("body[data-status='ready']", { timeout: 15_000 });

  const manifest = await Bun.file(`${PKG_ROOT}fixtures/snapshot/manifest.json`).json();
  const bannerText = await page.$eval("#banner", (el) => el.textContent ?? "");
  expect(bannerText).toContain(manifest.source_git_sha);
  expect(bannerText).toContain(manifest.source_digest);

  const panelIds = [
    "panel-trust-matrix",
    "panel-session-costs",
    "panel-role-memory",
    "panel-flywheel-funnel",
    "panel-review-burndown",
  ];
  for (const panelId of panelIds) {
    const rowCount = await page.$$eval(`#${panelId} tbody tr`, (rows) => rows.length);
    expect(rowCount).toBeGreaterThan(0);
  }

  const nonLocalUrls = requestedUrls.filter((url) => {
    const hostname = new URL(url).hostname;
    return hostname !== "localhost" && hostname !== "127.0.0.1";
  });
  expect(nonLocalUrls).toEqual([]);

  await page.close();
}, 30_000);
