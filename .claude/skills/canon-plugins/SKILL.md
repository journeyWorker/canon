---
name: canon-plugins
description: How to author a canon-plugin ledger-overlay `plugin.yaml` manifest (s16 plugin-extensibility, `plugin-overlay-registry`/`plugin-overlay-records`/`plugin-overlay-projection` specs), validate + write overlay records (`canon plugin sync` / `canon_plugin::overlay::write_overlay`), and project them onto a core read (`canon query --kind <k> --plugin <id>`) without ever mutating a core record — covers the `porting` plugin's covered/surface_ref worked example, the P1-P6 crate/CLI surface map, and the `canon selftest` `plugin-overlays` fixture suite. Use when adding a NEW plugin-namespaced overlay, debugging a `canon plugin sync`/`canon query --plugin` failure, or explaining why `RecordKind` stays a frozen 12-kind closure while canon still stays extensible.
---

# canon-plugins

s16 (`s16-plugin-extensibility`) is the mechanism `inventory-materialization`'s
own Requirement already promised: a `Scenario` core record carries
`envelope + project_id + scenario_id + title + description +
source_digest` ONLY — no `covered`/`surface_ref` field, ever again
(`canon_model::records::Scenario`'s own doc comment). A plugin declares
a NAMESPACED overlay kind (`<namespace>.<kind>`, e.g.
`porting.coverage`), writes overlay records into that namespace, and a
read-time JOIN projects the declared fields onto the core view —
`RecordKind`'s frozen 12-member closure never grows a 13th variant for
this.

## The pipeline

```
author canon/plugins/<id>/plugin.yaml     # THIS skill, "1. Author a manifest"
        │
        ▼
resolve_plugin_snapshot(project_dir)      # canon-plugin P1 — the ONE resolution entry point
        │
        ▼
canon plugin sync <plugin-id>             # canon-cli P4 — validate (P2) then write (P2) every
        │                                 # OverlaySource candidate through write_overlay
        ▼
kind=<namespace>.<kind>/{join_key}__{digest12}.json   # append-only, under the s15 foreign-
        │                                 # namespace seam (scan_corpus skips + reports, never descends)
        ▼
canon query --kind scenario --plugin <id> # canon-cli P3 — project_overlay JOINS the overlay
                                           # onto the core Scenario view, in memory, read-only
```

Core records are NEVER rewritten by any step above (design.md D3,
enforced twice: architecturally — this crate's write path only ever
calls `GitTier::write_namespaced`/`scan_namespaced_kind`, never the
core `Tier::write`/`read`; and at write time — `write_namespaced`
rejects a namespaced-kind string equal to any `RecordKind::as_str()`
value, so a misconfigured manifest can't alias `kind=scenario/` even
by accident).

## 1. Author a `plugin.yaml` manifest

`canon/plugins/<id>/plugin.yaml`:

```yaml
id: porting
namespace: porting
overlays:
  - kind: coverage
    attaches_to:
      core_kind: scenario           # s16 supports `scenario` ONLY
      join_key: [project_id, scenario_id]
    fields:
      - name: covered
        type: bool
      - name: surface_ref
        type: { list: string }
```

- `id`/`namespace`/`overlays[].kind` are REQUIRED — a missing field
  fails `serde_yaml::from_str` outright (excluded, never defaulted).
- `namespace` and every overlay `kind` MUST match the kebab-token
  grammar `[a-z0-9]+(-[a-z0-9]+)*` — rejected loud at resolution
  (`E-PLUGIN-GRAMMAR`) otherwise.
- `<namespace>.<kind>` (this overlay's on-disk identity) MUST NOT
  equal any `RecordKind::as_str()` value (`E-PLUGIN-CORE-COLLISION`)
  and MUST be unique across every installed plugin
  (`E-PLUGIN-DUP-OVERLAY`) — an ambiguous write target neither
  declaration should silently win.
- `attaches_to.core_kind` MUST be `scenario` — any other value fails
  loud (`E-PLUGIN-CORE-KIND`); a generic projection over other core
  kinds is explicit FUTURE work, out of s16's scope.
- `type` is a bare scalar (`bool`/`number`/`string`), `{ enum: […] }`,
  or `{ list: <type> }` — canon-plugin's own small structural checker
  (`canon_plugin::Type`/`type_accepts`), INSPIRED BY canon-vocab's
  identical shape but never importing that crate (design.md D2/R4 —
  see "The other `plugin.yaml`" below).
- Two packages under `canon/plugins/` declaring the SAME `id`: the
  later one (directory-sort order) is dropped, reported as a
  diagnostic, never silently merged (`E-PLUGIN-DUP-ID`).
- An absent `canon/plugins/` directory resolves an empty, valid
  snapshot — never a panic, never a hard error.

## 2. Validate + write an overlay record

A plugin-aware writer NEVER calls `write_namespaced` directly —
`canon_plugin::overlay::write_overlay(tier, decl, body)` always runs
`validate_overlay_body(decl, &body)` FIRST:

- The `OverlayEnvelope { schema, kind, at, actor }` (canon-plugin's OWN
  envelope — `canon_model::Envelope.kind: RecordKind` is closed and
  structurally cannot carry a namespaced string) is present + typed.
- Every REQUIRED `join_key` field is present + a JSON string.
- Every manifest-declared field is present + `type_accepts`s.
- No field outside (envelope ∪ join-key ∪ declared) is accepted — a
  closed field set, one level down from `RecordKind`'s own closure.

Only after validation succeeds does `write_overlay` derive
`natural_key` (the `__`-joined join-key field values, IN DECLARED
ORDER, from the body's OWN fields — never an independently supplied
string) and call `GitTier::write_namespaced(identity, natural_key,
body)`, which appends
`kind=<namespace>.<kind>/{natural_key}__{digest12}.json` —
byte-identical resubmission dedupes to the SAME path; a logically
different body for the same join key appends at a NEW path, never
overwriting.

In practice you drive this through the generic sync dispatcher, never
by hand:

```bash
canon plugin sync porting                       # every canon.yaml specs.roots[] entry
canon plugin sync porting --repo ../svc          # a specific repo
canon plugin sync porting --spec-root ./specs    # ad hoc: ignore canon.yaml entirely
```

`canon plugin sync <plugin-id>` resolves `<plugin-id>`'s manifest
(P1), looks up its registered `OverlaySource` impl by
`plugin_id()` STRING EQUALITY (today: exactly `porting`, registered in
`crates/canon-cli/src/plugin_sync.rs::registry()`), and writes every
`OverlaySource::overlay_candidates` result through the
validate-then-write pipeline above. The dispatcher itself never
special-cases `porting` or any other id — a SECOND donor-porting
plugin adds its own manifest + one `OverlaySource` impl the same way,
touching nothing else.

## 3. Project a read

```bash
canon query --kind scenario --plugin porting               # human table, projected fields appended
canon query --kind scenario --plugin porting --json         # machine-readable, merged record bodies
canon query --kind scenario                                 # NO --plugin: byte-identical to pre-s16 output
```

`--plugin <id>` resolves that plugin's snapshot, then
`project_overlay(core, overlay_raw, decl)` — a PURE, in-memory,
fold-latest-by-`(join_key, at)` JOIN — projects `decl.fields`-named
values onto each queried core record. Fail-soft in every direction
(never a panic, never an aborted whole-projection):

- No overlay record for a given core key → that record projects
  UNMODIFIED (no default/guessed value invented).
- A malformed overlay record (fails `validate_overlay_body` against
  the CURRENT manifest — e.g. the manifest's schema drifted after the
  record was written) → SKIPPED + diagnosed; every sibling record
  still projects.
- An absent plugin/manifest entirely → degrades to the unmodified core
  view plus a diagnostic, never a process error.
- Omitting `--plugin` altogether is byte-identical to `canon query`'s
  pre-s16 output — `run`/`format_human`/`format_json` are untouched;
  `--plugin` is an ADDITIVE `run_with_plugin`/`format_*_with_overlay`
  layer, never a branch bolted into the existing functions.

## The `porting` plugin — the full worked example

`porting` is s16's acceptance vehicle: it replaces the `covered`/
`surface_ref` fields P1 of s15 briefly added to core `Scenario` and
P3a removed (commit `d084e850`) — the EXACT two fields, now living as
a plugin overlay instead of a core field core clobbers on every sync.

1. `canon/plugins/porting/plugin.yaml` — the manifest shown above.
2. `PortingOverlaySource` (`crates/canon-cli/src/plugin_sync.rs`) reads
   a spec root's `inventory/**/*.yaml`
   (`canon_model::family::inventory::{InventoryFile, InventoryEntry}` —
   the SAME S11-validated files `canon-fmt::check` already covers) and,
   for every `(project_id, scenario_id)` `canon inventory sync` would
   index from that root's `.feature` corpus, emits one candidate:
   `covered = true` iff that `scenario_id` appears in ANY
   `InventoryEntry.covered_by` list; `surface_ref` = every
   inventory-entry key whose `covered_by` contains it (empty when
   uncovered). The candidate's envelope `at` is the SOURCE VERSION —
   the max `InventoryFile` envelope `at` across the root's
   `inventory/**` — so an unchanged inventory re-syncs to a
   byte-identical body (`deduped: true`, zero new files: logical
   idempotence, mirroring `canon inventory sync`'s own discipline) and
   a genuine coverage edit supersedes the prior overlay via the fold's
   latest-`at`-wins rule.
3. `canon plugin sync porting` writes/dedupes those candidates.
4. `canon query --kind scenario --plugin porting [--json]` projects
   `covered`/`surface_ref` back onto each `Scenario` for display.

`canon-gate`'s source carries ZERO reference to `porting`, the
`coverage` overlay kind, or `scan_namespaced_kind` — `canon gate
check`'s `uncovered-cell` verdict (and every other verdict) is
byte-identical with and without a `canon plugin sync porting` run
having happened. The overlay is READ BY NOTHING inside canon-gate; a
future consumer wiring it into a gate DECISION needs its own reviewed
change (s15's own R4: "covered ≠ coverage", restated for s16).

## The other `plugin.yaml` — do not confuse the two

`canon-vocab` (S10) has its OWN, UNRELATED `plugin.yaml` surface at
`canon/vocab/<id>/plugin.yaml` (directives + enums, task-atom authoring
vocabulary) — see the `typed-authoring-vocabulary` skill. DIFFERENT
directory, DIFFERENT schema, a DIFFERENT crate (`canon-vocab`, no
dependency from `canon-plugin` and vice versa). canon-vocab's loader
never reads `canon/plugins/`; canon-plugin's loader never reads
`canon/vocab/`. A ledger-overlay manifest misplaced under
`canon/vocab/<id>/` fails canon-vocab's OWN load (missing
`directives`/`enums`) rather than silently parsing as an authoring
vocabulary.

## P1-P6 surface map

| Phase | What | Where |
| --- | --- | --- |
| P1 manifest | `plugin.yaml` schema/loader, `resolve_plugin_snapshot`, the `Type` structural checker | `crates/canon-plugin/src/{manifest/**,resolve_plugin_snapshot.rs}` |
| P2 overlay write | `OverlayEnvelope`, `validate_overlay_body`, `compose_overlay_body`, `write_overlay`; `GitTier::write_namespaced`/`scan_namespaced_kind` | `crates/canon-plugin/src/overlay.rs`, `crates/canon-store/src/{git_tier,partition}.rs` |
| P3 projection | `project_overlay`; `canon query --plugin` | `crates/canon-plugin/src/project.rs`, `crates/canon-cli/src/query.rs` |
| P4 porting | `canon/plugins/porting/plugin.yaml`; `PortingOverlaySource`; `canon plugin sync` | `canon/plugins/porting/plugin.yaml`, `crates/canon-cli/src/plugin_sync.rs` |
| P5 corpus scaffold | `canon scenario new` / `canon feature new` (independent of P1-P4) | `crates/canon-cli/src/scaffold.rs` |
| P6 closure | this skill; the `plugin-overlays` `canon selftest` suite | `canon/skills/canon-plugins/SKILL.md`, `crates/canon-plugin/src/selftest.rs` |

## Selftest coverage (`canon selftest`, `plugin-overlays` suite)

`crates/canon-plugin/src/selftest.rs` registers the 10th `canon
selftest` suite: a SYNTHETIC `canon/plugins/<id>/plugin.yaml` +
overlay-record fixture, built entirely inside a scratch directory at
run time (never touches this repo's own `canon/plugins/porting/`).
Exercises resolution (zero diagnostics for a well-formed manifest),
`validate_overlay_body` rejecting three independently-malformed
candidate bodies, `write_overlay` writing a covered AND an uncovered
record, and `project_overlay`'s full fail-soft contract (covered,
uncovered, unmatched, and a schema-drifted on-disk record skipped +
diagnosed) — diffed two-sided (missing AND extra both fail) against a
checked-in diagnostic-code/message oracle.

Run it with `canon selftest` (all 10 suites) or `cargo test -p
canon-plugin selftest`.

## What this skill does NOT cover

- **`canon inventory sync`** (the upstream core index `porting`'s
  overlay source reads alongside, never through) — see the
  `canon-inventory` skill.
- **`canon gate check`/`canon gate selftest`** — see the
  `trust-spine-gate` skill; canon-gate never reads a plugin overlay
  (module doc above).
- **`canon scenario new`/`canon feature new`** (P5's corpus-authoring
  scaffold, independent of the plugin machinery above) — a template
  generator only, writes NO ledger record; see `crates/canon-cli/src/
  scaffold.rs`'s own doc comment.
- **A generic `project_overlay` over a core kind other than
  `scenario`.** Explicit FUTURE work, out of s16's scope.
- **Wiring an overlay field into a `canon gate` DECISION.** Explicit
  non-goal — needs its own reviewed change.
