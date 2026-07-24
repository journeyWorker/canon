---
name: canon-learn
description: How to run canon learn promote, choose a promotion gate mode (crn vs occurrence) in canon.yaml, and read a demoted strategy's status:demoted front-matter. Strategy memory is re-derived from a repo's verdict trajectories by canon ingest artifacts; promote graduates a distilled strategy into the git-tracked, reviewable strategies tier. Use when promoting a proven strategy for a role, configuring the promotion/demotion gate, or reading the trajectory→strategy→retrieval flywheel that canon retrieve queries.
---

# canon-learn

canon's role-namespaced strategy memory turns accumulated verdict
trajectories into retrievable, promotable insights. Two tiers:

- **Trajectories (warm, local).** Raw verdict-keyed rows under
  `<repo>/canon/learn/trajectories/`, written by ingest. Rebuildable,
  gitignored, never hand-committed.
- **Strategies (durable, git).** Distilled, reviewable
  `<repo>/canon/strategies/<role>/<id>.md` files — the tier a promotion
  writes into and a demotion soft-flags.

## Rebuilding strategy memory

Strategy memory is re-derived from the current trajectory set as part of
`canon ingest artifacts` (see `canon-artifact-ingest`): after it
persists a repo's verdict trajectories, it re-derives the distilled
strategies. This is NON-DESTRUCTIVE — a rebuild never drops a
hand-promoted strategy the trajectory rows don't touch. There is no
standalone `canon learn` rebuild command; the only subcommand is
`promote`.

## `canon learn promote <strategy_id> [--dry-run]`

```bash
canon learn promote 01J...ULID           # promote a distilled strategy
canon learn promote 01J...ULID --dry-run # preview: lint + resolve, no write
```

Graduates a distilled strategy (by its ULID) from the local warm tier UP
into the git-tracked `canon/strategies/<role>/<id>.md` tier. It:

- resolves the repo via the nearest-`canon.yaml`-ancestor walk;
- runs advisory lints (content-length ceiling, literal-absolute-path
  rejection) — printed, NON-blocking;
- writes the file with YAML front-matter (`status`, `regime_key`,
  `role`, `title`, `source_trajectory_ids`, `recorded_at`) + the
  strategy content as the body;
- `--dry-run` does everything EXCEPT the write.

An unknown `strategy_id` fails loud (nonzero exit). Promotion re-renders
the whole file, idempotent by content — re-promoting an unchanged
strategy is a byte-identical rewrite, so it does NOT preserve manual
edits to that file. A later demotion soft-flags the SAME file, preserving
git blame.

## Choosing a promotion gate: `crn` vs `occurrence`

A role's promotion gate is a `canon.yaml` `learn:`-section choice:

```yaml
# canon.yaml
learn:
  promotion:
    dev:
      mode: occurrence
      n_min: 8            # optional — defaults to 5
      window_days: 14     # optional — defaults to 30
    sim:
      mode: crn
  demotion:
    hard_delete: false                 # optional — soft-flag is the default
    strategies_root: canon/strategies  # optional
```

A role with no explicit `promotion.<role>` entry is not an error — it
defaults to `mode: occurrence, n_min: 5, window_days: 30` (the
conservative defaults).

- **`occurrence`** — for roles whose domain does NOT support
  deterministic replay (most of `dev`/`content`/`design`/`review`).
  Promotes when, inside the trailing `window_days`, at least `n_min`
  `Success`-verdict trajectories accumulate for the SAME `regime_key`
  AND zero `Failure`/`RolledBack`-verdict trajectories arrived for that
  regime. A contradicting failure RESETS the count (never averaged
  away). Samples older than the window don't count; a `Pending` sample
  (no covering verdict yet) is skipped. `RolledBack` resets the count
  like `Failure` (a stronger negative signal).
- **`crn`** — for roles that CAN run a deterministic simulator
  (`sim`-shaped domains): a paired common-random-number statistical
  corroboration gate. A CRN-capable role's trajectory-recording caller
  stamps `crn:config=<label>` / `crn:panel=<index>` tags on each
  trajectory it records.

## How verdicts feed scoring

Verdicts arrive from `canon ingest artifacts` (see
`canon-artifact-ingest`), persisted regime-keyed
(`<role>/<repo>/<area>/<hash>`). Each verdict scores its covering
trajectory into `Success` / `Failure` / `RolledBack` / `Pending`; the
promotion gate above reads a regime's accumulated verdicts to decide
promotion, and a contradicting trajectory arriving for an
already-promoted strategy triggers a demotion.

## Reading a demoted strategy

A promoted strategy that later collects a contradicting trajectory is
demoted append-only: the soft-flag (default) merges `status: demoted` +
`reason: <text>` into the strategy file's EXISTING front-matter, leaving
every other key and the whole body byte-unchanged:

```markdown
---
title: batch the parquet writes
description: avoids one fsync per row
status: demoted
reason: 'contradicting trajectory 01J... arrived for regime dev/app/auth-flow/deadbeef'
---
buffer writes and flush once per namespace
```

Set `demotion.hard_delete: true` to delete the file instead of
soft-flagging. A strategy demoted before it was ever promoted to the git
tier has no file to flag — that is not an error. A non-demoted strategy
simply has no `status`/`reason` keys. `canon retrieve` skips any
`status: demoted` strategy.

## The flywheel

```
ingest → trajectories → rebuild (canon ingest artifacts) → strategies
   ↑                                                   │
verdicts                       canon learn promote      │ (graduate)
                                                        ▼
             canon retrieve reads promoted strategies before a dispatch
```

## What this skill does NOT cover

- Retrieval at dispatch time (`canon retrieve`, the pre-dispatch hook) —
  see the `canon-retrieve` skill.
- Producing the verdict trajectories promotion reads — see the
  `canon-artifact-ingest` skill.
