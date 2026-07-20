import { expect, test } from "bun:test";
import { join } from "node:path";
import { describeParquetSchema, duckdbAvailable, EXPECTED_MART_SCHEMA } from "./fixture-schema";

// ReviewS9b finding 2: locks packages/dashboard's committed fixture
// Parquet schema against the S9 SHARED SNAPSHOT CONTRACT
// (`crates/canon-store/sql/views.sql`'s own mart_* SELECT lists,
// `test/fixture-schema.ts`'s own `EXPECTED_MART_SCHEMA`). Before this
// test existed, a schema drift in `fixtures/snapshot/*.parquet` (a
// renamed/reordered/added/dropped/retyped column) would still render a
// row in the dashboard's own `test/smoke.test.ts` — that test only
// asserts `rowCount > 0`, never the column shape — and pass silently.
// Reading each fixture's REAL on-disk schema via `duckdb`'s own
// `DESCRIBE` (never a value this repo's SQL sources merely assert)
// closes that gap: writer (`canon-report`) == fixture is enforced here,
// the same way `crates/canon-report/tests/snapshot.rs`'s
// `EXPECTED_CONTRACT` already enforces writer == `views.sql` on the
// Rust side.
const PKG_ROOT = new URL("..", import.meta.url).pathname;

for (const [mart, expectedColumns] of Object.entries(EXPECTED_MART_SCHEMA)) {
  test(`fixtures/snapshot/${mart}.parquet's schema matches the S9 shared snapshot contract`, () => {
    if (!duckdbAvailable()) {
      console.error("skipping: `duckdb` CLI not found on PATH");
      return;
    }
    const fixturePath = join(PKG_ROOT, "fixtures", "snapshot", `${mart}.parquet`);
    const actualColumns = describeParquetSchema(fixturePath);
    expect(actualColumns).toEqual(expectedColumns);
  });
}
