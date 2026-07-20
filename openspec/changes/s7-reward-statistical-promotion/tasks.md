## 1. Reward function registry

- [x] 1.1 Define the `RewardFn: Role -> VerdictEvent -> f64` registry +
      the `dev` role's default weighted-composite entry (pr-merged 0.4 +
      ci-pass 0.3 + no-rollback 0.3; rollback/ci-fail floor 0.1;
      human-approval shortcut 1.0), ported from `computeDevReward`'s
      formula shape. (`src/reward.rs::{RewardFn, RewardRegistry,
      compute_dev_reward, DevRewardSignals, dev_reward_fn}` — the pure
      weighted-composite formula operates on a `DevRewardSignals`
      struct with full fidelity to `computeDevReward`'s weights/floors/
      shortcut precedence, independently unit-tested on synthetic
      signal combinations; `dev_signals_from_verdicts` is the
      documented VerdictRow -> DevRewardSignals adapter closing the
      granularity gap between S4's collapsed `VerdictRow` shape and
      the donor monorepo's richer per-event signal set — see `reward.rs` module doc.)
- [x] 1.2 Draft default reward-weight entries for `content`/`design`/
      `review`/`planning`/`test` roles from S4's review→verdict table.
      (`src/reward.rs::default_reward_fn` + `RewardRegistry::builtin`'s
      per-role doc comments citing which review→verdict table row(s)
      apply to each — PROVISIONAL, drafted-not-fixed per design Open
      Questions: all five currently share the DEFAULT reward
      convention, `Corrective` read as a `review`-role success per
      design D1's own worked example; a future change may replace any
      of these with a role-specific weighted composite once real
      cross-role data exists.)
- [x] 1.3 Implement `mark_trajectory_verdict(trajectory_id, verdict,
      reward)` against S6's `Trajectory` store — `pending | success |
      failure | rolled-back` verdict enum, reward clamped to `[0, 1]`.
      (`src/mark_verdict.rs::mark_trajectory_verdict` +
      `src/verdict_outcome.rs::{VerdictOutcome, TrajectoryVerdict}` +
      `src/store/mod.rs::TrajectoryStore::{find_by_id, mark_verdict}` +
      `src/store/parquet_trajectory.rs` impl. First real caller of S6's
      write-back surface besides tests. Rejects `Pending` outright
      (never re-opens a resolved trajectory) and an unknown
      `trajectory_id` loudly (`LearnError::UnknownTrajectoryId`, never
      the donor monorepo's own silent no-op).
      Parquet wire format stays backward-compatible: an S6-era
      trajectory row with no `outcome`/`reward` keys deserializes as
      `Pending` (`#[serde(default)]` on `TrajectoryWire`), can then be
      marked, and `rebuild_namespace` never rewrites trajectory bytes
      (locking test: `store/parquet_trajectory.rs::tests::
      s6_era_row_reads_pending_can_be_marked_and_rebuild_never_touches_
      it_afterward`). Verified against SYNTHETIC `VerdictRow`s/fixtures
      only — no live webhook/production `canon ingest` driver exists
      yet (Migration Step 1).

## 2. Statistical promotion — CRN gate

- [x] 2.1 Port `matts.ts`'s pure statistics core to Rust: `seed_panels`,
      `decompose_band_variance` (df-aware F(1,df) table + `MIN_DF_
      RESIDUAL` floor), `paired_contrast` (df-aware two-sided t(df) table
      + `MIN_PANELS_FOR_SIGNIFICANCE` floor), `should_stop_scaling`.
      (`src/promotion/crn.rs::{seed_panels, decompose_band_variance,
      paired_contrast, should_stop_scaling}` — the FIXED versions
      ported verbatim per the vendor audit's F1/F2/F3 fix notes:
      df-aware `F_CRIT_1_TABLE`/`f_critical_1` and
      `T_CRIT_2SIDED_TABLE`/`t_critical_2sided` critical-value tables
      (not a fixed threshold), sample — not population — paired
      variance in `paired_contrast`, and the `MIN_DF_RESIDUAL`/
      `MIN_PANELS_FOR_SIGNIFICANCE` floors. Locked by
      `f_and_t_tables_satisfy_the_f_equals_t_squared_identity`
      (F(1,df) = t(df)² cross-check) plus 15 further unit tests
      covering ragged-input rejection, graceful degradation at
      `n_configs`/`n_panels` < 2, and the significance floors'
      boundaries.)
- [x] 2.2 Implement `corroborated_effect(batch) -> bool` as the promotion
      gate entry point for CRN-capable roles.
      (`src/promotion/crn.rs::{ContrastBatch, corroborated_effect}` —
      omnibus `config_effect_real` AND, for 2-config batches, an
      independently-agreeing `paired_contrast`; `CrnPromotionGate`
      (impl `PromotionGate`) wraps it as the promotion decision for
      CRN-capable roles, parsing CRN config/panel identity out of
      `Trajectory::tags` via the `crn:config=`/`crn:panel=` prefix
      convention `CRN_CONFIG_TAG_PREFIX`/`CRN_PANEL_TAG_PREFIX`
      document, excluding still-`Pending` trajectories from the
      decomposition.)
- [x] 2.3 Golden fixture: the documented MaTTS counter-example
      (2-config k=2 batch, per-panel diffs `[0.1, 0.3]`) must not read
      `configEffectReal: true`.
      (`src/promotion/crn.rs::tests::
      decompose_band_variance_the_matts_counter_example_never_reads_
      config_effect_real` (pure-core level) and `::tests::
      crn_gate_golden_fixture_matts_counter_example_rejects`
      (`CrnPromotionGate` level, fixture `Trajectory` rows through the
      real trait impl) — both assert `df_residual == 1 <
      MIN_DF_RESIDUAL == 2` forces `config_effect_real`/promotion
      false regardless of the observed gap. Plus `::tests::
      crn_gate_rejects_a_non_significant_contrast` (CRN reject: k=5,
      sufficient df=4 but F-ratio below `f_critical_1(4)`) and
      `::tests::crn_gate_accepts_a_clearly_significant_contrast` (CRN
      accept: k=5, F=3240 >> `f_critical_1(4)`=7.709, paired contrast
      significant). All green: `cargo test -p canon-learn` (107
      passed), `cargo clippy -p canon-learn --all-targets -- -D
      warnings` clean, commit `7166207014c701f2865472217aadc7252feb0837`.)

## 3. Statistical promotion — occurrence gate

- [x] 3.1 Implement `OccurrencePromotionGate`: `n_min` corroborating
      `success`-verdict trajectories for the same `regime_key` AND zero
      `failure`-verdict trajectories for that `regime_key` inside a
      configurable observation window; a contradicting failure resets the
      counter.
      (`src/promotion/occurrence.rs::OccurrencePromotionGate` (impl
      `PromotionGate`) — walks `samples` chronologically inside the
      trailing `window` (measured from the evaluation instant),
      incrementing a streak on `Success` and RESETTING it to `0` on
      `Failure`/`RolledBack` (never averaged away, per design D3);
      promotes when the final streak `>= n_min`. Samples strictly
      outside the window, or from a different `regime_key`, are
      excluded entirely (defense-in-depth even though callers are
      expected to pre-scope via `TrajectoryStore::query_by_regime_key`);
      `Pending` samples are skipped (neither corroborate nor
      contradict). `RolledBack` resets the streak exactly like
      `Failure` — a deliberate widening past the design text's literal
      "failure-verdict" wording, documented in the module doc:
      `VerdictOutcome::default_reward` treats `RolledBack` as a
      STRONGER negative signal than `Failure` (`0.1` vs `0.3`), so
      excluding it from contradiction detection would let a strategy
      promote despite an actual recorded rollback in its own regime.
      9 unit tests incl. `n_min_successes_with_zero_contradictions_
      promotes`, `a_contradicting_failure_resets_the_count_even_at_n_
      min_minus_one`, `successes_after_a_reset_can_still_reach_n_min`,
      `samples_outside_the_window_are_excluded_entirely`,
      `a_rolled_back_outcome_also_resets_the_count_not_just_failure`,
      `a_trajectory_from_a_different_regime_is_ignored`.)
- [x] 3.2 Add `promotion.<role>.mode: crn | occurrence` + `n_min`/window
      fields to the `policy.yaml` schema.
      (**Reconciled to `canon.yaml`'s `learn:` section instead of
      `policy.yaml`** — `src/config.rs::{PromotionMode,
      PromotionRoleConfig, LearnConfig::promotion,
      LearnConfig::promotion_config_for}`. The design doc's own D3 text
      says `policy.yaml`, but that would make `canon-learn` depend on
      `canon-policy`/`canon-gate` just to read a setting only
      `canon-learn` itself consumes — no `canon-gate`/`canon-policy`
      reader for a promotion-gate mode exists anywhere, and
      `LearnConfig`'s own established convention ("every crate parses
      only the top-level key(s) it owns", `config.rs` module doc) is
      exactly what this avoids. Full rationale in `config.rs`'s "S7
      task 3.2 reconciliation" doc block. Parses `learn.promotion.
      <role>.{mode, n_min, window_days}` (n_min/window_days optional,
      default `5`/`30`) + `learn.demotion.{hard_delete,
      strategies_root}` (task 4.1's policy, same reconciliation). A
      role with no explicit entry resolves to
      `PromotionRoleConfig::default_occurrence()` — the conservative
      `n_min: 5, window_days: 30` defaults the design doc's risk
      section calls for — never a missing-config error. 7 new
      `config::tests` covering explicit parsing, the malformed-role-slug
      reject path, and the default-fallback.)

## 4. Demotion

- [x] 4.1 Implement `demote_strategy(strategy_id, contradicting_
      trajectory_id)`: writes a demotion evidence record; soft-flags the
      git-tier file (`status: demoted` front-matter + reason) by default,
      hard-delete configurable per `canon.yaml` policy.
      (**WIDENED S7Core's frozen 2-arg stub** per a ReviewS7Core
      finding coordinated with Main: the original `demote_strategy
      (strategy_id, contradicting_trajectory_id) -> Result<
      DemotionRecord, LearnError>` had no way to reach the persistence
      context its own doc comment already described as its job. New
      signature: `demote_strategy(strategy_store: &dyn StrategyStore,
      strategy_id, contradicting_trajectory_id, git_tier_root: &Path,
      policy: DemotionPolicy) -> Result<DemotionRecord, LearnError>`
      (`src/promotion/demote.rs` — full rationale in its module doc).
      Zero real callers existed for the old stub (S7Core's own doc:
      "nothing in this crate calls it"), so this is a strict extension,
      never a breaking change to a real caller; `promotion/mod.rs`'s
      placeholder test is updated to the new shape.
      `StrategyStore` gained `find_by_id`/`mark_demoted`
      (`src/store/mod.rs`, `src/store/parquet_strategy.rs` impl —
      mirrors `TrajectoryStore::find_by_id`/`mark_verdict`'s exact
      precedent from task 1.3). Two independent effects: (1) **durable
      evidence** — `StrategyItem` gained a `demotion:
      Option<DemotionEvidence>` field (`src/strategy.rs`,
      `#[serde(default)]` — a pre-S7 row with no `demotion` key
      deserializes as `None`, same backward-compat contract
      `Trajectory::verdict_record` uses); `DemotionEvidence` is
      S1-envelope-shaped (`#[serde(flatten)] envelope: canon_model::
      envelope::Envelope`, `kind: RecordKind::EvidenceRecord` — the
      closest existing closed record kind, canon-model has no
      dedicated `Demotion` variant and adding one is out of this
      crate's insulated surface) + `contradicting_trajectory_id` +
      `reason`, persisted via `StrategyStore::mark_demoted` — durable
      the moment the parquet write returns. (2) **git-tier file
      update** — soft-flags (default) or hard-deletes
      `<git_tier_root>/<role>/<strategy_id>.md`, ONLY if that file
      already exists (`canon learn promote`, the writer that would
      have created it, is still unbuilt — a strategy demoted before
      ever being promoted has nothing to soft-flag, not an error).
      Soft-flag merges `status: demoted` + `reason` into the file's
      EXISTING YAML front matter via `serde_yaml::Value`, leaving every
      other front-matter key and the whole body byte-unchanged
      (append-only, §7 — a NEW commit demotes, nothing force-
      rewritten). Fails loud (`LearnError::UnknownStrategyId`) on an
      unmatched `strategy_id`, mirroring `mark_trajectory_verdict`'s
      "never a silent no-op" discipline. 8 unit tests in
      `promotion/demote.rs::tests` incl.
      `demote_strategy_persists_demotion_evidence_durably`,
      `a_strategy_never_promoted_to_the_git_tier_has_nothing_to_soft_
      flag`, `default_policy_soft_flags_an_existing_git_tier_file_
      leaving_other_front_matter_intact`, `hard_delete_policy_removes_
      the_git_tier_file`.)

## 5. Webhook receiver

- [x] 5.1 Implement the PR/CI webhook receiver: normalize GitHub
      `pull_request.merged`/`workflow_run.conclusion` payloads into S4's
      verdict-event shape.
      (`src/webhook.rs::{PullRequestMergedPayload, PullRequestPayload,
      WorkflowRunPayload, WorkflowRunInner, normalize_pull_request_
      merged, normalize_workflow_run}` — narrow serde structs (only the
      fields read), normalizing into the same `{role: dev, polarity,
      becomes}` shape S4's `derive_verdict` table already assigns a
      fixed `dev` role to for `PrMergeNoRevert`/`CiFailOrPrRevert`; a
      revert-shaped merge body (git's `"This reverts commit <sha>."`
      convention) normalizes to `WebhookLogEvent::Reverted` of the
      ORIGINAL sha, never the revert commit's own sha. A non-mapped
      action/conclusion returns `None` explicitly, mirroring
      `ArtifactEventKind::NonVerdict` — never a guessed verdict. This
      module never duplicates S4's ingest adapter registry/
      `ArtifactEventKind`/`derive_verdict` (design D5); it only
      reproduces the two table rows already hardcoded to `dev`.)
- [x] 5.2 Wire the receiver → reward function → `mark_trajectory_verdict`
      end-to-end.
      (`src/webhook.rs::compute_and_mark` — the shared tail every
      reward-writing path calls: builds the matching `VerdictRow`,
      calls `RewardRegistry::compute(&dev_role(), &[row])`, then
      `mark_trajectory_verdict`. Wired from `handle_workflow_run`
      (`"failure"` conclusion, immediate) and
      `evaluate_no_rollback_timer` (`Reverted`/`Satisfied` — task 5.5).
      End-to-end tests persist through a real `ParquetTrajectoryStore`
      and assert the written `TrajectoryVerdict`: `a_merged_pr_payload_
      end_to_end_marks_the_joined_trajectory_success_once_the_window_
      elapses` (Success, reward 1.0), `a_workflow_run_failure_marks_
      failure_immediately` (Failure, reward 0.1), `a_revert_inside_the_
      window_marks_failure_instead_of_success` (Failure, reward 0.1).)
- [x] 5.3 Gate the receiver behind `canon.yaml`'s `webhook.enabled`
      (local-only mode per §9 must work with zero network).
      (`src/webhook.rs::WebhookConfig::{enabled, from_manifest}` —
      `enabled: false` by default (`WebhookConfig::default`), mirrors
      `LearnConfig::from_manifest`'s "parse only the key(s) this module
      owns, missing section is not an error" discipline. Every pipeline
      entry point (`handle_pull_request_merged`, `handle_workflow_run`,
      `evaluate_no_rollback_timer`) checks `config.enabled` FIRST and
      returns `WebhookOutcome::Disabled` without touching a store or
      candidate slice — local-only mode works with zero network by
      construction. Locked by
      `disabled_config_is_a_clean_no_op_across_every_entry_point`,
      which asserts the store is untouched (still `Pending`) after
      calling all three entry points with a disabled config.)
- [x] 5.4 Implement the SHA→trajectoryId join via S1's join spine (`sha`/
      `pr` ↔ trajectory key): resolve the webhook payload's commit SHA to
      the trajectory that produced it BEFORE calling
      `mark_trajectory_verdict` — the donor monorepo's own webhook translator never
      built this join and improvised (borrowed the SHA itself as a
      trajectory-id slot, which never matches); this task closes that gap
      via S1's typed key rather than repeating it.
      (`src/webhook.rs::{sha_tag, resolve_trajectory_by_sha}` — resolves
      through `canon_model::ids::Sha` (S1's typed join-spine key, whose
      own `JOINS` doc reads "reward signals ↔ trajectory"), matched
      against a `sha:<40-hex>` tag on `Trajectory::tags` (this crate's
      frozen, free-form side channel — the same pattern `promotion::crn`
      uses for CRN panel/config identity) via `Sha::parse`, never a raw
      string compare. Every entry point calls this BEFORE any
      `mark_trajectory_verdict`; an unmatched SHA returns
      `WebhookOutcome::UnjoinedSha` explicitly, never mis-joined onto an
      unrelated trajectory or treated as if the SHA string were itself a
      `TrajectoryId`. Locked by `resolves_a_tagged_trajectory`,
      `a_sha_with_no_matching_trajectory_resolves_to_none_explicitly`,
      `never_mis_joins_a_sha_as_if_it_were_a_trajectory_id`,
      `an_unjoined_sha_is_reported_explicitly_never_mis_joined`,
      `a_workflow_run_with_unjoined_sha_is_reported_explicitly`.)
