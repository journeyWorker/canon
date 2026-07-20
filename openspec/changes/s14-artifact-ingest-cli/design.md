## Context

This change is derivative by design — it mirrors an already-proven shape
(`canon ingest sessions`, S3) rather than inventing a new one, and every
type it touches (`ArtifactAdapter`, `derive_verdict`, `Trajectory`,
`store_trajectory`, `rebuild_namespace`) is already frozen, public API from
S4/S6. The only genuinely new work is the driver glue: reading
`canon.yaml`'s `artifacts:` section, feeding the two adapter shapes
correctly, and wiring the persist step into `canon-learn`'s store.

## Goals / Non-Goals

**Goals:**
- `canon ingest artifacts` runs every registered `ArtifactAdapter`, feeds
  each correctly per its `ArtifactSourceKind`, derives verdicts, and
  persists trajectories into the SAME store S6/S8/S9 already read.
- The `handoff` records-source adapter is actually driven — proven by an
  integration test that seeds a real `Handoff` row and asserts the
  adapter's contribution is non-zero and explicitly reported, never
  silently folded into an "unconfigured, nothing found" shape.
- After ingest, `canon report`'s `mart_role_memory` and
  `mart_flywheel_funnel` render non-empty from the freshly-persisted data
  — proven against the real `duckdb` CLI, not a parallel Rust
  recomputation.

