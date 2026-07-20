# s30 plan-dialect-superpowers — tasks

## 1. Adapter (crates/canon-ingest)

- [x] 1.1 `plan_adapters/superpowers.rs`: discovery (D5 shape
      tolerance, immediate-child `*.md`, deterministic order),
      Change mapping (D2 slug identity, D4 Goal summary + status +
      mtime `at`), Task mapping (D3 shared `task_id_for`, checkbox
      status, duplicate/invalid handling), named diagnostics (D6
      vocabulary).
- [x] 1.2 Register one `PlanAdapterEntry` in `plan_registry.rs`;
      update the "deferred" doc comments in `plan_adapter.rs`,
      `plan_adapters/mod.rs`, `plan_registry.rs`.
- [x] 1.3 Unit tests covering every spec scenario (Change identity +
      Goal, goal-missing, checkbox status matrix, duplicate/invalid
      task numbers, not-a-plan-doc, absent root) in the adapter
      module, doc-commented against the spec scenarios.
- [x] 1.4 Extend `plan_selftest.rs`'s fixture corpus with a
      superpowers fixture + EXPECTED oracle so `canon selftest`
      exercises the dialect.

## 2. CLI + docs

- [x] 2.1 `crates/canon-cli/tests/plans_ingest.rs`: end-to-end
      `--dialect superpowers --source <fixture>` import test (exit
      0, query returns records, statuses correct) + a canon.yaml
      `plans.sources[].dialect: superpowers` config-path test.
- [x] 2.2 `canon/skills/canon-plan-import/SKILL.md`: superpowers
      moves from deferred to shipped (grammar, worked example,
      task_id join note; donor-json stays deferred); materialize via
      `canon skills install` (lock version bump, no generatedAt).

## 3. Verification

- [x] 3.1 `cargo test --workspace` green offline.
- [x] 3.2 `canon selftest` green including the new fixture;
      `openspec validate s30-plan-dialect-superpowers` passes.
