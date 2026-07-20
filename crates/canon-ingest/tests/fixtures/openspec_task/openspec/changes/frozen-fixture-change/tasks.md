## 1. Frozen fixture section

FROZEN fixture for `crates/canon-ingest/src/artifact_adapters/openspec_task.rs` (S4
wave-2, design D4). Row shapes follow canon-gate's own canonical checkbox
grammar (`crates/canon-gate/src/checkbox.rs::parse_line`/`format_line`): id
token immediately after `- [ ] `/`- [x] `, an optional `**DEFERRED to
§<to>**`/`**DROPPED**` annotation right after the id, and a checked row's
` — ✅ <evidence>` suffix.

- [x] 1.1 Implement the checkbox parser — ✅ https://github.com/example-org/canon/pull/482 merged
- [x] 1.2 Wire the CLI flag — ✅ verified manually against the fixture corpus, all green
- [ ] 1.3 **DEFERRED to §2.1** Backfill the legacy schema shim (blocked on S11 migration landing)
- [ ] 1.4 **DROPPED** Patch the old formatter in place (superseded by task 1.5's broader rewrite)
- [ ] 1.5 Not yet started — the broader rewrite mentioned above
