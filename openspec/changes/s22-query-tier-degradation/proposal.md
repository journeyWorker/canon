## Why

The eno-drift round-2 usability review (`target/usage-review/eno-drift/SYNTHESIS-ROUND2.md`
finding #1, marked **[blocker-class]**) and the Planner's live re-run
(`reviews/planner.json`, `top_remaining_ask`) name `canon query` — canon's
own flagship read verb — as categorically unusable against canon's OWN
production `canon.yaml` the moment it lists more than one tier:

> "`canon query` (any `--kind`) hard-fails on a repo whose `canon.yaml`
> names a `pg`/`r2` tier that isn't currently reachable, EVEN for a
> `--kind` routed entirely to `git` — traced to `query.rs` using the
> strict `tiers::build_tiers` instead of the lenient builder `ingest
> plans` already has." (planner.json, `new_frictions_this_round[0]`)
>
> "Make `canon query` tolerate the SAME per-tier degradation `canon
> ingest plans` already has — attach only the tier(s) the requested
> `--kind` actually routes to … so a planner's own multi-tier `canon.yaml`
> (canon's own repo is the reference example) doesn't hard-break the
> single most load-bearing read command the moment ANY one tier lacks
> credentials." (planner.json, `top_remaining_ask`)

**The bug, grounded in the shipped code:**

- `canon.yaml` (this repo's own, `canon.yaml:24-46`) declares all three
  tiers: `tiers.git`/`tiers.pg` (`dsn_env: CANON_PG_DSN`)/`tiers.r2`
  (`bucket_env: CANON_R2_BUCKET`) — a realistic multi-tier config, exactly
  the shape S2's design targets. `routing` sends `change`/`scenario`/
  `review`/`divergence`/`evidence_record`/`strategy_item` to `git`;
  `task`/`handoff`/`session`/`run`/`event` to `pg`; `trajectory` to `r2`.
- `crates/canon-cli/src/tiers.rs::build_tiers` (lines 78-100) constructs
  a handle for EVERY tier `canon.yaml` declares, unconditionally, before
  any query runs. Its `pg` arm (lines 88-95) is `std::env::var(&cfg.dsn_env)
  .map_err(|_| StoreError::TierUnavailable { .. })?` — a hard `?` — so
  the moment `tiers.pg` is present and `CANON_PG_DSN` is unset (the
  ordinary "I don't have a live Postgres right now" case), `build_tiers`
  itself errors, before `TierRegistry::query` (which would have correctly
  scoped the read to only the tiers the queried kind needs) ever runs.
- `crates/canon-cli/src/query.rs::run`/`run_with_plugin` (lines 273/485)
  both call `tiers::build_tiers` directly. So `canon query --kind change`
  — a `git`-routed kind that never touches `pg` at read time — dies with
  a `pg`-DSN error on canon's own repo, and every other `--kind` dies the
  identical way. The flagship read verb is unusable against the one repo
  big enough to prove the tool (SYNTHESIS-ROUND2 #1: "this is now the
  Planner's #1 ask, above the join gap").
- Meanwhile `crates/canon-cli/src/plans.rs::build_lenient_tiers` (lines
  575-607, backing `canon ingest plans`) already solves exactly this: for
  each of `pg`/`r2`, an unset DSN/unreachable bucket
  (`StoreError::TierUnavailable`) degrades to `None`; only a genuine
  config error (a malformed `tiers.pg.schema`, or any non-`TierUnavailable`
  `StoreError`) still propagates loud. `crates/canon-store/src/registry.rs`
  (`TierRegistry::handle`, lines 43-55; `tiers_for_read`, lines 63-72)
  already resolves lazily PER KIND and already produces a correctly named
  error — `StoreError::TierUnavailable { tier: Pg, reason: "tiers.pg not
  attached (no live DSN)" }` — the moment a query's OWN routed tier is
  `None`. `canon query` never reaches that correct machinery today because
  `build_tiers` fails first, for every kind, regardless of routing.

## What Changes

- **`canon query` (`run`/`run_with_plugin`) stops calling
  `tiers::build_tiers`.** It calls a new lenient, kind-scoped builder
  instead: only `tiers.pg`/`tiers.r2` are attempted AT ALL when the
  queried `--kind`'s own routing (or its `aging.to` destination) actually
  needs them — computed from `TierPolicy::tier_for`/`TierPolicy.aging`
  (already `pub`, no `canon-store` change needed). `tiers.git`, when
  configured, is still attached unconditionally (it is a local directory
  handle — `GitTier::new` does no I/O and cannot fail — exactly
  `build_lenient_tiers`'s existing, unconditional treatment of `git`), so
  `--plugin <id>`'s plugin-manifest/overlay resolution (which always
  needs `loaded.git`, regardless of the queried kind's own routing, per
  `run_with_plugin`'s existing `git_root`/`resolve_and_project` use) keeps
  working unchanged.
- **Attaching one of the scoped-in tiers still degrades, never
  hard-fails, on mere unreachability.** An unset `dsn_env`/unreachable
  bucket for a tier the queried kind DOES need degrades that ONE tier to
  `None`; a malformed config (bad schema ident, or any non-`TierUnavailable`
  `StoreError`) still propagates loud — identical to `build_lenient_tiers`'s
  existing per-tier contract, reused rather than reimplemented.
- **The only fatal outcome left is the query's OWN routed tier being
  unavailable** — surfaced by `TierRegistry::query`'s existing
  `handle()`/`tiers_for_read()` machinery exactly as it already does for
  every other caller, with no new error type: `StoreError::TierUnavailable
  { tier, reason }`, naming which tier and why (`"tiers.pg not attached
  (no live DSN)"` / `"tiers.r2 not attached (no live bucket)"`).
- **The lenient tier builder moves from a `plans.rs`-private function to
  a shared one in `tiers.rs`.** `build_lenient_tiers`/`LenientTiers`
  relocate out of `plans.rs` (generalizing their error type off
  `PlansError` onto `TierCliError`, which already wraps `StoreError` via
  `#[from]`) so `canon ingest plans` and `canon query` call the SAME
  per-tier attach/degrade logic — never two independently-maintained
  copies of "degrade `TierUnavailable`, propagate everything else." A new
  kind-scoped entry point sits beside the existing whole-policy one (which
  `ingest plans` keeps using — a plan-import pass may persist several
  kinds in one run and cannot scope to a single kind up front).
- **`canon tier age` is explicitly untouched.** It keeps calling the
  strict `tiers::build_tiers` (tiers.rs's own module doc already names
  both task 3.3 `canon tier age` and task 4.1 `canon query` as
  `build_tiers` consumers) — aging is a destructive move+delete that
  needs BOTH its source and destination tier live to run at all; there is
  no partial-success shape for it to degrade into, unlike a read.

### Added Capabilities

- `query-tier-degradation`: `canon query` (`run` and `run_with_plugin`,
  with and without `--plugin`) attaches only the tier(s) the requested
  `--kind` actually needs, degrading an unreachable-but-irrelevant tier to
  a no-op instead of a hard failure; a query whose OWN routed tier is
  unavailable still fails, loud and named (kind, tier, reason) — never
  silently, never generically.
- `uniform-lenient-tier-build`: the per-tier "unreachable degrades to
  `None`, malformed config still propagates" logic lives in exactly one
  shared function in `canon_cli::tiers`, used by both `canon ingest
  plans` and `canon query` (whole-policy and kind-scoped entry points
  over the same per-tier core) — no second, independently-drifting copy.

### Explicit non-goals

- **No change to the closed 12-`RecordKind` set.** `RecordKind::ALL.len()
  == 12` and its three assertion sites are untouched; this is a CLI
  tier-construction fix, never a model change.
- **No change to connector-never-authority.** `canon-gate` reads nothing
  importer/plugin-specific; `canon gate check` verdicts stay byte-identical
  (gate.rs never calls any tier builder at all — it is unaffected by
  construction, not merely re-verified).
- **No change to `canon inventory sync`'s single-`Scenario`-producer
  status.** Untouched.
- **No change to `canon tier age`'s all-or-nothing tier construction.**
  See "What Changes" — aging keeps `build_tiers`, deliberately, because
  its destructive move+delete has no partial-success story.
- **No change to `TierRegistry`, `Tier::read`/`write`, or any of
  `GitTier`/`PgTier`/`R2Tier`.** `crates/canon-store` is untouched end to
  end; `TierRegistry::handle`/`tiers_for_read`'s existing per-tier lazy
  resolution and `StoreError::TierUnavailable` naming are REUSED as-is,
  never modified or duplicated.
- **No change to `TierPolicy`'s shape, `canon.yaml`'s `tiers:`/`routing:`/
  `aging:` grammar, or `TierPolicy::tier_for`'s signature.** All already
  `pub`; this change only reads them from a new call site.
- **No new `canon query` flag.** `canon query --help`'s flag surface is
  unchanged; this is a pure internal behavior fix — the same invocation
  that hard-failed before now succeeds (or fails named) with identical
  arguments.
- **No change to `--since`/`--change-id`/`--status`/`--json`/`--plugin`
  semantics, `fold_pg_routed_kind`, `apply_scope`, or `rollup_for`.**
  `query.rs`'s post-`registry.query` pipeline is untouched; only the
  tier-construction step ahead of `TierRegistry::new` changes.
- **No retroactive relaxation of `canon ingest plans`'s existing
  whole-policy `build_lenient_tiers` contract.** Its behavior for
  `ingest plans` is byte-identical after the relocation — this change
  moves and generalizes the function's error type, it does not alter what
  it does for its existing caller.

## Impact

- **`canon-cli`**: `tiers.rs` gains the relocated `build_lenient_tiers`/
  `LenientTiers` (generalized onto `TierCliError`) plus a new kind-scoped
  entry point built on the same per-tier core; `plans.rs` drops its
  private copy and calls the shared one (byte-identical behavior for
  `ingest plans`, proven by re-running its existing tests unmodified);
  `query.rs`'s `run`/`run_with_plugin` call the new kind-scoped builder
  instead of `tiers::build_tiers`.
- **No other crate changes.** `canon-store`, `canon-model`, `canon-gate`,
  `canon-ingest`, `canon-plugin`, `canon-learn`, `canon-vocab` are all
  unchanged.