- [x] 5.5 Implement the no-rollback timer: a scheduled/deferred check that,
      `no_rollback.window` after a `pull_request.merged` event with no
      subsequent revert/rollback event, marks the `no-rollback` reward
      factor satisfied (task 1.1's weighted composite) — no production
      implementation of this timer exists anywhere in the donor monorepo today
      (`dev-reward-backfill.ts` defers it as a future webhook-receiver
      concern and never built it); this task is the first real
      implementation, not a port.
      (`src/webhook.rs::{NoRollbackStatus, check_no_rollback,
      evaluate_no_rollback_timer, WebhookConfig::no_rollback_window}` —
      `check_no_rollback` is a PURE function of `(sha, merged_at, window,
      event_log, as_of)`, never `Utc::now()` internally, so "wait
      `no_rollback.window` with no revert" is deterministically testable
      offline (mirrors `promotion::crn`'s own pure-statistics-core
      split). `handle_pull_request_merged` deliberately does NOT mark a
      verdict at merge time — only normalizes+joins — since the
      no-rollback factor cannot be honestly claimed until the window
      elapses; `evaluate_no_rollback_timer` is the actual scheduled/
      deferred check a future caller re-invokes with a later `as_of`,
      marking `Success` once `Satisfied` or `Failure` immediately once a
      revert lands inside the window (`Reverted` — mirrors
      `compute_dev_reward`'s own rollback-overrides-everything rule).
      `window_hours` defaults to 24h, configurable via `canon.yaml`
      `no_rollback.window_hours`, rejecting a non-positive value loud.
      Locked by `pending_before_the_window_elapses_with_no_revert`,
      `satisfied_once_the_window_fully_elapses_with_no_revert`,
      `a_revert_inside_the_window_prevents_satisfaction_even_before_the_
      window_elapses`, `a_revert_inside_the_window_still_reads_reverted_
      after_the_window_elapses`, `a_revert_of_a_different_sha_does_not_
      affect_this_sha`, `a_revert_landing_after_the_window_does_not_
      retroactively_flip_satisfied`, `awaiting_the_window_marks_nothing`.
      Deferred honestly (Migration Step 1 / task group 5 scope): NO live
      HTTP webhook endpoint and NO live GitHub delivery ship in this
      change — every test above drives the pipeline with synthetic
      payloads/event logs; `evaluate_no_rollback_timer` is a pure
      function a future scheduler/cron caller invokes with real
      `Utc::now()`, not a live daemon this change starts. All green:
      `cargo test -p canon-learn --lib` (152 passed, 32 in
      `webhook::tests`), `cargo clippy -p canon-learn --all-targets --
      -D warnings` clean.)

## 6. Fixtures + selftest

- [x] 6.1 Build golden fixture verdict streams producing expected
      promotions.
      (Occurrence half: `tests/occurrence_promotion.rs::
      a_stream_of_n_min_successes_with_zero_contradictions_promotes` —
      5 (conservative-default `n_min`) `Success`-verdict trajectories
      for one `regime_key`, zero contradictions, resolved through
      `LearnConfig::from_manifest("")` -> `promotion_config_for` ->
      `OccurrencePromotionGate::from_config` end-to-end, not a
      hand-built gate bypassing the config path. CRN half already
      golden-fixture-locked by task 2.3's `crn_gate_accepts_a_clearly_
      significant_contrast`.)
- [x] 6.2 Build golden fixture verdict streams producing expected
      rejections (below `n_min` / non-significant CRN contrast).
      (Occurrence half: `tests/occurrence_promotion.rs::
      a_stream_below_n_min_rejects` (4 successes, one short of the
      default `n_min: 5`) and `::a_stream_with_a_contradicting_failure_
      inside_the_window_rejects` (5 successes but a trailing `Failure`
      resets the streak to `0` — proves "resets the counter" is not
      just a unit-level claim but holds through a realistic verdict
      stream). CRN half already golden-fixture-locked by task 2.3's
      `crn_gate_rejects_a_non_significant_contrast` and
      `crn_gate_golden_fixture_matts_counter_example_rejects`.)
- [x] 6.3 Build a fixture where a contradicting trajectory arrives after
      promotion — assert `demote_strategy` fires and the git-tier file is
      soft-flagged.
      (`tests/occurrence_promotion.rs::a_contradicting_trajectory_
      after_promotion_demotes_the_strategy_and_soft_flags_its_git_tier_
      file` — builds real `n_min`-success eligibility through
      `OccurrencePromotionGate::evaluate` (proving `Promote` first, not
      assumed), appends a real `StrategyItem` + a real git-tier `.md`
      file (standing in for `canon learn promote`'s still-unbuilt
      output — the exact shape it would produce), appends a
      contradicting `Failure` trajectory, re-evaluates the gate
      (confirms it flips to `Reject`), calls `demote_strategy` against
      real `ParquetStrategyStore`/`ParquetTrajectoryStore` backends,
      then asserts BOTH effects: the `StrategyStore` row now carries
      `DemotionEvidence` (durable evidence) AND the git-tier file's
      front matter contains `status: demoted` + a `reason`, with its
      unrelated `title`/body content proven byte-unchanged.)

## 7. Companion skill

- [x] 7.1 Author the `canon-reward` companion skill under `canon/skills/`
      — reward function registry, promotion gate modes, reading a
      demotion record.
      (`canon/skills/canon-reward/SKILL.md` — reward function registry
      (`RewardRegistry`/`compute_for_trajectory`, the `dev` weighted
      formula vs the shared DEFAULT convention), `mark_trajectory_
      verdict`'s write-back contract, choosing a promotion gate mode
      via `canon.yaml` `learn.promotion.<role>` (with the `occurrence`
      vs `crn` rule summaries), and reading a demotion record (both the
      durable `DemotionEvidence` on a `StrategyItem` row and the
      git-tier `status: demoted` front-matter shape) — plus an explicit
      "what this skill does NOT cover" boundary (the webhook HTTP
      listener, `canon learn promote`, CEL/`canon-policy`). Materialized
      via `canon skills install` (`.claude/skills/canon-reward/
      SKILL.md`, `.codex/skills/canon-reward.md`, install lock bumped
      — `canon/skills/.install-lock.json`).)
