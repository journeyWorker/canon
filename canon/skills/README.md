# `canon/skills/` — companion-skill source of truth

Every spec's companion skill is authored **once**, here — never directly
under a consumer repo's `.claude/` or `.codex/` tree (design decision 9,
`skill-materialization` spec). `canon skills install` materializes this
directory into a consumer repo.

## Layout

```
canon/skills/
  <name>/
    SKILL.md          # the SoT — see shape below
  .install-lock.json   # written by `canon skills install`, never hand-edited
```

## `SKILL.md` shape

Mirrors `.claude/skills/<name>/SKILL.md`'s existing format exactly, so
materializing it into a consumer repo's `.claude/skills/<name>/SKILL.md` is
a byte-verbatim copy — no transformation, no round-trip loss:

```markdown
---
name: <kebab-case-skill-name>
description: <one paragraph — what it does and when to use it>
---

# <heading>

<body markdown — instructions, examples, anything Claude Code should read
when this skill is invoked>
```

`name` and `description` are the only frontmatter fields `canon skills
install` reads. Everything after the closing `---` is the skill body.

## Materialization targets

`canon skills install --source canon/skills --target <repo-root>` writes:

- `.claude/skills/<name>/SKILL.md` — verbatim copy of the source file.
- `.codex/skills/<name>.md` — canon's own flattened convention (design
  D4): a `# <name>` header, a `> <description>` blockquote, then the body
  with its frontmatter stripped. Codex has no native skill-directory
  concept; canon is the format authority for this shape.
- `canon/skills/.install-lock.json` — `{ "skills": { "<name>": {
  "contentHash": "sha256:<hex>", "version": <int> } } }`. Content-hash +
  monotonic version only — **never** a `generatedAt` field (decision 11;
  a donor CLI's manifest materialization surfaced the timestamp-churn
  failure mode this avoids). Re-running with no source change
  produces byte-identical output; a content change bumps only the changed
  skill's `version` by exactly one.

Gemini is never touched — Claude Code and Codex only (decision 11).

## Adding a new companion skill

1. `mkdir canon/skills/<name>` and write `SKILL.md` per the shape above.
2. Run `canon skills install --source canon/skills --target .` from the
   canon repo root to materialize it into canon's own `.claude/` and
   `.codex/` trees (canon uses its own materializer on itself).
3. Commit the source `SKILL.md`, the materialized `.claude/`/`.codex/`
   output, and the updated `.install-lock.json` together.
