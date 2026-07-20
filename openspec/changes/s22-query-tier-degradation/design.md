# Design — s22 query tier degradation

## Current state (accurate baseline, verified)

- **`canon-cli/src/tiers.rs::build_tiers`** (lines 78-100) parses
  `canon.yaml`, then attaches EVERY declared tier unconditionally: `git`
  via `GitTier::new` (no I/O, cannot fail), `pg` via `std::env::var(&cfg
  .dsn_env).map_err(|_| StoreError::TierUnavailable { .. })?` then
  `PgTier::connect`, `r2` via `build_r2_tier` — both fallible arms use a
  hard `?`, so an unset `dsn_env`/unreachable bucket fails the WHOLE
  function, before any `TierKind` is known to matter for the caller's
  actual request.
- **`canon-cli/src/query.rs::run`/`run_with_plugin`** (lines 273, 485)
  both call `tiers::build_tiers` directly — the ONLY two call sites in
  the workspace that feed `build_tiers`'s output into `TierRegistry::query`
  (`canon tier age`, the other consumer named in `tiers.rs`'s own module
  doc, feeds it into `TierRegistry::age_all` instead, a genuinely
  different, non-degradable operation — see D3 below).
- **`canon-cli/src/plans.rs::build_lenient_tiers`** (lines 575-607,
  private `fn`, `LenientTiers` type alias at line 551) already implements
  the correct per-tier contract: `git` unconditional; `pg`/`r2` each
  degrade `StoreError::TierUnavailable` to `None`, propagate anything
  else (a malformed `tiers.pg.schema` via `validate_schema_ident`, or a
  genuine `Io`/`Sql`/`Json` failure). Its return type is
  `Result<LenientTiers, PlansError>`, `plans.rs`-local.
- **`canon-store/src/registry.rs::TierRegistry`** (lines 39-99) already
  does the RIGHT thing once it's handed `Option<GitTier>`/`Option<PgTier>`/
  `Option<R2Tier>`: `tiers_for_read(kind)` (lines 63-72) resolves ONLY the
  kind's routed tier plus its `aging.to` destination (if any); `handle()`
  (lines 43-55) turns a `None` for a NEEDED tier into a named
  `StoreError::TierUnavailable { tier, reason }` — `"tiers.pg not attached
  (no live DSN)"` etc. — lazily, per read, never eagerly. This machinery
  is exactly what a scoped, lenient CLI-layer build unlocks; it needs no
  change itself.
- **`TierPolicy::tier_for`/`.routing`/`.aging`** (`canon-store/src/policy.rs`,
  lines 141-210) are all `pub` already — a `canon-cli` caller can compute
  "which `TierKind`(s) does this kind need" without any `canon-store`
  change or without reaching into `TierRegistry`'s own (private)
  `tiers_for_read`.
- **`run_with_plugin` needs `loaded.git` unconditionally**, independent of
  the queried kind's own routing: `git_root`/`resolve_and_project`
  (lines 486-504) resolve the plugin manifest/overlay snapshot off the
  project's git-tracked tree, not off the queried kind's routed tier.
  Because `GitTier` attachment is unconditional-and-free in
  `build_lenient_tiers` already (no I/O, cannot fail), kind-scoping only
  needs to apply to `pg`/`r2` — `git` is never in question.

## Decisions

- **D1 — Reuse `build_lenient_tiers` by relocating it, never
  reimplement it for `query.rs`.** The function moves from `plans.rs`
  (private) to `tiers.rs` (`pub(crate)`), alongside `build_tiers`/
  `build_r2_tier`, which it already depends on
  (`crate::tiers::build_r2_tier`). Its error type generalizes from
  `PlansError` to `TierCliError` — both already have a `Store(#[from]
  StoreError)` variant and `validate_schema_ident`/`PgTier::connect`/
  `build_r2_tier` all return `StoreError`, so the body is unchanged
  byte-for-byte, only the `Result<_, E>` annotation changes.
  `plans.rs::run` calls the relocated function and converts the
  `TierCliError` it gets back into its own `PlansError` (a one-line `?`
  via a small `From<TierCliError> for PlansError` impl, or an explicit
  `.map_err`) — `ingest plans`'s own observable behavior is unchanged;
  only where the shared logic lives changes.
