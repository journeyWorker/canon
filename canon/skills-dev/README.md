# `canon/skills-dev/` — canon-developer skill source of truth

Skills for people (and agents) **developing canon itself** — extending the
Rust workspace, the record-kind model, the storage internals. These are
NEVER materialized into a consumer repo; `canon skills install`'s default
`--source canon/skills` does not see this directory.

Audience rule:

- `canon/skills/` — for agents **using** the `canon` CLI in a consumer
  repo. CLI commands, `canon.yaml` shapes, artifact formats, error
  reading. No Rust internals, no crate paths.
- `canon/skills-dev/` (this directory) — for canon contributors. Crate
  layout, workspace discipline, model extension, CI matrix.

## Materializing into canon's own repo

canon's own `.claude/`/`.codex/` trees carry BOTH sets (its developers
also use canon on itself). From the repo root:

```bash
canon skills install --source canon/skills --target .
canon skills install --source canon/skills-dev --target .
```

Each source directory keeps its own `.install-lock.json`; the two locks
never share entries. `SKILL.md` shape and lock semantics are identical to
`canon/skills/` — see that directory's README.
