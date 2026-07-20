## Context

S11 is canon's first real proof that it is the format authority (design
decision 4): the schema + layout registry `canon-model` defines, and the
read-only validator `canon fmt --check` built against it, are grounded in a
live, in-production corpus's real, documented drift. The 2026-07-10 artifact
audit (design §5 S11, reproduced verbatim in proposal.md) read
the donor consumer repo's `spec/**` sample-by-sample; its central finding is that
`ledger/` and `divergences/` are **already Hive**
(`kind=<kind>/area=<area>/…` and `lane=<lane>/area=<area>/surface=<surface>/…`
respectively, both marked ✓ in the audit) — their gaps are field-level
(missing actor/session identity, free-text refs, abbreviated shas), not
layout. `features/` (plain `<area>/<surface>.feature` dirs) and `inventory/`
(flat, drifted-convention files) are the two families that lack the Hive
partition grammar entirely. This design keeps ledger/divergences' existing
directory layout untouched and declares the SAME Hive convention for
features/inventory — "ONE canonical partition grammar for the family" means
one `key=value/` Hive rule applied consistently, not that every kind shares
one literal top-level directory.

Grounded in the on-disk corpus (read directly, the donor consumer repo):
`spec/ledger/kind=review/area=idolive/idolive.hub.01.json` = `{"schema":1,
"kind":"review","scenario_id":"idolive.hub.01","upstream_ref":"routes/idolive/
replays/index.tsx#RouteComponent","pin":"9c93d024b","reviewer":"…","at":"…"}`
— a **file#symbol**-shaped `upstream_ref` in this sample (parseable), while the
audit table's cited free-text form ("reconciled vs upstream @… (see …)")
appears elsewhere in the corpus and is NOT parseable — the schema and
`canon fmt --check` must handle both. `spec/ledger/kind=run/*.json` (flat, no
`area=`) = `{"schema":1,"kind":"run","scenario_ids":[…],"lane":"unit",
"app_sha":"2745ca4c88…","harness_sha":"…","by":"flutter-test-machine",
"result":"pass","evidence":[]}` — confirms `by` is a bare string (not the
structured `actor` S1 mandates) and `evidence` is an untyped empty array.
`spec/inventory/` mixes `world-map.yaml` (area-level) and
`world-place-map.yaml` (surface-level) as sibling flat files, plus
`assets.lock` as a fourth ad-hoc format. `parity.py`'s
`_ledger_layout_problem` (tools/parity.py:1272) already codifies the
existing split (`review/design-review/code-review/clear` → Hive
`kind=/area=`; `run/drill` → flat under `kind=` only) — this is the pattern
S11's "layout enforcement (`_ledger_layout_problem` generalized)" deliverable
generalizes into canon-model, not replaces.

> **Scope note (2026-07-10, operator directive):** `canon migrate` (an
> in-place corpus rewrite tool), the in-place migration of the donor consumer repo's
> real corpus, and a `parity.py` sync patch were all part of an earlier
> version of this design and are now REMOVED from scope. The donor consumer repo
> conforming its own corpus to this format, and reconciling its own tooling
> (`parity.py` included), is that repo's own later, separate operational
> step — not something `canon-fmt` ships or commits to. Every design
> decision below that referenced the migration tool's runtime behavior has
> been narrowed to what it means for the SCHEMA (`canon-model`) and the
> VALIDATOR (`canon fmt --check`) — both of which this change still ships in
> full.

## Goals / Non-Goals

**Goals:**
- ONE canonical Hive partition rule (`kind=<kind>/[key=value/]*<leaf>`,
  declared per-kind in canon-model as a layout descriptor, not hardcoded
  special-casing) that `ledger/` and `divergences/` already satisfy and that
  `features/` and `inventory/` are declared onto.
- Schema envelopes (`schema`, `kind`, `at`, `actor`) on every artifact,
  **including YAML** (inventory) and Gherkin (features, via a structured
  comment line — Gherkin has no native metadata block).
- Expressiveness upgrades, as additive/optional schema fields: structured
  `actor {agent_id, role, session_id?, model?}` alongside a still-deserializable
  bare `by` string; refs as arrays of `{file, symbol, lines?}` alongside the
  raw `upstream_ref`/`port_ref` string (never deleted); optional `change_id`/
  `task_id` join fields; a full-40-char-sha grammar check (`app_sha`/
  `harness_sha` stay plain strings so an abbreviated value still
  deserializes, but `canon fmt --check` flags it); optional `checked`
  (aspects) content on passing reviews.
- `canon fmt --check` reporting exactly the audited gaps over
  `canon-fmt`'s own LOCAL fixture corpus (reproducing the donor consumer repo's real,
  audited drift shapes) — read-only, never mutating what it validates, and
  never reading a live consumer-repo checkout on its own.
