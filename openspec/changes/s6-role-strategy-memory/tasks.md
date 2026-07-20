## 1. `canon-learn` crate scaffold

- [x] 1.1 Scaffold `crates/canon-learn` depending on `canon-model` (S1
      `regime_key`/`RegimeKey`/`RoleId` join-spine grammar — the frozen
      input contract) and `canon-ingest` (S4's frozen
      `verdict::VerdictRow`). **Deviates from the literal plan**:
      `canon-model::records::Trajectory`/`StrategyItem` are envelope-
      level join-key carriers too minimal for the reasoning-bank shape
      (no title/description, singular `source_task_id`, no verdict
      payload) — `canon-learn` defines its OWN richer `Trajectory`/
      `StrategyItem` types instead (`src/trajectory.rs`,
      `src/strategy.rs`), reusing only the join-key newtypes. Does
      NOT depend on `canon-store` — see task 2.1's note; the raw tier
      is a bespoke operator-local parquet store, not a `canon-store`
      `Tier` adapter.
- [x] 1.2 Define the open `Role` registry type + built-in role set
      (`planning|design|dev|test|review|content|sim`) + `canon.yaml`
      registration for additional roles; reject unregistered roles at
      write time. (`src/role.rs::RoleRegistry`, `src/config.rs::
      LearnConfig` for the `canon.yaml` `learn:` section,
      `src/write.rs::store_trajectory` enforces the write-time
      rejection.)

## 2. Trajectory + strategy stores

- [ ] 2.1 ~~Implement the raw `Trajectory` cold store on the `lancedb`
      Rust crate (embedding + cosine-similarity search),
      role-partitioned.~~ **Superseded by the OQ2 resolution this
      change actually shipped under: parquet-first, no LanceDB/ANN
      dependency** (design.md's own risk section already flags why:
      the donor monorepo's donor LanceDB pattern store has ZERO production callers
      — every reasoning-bank write across the whole harness is
      process-local and lost on restart; parquet starts ahead because
      it has no "wire a real caller" step to skip). Shipped instead:
      `src/store::ParquetTrajectoryStore`, Hive-nested by the full
      `regime_key` tuple (`<role>/<repo>/<area>/<hash>/<id>.parquet`),
      behind the `TrajectoryStore` trait — a future
      `LanceDbTrajectoryStore` (or any vector-backed impl) is a pure
      additive impl of that trait, the documented seam this task's
      "role-partitioned" intent still holds under.
