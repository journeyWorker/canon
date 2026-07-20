import type { AsyncDuckDBConnection } from "@duckdb/duckdb-wasm";
import { getDuckDB } from "./duckdb-bundles";
import { ensureParquetExtension } from "./duckdb-extensions";

// Shape of `manifest.json` per the S9 SHARED SNAPSHOT CONTRACT (design.md
// D3): the declared table -> file map `canon report --snapshot` writes.
// The loader below reads ONLY this file — it never enumerates the
// snapshot directory (a Wasm app cannot list arbitrary server directory
// contents at runtime; D3's whole point).
export interface ManifestTableEntry {
  table: string;
  file: string;
}

export interface SnapshotManifest {
  generated_at: string;
  source_git_sha: string;
  source_digest: string;
  tables: ManifestTableEntry[];
}

export interface LoadedSnapshot {
  manifest: SnapshotManifest;
  conn: AsyncDuckDBConnection;
}

const SAFE_IDENTIFIER = /^[A-Za-z_][A-Za-z0-9_]*$/;
const SAFE_FILENAME = /^[A-Za-z0-9_.-]+$/;

function resolveBaseUrl(base: string): URL {
  const url = new URL(base, window.location.href);
  if (!url.pathname.endsWith("/")) {
    url.pathname += "/";
  }
  return url;
}

async function fetchOrThrow(url: URL, what: string): Promise<Response> {
  const resp = await fetch(url.toString());
  if (!resp.ok) {
    throw new Error(`failed to fetch ${what} (${resp.status} ${resp.statusText}): ${url}`);
  }
  return resp;
}

/**
 * Loads a snapshot directory's `manifest.json`, registers every listed
 * Parquet file's bytes with DuckDB-Wasm, and materializes one table per
 * manifest entry — `CREATE TABLE "<table>" AS SELECT * FROM
 * read_parquet('<file>')`, register-then-query by filename, never raw
 * bytes into SQL (task 5.3). `baseUrl` is resolved relative to the
 * document so both `./snapshot/` (this app's own committed fixture,
 * served under `dist/snapshot/` via vite.config.ts's `publicDir`) and an
 * absolute override (`?snapshot=`) work identically.
 */
export async function loadSnapshot(baseUrl: string): Promise<LoadedSnapshot> {
  const base = resolveBaseUrl(baseUrl);
  const manifestUrl = new URL("manifest.json", base);
  const manifest = (await (await fetchOrThrow(manifestUrl, "manifest.json")).json()) as SnapshotManifest;

  const { db, variant } = await getDuckDB();
  const conn = await db.connect();
  await ensureParquetExtension(conn, variant);

  for (const { table, file } of manifest.tables) {
    if (!SAFE_IDENTIFIER.test(table)) {
      throw new Error(`manifest.json table name is not a safe SQL identifier: ${table}`);
    }
    if (!SAFE_FILENAME.test(file)) {
      throw new Error(`manifest.json file name is not a safe path segment: ${file}`);
    }
    const fileUrl = new URL(file, base);
    const bytes = new Uint8Array(await (await fetchOrThrow(fileUrl, file)).arrayBuffer());
    await db.registerFileBuffer(file, bytes);
    await conn.query(`CREATE OR REPLACE TABLE "${table}" AS SELECT * FROM read_parquet('${file}')`);
  }

  return { manifest, conn };
}
