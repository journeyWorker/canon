# canon-plugins

> How to author a ledger-overlay `plugin.yaml` manifest, validate + write overlay records with `canon plugin sync`, and project them onto a core read via `canon query --kind <k> --plugin <id>` — without ever mutating a core record. Covers the `porting` covered/surface_ref worked example. Use when adding a plugin-namespaced overlay, or debugging a `canon plugin sync`/`canon query --plugin` failure.

# canon-plugins

A plugin adds fields to a core record WITHOUT changing that record's
schema. It declares a NAMESPACED overlay kind (`<namespace>.<kind>`, e.g.
`porting.coverage`), writes overlay records into that namespace, and a
read-time JOIN projects the declared fields onto the core view. Core
records are NEVER rewritten by any step — the overlay write path is
append-only under a foreign namespace, and a manifest cannot alias a
core kind's on-disk path even by accident. Today overlays attach to the
`scenario` core kind only.

## 1. Author a `plugin.yaml` manifest

`canon/plugins/<id>/plugin.yaml`:

```yaml
id: porting
namespace: porting
overlays:
  - kind: coverage
    attaches_to:
      core_kind: scenario           # `scenario` ONLY
      join_key: [project_id, scenario_id]
    fields:
      - name: covered
        type: bool
      - name: surface_ref
        type: { list: string }
```

- `id`/`namespace`/`overlays[].kind` are REQUIRED — a missing field fails
  the load outright, never defaulted.
- `namespace` and every overlay `kind` MUST match the kebab-token grammar
  `[a-z0-9]+(-[a-z0-9]+)*` — else rejected with `E-PLUGIN-GRAMMAR`.
- `<namespace>.<kind>` MUST NOT collide with a core record kind name
  (`E-PLUGIN-CORE-COLLISION`) and MUST be unique across installed plugins
  (`E-PLUGIN-DUP-OVERLAY`).
- `attaches_to.core_kind` MUST be `scenario` — any other value fails with
  `E-PLUGIN-CORE-KIND`.
- `type` is a bare scalar (`bool`/`number`/`string`), `{ enum: […] }`, or
  `{ list: <type> }`.
- Two packages declaring the SAME `id`: the later (directory-sort order)
  is dropped and diagnosed, never merged (`E-PLUGIN-DUP-ID`).
- An absent `canon/plugins/` directory resolves an empty, valid snapshot —
  never a panic.

## 2. Validate + write overlay records

Drive it through the sync command (never hand-write records):

```bash
canon plugin sync porting                       # every canon.yaml specs.roots[] entry
canon plugin sync porting --repo ../svc         # a specific repo
canon plugin sync porting --spec-root ./specs   # ad hoc: ignore canon.yaml
```

`canon plugin sync <plugin-id>` resolves the manifest, then for each
candidate validates FIRST (envelope present + typed; every required
`join_key` field present + a string; every declared field present + type-
matching; no field outside envelope ∪ join-key ∪ declared), then writes.
The on-disk path is derived from the join-key field values in declared
order plus a body digest — a byte-identical resubmission dedupes to the
same path; a different body for the same join key appends at a new path,
never overwriting.

## 3. Project a read

```bash
canon query --kind scenario --plugin porting          # human table, projected fields appended
canon query --kind scenario --plugin porting --json   # merged record bodies
canon query --kind scenario                           # no --plugin: byte-identical to the core view
```

`--plugin <id>` folds the plugin's overlay records (latest-by-
`(join_key, at)`) onto each queried core record. Fail-soft everywhere:

- No overlay record for a core key → that record projects UNMODIFIED (no
  value invented).
- A malformed overlay record (fails validation against the CURRENT
  manifest — e.g. the schema drifted after the record was written) →
  SKIPPED + diagnosed; siblings still project.
- An absent plugin/manifest → the unmodified core view + a diagnostic,
  never a process error.
- Omitting `--plugin` is byte-identical to `canon query`'s output without
  plugins — it's a purely additive layer.

## The `porting` worked example

`porting` moves the `covered`/`surface_ref` fields off the core
`scenario` record and into a plugin overlay:

1. `canon/plugins/porting/plugin.yaml` — the manifest above.
2. Its overlay source reads a spec root's `inventory/**/*.yaml` (the same
   inventory files `canon fmt --check` covers) and, for every
   `(project_id, scenario_id)` `canon inventory sync` would index, emits
   one candidate: `covered = true` iff that `scenario_id` appears in ANY
   inventory entry's `covered_by` list; `surface_ref` = every inventory-
   entry key whose `covered_by` contains it (empty when uncovered). The
   candidate's envelope `at` is the source version (the max inventory-file
   `at`), so an unchanged inventory re-syncs to a byte-identical body
   (zero new files) and a genuine coverage edit supersedes the prior
   overlay via latest-`at`-wins.
3. `canon plugin sync porting` writes/dedupes those candidates.
4. `canon query --kind scenario --plugin porting [--json]` projects
   `covered`/`surface_ref` back onto each scenario.

`canon gate check`'s verdicts are byte-identical with and without a
`canon plugin sync porting` run — the gate reads NO plugin overlay. Wiring
an overlay field into a gate decision needs its own reviewed change
("covered ≠ coverage").

## Do not confuse the two `plugin.yaml` surfaces

The typed-authoring vocabulary (`canon/vocab/<id>/plugin.yaml`: directives
+ enums for task-atom authoring — see `canon-vocab`) is a
DIFFERENT directory and a DIFFERENT schema. A ledger-overlay manifest
misplaced under `canon/vocab/<id>/` fails that loader (missing
`directives`/`enums`), not silently parsing as an authoring vocabulary.

## What this skill does NOT cover

- **`canon inventory sync`** (the core index `porting`'s source reads) —
  see `canon-inventory`.
- **`canon gate check`** — see `canon-gate`; the gate never reads a
  plugin overlay.
- **`canon scenario new`/`canon feature new`** (corpus-authoring
  scaffold, independent of the plugin machinery) — template generators
  that write no ledger record.
- **A projection over a core kind other than `scenario`, or wiring an
  overlay field into a `canon gate` decision** — out of scope; each needs
  its own reviewed change.