- [x] 2.2 Implement `store_trajectory` (write-only surface S7 calls
      into; this change ships the write path, S7 owns the reward
      computation calling it) — `src/write.rs::store_trajectory`,
      role-registry-gated. **`mark_trajectory_verdict`'s write-back
      stub is NOT implemented** — reward computation is S7's own scope
      (design.md Non-Goals: "Reward computation / statistical
      promotion gating — S7"); this change has nothing for S7 to
      backfill onto without S7's own reward-signal shape existing
      first, so no stub was fabricated.
- [x] 2.3 Implement the distilled `StrategyItem` store (title/
      description/content + `source_trajectory_ids` provenance),
      non-destructive: `delete_for_regime_key` + `rebuild_namespace`
      never touch raw trajectories (`src/store::ParquetStrategyStore`,
      `src/rebuild.rs::rebuild_namespace`). Scoped by the FULL
      `regime_key` tuple, not role-alone (`rebuild_namespace(regime_
      key)` — matches the S6 assignment's OQ2 contract text over this
      task's own "for a role" shorthand; a role-only rebuild would
      conflate every repo/area sharing a role into one delete-rebuild
      unit, which the join-spine's own `<role>/<repo>/<area>/<hash>`
      grammar treats as four independent segments).
- [ ] 2.4 ~~Implement `search_similar_trajectories` / `search_similar_
      strategies`, both scoped by role (never cross-role by
      default).~~ **NOT implemented** — "similar" implies embedding/
      vector similarity, which the OQ2 parquet-first pivot (task 2.1's
      note) explicitly defers. Shipped instead: exact-`regime_key`-
      match retrieval (`TrajectoryStore::query_by_regime_key`,
      `StrategyStore::query_by_regime_key`, `src/retrieve.rs::
      retrieve`) — role-scoped by construction (a `regime_key`'s first
      segment IS the role; a different role is a different key,
      never a filter that could leak across).

## 3. Canonical regime key

- [x] 3.1 `regime_key(role, repo, area, hash) -> String` as the single
      canonical serialization — already landed as the S1 foundation
      (`canon_model::ids::regime_key`); this change's job was to
      REUSE it, never redefine it, at every write/read path in
      `canon-learn` (`Trajectory::new`'s role-agreement check,
      `store::path::namespace_dir`, `distill`, `rebuild_namespace`,
      `retrieve` all key off the caller-supplied `RegimeKey`, never a
      second derivation).
- [x] 3.2 Fixture test: a same-regime write is always the top hit for
      a same-regime read; a different-role regime never collides on
      the retrieval key. (`store::parquet_trajectory::tests::a_
      different_regime_key_never_sees_another_regimes_trajectories`,
      `tests/fixture_round_trip.rs`'s cross-namespace assertions.)

## 4. Git-tier promotion

- [x] 4.1 `canon learn promote <strategy_id>` — promote a distilled
      `StrategyItem` (by id) from the operator-local parquet warm tier
      into the git-tracked `canon/strategies/<role>/<id>.md` tier.
      — ✅ `crates/canon-learn/src/promotion/promote.rs`
      (`promote_strategy`, `plan_promotion`, `render_strategy_file`) +
      `canon learn promote [--dry-run]`
      (`crates/canon-cli/src/learn.rs::run_promote`, wired in `main.rs`).
      Tests `crates/canon-cli/tests/learn_promote.rs::{promote_materializes_a_seeded_strategy_as_a_git_tier_file,
      an_unknown_strategy_id_fails_loud,
      dry_run_previews_without_writing_the_git_tier_file}`.
- [x] 4.2 Content-length + literal-path-pattern advisory lint on
      promote — non-blocking (printed, never fails the promote).
      — ✅ `promote.rs::lint_strategy` (`CONTENT_ADVISORY_CEILING` + a
      literal machine-specific absolute-path check); test
      `promote.rs::tests::lint_flags_a_literal_absolute_path_and_an_oversized_content_without_blocking`.

## 5. Fixtures + selftest

- [x] 5.1 Fixture corpus exercising the store→distill→rebuild→search
      round-trip (`tests/fixture_round_trip.rs::store_distill_
      rebuild_search_round_trip_over_a_fixture_corpus`, using
      SYNTHETIC `VerdictRow` struct literals — at the time, the
      production `canon ingest` artifact-driver that would feed real
      `VerdictRow`s into this crate end-to-end did not exist yet
      (deferred residual); no end-to-end real-ingest proof was claimed
      here).
      **Update (2026-07-11, `s14-artifact-ingest-cli`):** that driver
      has now shipped — `crates/canon-cli/src/artifact_ingest.rs` feeds
      real `VerdictRow`s (derived from real `ArtifactAdapter` output,
      never synthetic) through this crate's own `store_trajectory` +
      `rebuild_namespace`, proven end-to-end by
      `crates/canon-cli/tests/artifact_ingest.rs::
      ingest_artifacts_drives_both_source_shapes_persists_trajectories_and_feeds_real_marts`
      (asserts a real trajectory round-trips through this crate's own
      `ParquetTrajectoryStore` and that `canon-report`'s
      `mart_role_memory`/`mart_flywheel_funnel` render non-empty from
      it). This fixture (synthetic data) is retained as-is — it exercises
      this crate's store/distill/rebuild/search logic in isolation, which
      the S14 end-to-end test does not re-cover.
- [x] 5.2 Fixture asserting a promoted strategy appears as a git-tier
      file.
      — ✅ `crates/canon-cli/tests/learn_promote.rs::promote_materializes_a_seeded_strategy_as_a_git_tier_file`
      (seeds a strategy, runs the real `canon learn promote`, asserts the
      `canon/strategies/<role>/<id>.md` git-tier file exists with the
      rendered front-matter + body).

## 6. Donor cutover plan

- [ ] 6.1 Field-by-field migration-mapping document — **NOT done by
      this wave**, out of this delegation's assigned scope (the S6
      assignment's "Change" list is items 1-5 only; this task is a
      separate deliverable for a future wave, PLAN ONLY per design.md
      Non-Goals, no donor-repo edits either way).

## 7. Companion skill

- [x] 7.1 `canon-learn` companion skill under `canon/skills/`.
      — ✅ `canon/skills/canon-learn/SKILL.md` (documents strategy-memory
      rebuild via `canon ingest artifacts` + `canon learn promote <strategy_id>
      [--dry-run]`, the two-tier trajectory/strategy stores, role
      namespacing, and the ingest→distill→promote→retrieve flywheel);
      materialized via `canon skills install` (`.claude/skills/canon-learn/`
      + `.codex/skills/canon-learn.md` + `.install-lock.json` bump).
