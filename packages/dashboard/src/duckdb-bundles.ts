// Self-hosted DuckDB-Wasm instantiation (design.md D4): manual bundle
// selection, never `duckdb.getJsDelivrBundles()`. The `?url` suffix makes
// Vite resolve each asset from the pinned `@duckdb/duckdb-wasm` npm
// package already on disk (fetched once by `bun install`, not at page
// load) and copy it into `dist/assets/` as a real, content-hashed file at
// build time — the documented Vite integration pattern from duckdb.org's
// Wasm instantiation docs, adapted from the doc's Webpack example. Only
// the `mvp` and `eh` bundles are wired (matches D4's literal "wasm/
// worker/eh" wording) — `coi` needs COOP/COEP cross-origin-isolation
// response headers this static app makes no promise about serving, so it
// is intentionally omitted rather than half-wired.
import * as duckdb from "@duckdb/duckdb-wasm";
import mvpWasmUrl from "@duckdb/duckdb-wasm/dist/duckdb-mvp.wasm?url";
import mvpWorkerUrl from "@duckdb/duckdb-wasm/dist/duckdb-browser-mvp.worker.js?url";
import ehWasmUrl from "@duckdb/duckdb-wasm/dist/duckdb-eh.wasm?url";
import ehWorkerUrl from "@duckdb/duckdb-wasm/dist/duckdb-browser-eh.worker.js?url";

export type BundleVariant = "mvp" | "eh";

const MANUAL_BUNDLES: duckdb.DuckDBBundles = {
  mvp: { mainModule: mvpWasmUrl, mainWorker: mvpWorkerUrl },
  eh: { mainModule: ehWasmUrl, mainWorker: ehWorkerUrl },
};

export interface DuckDBHandle {
  db: duckdb.AsyncDuckDB;
  /** Which manual bundle `selectBundle` picked — the core `parquet`
   * extension binary (src/duckdb-extensions.ts) must match this variant. */
  variant: BundleVariant;
}

let handlePromise: Promise<DuckDBHandle> | undefined;

/** Lazily instantiates the one shared self-hosted AsyncDuckDB instance. */
export function getDuckDB(): Promise<DuckDBHandle> {
  handlePromise ??= (async () => {
    const bundle = await duckdb.selectBundle(MANUAL_BUNDLES);
    const variant: BundleVariant = bundle.mainModule === MANUAL_BUNDLES.eh?.mainModule ? "eh" : "mvp";
    const worker = new Worker(bundle.mainWorker!);
    const logger = new duckdb.ConsoleLogger(duckdb.LogLevel.WARNING);
    const db = new duckdb.AsyncDuckDB(logger, worker);
    await db.instantiate(bundle.mainModule, bundle.pthreadWorker);
    return { db, variant };
  })();
  return handlePromise;
}
