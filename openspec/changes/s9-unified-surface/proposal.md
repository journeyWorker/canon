## Why

Per the design doc's problem statement (§1), accumulated agent-run signal today is
**write-only**: `canon report`'s predecessors in this team are hand-typed prose
(`PORT-PROGRESS.md`-style, banned by D16) or nonexistent, and the donor monorepo already shipped a
cross-worktree OpenSpec rollup endpoint (`GET /api/openspec/changes`) that has **zero
consumers** — no frontend ever calls it (confirmed: only its own test and a Bruno
request collection reference the route; the app's server mounts it and
nothing else touches it). S1–S8 (foundation, ingest, enforcement, learning) produce
join-spine-connected records but expose no surface a human can read. S9 closes that gap:
one generated, byte-stable report and one dashboard that mirrors it — never a second
place that can drift from what the gate/model actually computed.

## What Changes

- New `canon report` command (Rust, `crates/canon-report`) that generates a markdown
  status report from the S1 model over S2's tiered storage (`stg_/int_/mart_` DuckDB
  views, specdb.sql pattern) — **generated, never hand-edited** (D16): the report
  embeds input digests (corpus/policy/ledger-head hashes, no timestamps per decision 11)
  so every number traces to named inputs.
- New `canon report --check`: regenerates in memory and diffs against the committed/
  existing file; exit 0 on no drift, exit 1 with a stable message on drift (parity.py
  `cmd_report` pattern) — the only sanctioned freshness check, never a human "looks
  current" judgment.
- New `canon report --snapshot <dir>`: exports the same marts to Parquet (DuckDB's
  version-stable cross-engine interchange, not the native `.duckdb` binary — storage-
  format skew between writer and any Wasm reader) plus a `manifest.json`
  (`generated_at`, `source_git_sha`, `source_digest`, `tables: [{table, file}]`) —
  the **declared** table→file map the dashboard loader reads (a browser cannot
  enumerate a snapshot directory at runtime).
- New `packages/dashboard`: a static Bun/Vite + TypeScript web app using
  `@duckdb/duckdb-wasm` directly (no wrapper layer) against a **self-hosted** wasm/
  worker bundle — not the runtime jsDelivr CDN fetch the donor parity harness's Dart-wrapped dev
  tool accepted, because canon's own security note requires local-only/zero-network
  operation for OSS-style consumers (§9). The app loads the Parquet snapshot into an
  in-memory DuckDB-Wasm database via `manifest.json`, renders a freshness banner from
  its provenance fields, and issues **thin SELECT/filter queries only** — it never
  re-derives a mart (query-authority pattern: a mart-logic change is a `canon-report`
  edit + re-export, the app is unaffected and can never contradict `canon report`).
- New `canon dashboard` command: serves the built dashboard app locally against a
  snapshot directory.
- Five panels, each backed by one or more S9-owned marts atop the join-spine data
  earlier waves produce: change/task **trust matrix** (covered × green × who — S5),
  **session costs** (by role/repo/session — S3 ingest, the vendored upstream launcher's `session_id` join
  key), **role memory** (strategies, hit rate, effect — S6), **flywheel health**
  funnel (verdicts → distilled → retrieved → applied — S4/S6/S7/S8), **review-
  feedback burn-down** (S4's verdict stream, progress-over-time).
- Wires the donor monorepo's orphaned `GET /api/openspec/changes` as an **optional, donor-specific**
  supplemental data source for the trust-matrix panel: when `canon.yaml` configures a
  `dashboardRollupUrl` (the donor monorepo only), `canon report`'s ingest step calls the endpoint
  (sending the required `X-User-Id` header) and folds its `RolledChange[]` rows
  (`worktree`, `branch`, `slug`, `route`, `created`, `proposalTitle`) into the panel
  as a same-shape supplement to canon's own git-tier change scan — read-only, no
  change to the donor monorepo's route or its `walkWorktrees` core.
- Companion skill (`canon/skills/canon-report-dashboard/` or similar, decision 9):
  teaches agents to run `canon report` after work that should move the trust matrix,
  never to hand-edit the report, and how to read the dashboard panels.

## Capabilities

### New Capabilities

- `canon-report`: the generated-never-edited markdown status report over the S1
  model + S2 tiers, its `--check` byte-stability contract, and the Parquet +
  `manifest.json` snapshot export other surfaces mirror from.
- `unified-dashboard`: the browser-based DuckDB-Wasm dashboard that loads
  `canon-report`'s snapshot, renders the freshness banner and five panels, never
  re-derives a mart, and wires the donor monorepo's orphaned OpenSpec rollup endpoint as one
  panel data source.

### Modified Capabilities

_(none — `openspec/specs/` has no prior capabilities for this change to modify.)_

## Impact

- New crate `crates/canon-report` (report generation, digest computation, Parquet
  export via `arrow`), wired into `canon-cli` as `canon report` / `canon dashboard`.
- New package `packages/dashboard` (Bun/Vite + TS, `@duckdb/duckdb-wasm`,
  self-hosted wasm/worker assets — no runtime CDN dependency).
- `canon.yaml` (per-repo config, S1) gains an optional `dashboardRollupUrl` field,
  consumed only by repos that set it (the donor monorepo).
- New skill(s) under `canon/skills/`, installed via `canon skills install` per
  decision 9 (materialized for Claude Code + Codex only, timestamp-free lock).
- No changes to the donor monorepo's OpenSpec rollup route or the donor CLI's
  openspec module — canon only becomes its first consumer.
- Depends on S1 (model + join spine), S2 (tiered storage + DuckDB views), S3/S4
  (ingest feeding the marts), S5 (trust ladder), S6/S7/S8 (learning/reward/
  retrieval events) already landing per the wave order (W0–W2 before W3).
