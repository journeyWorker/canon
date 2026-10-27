# `canon/skills/` — canonical user-facing skill bundle

The user-facing companion skill is authored once, here, and embedded into the
`canon` binary at build time (`crates/canon-cli/build.rs`, read through
`guide.rs`). Since 0.28 the guide is not projected into a consumer repo: the
installed `SKILL.md` is a short stub, and `canon skills show <name>` prints the
guide that matches the running binary, so it cannot drift from the commands.

```
canon/skills/
  SKILL.src.md                         # the core guide: `canon skills show canon`; its frontmatter is copied into the stub
  reference/<topic>.md                 # topic guides: `canon skills show canon-gate`, ...
  reference/role-<role>.md             # planner, implementer, reviewer, orchestrator: `canon skills show role-planner`, ...
  reference/brief-<pass>.md            # five brief refinement passes: `canon skills show brief-polish`, ...
  scripts/canon-retrieve-pre-dispatch.sh   # projected next to the stub
```

A guide name is `canon` (the core guide, `SKILL.src.md`) or the stem of a
`reference/<stem>.md` file. `canon skills show <name>` prints one;
`canon skills show canon --full` prints the core guide followed by every
reference. Guide text therefore never points at a `reference/<file>.md` path
(it does not exist in the consumer's repo): it says `canon skills show <stem>`.
Every command a guide prints is parsed against the generated CLI spec
(`docs/cli/canon.cli.json`) by `crates/canon-cli/tests/docs_lint.rs`, and
blocks between `canon:generated … begin/end` markers are regenerated from that
spec: edit the spec's source, not the block.

The 17 topic references retain the former companion-skill bodies without their
repeated YAML frontmatter (`canon-review.md` is new in 0.12). The core guide
routes an agent to a topic; it does not replace Canon's explicit CLI commands or
internal Rust modules.

## Materialization

```bash
canon init --providers=claude,codex               # or answer init's prompt; records skills.providers
canon skills install                              # follows canon.yaml skills.providers
canon skills install --providers=claude,codex,omp,pi   # any subset or none; rewrites skills.providers
canon skills check                                # read-only drift check
canon skills doctor                               # diagnostics, no deletion
```

Each selected harness receives a stub `SKILL.md` and the `scripts/**` sidecars;
the guides themselves are not copied. The stub is `SKILL.src.md`'s frontmatter
(`name`, `description`: that is what makes the harness route build requests to
the skill) plus a short body: what canon is, the loop in five lines, and
`canon skills show canon` for the full guide. It names the canon version that
wrote it, so `canon skills check` reports a stub another canon wrote and
`canon skills install` rewrites it.

- Claude: `.claude/skills/canon/SKILL.md` and `.claude/skills/canon/scripts/**`
- Codex: `.agents/skills/canon/SKILL.md` and `.agents/skills/canon/scripts/**`
  (Codex scans `.agents/skills` from the working directory up to the repository
  root, and never reads `.codex/skills`)
- OMP: `.omp/skills/canon/SKILL.md` and `.omp/skills/canon/scripts/**`
- Pi: `.pi/skills/canon/SKILL.md` and `.pi/skills/canon/scripts/**`

A repo upgraded from 0.27 holds a full `SKILL.md` and a `reference/**` tree:
`canon migrate` (preview), then `canon migrate --commit --plan-digest <digest>`
with the digest it printed, replaces them with the stub (backing the old files
up under `.canon/migrate-backup/`, local and git-ignored; `--revert` needs it), and `canon skills install` refreshes the
rest.

OMP/Pi projections are project-local passive bundles. The
`canon-retrieve-pre-dispatch.sh` script is provided only as a sidecar; no OMP
or Pi native hook is installed. Provider selection is `claude`, `codex`, `omp`,
and/or `pi` (or `none`); invalid names fail before any write. Without
`--providers`, the commands serve `canon.yaml`'s `skills: { providers: [...] }`
(0.17); a repo without that key uses the providers its install lock records;
with neither, each provider whose marker directory (`.claude`, `.codex`,
`.omp`, `.pi`) exists. `.agents` alone does not select Codex: other tools use
it too. With nothing detected, nothing is installed and the command names
`canon skills install --providers=<list>`. An explicit `--providers` on
install rewrites `skills.providers` in an existing `canon.yaml`, keeping
every other byte. `canon init` asks for the list on a terminal (the detected
ones checked), or takes `--providers`, records it and installs.

The source checkout is never modified by a canonical install. The target owns
a timestamp-free content-addressed manifest at
`.canon/skills/.install-lock.json`. Re-running an unchanged install is a
byte-identical no-op. Existing symlinks are never overwritten. `doctor`
reports stale/missing projections and old `canon-*` remnants in all four
managed roots and in `.codex/skills` without deleting user data.

`doctor` also names a canon projection directory (`.claude/skills/canon`,
`.agents/skills/canon`, `.omp/skills/canon`, `.pi/skills/canon`) whose
provider is not selected. Canon no longer updates it; the line is advisory,
`check` ignores it, and `doctor` still reports healthy:

```
stray: ./.agents/skills/canon — codex is not a selected provider (from canon.yaml skills.providers), so canon no longer updates it; remove it with `rm -r ./.agents/skills/canon`, or select it with `canon skills install --providers=claude,codex`
```

The lock also records `canon_version`, the canon that wrote it, and
`source_commit`, the `HEAD` of the git checkout the source was in (omitted
when the source is not in one). The commit is provenance only: a new commit
that leaves the bundle unchanged does not rewrite the lock. When the lock's
`canon_version` is not this canon's, `check` exits 1 and both `check` and
`doctor` (as an `outdated:` line) say so:

```
<lock> — outdated (installed by canon 0.15.0, this canon is 0.16.0 — run `canon skills install --providers=claude,codex`)
outdated: <lock> installed by an unknown canon version (the lock predates 0.16), this canon is 0.16.0 — run `canon skills install --providers=claude,codex`
```

A lock written before 0.16 has no `canon_version`, so it is reported as
installed by an unknown canon version, with the same fix. Installing
rewrites the lock with this canon's version.

`doctor` also reads the other skills in `.claude/skills`, `.agents/skills`,
`.omp/skills` and `.pi/skills` (each `<dir>/SKILL.md` other than `canon` and
`canon-*`) and flags any whose frontmatter `description` claims build work:

```
competing-skill: <dir> (description claims: build, sites) — if an agent picks `<name>` for build work, canon's loop will not run
```

A description claims build work when one sentence (text between `.`, `!`,
`?`, `;` or a line break) holds both a build verb and a build object, matched
as whole words, case-insensitive:

- verbs: `build`, `implement`, `create`, `make`, `scaffold`, `develop`,
  `code`, each with its `-s`, `-ing` and past forms (`built`, `made`, …);
- objects: `site`, `website`, `app`, `application`, `game`, `feature`,
  `frontend`, `ui`, `page`, `component`, `tool`, `library`, `service`, `api`,
  `cli`, `server`, `backend`, `project`, `prototype`, `product`, `program`,
  `script`, singular or plural.

A word is a run of letters and digits, so `web-app` holds `app` and `rebuild`
holds nothing. "Build websites and apps" claims build work; "Manage feature
branches" (an object, no verb) and "Build release notes" (a verb, no object)
do not. The line lists the verbs and objects of the claiming sentences. A
skill without a readable frontmatter `description` is skipped.
The line is advisory: it never makes the projections unhealthy, `doctor` still
exits 0, and canon never edits another skill. User-level skill roots are not
scanned.

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
canon skills install --skills-source canon/skills-dev
```

That source keeps its own `.install-lock.json` and materializes individual
developer skills for compatibility. Do not present those skills as the
canonical provider-neutral `canon` projection.
