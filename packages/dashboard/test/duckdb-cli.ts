// Thin wrapper over the native `duckdb` CLI, shared by this package's
// two fixture-checking test files. The CLI is the same binary
// `scripts/build-fixture-snapshot.ts` already requires on PATH to
// author `fixtures/snapshot/*.parquet` in the first place, so reading
// the committed fixtures back through it adds no new dependency — and
// it reads the REAL on-disk bytes, never a value this repo's SQL
// sources merely assert.

import { spawnSync } from "node:child_process";

/** True when the native `duckdb` CLI is reachable on PATH. */
export function duckdbAvailable(): boolean {
  return spawnSync("duckdb", ["--version"]).status === 0;
}

/** Runs `sql` through `duckdb -json` and returns the parsed rows. */
export function queryJson<T = Record<string, unknown>>(sql: string): T[] {
  const result = spawnSync("duckdb", ["-json", "-c", sql], { encoding: "utf-8" });
  if (result.status !== 0) {
    throw new Error(`duckdb query failed: ${result.stderr}\n--- sql ---\n${sql}`);
  }
  const trimmed = result.stdout.trim();
  return (trimmed ? JSON.parse(trimmed) : []) as T[];
}

/** SQL string literal for a filesystem path (doubles embedded quotes). */
export function sqlLiteral(path: string): string {
  return `'${path.replace(/'/g, "''")}'`;
}
