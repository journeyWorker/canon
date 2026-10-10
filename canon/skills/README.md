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

The 17 topic references retain the former companion-skill bodies without their
repeated YAML frontmatter (`canon-review.md` is new in 0.12). The umbrella
source routes an agent to a topic; it does not replace Canon's explicit CLI
commands or internal Rust modules.

## Materialization

```bash
canon skills install                              # detect .claude/.agents|.codex/.omp/.pi
canon skills install --providers=claude,codex,omp,pi
canon skills check                                # read-only drift check
canon skills doctor                               # diagnostics, no deletion
```

Claude receives:

- `.claude/skills/canon/SKILL.md`
- `.claude/skills/canon/reference/**`
- `.claude/skills/canon/scripts/**`

Codex receives `.agents/skills/canon/SKILL.md` plus matching
`.agents/skills/canon/reference/**` and `scripts/**` sidecars: Codex scans
`.agents/skills` from the working directory up to the repository root, and
never reads `.codex/skills`. OMP and Pi are directory-shaped, verbatim
projections too:

- `.omp/skills/canon/SKILL.md`, `.omp/skills/canon/reference/**`, and
  `.omp/skills/canon/scripts/**`
- `.pi/skills/canon/SKILL.md`, `.pi/skills/canon/reference/**`, and
  `.pi/skills/canon/scripts/**`

OMP/Pi projections are project-local passive bundles. The
`canon-retrieve-pre-dispatch.sh` script is provided only as a sidecar; no OMP
or Pi native hook is installed. Provider selection is `claude`, `codex`, `omp`,
and/or `pi`; invalid names fail before any write. Without `--providers`, each
provider whose root exists is selected; an existing `.agents` or `.codex`
directory selects Codex. If no provider root exists, Claude and Codex are
selected for backwards compatibility.

The source checkout is never modified by a canonical install. The target owns
a timestamp-free content-addressed manifest at
`.canon/skills/.install-lock.json`. Re-running an unchanged install is a
byte-identical no-op. Existing symlinks are never overwritten. `doctor`
reports stale/missing projections and old `canon-*` remnants in all four
managed roots and in `.codex/skills` without deleting user data.

Canon 0.13.0 and earlier projected Codex to a flattened
`.codex/skills/canon.md` plus `.codex/skills/canon/**`. Install migrates that
legacy projection: it removes each legacy file the manifest records whose
bytes still match the recorded hash, keeps any edited or unrecorded file
(including user files under `.codex/skills`), and removes `.codex/skills` only
when it ends up empty. `check` (exit 1) and `doctor` report a leftover legacy
Codex projection as a `legacy-remnant` together with the fix command,
`canon skills install --providers=<selected>`.

## Developer-only legacy source

`canon/skills-dev/` is contributor-only legacy tooling, intentionally separate
from the canonical user-facing bundle. It is not a normal install source.
Maintainers developing Canon MAY materialize it explicitly:

```bash
canon skills install --source canon/skills-dev --target .
```

That source keeps its own `.install-lock.json` and materializes individual
developer skills for compatibility. Do not present those skills as the
canonical provider-neutral `canon` projection.
