## Context

S12 generalizes the donor CLI's context command (design §3 donor table: the donor vocabulary project, the donor CLI's snapshot builder `run_context`) from the donor vocabulary project's single-document authoring
surface to canon's whole artifact family (S11's schema registry + S5's
policy resolution). Read directly from the donor CLI's source:

- `run_context` (line 537) builds the same `CheckInput` (`build_input`) and
  folds the same environment (`fold_env`) the checker (`run_check`) uses —
  "parse + fold exactly as `compile` does (minus codegen)" — then calls
  `authoring_surface(&input, &folded.env.state, &branch_paths)` and either
  serializes it (`--json`) or renders `context_outline(&surface)`.
  `authoring_surface`'s own doc comment states the determinism contract
  verbatim: "every map is a BTreeMap (key-sorted by construction) and every
  array is emitted in a stable order (directives by name, state paths by
  path, components by name; attrs/params in declaration order)".
- The surface JSON has a fixed shape: `capabilityVersion`, `directives`
  (name, attrs, semantics), `enums`/`assetKinds`/`providers` (BTreeMaps,
  serde-key-sorted), `stateSchema` (path-sorted, with `domain` for enum-typed
  paths), `components` (name-sorted, params in declaration order).
- The "expected one of" contract (design §5 S12) is `the donor's checker crate/src/
  directives.rs` `check_enum_member` (lines 249-271): a failing enum-value
  diagnostic reads `format!("`{got}` is not a valid value for `{key}` of
  `::{tag}` (expected one of: {})", members.join(", "))` — `members` is the
  exact same `Vec<String>` `authoring_surface`'s `attr_type_str`/
  `state_type_str` put in the surface's `domain` field for that same enum.

## Goals / Non-Goals

**Goals:**
- `canon context [--repo <dir>] [--json]` emitting: record kinds + envelope
  fields, enum domains (verdicts, statuses, lanes, roles), join-key
  grammars, partition layout, policy-derived requirements, capability
  version.
- Invariant 1 (capability query, not validation): `canon context` exits 0
  and emits a full surface even when `canon fmt --check`/`canon gate` would
  report diagnostics against the same repo's corpus.
- Invariant 2 (same registry as the validator): `canon context` and
  `canon fmt`/`canon gate` both call one `SchemaRegistry`/`PolicyResolution`
  API — no second, independently-loaded copy of schema or policy data.
- Invariant 3 (deterministic output): BTreeMap-keyed sections, stable array
  ordering, byte-identical output across repeated runs on unchanged input;
  `--json` and the default human outline agree because both render from the
  one resolved surface value.
- "Expected one of: …" validator diagnostics sourced from the identical
  enum-domain lookup `canon context` uses for its `domain` fields.

**Non-Goals:**
- LSP serving (design §5 S12 says "capability-snapshot resolution feeds
  checker, `canon context`, and (later) LSP from one source" — the LSP
  consumer is S10-scoped, not built here).
- A `plugin.yaml`-driven extension mechanism for the vocabulary itself
  (S10, wave W4) — S12 reads whatever schema/policy registry S1/S5/S11
  already expose; it does not add a new authoring mechanism.
- Authoring-surface content for kinds not yet schema-registered — `canon
  context` reflects exactly the registry's current state; a kind with no
  registered schema simply does not appear (not an error, not a gap this
  change backfills).

## Decisions