- Acceptance is schema- and validator-level: the fixture corpus, run through
  `canon fmt --check`, reports exactly the audited gap table — no unaudited
  gap, no missed gap.

**Non-Goals:**
- Changing `ledger/`'s or `divergences/`'s existing directory layout — both
  are already Hive and marked ✓ in the audit; S11 upgrades their record
  *fields*, not their partition keys.
- **Any corpus rewrite tool (`canon migrate`), any in-place migration run
  against the donor consumer repo or any other consumer repo, and any `parity.py`
  patch.** All three are explicitly out of scope per the 2026-07-10 operator
  directive (see the Context scope note) — `canon-fmt` only ever validates.
  If a consumer repo ever needs a one-shot rewrite onto this format, that is
  a separate, later, throwaway script scoped to that repo, not part of this
  change or `canon-fmt`'s ongoing surface.
- Flutter-specific checks (D20 invention detection, dart test ingest) — stay
  local to `parity.py`, untouched.
- Backfilling historical `change_id`/`task_id` joins on any existing record
  — those fields are added as optional and stay absent on anything that
  predates this schema; only new records (post-S11, via S4's future
  artifact-ingest) populate them going forward.
- Reading, or depending on the existence of, any live consumer-repo
  checkout. Every `canon-fmt` test exercises a corpus root the caller passes
  in (a fixture tmpdir, or whatever path `canon fmt --check <root>` is
  invoked with) — never the donor consumer repo or any other path baked
  into the crate.

## Decisions

**D1 — Layout descriptor is declarative per kind, not a hardcoded special
case per family.**
canon-model registers each artifact kind with a `LayoutDescriptor {kind:
&str, partition_keys: &[&str], leaf_grammar: LeafGrammar}` — e.g. `review` =
`partition_keys: ["area"]`, `leaf: "<scenario_id>.json"`; `run`/`drill` =
`partition_keys: []`, `leaf: "<at-compact>-<lane>-<app_sha8>-<rand6>.json"`;
`divergence` = `partition_keys: ["lane","area","surface"]`, `leaf:
"<round>-<round>-<app_sha8>-<rand6>.jsonl"`; `feature` (new) =
`partition_keys: ["area"]`, `leaf: "<surface>.feature"`; `inventory` (new) =
`partition_keys: ["area"]`, `leaf: "[surface=<surface>/]<key>.yaml"`. A
single `layout_problem(record, path, descriptor) -> Option<Violation>`
function (generalizing `_ledger_layout_problem`) validates ANY kind against
its descriptor. Rationale: the audit's "third partition grammar" / "fourth
ad-hoc format" complaint is about **inconsistent, undeclared** layouts, not
about every kind needing identical depth — a `run` record covering multiple
scenarios genuinely cannot nest under one `area=`; the fix is declaring that
exception in one registry instead of leaving it an unstated special case
duplicated across tooling.
Alternative considered: force every kind (including multi-scenario `run`/
`drill`) under a synthetic `area=_multi/` bucket — rejected: fabricates a
meaningless partition value and breaks the "layout enforced" invariant's
usefulness (an `area=` that never discriminates anything is worse than a
declared exception).

**D2 — `features/` gains `kind=feature/area=<area>/` Hive prefix; filename
convention (`<surface>.feature`) is preserved.**
`spec/features/<area>/<surface>.feature` becomes
`spec/features/kind=feature/area=<area>/<surface>.feature` (or, if root
`spec/features/` is itself accepted as implying `kind=feature` — a
consumer-repo-side decision when it conforms its own corpus; either way the
descriptor from D1 governs enforcement, not the literal string). Authoring
provenance (who/when/which session authored a scenario — the audit's second
`features/` gap) is a structured comment line immediately after each
`Feature:`/`Scenario:` header: `# canon: {"schema":1,"at":"<iso8601>",
"actor":{"agent_id":"…","role":"…","session_id":"…"}}` — valid Gherkin (a
`#`-prefixed line is a comment to every Gherkin parser, D2 of the parity
DECISIONS confirms `gherkin-official` is the parsing engine and comments are
part of its grammar), so this is purely additive and never breaks
`parity.py`'s existing `.feature` parsing. `canon fmt --check` flags a
header with no such comment (`missing-provenance`).

