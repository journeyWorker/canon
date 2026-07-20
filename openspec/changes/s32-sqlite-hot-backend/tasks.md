# s32 sqlite-hot-backend — tasks

## 1. canon-store

- [x] 1.1 `Backend::Sqlite` (class `LiveDb`,
      `read_directly_by_report: false`) + canon.yaml parse:
      `{ backend: sqlite, path: <p> }`, path required (loud hint),
      relative-to-canon.yaml resolution; s28 class check covers it
      for free — add the local-rung-rejection test.
- [x] 1.2 `SqliteTier`: sqlx sqlite (add `"sqlite"` feature), WAL +
      busy_timeout on connect, `records_history` DDL mirroring pg
      (minus schema namespace), `write`/`write_batch` (s31 chunk
      pattern)/`read`; error mapping into the existing
      `StoreError` taxonomy (unreachable/corrupt file →
      `TierUnavailable`-class reason naming the path).
- [x] 1.3 Tests (all offline, in-process): dedup no-op, batch==loop,
      TierQuery read-back, WAL/busy pragmas applied, missing-path
      parse failure.

## 2. canon-cli + docs

- [x] 2.1 tiers builder: construct `SqliteTier` for sqlite entries
      (lenient + strict arms), `canon init --check-config` validates
      it; no env contract added.
- [x] 2.2 `canon init` template: hot → `{ backend: sqlite, path:
      canon/hot.db }`, commented postgres stanza as the swap,
      gitignore `canon/hot.db*`; template tests updated.
- [x] 2.3 Skills: `tiered-storage` + `canon-session-ingest` — sqlite
      default, pg swap path, single-writer caveat; rematerialize via
      `canon skills install`.
- [x] 2.4 Website `concepts/tiered-storage.mdx` EN+KR: sqlite in the
      backend table + env-contract section notes sqlite needs none.

## 3. Verification

- [x] 3.1 `cargo test --workspace` green offline; `canon selftest` +
      `canon gate selftest` green.
- [x] 3.2 Live smoke: temp dir → `canon init` → `canon ingest
      sessions` with NO env/services → records in `canon/hot.db`,
      `canon query --kind session` returns them; this repo's pg
      config still ingests unchanged.
