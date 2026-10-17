# canon-inventory

`canon inventory sync` builds canon's scenario ledger-INDEX: it turns a
hand-authored, `canon format`-validated `.feature` corpus into the scenario
records every downstream stage joins against.

## You rarely run it yourself

Since 0.16 the commands that read the index refresh it first:
`canon status`, `canon gate check`, `canon evidence add`,
`canon review add` and `canon subject status` index added, edited and
deleted `.feature` files themselves, with the same code this command runs,
scoped to the changed files, and say so in one stderr line:

```
canon status: refreshed the scenario index from 1 changed feature file(s) (.canon/specs/kind=feature/area=cart/add.feature): 1 record(s) written
```

So the working loop has no sync step and no `canon format` step: write the
scenario (`canon scenario new` writes it format-clean) and run the next
command. Run `canon inventory sync` when:

- a reader prints `NOT refreshed` — a root has a format violation (a
  hand-written `Feature:`/`Scenario:` header with no `# canon:` provenance
  comment, an emptied file, …) or a duplicate id, so its files were not
  indexed. `canon gate check` then fails (`uncovered-cell <root>`),
  `shipped` is refused and `canon status` lists only the sync. `canon
  inventory sync` names each problem (`canon format <root>` names the format ones);
  fix them and the next command indexes the files;
- `canon status` says the ledger holds no scenario records while the
  `.feature` files hold some (the same refusal, on an index never built);
- you want the whole index rebuilt from every root at once.

A spec root still on the pre-0.17 layout (a `.feature` file under
`<root>/features/`) is not refreshed and fails closed the same way: the
reader names the root and `canon migrate` on stderr and writes no
tombstones, `canon gate check` fails (`uncovered-cell <root>`) whether or
not the index already holds its scenarios, `shipped` is refused, and
`canon status` lists only `canon migrate`. `canon inventory sync` does not
fix it; run `canon migrate`, which moves `features/kind=feature/` to
`<root>/kind=feature/`.

`canon migrate --dry-run` prints the plan without writing anything, and
`canon migrate` applies it. Commit first: it refuses while a path it
moves has uncommitted changes, and when a destination already exists.
A root at the repo-level `specs` moves to `.canon/specs`, and
`canon.yaml` records `former_roots: [specs]`, so reviews pinned before
the move still find their scenarios. Changes `canon change new` wrote
under `openspec/changes/` move to `.canon/changes/<slug>/`
(`proposal.md` becomes `brief.md`). An `Approved:` line in an external
`proposal.md` is copied into a canon pointer `brief.md`. The command
also writes `skills.providers` from the install lock, adds the missing
canon `.gitignore` lines, and ends with a full index rebuild. It is
idempotent, and a run that stopped partway is finished by the next run:
`canon.yaml` is written atomically, a half-moved change is completed,
and the openspec source `canon init` wrote is removed only after every
pointer brief is written and nothing unread is left under
`openspec/changes/` (each unread directory is named).

The reader's refresh treats a file as stale when its latest index
generation has another `source_digest` or `scenario_digest`. A
generation written before 0.15 has no `scenario_digest`, so a repo that
kept an old index gets it rewritten on the first read.

## Where the index sits in the loop

```
author .feature corpus (canon scenario new)
        │
        ▼
scenario index                  # refreshed by every reader; `canon inventory sync` rebuilds it
        │
        ▼
canon review add                # attests a review — see canon-inventory downstream / canon-gate
canon divergence {add,promote,resolve,defer,accept,status}   # tracks/resolves a divergence
canon gate check [--release]    # trust-ladder / staleness / coverage — see canon-gate
        │
        ▼
canon ingest artifacts          # with artifacts.native_records: true in canon.yaml —
                                # feeds review/divergence verdicts into the flywheel
                                # (see canon-artifact-ingest)
```

Every stage after the index joins on the SAME `(project_id, scenario_id)`
pair it materializes — one identity scheme, no second one.

## `canon inventory sync [--repo <dir>] [--spec-root <dir>]`