**D3 — `inventory/` gains `kind=inventory/area=<area>/` Hive prefix +
in-document schema envelope; `assets.lock` becomes a generated-only,
enveloped artifact.**
Each `spec/inventory/<name>.yaml` conforms to
`spec/inventory/kind=inventory/area=<area>/<key>.yaml` (surface-level files
gain an additional `surface=<surface>/` segment where the audit identifies
one, e.g. today's `world-place-map.yaml`), and carries top-level `schema`/
`kind`/`at`/`actor` keys alongside its existing content keys (`upstream:`,
`covered_by:`, …) — additive YAML keys, so any reader that only looks up
`upstream`/`covered_by` is unaffected. `assets.lock` — a single generated
lockfile, not a partitioned corpus — carries a schema envelope and lives at
`spec/inventory/kind=inventory-lock/assets.lock.yaml`, generated-only (D16
pattern: regenerate + diff-check, never hand-edited). `canon fmt --check`
flags a flat, envelope-less inventory file (`layout-grammar`/
`missing-envelope`) and the legacy `assets.lock` path
(`layout-grammar`, "a fourth ad-hoc format").

**D4 — Refs are `{file, symbol, lines?}` arrays; unparseable free text is
reported, never guessed.**
The schema's `refs: [{file, symbol, lines?}]` array is the structured
replacement for today's `upstream_ref`/`port_ref` strings, built from the
`<file>#<symbol>` pattern already used by the well-formed samples (e.g.
`routes/idolive/replays/index.tsx#RouteComponent`), adding `lines` where a
`lines-<a>-<b>` suffix or companion field exists. A ref string that does not
match this pattern (the audit's free-text example, "reconciled vs upstream @…
(see …)") is left exactly as-is (both `upstream_ref`/`port_ref` and any
partially-populated `refs` survive on the same record) and `canon fmt
--check` reports it with class `free-text-ref` — never a fabricated `{file,
symbol}` guess. `;`-/`,`-joined code/design-review ref strings are each
independently checked against the same `<file>#<symbol>` rule; a joined-but-
unsplit string is reported `joined-ref`.

**D5 — `actor` is structured, with `role` genuinely optional — schema
expresses "unknown", not just "known".**
A ledger record's `actor` is `{agent_id, role: Option<RoleId>, session_id?,
model?}`, never a bare `by: String` — but `role` stays `Option` because a
record whose only source data is a bare `by: "flutter-test-machine"` string
carries no role information at all (`Actor::new_unattributed` constructs
exactly this shape). The legacy `by: Option<String>` field stays on the
schema, additive and optional, so a record that only ever populated `by`
(never `actor`) still deserializes against this schema version.
`canon-model::ids::Sha`'s strict 40-hex newtype is NOT used for
`app_sha`/`harness_sha` (they stay plain `String`) so an abbreviated value
still deserializes; `canon fmt --check`'s OWN `abbreviated-sha` check (not
JSON-schema rejection) is what flags a value shorter than 40 hex characters.
This design deliberately does not specify HOW an abbreviated sha would ever
be expanded to its full form — that was `canon migrate`'s job, and is out of
scope (Non-Goals).

**D6 — Divergences gain bidirectional back-refs as an additive schema
field, not a restructure.**
The audit's "back-refs one-way" divergences gap is addressed by an additive
`divergence_refs: [<file>]` field on the ledger review/code-review/design-
review record schema, reciprocal to the divergence record's existing
`ledger_refs: [<scenario_id>]` field. `canon fmt --check` flags a ledger
record named by a divergence's `ledger_ref` that carries no reciprocal
`divergence_refs` entry (`one-way-backref`) — the schema makes the
reciprocal field expressible and the validator makes its absence visible;
POPULATING it (for an existing record) is, like every other upgrade, a
consumer-repo-side concern this change does not perform.

## Risks / Trade-offs

- **Risk:** a schema that is too permissive (every new field optional) could
  make `canon fmt --check` under-report real gaps.
  **Mitigation:** the fixture corpus (`crates/canon-fmt/fixtures/
  consumer-corpus/`) is built to exercise every audited gap category at
  least once, and a test
  (`fmt_check_detects_every_audited_gap_category_on_fixtures`) asserts all
  ten `FmtFailureClass` variants are observed on it — a schema/validator
  change that silently stops flagging a category fails this test.
- **Risk:** free-text `upstream_ref` could be a large fraction of review
  records if the audit's cited example is common, leaving much of a
  consumer corpus incomplete against the upgraded schema indefinitely (with
  no migration tool to close the gap automatically).
  **Trade-off accepted:** additive-where-possible (design §5 S11) means an
  incomplete record still validates against the schema's PRIOR
  required-field set (envelope + pre-existing fields) — only the NEW `refs`
  array is absent — so `canon fmt --check` reports it as incomplete, not
  broken, and any external tooling that never read a structured `refs`
  array before (e.g. `parity.py`) is unaffected by its absence.
