import { defineConfig } from "vite";

// packages/dashboard is a static, zero-network-after-load app (design.md
// D4): `publicDir: "fixtures"` serves the committed
// `fixtures/snapshot/{manifest.json,*.parquet}` at `/snapshot/*` verbatim
// in both `vite dev` and `vite build` (copied byte-for-byte into
// `dist/snapshot/`, never processed) — the same fixture the smoke test
// (test/smoke.test.ts) and local dev both load, no duplicate copy to
// drift out of sync. `assetsInlineLimit: 0` guarantees the `?url`-imported
// DuckDB-Wasm binary/worker assets (src/duckdb-bundles.ts) are always
// emitted as real hashed files under dist/assets/, never base64-inlined,
// so they stay separately cacheable and are unambiguously "shipped
// files", not embedded script text.
export default defineConfig({
  publicDir: "fixtures",
  build: {
    target: "es2022",
    outDir: "dist",
    assetsInlineLimit: 0,
  },
});