```bash
canon inventory sync                        # every canon.yaml specs.roots[] entry
canon inventory sync --repo ../svc          # a specific repo
canon inventory sync --spec-root ./specs    # ad hoc: ignore canon.yaml entirely
```

Per configured root, in order:

1. **Validate** — runs `canon format --check` over the root. ANY violation
   (missing provenance, layout-grammar, …) ABORTS THE WHOLE ROOT: zero
   records written for it, the violation(s) reported. Never a partial sync.
2. **Scan** — walks `<root>/kind=feature/area=<area>/<surface>.feature`, pairing each
   `@<area>.<surface>.<nn>` tag with its header label as `title` and a
   `source_digest` (sha256 over the raw `.feature` bytes). `@subject:<id>`
   → `subject_id`, `@lane:<v>` → `lane`, `@case:<v>` → `case` (first tag
   wins; a second, a malformed value, or a value outside a declared
   `lane`/`case` enum is dropped with a counted, non-fatal diagnostic). Any other `@name:value`
   namespace is counted as a diagnostic so a dropped classification is
   visible; plain tags (`@p2`) are ignored. The index derives from the
   `.feature` corpus alone. See `canon-authoring` for the organizing rule.
3. **Materialize** — upserts ONE scenario index record per
   `(project_id, scenario_id)` via the append-only git-tier write.
   Logically idempotent: an unchanged `source_digest` is a no-op; a
   changed `.feature` file appends exactly one new record. A `.feature`
   edit re-materializes every scenario in that file (file granularity).
4. **Duplicate guard** — a `scenario_id` scanned more than once WITHIN
   one root's corpus aborts that root (0 writes), reported as a sync
   error. Two DIFFERENT roots sharing a `scenario_id` stay distinct —
   `project_id` isolates them.

The scenario index shape is deliberately general: `title` +
`source_digest`, nothing else. It carries no coverage field — coverage
stays `canon gate check`'s own authority (see `canon-gate`).

## `specs.roots[]` config (`canon.yaml`)

```yaml
specs:
  roots:
    - id: app-a          # STABLE LITERAL — never the checkout directory name
      root: apps/a/specs
    - id: app-b
      root: apps/b/specs
```

- **Absent `specs:` key** → the single default root
  `{id: root, root: .canon/specs}`, the one `canon init` writes. An entry
  with no `root:` also resolves to `.canon/specs`. The default root is
  authored and git-tracked; no canon gitignore line covers it.
- **Present `specs:` with an empty/missing `roots[]`** → fails LOUD; a
  present-but-incomplete config never silently resolves to zero roots.
- **A malformed entry** (missing `id`, `roots` not a list, `id` not a
  valid project id) → fails LOUD, never a silent fallback to the default.
- `--spec-root <dir>` bypasses `specs.roots[]` entirely and syncs
  exactly that one directory under the same stable `root` id the
  absent-`specs:` default uses.

## Sync-abort causes

- **A `canon format --check` validation violation** in the root's corpus →
  whole root aborts, 0 writes, violations reported.
- **A root on the pre-0.17 layout** (a `.feature` file under
  `<root>/features/`) → a `layout-grammar` violation at path `features`;
  the root aborts, 0 writes, exit 1. Run `canon migrate`. An empty
  leftover `features/` directory is not a refusal.
- **A duplicate `scenario_id`** within one root → that root aborts, 0
  writes, reported as a sync error (a distinct lane from fmt violations).

## How the index feeds the rest

- **Review** — `canon review add` attests a verdict against a synced
  scenario. (See `canon-inventory` downstream commands and each command's `--help`.)
- **Divergence** — `canon divergence {add,promote,resolve,defer,accept,status}`
  tracks/resolves divergences against synced scenarios.
- **Gate** — `canon gate check` enforces trust-ladder / staleness /
  coverage over the corpus (see `canon-gate`).
- **Ingest** — `canon ingest artifacts` with `artifacts.native_records:
  true` in `canon.yaml` feeds review/divergence verdicts into the
  reward flywheel (see `canon-artifact-ingest`).