- **D2 — Kind-scoped attachment is computed in `canon-cli`, not
  `canon-store`.** `TierPolicy::tier_for(kind)` plus `TierPolicy.aging
  .get(&kind)` — the exact two facts `TierRegistry::tiers_for_read`
  already combines internally — are both `pub` today. A new
  `tiers::build_lenient_tiers_for_kind(policy, project_dir, kind) ->
  Result<LoadedTiers, TierCliError>` computes the small (`≤2`-element)
  set of `TierKind`s the kind needs, attaches `git` unconditionally
  (D1's existing behavior, unchanged), and attempts `pg`/`r2` ONLY when
  that `TierKind` is in the needed set — skipping the `std::env::var`/
  `PgTier::connect`/`build_r2_tier` call entirely for a tier the kind
  will never read. Rejected alternative: expose `TierRegistry`'s private
  `tiers_for_read` as `pub` and call it from `canon-cli`. Rejected
  because it would require handing `canon-cli` a `TierRegistry` before
  the registry itself exists (a chicken-and-egg: the registry needs the
  already-attached tier handles as constructor arguments) — recomputing
  the same two-line lookup from the already-`pub` `TierPolicy` fields is
  simpler and needs no `canon-store` API growth.
- **D3 — `canon tier age` explicitly keeps the strict `build_tiers`,
  not the lenient builder.** Aging is `TierRegistry::age_all()` — a
  read-then-write-then-delete move from a rule's source tier to its
  `to` tier; if EITHER side is unattached, there is no correct partial
  action (moving nothing is silently wrong; moving without deleting the
  source, or vice versa, is data-loss-adjacent). `canon tier age`'s
  all-or-nothing failure is the deliberately correct contract, unlike a
  read. This change touches `canon query` only.
- **D4 — Two entry points share one per-tier "attach-or-degrade" core,
  rather than duplicating the `git`/`pg`/`r2` match arms.** `ingest
  plans` needs the whole-policy variant (a plan-import pass may persist
  several different kinds' worth of records in one run, so it cannot
  scope up front to a single kind); `query` needs the kind-scoped
  variant. Both call the SAME per-tier `attach_pg`/`attach_r2` helpers
  (degrade-`TierUnavailable`-to-`None`, propagate anything else); the
  whole-policy variant always includes `pg`/`r2` in its "attempt" set,
  the kind-scoped variant filters that set down to `tiers_for_read`'s
  two-tier equivalent before calling the same helpers. No second
  degrade/propagate decision tree exists anywhere in the CLI.

## Risks

- **R1 — Reduced attach scope must never starve `--plugin`'s
  git-tree resolution.** Mitigated by D2's explicit "attach `git`
  unconditionally, scope only `pg`/`r2`" rule — `run_with_plugin`'s
  `git_root`/`resolve_and_project` keep receiving `loaded.git` exactly as
  today regardless of which kind was queried or whether `pg`/`r2` were
  scoped out. An acceptance test queries a `pg`-routed kind (which scopes
  `git` OUT of the *read* fan-out, since `tiers_for_read(Task)` is `[Pg]`
  alone) WITH `--plugin` and asserts the plugin resolution still succeeds
  — proving `git` attachment is unconditional, not itself scoped by
  `tiers_for_read`.
- **R2 — A kind whose `aging.to` differs from its routed tier must
  still get BOTH tiers attached (leniently), or a record that has
  already aged out becomes invisible instead of degrading correctly.**
  `canon.yaml`'s own example: `handoff`/`event` route to `pg` but age to
  `r2`. Mitigated by computing the needed set exactly as
  `TierRegistry::tiers_for_read` does — routed tier ALWAYS included, plus
  `aging.to` when it differs — never just the routed tier alone.
  Acceptance test: querying `--kind handoff` attempts BOTH `pg` and `r2`
  (each independently lenient), never `pg` alone.
- **R3 — Relocating `build_lenient_tiers` off `PlansError` onto
  `TierCliError` must not change `canon ingest plans`'s own observable
  error text/exit code.** Mitigated by re-running `plans.rs`'s existing
  test suite unmodified (task-list closure) — any wording drift in a
  propagated (non-`TierUnavailable`) error is a real regression, caught
  by the existing assertions, not merely a code-review claim.
