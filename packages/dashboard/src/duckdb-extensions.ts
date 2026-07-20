import type { AsyncDuckDBConnection } from "@duckdb/duckdb-wasm";
import type { BundleVariant } from "./duckdb-bundles";
import parquetExtensionEhUrl from "../vendor/duckdb-extensions/v1.4.3/wasm_eh/parquet.duckdb_extension.wasm?url";
import parquetExtensionMvpUrl from "../vendor/duckdb-extensions/v1.4.3/wasm_mvp/parquet.duckdb_extension.wasm?url";

// `read_parquet()` needs DuckDB's `parquet` core extension, which is NOT
// statically compiled into `@duckdb/duckdb-wasm`'s core mvp/eh module —
// DuckDB core (v1.4.3, bundled by the pinned `@duckdb/duckdb-wasm@1.32.0`)
// treats `parquet` as an autoloadable core extension, fetched on first use
// from `https://extensions.duckdb.org/<version>/<platform>/
// parquet.duckdb_extension.wasm`. Left as the library default, every
// `read_parquet()` call would silently hit that CDN — exactly the runtime
// network dependency design.md D4 forbids, and one `?url`-imported core
// bundle alone does not prevent.
//
// Fix: both platform variants of that exact extension binary are vendored
// under vendor/duckdb-extensions/ (one-time download, re-fetch only when
// the pinned duckdb-wasm version's bundled DuckDB core version changes)
// and resolved through the SAME `?url` self-hosting pattern as the core
// bundle. `LOAD '<local url>'` — an explicit path/URL, never bare `LOAD
// parquet` / `INSTALL parquet` — makes DuckDB fetch exactly that local
// asset instead of resolving through its default repository URL template,
// so no version/platform-string guessing is needed and no network call
// happens. `autoinstall_known_extensions`/`autoload_known_extensions` are
// also disabled first, belt-and-suspenders, so any future core-extension
// dependency fails loudly instead of silently reaching the network.
const PARQUET_EXTENSION_URL: Record<BundleVariant, string> = {
  eh: parquetExtensionEhUrl,
  mvp: parquetExtensionMvpUrl,
};

let loaded = false;

export async function ensureParquetExtension(conn: AsyncDuckDBConnection, variant: BundleVariant): Promise<void> {
  if (loaded) return;
  await conn.query("SET autoinstall_known_extensions=false");
  await conn.query("SET autoload_known_extensions=false");
  await conn.query(`LOAD '${PARQUET_EXTENSION_URL[variant]}'`);
  loaded = true;
}
