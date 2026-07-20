# Tasks — s22 query tier degradation

Sequencing follows design.md: **P1 (relocate the shared builder) lands
before P2 (kind-scoped entry point), which lands before P3 (wire
`query.rs` onto it).** P4 (tests) covers P1-P3 together; P5 closes.
`canon tier age` (`tier.rs`) is untouched throughout — no task touches it.

## 1. Relocate `build_lenient_tiers` into `tiers.rs`, shared (P1)

- [x] 1.1 `canon-cli/src/tiers.rs`: move `LenientTiers`
      (`plans.rs:551`) and `build_lenient_tiers` (`plans.rs:575-607`)
      into this module as `pub(crate)`, generalizing the return type from
      `Result<LenientTiers, PlansError>` to `Result<LenientTiers,
      TierCliError>` — body unchanged (every fallible call inside
      already returns `StoreError`, which `TierCliError::Store(#[from]
      StoreError)` already converts).
- [x] 1.2 `canon-cli/src/plans.rs`: delete the now-relocated
      `LenientTiers`/`build_lenient_tiers`; the `build_lenient_tiers`
      call site (`plans.rs:265`) calls `tiers::build_lenient_tiers`
      instead, converting the resulting `TierCliError` into `PlansError`
      (a `From<TierCliError> for PlansError` impl, or an explicit
      `.map_err`) so `ingest plans`'s own error text/exit code is
      byte-identical to before this task.
- [x] 1.3 Test: `ingest plans`'s existing tier-degradation tests
      (unset `CANON_PG_DSN`, malformed `tiers.pg.schema`) re-run
      unmodified and stay green — proves the relocation changed WHERE
      the logic lives, not WHAT it does for its existing caller.

## 2. Kind-scoped lenient attachment (P2 — after P1)

- [x] 2.1 `canon-cli/src/tiers.rs`: add
      `fn tiers_needed_for(policy: &TierPolicy, kind: RecordKind) ->
      Vec<TierKind>` — `policy.tier_for(kind)?` always included, plus
      `policy.aging.get(&kind).map(|r| r.to)` when present and distinct
      from the routed tier (mirrors `TierRegistry::tiers_for_read`'s own
      two-fact combination, computed independently in `canon-cli` since
      `tier_for`/`aging`/`routing` are already `pub` — design.md D2).
      Implemented as `Result<Vec<TierKind>, StoreError>` (not a bare
      `Vec`) — `tier_for` itself is fallible (`UnroutedKind`), so the
      `?` the task body itself describes requires a `Result` return.