**D1 — `canon context` mirrors `run_context`'s two-phase flow: resolve, then
render.**
Phase 1 (`resolve_surface(repo, opts) -> AuthoringSurface`) loads the
project's `canon.yaml`, resolves the schema registry (S1/S11) and policy
(S5's `policy.yaml`-derived requirements) exactly as `canon fmt`/`canon
gate` do — literally the same `SchemaRegistry::load`/`PolicyResolution::
resolve` calls (D2 below), never a parallel loader. Phase 2 renders the
resolved `AuthoringSurface` value as either pretty JSON or the compact
outline — rendering never re-touches disk or re-resolves anything, matching
`run_context`'s "parse + fold once, render twice" shape. Rationale: this is
the literal mechanism invariant 2 requires — the split exists specifically
so "resolve" has exactly one call site regardless of output mode.

**D2 — Same-registry invariant is enforced by a single shared crate
function, not a convention.**
`canon-model` (S1) exposes `SchemaRegistry::load(repo: &Path) ->
SchemaRegistry` and canon-gate (S5) exposes `PolicyResolution::resolve(repo,
&SchemaRegistry) -> PolicyResolution`; `canon fmt`, `canon gate`, and `canon
context` all call these two functions and nothing else to obtain
schema/policy data — no command constructs its own copy of enum domains,
partition descriptors, or policy-derived requirements. Rationale: "no
second registration site" (design §5 S12 acceptance) is a code-structure
guarantee, not a documentation promise — if a schema/policy change requires
touching two call sites to reflect in both validator errors and context
output, the invariant is already broken; D2 makes that structurally
impossible by construction (one function, multiple callers).

**D3 — Authoring surface schema generalizes `authoring_surface`'s shape from
one DSL document to canon's whole artifact family.**
The emitted JSON root carries: `capabilityVersion` (S1 schema version),
`kinds` (BTreeMap by kind name → `{schema_version, envelope_fields,
partition: LayoutDescriptor}` — S11's registry, D1 of the S11 design),
`enums` (BTreeMap by enum name → sorted member list — verdicts, statuses,
lanes, roles, polarity), `joinKeys` (BTreeMap by key name → grammar string,
S1's join-spine table), `policy` (policy-derived requirements resolved for
this repo, S5). Every map is a Rust `BTreeMap` (key-sorted by construction,
mirroring `authoring_surface`'s doc-comment contract verbatim) and every
array is declaration-order or name-sorted per field, never insertion-order
from a `HashMap`.

**D4 — Capability-query invariant: `canon context` never calls the
validator.**
`resolve_surface` reads schema/policy only — it never runs `canon fmt`'s
corpus walk or `canon gate`'s evidence checks, so a corpus with violations
cannot make `canon context` fail or omit anything; the surface describes
what CAN be authored, independent of what currently IS valid. Rationale:
identical to `run_context`'s own contract ("emitted even when the corpus has
diagnostics" — `run_context` never calls `run_check`, it re-derives its own
fold from `build_input`/`fold_env` directly).

**D5 — `--json` vs. outline: one render function each, over one resolved
value.**
`render_json(&surface) -> String` (`serde_json::to_string_pretty`, matching
`authoring_surface`'s own serialization) and `render_outline(&surface) ->
String` (mirroring `context_outline`'s compact, per-section human listing)
both take `&AuthoringSurface` and produce their output with no additional
resolution step. The two are tested against each other in fixtures: every
kind/enum/join-key present in the JSON output must also appear (by name) in
the outline, so the two never silently diverge in content, only in format.

**D6 — "Expected one of: …" diagnostics call the identical enum-domain
lookup `canon context`'s `enums` field uses.**
canon-model's schema validator, on an enum-value mismatch, emits `format!(
"`{got}` is not a valid value for `{field}` of `{kind}` (expected one of:
{})", registry.enum_domain(kind, field).join(", "))` — the exact
the donor checker's enum-member check message shape (design §5 S12:
"the donor's checker crate `directives.rs:264-267` pattern"), with `registry.enum_domain`
being the same call `resolve_surface` (D1) uses to populate the `enums`
BTreeMap. A schema change that adds/removes an enum member is therefore
reflected in both the validator's next diagnostic and `canon context`'s next
run from the single edit to the registry — no second list to update.

**D7 — `--repo <dir>` defaults to the resolved project root (`canon.yaml`
discovery), matching every other canon command's root resolution.**
`canon context` with no `--repo` flag resolves the project root the same
way `canon fmt`/`canon gate` do (nearest `canon.yaml` walking up from cwd) —
no bespoke resolution logic for this one command, consistent with the
same-registry invariant applying to root discovery too, not just
schema/policy loading.

## Risks / Trade-offs

- **Risk:** a large artifact family (S11's full schema set) makes the JSON
  surface large enough that "compact human outline for prompt injection"
  (design §5 S12) stops being compact.
  **Mitigation:** the outline renderer (D5) is explicitly a summary (kind
  names + enum names + join-key names, not full schema bodies) — matching
  `context_outline`'s own "short at-a-glance view" contract, not a full
  JSON-to-text dump.
- **Risk:** determinism (invariant 3) is easy to violate accidentally by
  introducing a `HashMap` anywhere in the resolution path (iteration order
  is not stable across runs/platforms).
  **Mitigation:** D3 mandates `BTreeMap` everywhere in `AuthoringSurface`
  by construction; the byte-stability fixture test (design §5 S12
  acceptance: "context output over a fixture repo is byte-stable") runs the
  command twice and diffs, catching any accidental non-determinism.
- **Risk:** D2's single-shared-function contract could be violated later by
  a new command (e.g. a future S10 checker) that adds its own schema
  loading path, silently reintroducing a second registration site.
  **Trade-off accepted:** S12 establishes the pattern and the fixture that
  proves it holds today; enforcing it against future additions is a review
  discipline (grep for a second `SchemaRegistry::load`/ad hoc schema
  literal), not a mechanism this change can install for all time.
