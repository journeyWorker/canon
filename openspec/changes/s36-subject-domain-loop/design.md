# s36 subject-domain-loop — design

Scope note: this document covers the **canon-model foundation slice**
(the reviewed 13th record kind and its join-spine key). The store
routing/aging, CLI subcommands, `canon/vocab` domain enum, `canon-learn`
hierarchical regime fallback, and the `canon-subject` skill are later
groups in `tasks.md`; their design decisions are appended as those waves
land.

## D1 — `Subject`: the reviewed 13th record kind

Design D1 of the state-model spec fixed `RecordKind` as a CLOSED set and
declared that a new kind is "a reviewed, breaking `canon-model` change —
never a `kind: String` escape hatch". `Subject` is the first exercise of
that process: the enum grows from twelve to thirteen, `RecordKind::ALL`
becomes `[RecordKind; 13]`, `as_str` gains `subject`, and every
structural `== 12` assertion (`RecordKind::ALL.len()`, the schema-export
count, the fixture-corpus count) moves to `13` so "thirteen kinds" stays
asserted by construction, not by a comment that can drift.

Why a first-class kind rather than reusing `Change`/`Scenario`: a
`Change` is an imported plan unit (openspec change dir), a `Scenario` is
a behavior-spec ledger row; neither is the durable product unit a team
plans → designs → builds → verifies → ships across MANY changes and many
scenarios. Naming is deliberate — the management unit is **Subject**
(after a musical canon's subject taken up by many voices), NEVER
"feature", because `.feature` files are canon's Gherkin behavior corpus
and "read the feature docs" must stay unambiguous.

Wire shape (`schema` 1, snake_case), composing `Envelope` via
`#[serde(flatten)]` like every other kind:

- `subject_id: SubjectId` — new join-spine key, kebab-case slug
  (`[a-z0-9]+(-[a-z0-9]+)*`), the durable product-unit identifier. By-id
  and flat-partitioned like `Change` (`is_area_scoped` is false: no
  mandatory `scenario_id`, so no Hive `area=` segment).
- `title: String`, `summary: String`.
- `domain: String` — see D2.
- `status: SubjectStatus` — closed enum
  `proposed | specced | building | verifying | shipped | retired`
  (snake_case wire). The enum owns only the closed set + spelling;
  transition gating is policy/CEL at the CLI + s35 gate seam (later
  group), NOT this crate.
- `owner_role: RoleId` — the accountable role.
- `change_ids: Vec<ChangeId>`, `scenario_ids: Vec<ScenarioId>` — the
  join links accumulated as work is adopted/specced against the subject;
  both `#[serde(default, skip_serializing_if = "Vec::is_empty")]`, so a
  freshly-authored subject with no links yet is still a valid, minimal
  record and never reserializes spurious empty arrays.

`subject_id` joins **subject ↔ change ↔ scenario** and is added to the
`JOIN_SPINE.md` regeneration source (`join_spine_doc::rows()` — nine rows
now, its count test moved to nine), so the committed doc and the schema
`description` fields regenerate from the ONE `SubjectId::GRAMMAR`/`JOINS`
literal, exactly like the other eight keys.

## D2 — `domain`: validated SHAPE only, vocabulary lives in `canon/vocab`

`domain` is a kebab-case slug validated at PARSE (a
`deserialize_with = "deserialize_domain_slug"` that rejects a
present-but-malformed value into a hard `Deserialize` failure —
"malformed is never silently kept"). canon-model validates the slug
SHAPE and nothing more: the CLOSED base vocabulary (`planning`,
`design`, `dev`, `data`, `test`) lives in the `canon/vocab` plugin (the
S10 typed-vocabulary mechanism) and consumer repos extend it there. This
mirrors EXACTLY how `HandoffBody.domain` keeps its per-domain vocabulary
out of canon-model — the model never encodes which domains a given repo
activates, so adding a domain is a vocab-plugin edit, never a
canon-model change. The kebab check reuses `ids::is_kebab_slug`
(`pub(crate)`) — the same grammar `ChangeId`/`SubjectId` enforce — so the
domain slug shape can never drift from the id slug shape.

`domain` is a plain `String` (not a newtype): the value's MEANING is
owned by the vocab plugin, so a distinct Rust type here would falsely
imply canon-model knows the domain set. Shape validation is enough for
the model's contract; enum-membership validation is the vocab layer's.

## D3 — `Change.subject_id`: additive on the wire

`Change` gains `subject_id: Option<SubjectId>` with
`#[serde(default, skip_serializing_if = "Option::is_none")]`, so a
pre-s36 `Change` deserializes to `None` and reserializes byte-identically
(no spurious `"subject_id": null`). It is a plain `pub` field left `None`
by `Change::new` and stamped on by `canon-cli`'s `subject adopt` at
adoption time — mirroring how `Session.project_key` is set OUTSIDE this
crate — never derived here. This keeps the additive-enum + additive-field
guarantee: existing `canon-model` dependents need no change beyond a
minimal match arm for the new kind (see the store note below).

### Downstream match-exhaustiveness

An additive `RecordKind::Subject` forces one new arm in each of
`canon-store::partition`'s two exhaustive `match kind` sites
(`resolve_partition`, `validate_body`). Both are the minimal, correct
by-id/flat arm (natural key = `subject_id`, no `area`; body validated by
`Subject`'s own `Deserialize`), mirroring `Change`. Store routing/aging
config for the `subject` kind is a later group.

## D-report — the `mart_subjects` report panel (surface)

`canon report` gains a seventh panel, **Subjects** (`mart_subjects`),
following EXACTLY the existing panel pattern (the marts/render/snapshot/
views layers) that s24's `scope_status` established — no new rendering
or snapshot machinery, just one more `fetch_*` + one more `ReportMarts`
field + one more `SNAPSHOT_TABLES` entry + the view.

**The view (`crates/canon-store/sql/views.sql`).** `mart_subjects` is a
per-domain rollup, one row per `subject` record: `domain`, `subject_id`,
`title`, `status`, `scenario_count`, `covered_scenarios`. It reads ONLY
`stg_records` (no second Rust-side aggregation — design D1 of S9):
`kind = 'subject'` for the subject fields (the `scenario_ids` link array
is `UNNEST`ed exactly as `int_task_scenario_refs` unnests
`Task.scenario_refs`), and `kind = 'evidence_record'` for coverage.
`covered_scenarios` counts a subject's linked scenarios whose LATEST
scenario-keyed evidence verdict is non-`Divergent` (`faithful` |
`not_applicable`) — the same last-wins-by-`at` fold `mart_trust_matrix`'s
`green` and `canon-gate::ledger::latest_verdicts` (the `verifying →
shipped` gate) use, so the panel surfaces the SAME coverage the gate
enforces, read-only. A subject with no linked scenarios yields a valid
`scenario_count = 0` / `covered_scenarios = 0` row (never dropped); a
missing/empty subject corpus yields zero rows (the panel renders the
documented `_No rows._` placeholder), never an error.

**Read-only, never a gate input.** Like every mart, this view is
reporting only — `canon-gate` reads nothing `canon-report` produces
(s24's `gate_independence` acceptance holds unchanged). The panel and
its `--snapshot` Parquet export are the SAME numbers, exported in the
report's declared panel order (the shared snapshot contract's parquet
column set = the view's own `SELECT` list, name-for-name).

**Ownership seam.** The `mart_subjects` view block lives in
`canon-store`'s `views.sql` (owned by the CLI/store slice this wave);
`canon-report` consumes it by name through the existing `duckdb -init`
driver, adding no dependency on the store's routing/aging config.
