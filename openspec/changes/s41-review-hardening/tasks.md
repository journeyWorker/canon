# s41 review-hardening — tasks

## 1. Trajectory identity and supersession

- [x] 1.1 `trajectory_content_digest` folds an identity version plus the
      rendered `task`/`context`; `trajectory_text` is computed BEFORE the
      duplicate check rather than after it.
- [x] 1.2 A separate derivation key names the logical verdict set, and a
      stored row from this driver sharing it is superseded in place by
      reusing its id. Depends on 1.1.
- [x] 1.3 Only this driver's own tagged rows are superseded; a
      fixture-seeded or foreign row with identical verdicts is never
      overwritten. Depends on 1.2.
- [x] 1.4 A pre-s38-shaped stored trajectory converges onto the new text
      with no store deletion, proven against a real parquet store.
      Depends on 1.2.

## 2. Event identity

- [x] 2.1 One `event_identity` carrying `project_id` for scenario
      variants, with an unknown form unreachable as a real `ProjectId`.
- [x] 2.2 Both the antecedent lookup and `regime_hash` consume that same
      string. Depends on 2.1.

## 3. Deterministic strategy identity

- [x] 3.1 `StrategyId` derived from the distilled row's own content, with
      a length-prefixed preimage so free artifact text cannot alias
      across field boundaries.
- [x] 3.2 `rebuild_namespace` is a fixpoint: a rebuild that re-derives
      the same strategies leaves `retrieved`/`applied` unchanged.
      Depends on 3.1.
- [x] 3.3 A promoted id still resolves after a rebuild, and promotion
      targets the same git-tier path rather than a second file.
      Depends on 3.1.

## 4. Dispatch correctness

- [x] 4.1 `--task` resolves membership from each adapter's parsed task
      set and rejects only after every configured source is searched.
- [x] 4.2 `dispatch end` holds an exclusive lock across read → validate →
      replace, permits only `Running + ended_at: None` → terminal, and
      verifies the manifest's `run_id` matches its filename. A rejected
      end leaves the manifest byte-identical.
- [x] 4.3 `dispatch diff` deserializes manifests as typed `Run`s, records
      scan failures in notes, and still always exits success on a
      successful read.

## 5. Cursor correctness

- [x] 5.1 The Claude adapter is at parse version 2, because s37 changed
      its sidechain parse output; the other three verified unchanged and
      left at 1.

## 6. Ordering and counting

- [x] 6.1 Verdicts sorted by a total data-derived key before rendering.
- [x] 6.2 Scenario titles resolve equal-`at` ties by `(at, schema,
      digest)`.
- [x] 6.3 Each omitted antecedent counted once.
- [x] 6.4 A `Deferred` reason is reachable from its serialized position.

## 7. Release hygiene

- [x] 7.1 Every `package.json` carries the Cargo workspace version,
      asserted by the release-safety checker that CI already runs.
- [x] 7.2 Versions aligned to `0.3.0` across the workspace and all six
      manifests. Depends on 7.1.
- [x] 7.3 Skills re-materialized into `.claude/`/`.codex/`, with the
      install locks updated.

## 8. Verification

- [x] 8.1 `cargo test --workspace` green; generated-output drift clean.
- [x] 8.2 Upgrade convergence and idempotence demonstrated on this repo:
      three consecutive ingests after the one-time migration report
      `persisted 0 / duplicate 22` with a stable file count.
- [x] 8.3 `gate check` clean, `format` 0 violations, `report --check` no
      drift, website builds, release-safety checker passes.
