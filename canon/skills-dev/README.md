# `canon/skills-dev/` — developer-only legacy skill source

Skills for people (and agents) **developing Canon itself** — extending the Rust
workspace, the record-kind model, and storage internals. This directory is
never included in the user-facing `canon` bundle and is not installed by
default.

Use the canonical bundle for agents using the CLI:

- `canon/skills/` — one provider-aware `canon` skill plus lazy references.
- `canon/skills-dev/` — individual developer procedures and crate internals.

## Materializing developer guidance

From the repository root:

```bash
canon skills install --source canon/skills-dev --target .
```

The legacy materializer continues to produce `.claude/skills/<name>/SKILL.md`
and `.codex/skills/<name>.md` and keeps its own source `.install-lock.json`.
The canonical installer never mutates this source or its lock. Do not use
`--providers` with this legacy source; provider selection applies to the
canonical `SKILL.src.md` bundle only.
