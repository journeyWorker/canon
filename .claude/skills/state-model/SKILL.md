---
name: state-model
description: How to extend canon-model's closed record-kind set, join-spine keys, and Handoff body template registry — adding/bumping a record kind, adding a join-key newtype, and registering a new Handoff domain template. Use when touching crates/canon-model, adding a new canon.yaml handoff_templates entry, or regenerating JOIN_SPINE.md / schemas/*.schema.json.
---

# state-model

`canon-model` (`crates/canon-model`) is canon's closed, versioned
artifact-family type set (S1). This skill covers the three ways it
grows, and the generated-output discipline every change to it must
respect.

## The record-kind set is closed — review before extending

`RecordKind` (`src/envelope.rs`) has exactly twelve variants (design D1):
`Change`, `Task`, `Scenario`, `Session`, `Run`, `Event`, `Handoff`,
`Review`, `Divergence`, `Trajectory`, `StrategyItem`, `EvidenceRecord`.
This is deliberate friction, not an oversight — an open `kind: String` +
untyped `payload` escape hatch is exactly what let an internal monorepo accumulate three
uncoordinated management systems before canon existed (design D1's
rejected alternative).

Before adding a thirteenth kind:

1. Confirm the new artifact family genuinely doesn't fit an existing
   kind's fields (extending an existing kind's `schema` version, below,
   is almost always the right move first).
2. If it truly needs a new kind, this is a reviewed, explicitly-scoped
   `canon-model` change — not a drive-by addition inside an unrelated
   spec's implementation. Add the variant to `RecordKind` AND to
   `RecordKind::ALL` (both are asserted in sync by
   `envelope::tests::all_twelve_kinds_present_exactly_once`), add the
   struct in `src/records.rs` (or its own module, for something
   `Handoff`-sized), implement `CanonRecord` for it, add it to
   `schema_export::record_schemas()`, and add a well-formed fixture
   under `fixtures/well-formed/<kind>.json`.
3. Every record type composes `Envelope` via `#[serde(flatten)]` — never
   add an ad hoc `actor`/`by` field. `envelope::CanonRecord` is the one
   dispatch trait every kind implements; use it instead of re-deriving a
   kind ↔ type mapping in a new caller.

## Bumping a kind's `schema` version

`Envelope.schema: u32` (design D2) is the per-kind version integer,
bumped on any breaking field change to that kind. Bump it when:

- A required field is added/removed/retyped on an existing record kind.
- A `FailureClass` string is renamed (evidence-integrity spec: "renaming
  a failure class requires a coordinated migration" — ship the rename
  together with updated fixtures referencing the old string, in the same
  change).

Non-breaking additions (a new `Option<T>` field with `#[serde(default)]`)
do not require a bump — `schema_export`'s own scenario ("a field
addition is reflected without a second registration site") assumes
additive changes are the common case.

## Adding a join-spine key newtype

The eight join-spine keys (`src/ids.rs`) are declared through the
`join_key_newtype!` macro: one literal `grammar`/`joins` pair per
invocation, expanded into the type's own rustdoc comment, its
`GRAMMAR`/`JOINS` associated constants, and its `JsonSchema` impl — all
three can never drift relative to each other because they come from the
same macro-invocation literal. `crate::join_spine_doc::rows()` reads
those same constants to build the generated `JOIN_SPINE.md`.

Adding a ninth key (should the design ever call for one) means: a new
`join_key_newtype!` invocation, a hand-written `parse`/grammar-check
`impl` block below it (kept out of the macro so grammars stay ordinary,
testable Rust), unit tests for accept/reject cases, and a new row added
to `join_spine_doc::rows()`.

## Registering a new Handoff domain template

`Handoff`'s state-machine fields (`id`, `state`, `chain_id`, …) are
fixed and wire-compatible with a prior session store's `handoffs` table; the body
(`HandoffBody { domain, template_version, fields }`) is per-domain and
template-validated (design D4/D5). To register a new domain (디자인,
개발, 테스트, …):

1. Implement `handoff::HandoffTemplate` for the new domain (see
   `GihoekTemplate` in `src/handoff.rs` for the 기획 reference
   implementation): `domain()`, `validate(fields) -> Result<(), Vec<EvidenceViolation>>`,
   `render(fields) -> String`.
2. Add the domain string to this repo's root `canon.yaml`'s
   `handoff_templates:` list — a template compiled into `canon-model`
   but absent from `canon.yaml` is treated as unregistered
   (`TemplateRegistry::from_manifest`'s per-repo activation gate).
3. Construct the registry with the new template in `available`:
   `TemplateRegistry::from_manifest(canon_yaml, vec![Box::new(GihoekTemplate), Box::new(YourTemplate)])`.
4. A `Handoff` whose `body.domain` isn't both compiled AND listed in
   `canon.yaml` fails construction with a structured
   `unregistered-handoff-domain` `EvidenceViolation` — never a silent
   accept.

## Regenerating `JOIN_SPINE.md` / `schemas/*.schema.json`

Both are generated, never hand-edited (design D3). After changing a
join-key grammar doc comment or a record kind's fields:

```bash
cargo xtask write          # regenerate + overwrite the committed files
cargo xtask check-generated # regenerate in memory, diff, exit non-zero on drift
```

`cargo test --workspace` already runs the same check
(`canon_model::gen::tests::committed_generated_output_matches_current_source`)
— drift fails the test suite directly, not only a separate CI step.
