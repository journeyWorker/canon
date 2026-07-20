# canon-learn

> How to run canon learn promote and how strategy memory is rebuilt — the S6 (role-strategy-memory) layer. Strategy memory is re-derived non-destructively from a repo's verdict trajectories (parquet warm tier) as part of canon ingest artifacts (rebuild_namespace); the sole canon learn subcommand, promote, graduates a distilled StrategyItem into the git-tracked, PR-reviewable strategies tier. Use when promoting a proven strategy for a role, or when reasoning about the trajectory→strategy→retrieval flywheel that canon retrieve (S8) reads.

# canon-learn

S6 (`s6-role-strategy-memory`) is canon's ReasoningBank layer: the
role-namespaced strategy memory that turns accumulated verdict
trajectories into retrievable, promotable strategy insights. Strategy
memory is rebuilt (re-derived) from trajectories by `canon ingest
artifacts` (`rebuild_namespace`); `canon learn promote` graduates a
distilled strategy into the durable git tier that `canon retrieve` (S8)
reads before a task dispatch.

## Two tiers, two stores

- **Warm tier — trajectories (parquet).** `ParquetTrajectoryStore` under
  `<repo>/canon/learn/trajectories/`: the raw verdict-keyed trajectory
  rows written by ingest (S3/S4). Local, rebuildable, never
  hand-committed (`.gitignore`d).
- **Durable tier — strategies (git).** `ParquetStrategyStore` +
  git-tracked `<repo>/canon/strategies/<role>/<id>.md` files: the
  distilled, PR-reviewable `StrategyItem`s. This is the tier a promotion
  writes into and a demotion soft-flags.

Both sit behind the `TrajectoryStore` / `StrategyStore` traits (parquet
is the shipped backend, OQ2 — a vector-backed impl is a future ADDITIVE
adapter, never a rewrite).

## Rebuilding strategy memory (non-destructive)

Strategy memory is rebuilt from the current trajectory set as part of
`canon ingest artifacts` (S14): after that command persists a repo's
verdict trajectories, it calls `rebuild_namespace` to re-derive the
distilled strategies. There is NO standalone bare `canon learn` rebuild
command — the only `canon learn` subcommand is `promote` (below).

The rebuild is NON-DESTRUCTIVE (`rebuild_namespace`): it re-derives
distilled strategies from trajectories without ever deleting a strategy
the raw trajectory rows don't touch — a rebuild never silently drops a
hand-promoted strategy.

## `canon learn promote <strategy_id> [--dry-run]`

```bash
canon learn promote 01J...ULID           # promote a distilled strategy
canon learn promote 01J...ULID --dry-run # preview: lint + resolve, no write
```

Promotes a distilled `StrategyItem` (by its `StrategyId` ULID) from the
operator-local parquet warm tier UP into the git-tracked, PR-reviewable
`canon/strategies/<role>/<id>.md` tier (S6 task group 4). It:

- resolves the repo via the shared `canon.yaml` root walk, opens the
  strategy store + the git-tier strategies root (`LearnConfig`);
- runs advisory lints (content-length ceiling + literal-absolute-path
  rejection) — NON-blocking (printed, never fails the promote);
- writes the strategy's git-tier file with YAML front-matter
  (`status`, `regime_key`, `role`, `title`, `source_trajectory_ids`,
  `recorded_at`) + the strategy content as the body;
- `--dry-run` does everything EXCEPT the write (plan + lint + render).

An unknown `strategy_id` fails loud (nonzero exit), never a silent no-op.
Promotion RE-RENDERS and rewrites the whole `<role>/<id>.md` file
(`fs::write`, idempotent by content — re-promoting an unchanged strategy
is a byte-identical rewrite, so it does NOT preserve manual edits to that
file); a later demotion soft-flags the SAME file by merging
`status: demoted` into its front-matter, preserving git blame.

## The flywheel

```
ingest (S3/S4) → trajectories → rebuild (canon ingest artifacts) → strategies
      ↑                                                      │
   verdicts                              canon learn promote │ (graduate)
                                                             ▼
                     canon retrieve (S8) reads promoted strategies
                          before a task dispatch (pre-dispatch hook)
```

Promotion/demotion DECISIONS (the statistical reward gate) are S7's job
(`canon.yaml` `learn:` config); `canon learn promote` is the mechanism
that ACTS on a decision by writing the git tier.

## What this skill does NOT cover

- The reward/promotion statistics (when a strategy SHOULD be promoted) —
  S7; `canon learn promote` only performs a decided promotion.
- Retrieval at dispatch time (`canon retrieve`, the pre-dispatch hook) —
  see the `canon-retrieve` skill.
- Producing the verdict trajectories promotion reads — see the
  `canon-artifact-ingest` skill.
