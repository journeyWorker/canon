#!/usr/bin/env bun
// Regenerates packages/dashboard's committed fixture snapshot
// (fixtures/snapshot/*.parquet + manifest.json) from
// build-fixture-snapshot.sql. Run with `bun run fixture:build` from
// packages/dashboard. Requires the `duckdb` CLI on PATH (used only to
// author this fixture — the shipped app never depends on the native CLI,
// only on @duckdb/duckdb-wasm).
//
// manifest.json's shape mirrors the S9 SHARED SNAPSHOT CONTRACT
// `canon report --snapshot` produces (design.md D3): `generated_at`,
// `source_git_sha`, `source_digest`, `tables: [{table, file}]`. Unlike a
// real snapshot, this fixture's `source_git_sha`/`generated_at` are fixed,
// clearly-synthetic placeholders (not tied to any real commit or wall
// clock) — this manifest is never drift-checked (D3), it only has to be a
// stable, reviewable dev/test fixture. `source_digest` IS real: sha256
// over the concatenated bytes of the seven parquet files, in table order,
// 12 hex chars — the same `digest12` shape
// `crates/canon-report/src/digest.rs` uses.

import { createHash } from "node:crypto";
import { spawnSync } from "node:child_process";
import { readFileSync, writeFileSync, existsSync, mkdirSync } from "node:fs";
import { join, dirname } from "node:path";

const PKG_ROOT = join(dirname(new URL(import.meta.url).pathname), "..");
const SNAPSHOT_DIR = join(PKG_ROOT, "fixtures", "snapshot");
const SQL_FILE = join(PKG_ROOT, "scripts", "build-fixture-snapshot.sql");

// Declared table -> file map, in the exact order `tables[]` is written —
// this list (not directory enumeration) is what the dashboard's loader
// reads at runtime (design.md D3's "never enumerate the directory").
const TABLES = [
  "mart_trust_matrix",
  "mart_session_costs",
  "mart_role_memory",
  "mart_flywheel_funnel",
  "mart_review_burndown",
  "mart_scope_status",
  "mart_subjects",
] as const;

function main(): void {
  mkdirSync(SNAPSHOT_DIR, { recursive: true });

  const duckdb = spawnSync("duckdb", ["-c", `.read ${SQL_FILE}`], {
    cwd: PKG_ROOT,
    stdio: "inherit",
  });
  if (duckdb.status !== 0) {
    console.error("duckdb CLI failed while building the fixture snapshot");
    process.exit(duckdb.status ?? 1);
  }

  const concatenated = Buffer.concat(
    TABLES.map((table) => {
      const file = join(SNAPSHOT_DIR, `${table}.parquet`);
      if (!existsSync(file)) {
        throw new Error(`expected COPY output missing: ${file}`);
      }
      return readFileSync(file);
    }),
  );

  const manifest = {
    generated_at: "2026-07-11T00:00:00Z",
    source_git_sha: "fixture0000",
    source_digest: createHash("sha256").update(concatenated).digest("hex").slice(0, 12),
    tables: TABLES.map((table) => ({ table, file: `${table}.parquet` })),
  };

  writeFileSync(
    join(SNAPSHOT_DIR, "manifest.json"),
    JSON.stringify(manifest, null, 2) + "\n",
  );

  console.log(`wrote ${SNAPSHOT_DIR}/manifest.json`);
  console.log(manifest);
}

main();