- [x] 2.2 `canon-cli/src/tiers.rs`: add
      `pub(crate) fn build_lenient_tiers_for_kind(canon_yaml_path: &Path,
      kind: RecordKind) -> Result<LoadedTiers, TierCliError>` — parses
      `canon.yaml` (reusing `build_tiers`'s existing parse step), attaches
      `git` unconditionally when `tiers.git` is configured (never scoped —
      design.md D2/R1), and attempts `pg`/`r2` ONLY when
      `tiers_needed_for` includes that `TierKind`, reusing the SAME
      per-tier degrade-`TierUnavailable`-to-`None`/propagate-else helpers
      `build_lenient_tiers` (task 1.1) uses — factor them into small
      `attach_pg`/`attach_r2` functions both builders call, so there is
      exactly one degrade/propagate decision in the module (design.md D4).
- [x] 2.3 Tests: for a `canon.yaml` declaring `git`+`pg`+`r2` with
      `CANON_PG_DSN` unset and no `CANON_R2_BUCKET` reachable —
      `tiers_needed_for(git-routed kind)` returns `[Git]` only (never
      attempts `pg`/`r2` at all, asserted by NOT requiring either env var
      to be set for the call to succeed); `tiers_needed_for(pg-routed,
      non-aged kind)` returns `[Pg]`; `tiers_needed_for(pg-routed,
      r2-aged kind, e.g. `handoff`/`event` per `canon.yaml:38,43,48-51`)`
      returns `[Pg, R2]` (both, per design.md R2). See
      `tiers.rs::lenient_tier_tests`.

## 3. Wire `canon query` onto the scoped-lenient builder (P3 — after P2)

- [x] 3.1 `canon-cli/src/query.rs::run`: replace the
      `tiers::build_tiers(&canon_yaml_path)?` call (line 273) with
      `tiers::build_lenient_tiers_for_kind(&canon_yaml_path, kind)?` —
      every downstream line (`TierRegistry::new`, `registry.query`,
      `fold_pg_routed_kind`, `apply_scope`, `rollup_for`) is unchanged.
- [x] 3.2 `canon-cli/src/query.rs::run_with_plugin`: identical
      replacement at line 485; `git_root`/`resolve_and_project`
      (lines 486-504) are unchanged and keep receiving `loaded.git`
      exactly as today (task 2.2's unconditional `git` attachment).
- [x] 3.3 Test: `canon query --kind change` (git-routed) against a
      `canon.yaml` declaring `tiers.pg`/`tiers.r2` with NEITHER
      `CANON_PG_DSN` nor `CANON_R2_BUCKET` set — succeeds, returns the
      expected records, exit 0 (the exact SYNTHESIS-ROUND2 #1 repro,
      now fixed). See `tests/query_tier_degradation.rs::
      git_routed_kind_query_succeeds_when_pg_and_r2_are_both_unreachable`;
      also manually re-run against this repo's own `canon.yaml`
      (task 4.3).
- [x] 3.4 Test: `canon query --kind task` (pg-routed, per
      `canon.yaml:39`) against the SAME `canon.yaml`, `CANON_PG_DSN`
      unset — fails with an error naming `kind: task`/`tier: pg`/the
      "no live DSN" reason (via `StoreError::TierUnavailable`,
      unmodified), non-zero exit — never a silent empty result, never an
      unnamed generic error. See `tests/query_tier_degradation.rs::
      pg_routed_kind_query_fails_naming_pg_and_the_no_live_dsn_reason`.
- [x] 3.5 Test (offline-adapted): `canon query --kind handoff`
      (pg-routed, r2-aged) against a `canon.yaml` with BOTH `pg`/`r2`
      unreachable fails NAMED, never silently — see
      `tests/query_tier_degradation.rs::
      pg_routed_r2_aged_kind_query_fails_named_never_silently`. The
      literal "`pg` live, `r2` down" half of this scenario needs a
      genuinely reachable Postgres — `PgTier::connect` is eager/network
      (`pg_tier.rs`'s own module doc: only `tests/pg_tier_live.rs`,
      gated behind the `live-pg` feature, exercises real Postgres) —
      out of scope for `cargo test -p canon-cli`'s offline suite. The
      "BOTH tiers independently ATTEMPTED, never narrowed to `pg`
      alone" half of design.md R2 is proven at BUILD time instead, via
      `tiers.rs::lenient_tier_tests::
      kind_scoped_build_attempts_both_tiers_for_a_pg_routed_r2_aged_kind`.
- [x] 3.6 Test (offline-adapted): `canon query --kind <k> --plugin
      <id>` where `<k>`'s OWN routing excludes `git` entirely, proving
      `git` attachment for `--plugin`'s manifest resolution is
      unconditional and unaffected by `pg`/`r2` scoping (design.md R1)
      — see `tests/query_tier_degradation.rs::
      plugin_git_attachment_is_unconditional_even_when_the_queried_kinds_own_routing_excludes_git`.
      Uses an `r2`-routed kind (offline-reachable via the
      `CANON_R2_LOCAL_ROOT` debug test seam) rather than the literal
      `pg`-routed/live-DSN example in the task body — same tier-
      agnostic mechanism, no live Postgres needed (see 3.5's note).
- [x] 3.7 Test: re-run `query.rs`'s existing byte-identical `run` vs.
      `run_with_plugin`-with-no-projection tests (task 3.4 of the
      original `plugin-overlay-projection` work) unmodified — confirms
      this change did not reintroduce a divergence between the two
      functions' non-`--plugin` behavior. `cargo test -p canon-cli
      --test query --test query_plugin` reruns green, unmodified.

## 4. Closure

- [ ] 4.1 `cargo build --workspace` + `cargo clippy --workspace
      --all-targets -- -D warnings` + `cargo test --workspace
      --no-fail-fast` (bare, no pipe masking) all green. NOT run here
      (parallel-crate assignment: scoped to `cargo {build,test,clippy}
      -p canon-cli` only, all green — see report; the parent runs the
      authoritative whole-workspace gate at the end).
- [ ] 4.2 `bunx openspec validate --strict s22-query-tier-degradation`
      green. Not run here — parent's closure step.
- [x] 4.3 Manual re-run of the SYNTHESIS-ROUND2 #1 repro against the
      rebuilt binary, from this repo's own root: `canon query --kind
      change --repo .` (no `CANON_PG_DSN`/`CANON_R2_BUCKET` set) now
      succeeds; `canon query --kind task --repo .` still fails, but
      named (`tier: pg`, "no live DSN"), not a generic panic/hard crash.
      Confirmed: 28 `change` records returned; `task` fails with
      `tier Pg is not configured/attached (tiers.pg not attached (no
      live DSN))`.
- [ ] 4.4 `canon selftest` all suites green. Not run here (whole-repo
      closure step; `canon-cli`'s own `selftest`/`inventory_selftest`
      unit-test suites — the offline fixture corpus that binary
      exercises — are green as part of `cargo test -p canon-cli`, see
      report).
- [x] 4.5 Structural invariants re-asserted green: `RecordKind::ALL
      .len() == 12` at all three assertion sites; no `canon-gate`/
      `canon-learn` source reference to anything importer/plugin-specific
      (connector-never-authority, unaffected by this change but
      re-checked as a regression guard); `canon gate check` verdicts
      byte-identical before/after (gate.rs never called a tier builder,
      so this is a no-op re-run, not a new test). Verified: `tests/
      gate.rs`'s 18 tests and `tests/plans_ingest.rs::
      canon_gate_check_verdicts_are_byte_identical_with_and_without_a_prior_plan_import`
      all pass unmodified.