**Non-Goals:**
- Full write-time idempotence for artifact verdicts/trajectories (S4
  tasks.md group 6, "Idempotence," is unshipped upstream — this driver
  does not attempt to backfill it; a repeat `canon ingest artifacts` pass
  over an unchanged corpus persists fresh trajectories rather than
  deduping, exactly as today's upstream state allows). Closing that gap is
  S4's own remaining task group, not this driver's job.
- Any change to `ArtifactAdapter`, `derive_verdict`, `attach_regime_key`,
  `Trajectory`, `TrajectoryStore`, or `rebuild_namespace` — every one of
  these is called exactly as already shipped.
- A `canon.yaml` schema/parser for `ArtifactSourceConfig` living inside
  `canon-ingest` — that type's own doc comment already names the CLI layer
  as where this parsing belongs (D1 below).
- Git-tier strategy promotion (`canon learn promote`) — untouched, out of
  scope, S6/S7's own territory.

## Decisions

1. **`ArtifactSourceConfig` is parsed inside `canon-cli`, never inside
   `canon-ingest`.** `ArtifactSourceConfig` already derives `Deserialize`;
   its own doc comment says parsing it out of a real `canon.yaml` "is
   wave-2/CLI wiring ... deriving `Deserialize` so a future
   `serde_yaml::from_str::<ArtifactSourceConfig>` ... needs no bespoke
   parser." This driver is that future wiring — a small
   `ArtifactsSectionManifest { artifacts: ArtifactSourceConfig }` wrapper,
   parsed with `serde_yaml` (already a dependency of `canon-store`/
   `canon-learn`/etc., newly added directly to `canon-cli` only). *Alternative
   rejected:* adding a `from_manifest` method to `canon-ingest` mirroring
   `LearnConfig::from_manifest` — this would add a new production
   dependency (`serde_yaml`) to a crate whose own frozen doc comment
   already anticipates the parser living one layer up; no other
   `canon-ingest` type owns its own YAML section today (session-adapter
   roots are `home`-resolved by the CLI the same way), so this would be a
   new, unjustified convention rather than reused API.

2. **The `handoff` records-source read reuses `canon_cli::tiers::build_tiers`
   + `TierRegistry::query`, the exact machinery `canon query` already
   uses — never a bespoke `Tier` construction.** One `TierQuery::kind
   (RecordKind::Handoff)` call resolves through this repo's own
   `routing`/`tiers` config, honoring PG/git/R2 exactly as configured.
   *Alternative rejected:* a dedicated `Tier` handle constructed ad hoc for
   this one read — would duplicate `build_tiers`'s DSN-resolution/error
   handling and risk drifting from `canon query`'s own behavior for the
   identical config surface.

3. **`regime_key`'s `<hash>` segment is `content_digest` of the source
   event's own join key (`scenario:<id>` / `handoff:<id>` / `task:<id>`),
   not a commit SHA.** No artifact source (ledger record, divergence line,
   handoff row, most openspec-task flips) carries a commit SHA uniformly;
   the join key IS the stable "unit of work" identity every adapter
   already keys its `ArtifactEvent` by. Reusing S3's existing
   `content_digest` primitive (never a new hashing scheme) means two
   events sharing one join key — e.g. an open review finding and its later
   remediation — deterministically fold onto the SAME `regime_key`, and
   therefore the SAME `Trajectory`, exactly as `Trajectory`'s own doc
   comment names as the intended "more than one verdict" case.
   *Alternative rejected:* hashing the full event `detail` JSON — would
   give every event (including a finding and its own later remediation) a
   DIFFERENT hash, defeating that grouping.

4. **A records-source read failure degrades ONLY that adapter, never the
   whole pass — a per-adapter seam, not `crate::ingest`'s whole-batch
   `unwritten` fallback.** `canon-learn`'s `ParquetTrajectoryStore::open`
   has no "unreachable store" failure mode at all (it is a bare `PathBuf`;
   directories are created lazily on write) — the only genuine "can't
   proceed" condition in this whole pipeline is the records-source
   `Tier::read` step (no live PG DSN, `handoff` unrouted, malformed
   `canon.yaml`). That failure is reported as `status: "unavailable"` with
   an explicit reason on ONLY the `handoff` adapter's own summary entry;
   every `Path`-source adapter, and persistence of whatever verdicts were
   derived, still runs. *Alternative rejected:* mirroring
   `crate::ingest`'s exact whole-batch "unwritten, print JSON instead"
   shape — that shape exists there because ALL of S3's records share one
   store and one upfront routing check; here the two source kinds are
   already independent inputs into the same downstream persist step, so
   collapsing a `handoff`-only failure into a whole-pass fallback would
   silently drop unrelated `ledger`/`divergence`/`openspec-task` verdicts
   too.

5. **`rebuild_namespace` runs immediately after every successful
   trajectory persist, once per touched `regime_key`.** Without this,
   `mart_role_memory` (which reads ONLY `stg_strategy_items`, the
   distilled tier) stays empty forever, even after a fully successful
   ingest — defeating this whole change's stated purpose. `rebuild_namespace`
   is non-destructive (S6 design decision 3) and idempotent per call (it
   deletes-then-re-derives ONLY the touched regime's strategy items from
   its own retained raw trajectories), so calling it once per persisted
   regime per pass is safe and cheap. *Alternative rejected:* leaving
   distillation to a separate, not-yet-built `canon learn rebuild` command
   — would ship a driver that satisfies S6/S8's read path but still leaves
   S9's `mart_role_memory` panel empty, the exact gap this change exists
   to close.

## Risks / Trade-offs

- [Risk] No write-time idempotence (Non-Goal) means `--watch`/repeated
  runs accumulate duplicate trajectories over an unchanged corpus →
  [Mitigation] this is the SAME state S4's own upstream tasks.md already
  documents as unshipped (group 6); closing it here would silently expand
  this change's scope into S4's own remaining work. Documented in both
  `--watch`'s own CLI help text and this file.
- [Risk] `content_digest`-of-join-key (decision 3) means the SAME
  scenario/handoff/task re-ingested after its underlying content changes
  keeps the SAME `regime_key` and therefore folds new verdicts onto the
  SAME (growing) trajectory rather than a fresh one → [Mitigation] this is
  the intended behavior per `Trajectory`'s own doc comment ("a code-review
  finding followed by its later remediation, both folded onto the same
  regime"), not a bug; `store_trajectory`'s own contract already commits
  to "no dedup, caller mints a fresh id" at the record level, this
  decision only affects which regime a NEW trajectory record joins.

## Migration Plan

- Purely additive: a new subcommand, a new `canon.yaml` section (optional,
  defaults to unconfigured/no-op), and a new module. No existing command,
  file layout, or store schema changes.
- Rollback: removing the `canon ingest artifacts` subcommand (or simply
  never invoking it) leaves S3/S4/S6/S7/S8/S9 exactly as they already
  behave today — nothing else depends on this driver having run.

## Open Questions

- Whether idempotent artifact-verdict re-ingest (S4 tasks.md group 6)
  should land as an S4 follow-up or fold into a future wave of this
  change — deferred; out of this change's own scope either way.
