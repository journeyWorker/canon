# canon-inventory

> How to run canon inventory sync — the validate-then-materialize step that turns a validated `.feature` spec corpus into canon's scenario ledger index, and how that index feeds review, divergence, gate, and ingest. Use when authoring a new spec corpus, wiring a monorepo's `specs.roots[]` config, debugging a sync abort (a fmt violation or a duplicate scenario id), or explaining how canon's ledger records get produced.

# canon-inventory

`canon inventory sync` is the FIRST stage of canon's unified spec loop:
it turns a hand-authored, `canon fmt`-validated `.feature` corpus into
the scenario ledger-INDEX records every downstream stage joins against.

## The unified loop

```
author .feature corpus  →  canon fmt --check (validate)
        │
        ▼
canon inventory sync            # THIS skill — materializes scenario index records
        │
        ▼
canon review add                # attests a review — see canon-inventory downstream / canon-gate
canon divergence {stage,promote,resolve,defer,status}   # tracks/resolves a divergence
canon gate check [--release]    # trust-ladder / staleness / coverage — see canon-gate
        │
        ▼
canon ingest artifacts          # with artifacts.native_records: true in canon.yaml —
                                # feeds review/divergence verdicts into the flywheel
                                # (see canon-artifact-ingest)
```

Every stage after `sync` joins on the SAME `(project_id, scenario_id)`
pair `sync` materializes — one identity scheme, no second one.

## `canon inventory sync [--repo <dir>] [--spec-root <dir>]`

```bash
canon inventory sync                        # every canon.yaml specs.roots[] entry
canon inventory sync --repo ../svc          # a specific repo
canon inventory sync --spec-root ./specs    # ad hoc: ignore canon.yaml entirely
```

Per configured root, in order:

1. **Validate** — runs `canon fmt --check` over the root. ANY violation
   (missing provenance, layout-grammar, …) ABORTS THE WHOLE ROOT: zero
   records written for it, the violation(s) reported. Never a partial sync.
2. **Scan** — walks `<root>/features/**/*.feature`, pairing each
   `@<area>.<surface>.<nn>` tag with its header label as `title` and a
   `source_digest` (sha256 over the raw `.feature` bytes). The index
   derives from the `.feature` corpus alone.
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

- **Absent `specs:` key** → the single default root `{id: root, root: specs}`.
- **Present `specs:` with an empty/missing `roots[]`** → fails LOUD; a
  present-but-incomplete config never silently resolves to zero roots.
- **A malformed entry** (missing `id`, `roots` not a list, `id` not a
  valid project id) → fails LOUD, never a silent fallback to the default.
- `--spec-root <dir>` bypasses `specs.roots[]` entirely and syncs
  exactly that one directory under the same stable `root` id the
  absent-`specs:` default uses.

## Sync-abort causes

- **A `canon fmt --check` validation violation** in the root's corpus →
  whole root aborts, 0 writes, violations reported.
- **A duplicate `scenario_id`** within one root → that root aborts, 0
  writes, reported as a sync error (a distinct lane from fmt violations).

## How the index feeds the rest

- **Review** — `canon review add` attests a verdict against a synced
  scenario. (See `canon-inventory` downstream commands and each command's `--help`.)
- **Divergence** — `canon divergence {stage,promote,resolve,defer,status}`
  tracks/resolves divergences against synced scenarios.
- **Gate** — `canon gate check` enforces trust-ladder / staleness /
  coverage over the corpus (see `canon-gate`).
- **Ingest** — `canon ingest artifacts` with `artifacts.native_records:
  true` in `canon.yaml` feeds review/divergence verdicts into the
  reward flywheel (see `canon-artifact-ingest`).
