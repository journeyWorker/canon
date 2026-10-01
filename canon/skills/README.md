# `canon/skills/` — canonical user-facing skill bundle

The user-facing companion skill is authored once in `SKILL.src.md`. Canon's
installer projects that source as one `canon` skill for each selected provider;
references and scripts remain lazy-loadable sidecars.

```
canon/skills/
  SKILL.src.md                         # read-only authored source
  reference/<topic>.md                 # preserved actionable guidance
  scripts/canon-retrieve-pre-dispatch.sh
```

The 16 topic references retain the former companion-skill bodies without their
repeated YAML frontmatter. The umbrella source routes an agent to a topic; it
does not replace Canon's explicit CLI commands or internal Rust modules.

## Materialization

```bash
canon skills install                              # detect .claude/.codex
canon skills install --providers=claude,codex     # explicit, deterministic
canon skills check                                # read-only drift check
canon skills doctor                               # diagnostics, no deletion
```

Claude receives:

- `.claude/skills/canon/SKILL.md`
- `.claude/skills/canon/reference/**`
- `.claude/skills/canon/scripts/**`

Codex receives the flattened `.codex/skills/canon.md` plus matching
`.codex/skills/canon/reference/**` and `scripts/**` sidecars. Provider
selection is `claude` and/or `codex`; invalid names fail before any write. If
neither target exists, both are selected for backwards compatibility.

The source checkout is never modified by a canonical install. The target owns
a timestamp-free content-addressed manifest at
`.canon/skills/.install-lock.json`. Re-running an unchanged install is a
byte-identical no-op. Existing symlinks are never overwritten. `doctor`
reports stale/missing projections and old `canon-*` remnants without deleting
user data.

## Developer-only source

`canon/skills-dev/` is intentionally separate and remains a legacy directory
source. Install it explicitly when developing Canon:

```bash
canon skills install --source canon/skills-dev --target .
```

That source keeps its own `.install-lock.json` and still materializes its
individual developer skills for compatibility. It is not part of the
canonical user-facing projection.
